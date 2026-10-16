use super::dto::{ImportResource, Input};
use crate::{
    entities::{ai_imports as imports, exercises},
    modules::{
        notes::{
            document::{validate, Reference},
            service as notes,
        },
        places::service as places,
    },
    AppError,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect, Set,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
fn invalid() -> AppError {
    AppError::validation("content", "Section Học cùng AI không hợp lệ.")
}
fn receipt(model: imports::Model) -> ImportResource {
    model.into()
}
pub async fn prepare(
    db: &DatabaseConnection,
    user: i64,
    session: i64,
    mut input: Input,
) -> Result<ImportResource, AppError> {
    let (_, place) = notes::access(db, user, session, false).await?;
    if input.key.is_nil() || input.exercises.len() > 100 {
        return Err(invalid());
    }
    for definition in &mut input.exercises {
        definition.validate(false)?;
        if definition.revision.is_some() {
            return Err(invalid());
        }
    }
    let refs = validate(&input.content, 3)?;
    let mut slots = Vec::new();
    for reference in refs {
        match reference {
            Reference::Exercise(id) => slots.push(id),
            _ => return Err(invalid()),
        }
    }
    slots.sort();
    if slots != (1..=input.exercises.len() as i64).collect::<Vec<_>>() {
        return Err(invalid());
    }
    let heading = &input.content["content"][1];
    if heading["type"] != "heading" || heading["attrs"]["level"] != 2 {
        return Err(invalid());
    }
    let questions = input
        .exercises
        .iter()
        .map(|d| serde_json::to_value(&d.questions[0]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    fn paths(node: &Value, path: String, found: &mut Vec<(String, usize)>) {
        if node["type"] == "exercise" {
            found.push((
                format!("{path}/attrs/exerciseId"),
                node["attrs"]["exerciseId"]
                    .as_i64()
                    .expect("validated slot") as usize
                    - 1,
            ));
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            for (i, child) in children.iter().enumerate() {
                paths(child, format!("{path}/content/{i}"), found)
            }
        }
    }
    let mut replacements = Vec::new();
    paths(&input.content, String::new(), &mut replacements);
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&input).map_err(|e| AppError::Internal(e.to_string()))?)
    );
    let txn = places::write_tx(db, place.user_id, place.id).await?;
    notes::access(&txn, user, session, false).await?;
    if let Some(mut model) = imports::Entity::find_by_id(input.key).one(&txn).await? {
        if model.user_id != user || model.session_id != session {
            return Err(AppError::NotFound("Không tìm thấy lượt nhập.".into()));
        }
        if model.fingerprint != fingerprint {
            return Err(AppError::Conflict(
                "Mã nhập này đã dùng cho nội dung khác.".into(),
            ));
        }
        if !model.attached {
            let mut row: imports::ActiveModel = model.into();
            row.created_at = Set(chrono::Utc::now().fixed_offset());
            model = row.update(&txn).await?;
        }
        let response = receipt(model);
        txn.commit().await?;
        return Ok(response);
    }
    let mut ids = Vec::new();
    for (definition, question) in input.exercises.into_iter().zip(questions) {
        let model = exercises::ActiveModel {
            session_id: Set(session),
            user_id: Set(user),
            title: Set(definition.title),
            question: Set(question),
            revision: Set(1),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        ids.push(model.id);
    }
    for (path, index) in replacements {
        *input.content.pointer_mut(&path).expect("validated path") = Value::from(ids[index]);
    }
    input.content["content"][1]["attrs"]["aiImportId"] = Value::String(input.key.to_string());
    let model =
        imports::ActiveModel {
            id: Set(input.key),
            session_id: Set(session),
            user_id: Set(user),
            fingerprint: Set(fingerprint),
            content: Set(input.content),
            exercise_ids: Set(
                serde_json::to_value(ids).map_err(|e| AppError::Internal(e.to_string()))?
            ),
            attached: Set(false),
            created_at: Set(chrono::Utc::now().fixed_offset()),
        }
        .insert(&txn)
        .await?;
    let value = receipt(model);
    txn.commit().await?;
    Ok(value)
}
pub async fn references<C: ConnectionTrait>(
    db: &C,
    session: i64,
    refs: &[Reference],
) -> Result<(), AppError> {
    let keys: Vec<_> = refs
        .iter()
        .filter_map(|r| {
            if let Reference::AIStudy(id) = r {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    if keys.is_empty() {
        return Ok(());
    }
    let found = imports::Entity::find()
        .filter(imports::Column::SessionId.eq(session))
        .filter(imports::Column::Id.is_in(keys.iter().copied()))
        .all(db)
        .await?;
    if keys.iter().any(|key| !found.iter().any(|m| m.id == *key)) {
        return Err(invalid());
    }
    Ok(())
}
/// Called inside the note transaction, after revision checks. Keep resources ever used for undo.
pub async fn attach<C: ConnectionTrait>(
    db: &C,
    session: i64,
    refs: &[Reference],
) -> Result<(), AppError> {
    let keys: Vec<_> = refs
        .iter()
        .filter_map(|r| {
            if let Reference::AIStudy(id) = r {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    let exercise_ids: Vec<_> = refs
        .iter()
        .filter_map(|r| {
            if let Reference::Exercise(id) = r {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    if keys.is_empty() && exercise_ids.is_empty() {
        return Ok(());
    }
    let pending: Vec<(uuid::Uuid, Value)> = imports::Entity::find()
        .select_only()
        .columns([imports::Column::Id, imports::Column::ExerciseIds])
        .filter(imports::Column::SessionId.eq(session))
        .filter(imports::Column::Attached.eq(false))
        .into_tuple()
        .all(db)
        .await?;
    let mut used = keys;
    for (key, data) in pending {
        let ids: Vec<i64> =
            serde_json::from_value(data).map_err(|e| AppError::Internal(e.to_string()))?;
        if ids.iter().any(|id| exercise_ids.contains(id)) {
            used.push(key);
        }
    }
    if !used.is_empty() {
        imports::Entity::update_many()
            .col_expr(
                imports::Column::Attached,
                sea_orm::sea_query::Expr::value(true),
            )
            .filter(imports::Column::SessionId.eq(session))
            .filter(imports::Column::Id.is_in(used))
            .exec(db)
            .await?;
    }
    Ok(())
}
