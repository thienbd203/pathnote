mod dto;
mod phonemes;
use crate::{
    access_token::CurrentUser,
    entities::{ipa_library as library, users},
    state::AppState,
    AppError,
};
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        DefaultBodyLimit, Query as AxumQuery, State,
    },
    routing::{get, post},
    Json, Router,
};
use dto::*;
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ipa-vocabulary", get(words))
        .route("/ipa-pairs", get(pairs))
        .route("/ipa-vocabulary/import", post(import_words))
        .route("/ipa-pairs/import", post(import_pairs))
        .layer(DefaultBodyLimit::max(262144))
}
struct Prepared {
    key: String,
    label: String,
    content: Value,
}
fn hash(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
async fn list(s: AppState, user: i64, q: Query, kind: &str) -> Result<Json<Value>, AppError> {
    if q.page == 0 || q.page > i32::MAX as u64 {
        return Err(AppError::validation("page", "Trang không hợp lệ."));
    }
    let locale = locale(&q.locale);
    let page = library::Entity::find()
        .filter(library::Column::UserId.eq(user))
        .filter(library::Column::Locale.eq(locale))
        .filter(library::Column::Kind.eq(kind))
        .order_by_asc(library::Column::Id)
        .paginate(s.db.as_ref(), 100);
    let total = page.num_items().await?;
    let rows = page.fetch_page(q.page - 1).await?;
    let data = if kind == "word" {
        json!({"version":1,"locale":locale,"entries":rows.into_iter().map(|m|m.content).collect::<Vec<_>>()})
    } else {
        let mut sets: BTreeMap<String, Value> = BTreeMap::new();
        for m in rows {
            let sounds = m.content["sounds"].clone();
            let set = sets
                .entry(sounds.to_string())
                .or_insert_with(|| json!({"sounds":sounds,"pairs":[]}));
            set["pairs"]
                .as_array_mut()
                .expect("pairs")
                .push(json!({"left":m.content["left"],"right":m.content["right"]}));
        }
        json!({"version":1,"locale":locale,"sets":sets.into_values().collect::<Vec<_>>()})
    };
    Ok(Json(
        json!({"data":data,"meta":{"current_page":q.page,"last_page":total.div_ceil(100).max(1),"total":total}}),
    ))
}
async fn words(
    State(s): State<AppState>,
    u: CurrentUser,
    q: Result<AxumQuery<Query>, QueryRejection>,
) -> Result<Json<Value>, AppError> {
    let AxumQuery(q) = q.map_err(|_| invalid())?;
    list(s, u.id, q, "word").await
}
async fn pairs(
    State(s): State<AppState>,
    u: CurrentUser,
    q: Result<AxumQuery<Query>, QueryRejection>,
) -> Result<Json<Value>, AppError> {
    let AxumQuery(q) = q.map_err(|_| invalid())?;
    list(s, u.id, q, "pair").await
}
async fn persist(
    s: AppState,
    user: i64,
    locale: &str,
    kind: &str,
    entries: Vec<Prepared>,
) -> Result<Json<Value>, AppError> {
    let txn = s.db.begin().await?;
    users::Entity::find_by_id(user)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    let mut keys: HashSet<_> = library::Entity::find()
        .filter(library::Column::UserId.eq(user))
        .filter(library::Column::Locale.eq(locale))
        .filter(library::Column::Kind.eq(kind))
        .filter(library::Column::Key.is_in(entries.iter().map(|e| e.key.clone())))
        .select_only()
        .column(library::Column::Key)
        .into_tuple::<String>()
        .all(&txn)
        .await?
        .into_iter()
        .collect();
    let mut skipped = Vec::new();
    let mut models = Vec::new();
    for entry in entries {
        if !keys.insert(entry.key.clone()) {
            skipped.push(entry.label);
            continue;
        }
        models.push(library::ActiveModel {
            user_id: Set(user),
            locale: Set(locale.into()),
            kind: Set(kind.into()),
            key: Set(entry.key),
            content: Set(entry.content),
            ..Default::default()
        });
    }
    let added = models.len();
    for chunk in models.chunks(100) {
        library::Entity::insert_many(chunk.iter().cloned())
            .on_conflict(
                OnConflict::columns([
                    library::Column::UserId,
                    library::Column::Locale,
                    library::Column::Kind,
                    library::Column::Key,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&txn)
            .await?;
    }
    txn.commit().await?;
    Ok(Json(
        json!({"data":{"version":1,"locale":locale},"meta":{"added":added,"skipped":skipped}}),
    ))
}
async fn import_words(
    State(s): State<AppState>,
    u: CurrentUser,
    input: Result<Json<Words>, JsonRejection>,
) -> Result<Json<Value>, AppError> {
    let Json(mut input) = input.map_err(|_| invalid())?;
    if input.version != 1 || input.entries.is_empty() {
        return Err(invalid());
    }
    let mut entries = Vec::new();
    for e in &mut input.entries {
        e.validate(&input.locale)?;
        entries.push(Prepared {
            key: hash(&json!(key(&e.word))),
            label: e.word.clone(),
            content: value(e)?,
        });
    }
    persist(s, u.id, locale(&input.locale), "word", entries).await
}
async fn import_pairs(
    State(s): State<AppState>,
    u: CurrentUser,
    input: Result<Json<Pairs>, JsonRejection>,
) -> Result<Json<Value>, AppError> {
    let Json(mut input) = input.map_err(|_| invalid())?;
    if input.version != 1
        || input.kind != "ipa-pairs"
        || !(1..=50).contains(&input.pairs.len())
        || input.sounds[0] == input.sounds[1]
        || input
            .sounds
            .iter()
            .any(|s| !phonemes::SOUNDS.contains(&s.as_str()))
    {
        return Err(invalid());
    }
    let mut sounds = input.sounds.clone();
    sounds.sort();
    let mut entries = Vec::new();
    for pair in &mut input.pairs {
        pair.left.validate(&input.locale)?;
        pair.right.validate(&input.locale)?;
        if !phonemes::minimal(pair, &input.sounds, &input.locale) {
            return Err(invalid());
        }
        let label = format!("{} ↔ {}", pair.left.word, pair.right.word);
        let mut names = [key(&pair.left.word), key(&pair.right.word)];
        names.sort();
        let (left, right) = if sounds == input.sounds {
            (&pair.left, &pair.right)
        } else {
            (&pair.right, &pair.left)
        };
        entries.push(Prepared {
            key: hash(&json!([sounds, names])),
            label,
            content: json!({"sounds":sounds,"left":left,"right":right}),
        });
    }
    persist(s, u.id, locale(&input.locale), "pair", entries).await
}
