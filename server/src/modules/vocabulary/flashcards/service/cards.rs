use super::super::{dto, personal, queue};
use super::runs;
use crate::{
    entities::{flashcard_attempts as attempts, flashcard_settings as settings},
    modules::notes::service as notes,
    AppError,
};
use sea_orm::{sea_query::OnConflict, ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;
pub async fn configure(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: dto::SettingsInput,
) -> Result<dto::Overview, AppError> {
    let mut f = dto::Filters {
        session_id: input.session_id,
        course_id: input.course_id,
        topic: input.topic,
        q: input.q,
        ..Default::default()
    };
    f.validate()?;
    if !input.retention.is_finite() || !(0.7..=0.97).contains(&input.retention) {
        return Err(AppError::validation(
            "retention",
            "Mức ghi nhớ từ 70% đến 97%.",
        ));
    }
    queue::authorize(db, user, &f).await?;
    settings::Entity::insert(settings::ActiveModel {
        user_id: Set(user),
        scope: Set(f.scope()),
        direction: Set(input.direction.label().into()),
        retention: Set(input.retention),
    })
    .on_conflict(
        OnConflict::columns([settings::Column::UserId, settings::Column::Scope])
            .update_columns([settings::Column::Direction, settings::Column::Retention])
            .to_owned(),
    )
    .exec(db)
    .await?;
    queue::overview(db, user, &f).await
}
pub async fn next(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    mut f: dto::Filters,
) -> Result<dto::Next, AppError> {
    f.validate()?;
    queue::authorize(db, user, &f).await?;
    let run_id = f.run_id;
    let saved_run = if let Some(id) = run_id {
        Some(runs::load(db, user, id).await?)
    } else {
        None
    };
    if let Some(run) = &saved_run {
        f = serde_json::from_value(run.filters.clone())
            .map_err(|_| AppError::Internal("Invalid saved flashcard filters".into()))?;
        queue::authorize(db, user, &f).await?;
        if run.finished_at.is_some() {
            return Ok(dto::Next {
                card: None,
                overview: queue::overview(db, user, &f).await?,
            });
        }
    }
    let (direction, retention) = if let Some(run) = &saved_run {
        (
            serde_json::from_value::<dto::Direction>(run.settings["direction"].clone())
                .unwrap_or_default(),
            run.settings["retention"].as_f64().unwrap_or(0.9) as f32,
        )
    } else {
        queue::configuration(db, user, &f).await?
    };
    let configured_direction = direction;
    let Some((card, direction)) = queue::next(db, user, &f, direction).await? else {
        if let Some(id) = run_id {
            runs::finish(db, user, id, true).await?;
        }
        return Ok(dto::Next {
            card: None,
            overview: queue::overview_with_settings(db, user, &f, (direction, retention)).await?,
        });
    };
    let memory = queue::memory(db, user, &card, direction).await?;
    let profile = personal::load(db, user).await?;
    let id = Uuid::new_v4();
    attempts::ActiveModel {
        id: Set(id),
        user_id: Set(user),
        card_id: Set(card.id),
        run_id: Set(run_id),
        direction: Set(direction.into()),
        study_key: Set(queue::key(&card)),
        version: Set(memory.as_ref().map_or(0, |m| m.reviews)),
        parameter_version: Set(profile.version()),
        parameters: Set(Some(
            serde_json::to_value(&profile.parameters)
                .map_err(|e| AppError::Internal(e.to_string()))?,
        )),
        snapshot: Set(None),
        delta_days: Set(None),
        retention: Set(retention),
        revealed: Set(false),
        rating: Set(None),
        due_at: Set(None),
        reviewed_at: Set(None),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(db)
    .await?;
    let (source_session, source_place) = notes::access(db, user, card.session_id, false).await?;
    let source = dto::Source {
        id: source_session.id,
        title: source_session.title,
        shared: source_place.user_id != user,
    };
    Ok(dto::Next {
        card: Some(dto::Front {
            id,
            card_id: card.id,
            direction: direction.into(),
            front: if direction == "vi_en" {
                card.meaning
            } else {
                card.word.clone()
            },
            source,
            topics: card.topics,
            part_of_speech: card.part_of_speech,
            new: memory.is_none(),
            audio: (direction == "en_vi").then_some(dto::Audio {
                text: card.word,
                locale: card.locale,
            }),
        }),
        overview: queue::overview_with_settings(db, user, &f, (configured_direction, retention))
            .await?,
    })
}
