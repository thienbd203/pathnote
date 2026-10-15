mod dto;
mod history;
mod log;
pub(crate) mod optimization;
mod personal;
mod queue;
mod runs;
mod scheduler;
use crate::{
    access_token::CurrentUser,
    entities::{
        flashcard_attempts as attempts, flashcard_memory as memory, flashcard_settings as settings,
        users, vocabulary_cards as cards,
    },
    modules::{notes::service as notes, places::service as places},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Path, Query, State,
    },
    routing::{get, post},
    Json, Router,
};
use sea_orm::{
    sea_query::{LockType, OnConflict},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Set,
    TransactionTrait,
};
use serde_json::{json, Value};
use uuid::Uuid;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/flashcards", get(overview).put(configure))
        .route("/flashcards/next", post(next))
        .route("/flashcards/runs", post(runs::create))
        .route("/flashcards/runs/{id}", get(runs::show))
        .route("/flashcards/runs/{id}/finish", post(runs::end))
        .route("/flashcards/history", get(log::list))
        .route("/flashcards/optimization", get(optimization::status))
        .route("/flashcards/{id}/reveal", post(reveal))
        .route("/flashcards/{id}/grade", post(grade))
}
async fn overview(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Query<dto::Filters>, QueryRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Query(mut filters) =
        input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    filters.validate()?;
    queue::authorize(s.db.as_ref(), user.id, &filters).await?;
    Ok(ApiResponse::ok(
        queue::overview(s.db.as_ref(), user.id, &filters).await?,
    ))
}
async fn configure(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Json<dto::SettingsInput>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(input) =
        input.map_err(|_| AppError::validation("settings", "Cấu hình không hợp lệ."))?;
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
    queue::authorize(s.db.as_ref(), user.id, &f).await?;
    settings::Entity::insert(settings::ActiveModel {
        user_id: Set(user.id),
        scope: Set(f.scope()),
        direction: Set(input.direction.label().into()),
        retention: Set(input.retention),
    })
    .on_conflict(
        OnConflict::columns([settings::Column::UserId, settings::Column::Scope])
            .update_columns([settings::Column::Direction, settings::Column::Retention])
            .to_owned(),
    )
    .exec(s.db.as_ref())
    .await?;
    Ok(ApiResponse::ok(
        queue::overview(s.db.as_ref(), user.id, &f).await?,
    ))
}
async fn next(
    State(s): State<AppState>,
    user: CurrentUser,
    input: Result<Json<dto::Filters>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(mut f) = input.map_err(|_| AppError::validation("filters", "Bộ lọc không hợp lệ."))?;
    f.validate()?;
    queue::authorize(s.db.as_ref(), user.id, &f).await?;
    let run_id = f.run_id;
    let saved_run = if let Some(id) = run_id {
        Some(runs::load(s.db.as_ref(), user.id, id).await?)
    } else {
        None
    };
    if let Some(run) = &saved_run {
        f = serde_json::from_value(run.filters.clone())
            .map_err(|_| AppError::Internal("Invalid saved flashcard filters".into()))?;
        queue::authorize(s.db.as_ref(), user.id, &f).await?;
        if run.finished_at.is_some() {
            return Ok(ApiResponse::ok(
                json!({"card":null,"overview":queue::overview(s.db.as_ref(),user.id,&f).await?}),
            ));
        }
    }
    let (direction, retention) = if let Some(run) = &saved_run {
        (
            serde_json::from_value::<dto::Direction>(run.settings["direction"].clone())
                .unwrap_or_default(),
            run.settings["retention"].as_f64().unwrap_or(0.9) as f32,
        )
    } else {
        queue::configuration(s.db.as_ref(), user.id, &f).await?
    };
    let configured_direction = direction;
    let Some((card, direction)) = queue::next(s.db.as_ref(), user.id, &f, direction).await? else {
        if let Some(id) = run_id {
            runs::finish(s.db.as_ref(), user.id, id, true).await?;
        }
        return Ok(ApiResponse::ok(
            json!({"card":null,"overview":queue::overview_with_settings(s.db.as_ref(),user.id,&f,(direction,retention)).await?}),
        ));
    };
    let memory = queue::memory(s.db.as_ref(), user.id, &card, direction).await?;
    let profile = personal::load(s.db.as_ref(), user.id).await?;
    let id = Uuid::new_v4();
    attempts::ActiveModel {
        id: Set(id),
        user_id: Set(user.id),
        card_id: Set(card.id),
        run_id: Set(run_id),
        direction: Set(direction.into()),
        study_key: Set(queue::key(&card)),
        version: Set(memory.as_ref().map_or(0, |m| m.reviews)),
        parameter_version: Set(profile.version()),
        parameters: Set(Some(json!(profile.parameters))),
        snapshot: Set(None),
        delta_days: Set(None),
        retention: Set(retention),
        revealed: Set(false),
        rating: Set(None),
        due_at: Set(None),
        reviewed_at: Set(None),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(s.db.as_ref())
    .await?;
    let (source_session, source_place) =
        notes::access(s.db.as_ref(), user.id, card.session_id, false).await?;
    let source = json!({"id":source_session.id,"title":source_session.title,"shared":source_place.user_id!=user.id});
    Ok(ApiResponse::ok(
        json!({"card":{"id":id,"card_id":card.id,"direction":direction,"front":if direction=="vi_en" {&card.meaning}else{&card.word},"source":source,"topics":card.topics,"part_of_speech":card.part_of_speech,"new":memory.is_none(),"audio":if direction=="en_vi" {Some(json!({"text":card.word,"locale":card.locale}))}else{None}},"overview":queue::overview_with_settings(s.db.as_ref(),user.id,&f,(configured_direction,retention)).await?}),
    ))
}
async fn ticket<C: ConnectionTrait>(
    db: &C,
    user: i64,
    id: Uuid,
    lock: bool,
) -> Result<attempts::Model, AppError> {
    let mut query = attempts::Entity::find_by_id(id).filter(attempts::Column::UserId.eq(user));
    if lock {
        query = query.lock(LockType::Update);
    }
    query
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy thẻ ôn.".into()))
}
async fn card<C: ConnectionTrait>(
    db: &C,
    user: i64,
    t: &attempts::Model,
) -> Result<cards::Model, AppError> {
    let card = super::library::scope(user, None)
        .filter(cards::Column::Id.eq(t.card_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy từ vựng.".into()))?;
    if queue::key(&card) != t.study_key {
        return Err(AppError::Conflict(
            "Từ hoặc nghĩa đã thay đổi. Hãy lấy thẻ mới.".into(),
        ));
    }
    Ok(card)
}
fn fresh(t: &attempts::Model, m: Option<&memory::Model>) -> Result<(), AppError> {
    if chrono::Utc::now().fixed_offset() - t.created_at > chrono::Duration::hours(24)
        || t.version != m.map_or(0, |m| m.reviews)
    {
        return Err(AppError::Conflict(
            "Thẻ này đã được ôn hoặc hết hạn. Hãy lấy thẻ mới.".into(),
        ));
    }
    Ok(())
}
async fn reveal(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<Value>, AppError> {
    let t = ticket(s.db.as_ref(), user.id, id, false).await?;
    let c = card(s.db.as_ref(), user.id, &t).await?;
    let (_, place) = notes::access(s.db.as_ref(), user.id, c.session_id, false).await?;
    let txn = s.db.begin().await?;
    places::lock_for_write(&txn, place.user_id, place.id).await?;
    let t = ticket(&txn, user.id, id, true).await?;
    let c = card(&txn, user.id, &t).await?;
    let profile = personal::for_ticket(&t)?;
    let m = queue::memory(&txn, user.id, &c, &t.direction).await?;
    let m = profile.memory(&txn, user.id, m).await?;
    fresh(&t, m.as_ref())?;
    let options = scheduler::outcomes(
        &profile.scheduler()?,
        m.as_ref(),
        t.retention,
        chrono::Utc::now().fixed_offset(),
    )?;
    let mut model: attempts::ActiveModel = t.into();
    model.revealed = Set(true);
    model.update(&txn).await?;
    txn.commit().await?;
    Ok(ApiResponse::ok(
        json!({"back":{"word":c.word,"meaning":c.meaning,"ipa":c.ipa,"locale":c.locale,"part_of_speech":c.part_of_speech,"examples":c.examples},"options":options}),
    ))
}
async fn grade(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    input: Result<Json<dto::Grade>, JsonRejection>,
) -> Result<ApiResponse<Value>, AppError> {
    let Json(input) =
        input.map_err(|_| AppError::validation("rating", "Đánh giá không hợp lệ."))?;
    if !(1..=4).contains(&input.rating) {
        return Err(AppError::validation(
            "rating",
            "Chọn một trong bốn mức ghi nhớ.",
        ));
    }
    let t = ticket(s.db.as_ref(), user.id, id, false).await?;
    let c = card(s.db.as_ref(), user.id, &t).await?;
    let (_, place) = notes::access(s.db.as_ref(), user.id, c.session_id, false).await?;
    let txn = s.db.begin().await?;
    places::lock_for_write(&txn, place.user_id, place.id).await?;
    // Serializes updates to this user's memory, including two independently opened cards.
    users::Entity::find_by_id(user.id)
        .lock(LockType::Update)
        .one(&txn)
        .await?;
    let t = ticket(&txn, user.id, id, true).await?;
    let c = card(&txn, user.id, &t).await?;
    if let Some(rating) = t.rating {
        if rating != i32::from(input.rating) {
            return Err(AppError::Conflict("Thẻ đã được đánh giá.".into()));
        }
        return Ok(ApiResponse::ok(json!({"rating":rating,"due_at":t.due_at})));
    }
    if let Some(run_id) = t.run_id {
        let run = crate::entities::flashcard_runs::Entity::find_by_id(run_id)
            .filter(crate::entities::flashcard_runs::Column::UserId.eq(user.id))
            .lock(LockType::Update)
            .one(&txn)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy lượt ôn.".into()))?;
        if run.finished_at.is_some() {
            return Err(AppError::Conflict("Lượt ôn đã kết thúc.".into()));
        }
    }
    if !t.revealed {
        return Err(AppError::Conflict(
            "Hãy xem đáp án trước khi đánh giá.".into(),
        ));
    }
    let profile = personal::for_ticket(&t)?;
    let m = queue::memory(&txn, user.id, &c, &t.direction).await?;
    let m = profile.memory(&txn, user.id, m).await?;
    fresh(&t, m.as_ref())?;
    let now = chrono::Utc::now().fixed_offset();
    let result = scheduler::outcomes(&profile.scheduler()?, m.as_ref(), t.retention, now)?
        .remove(input.rating as usize - 1);
    memory::Entity::insert(memory::ActiveModel {
        user_id: Set(user.id),
        card_id: Set(c.id),
        direction: Set(t.direction.clone()),
        identity: Set(queue::identity(&c)),
        study_key: Set(t.study_key.clone()),
        parameter_version: Set(profile.version()),
        stability: Set(result.stability),
        difficulty: Set(result.difficulty),
        step: Set(result.step),
        reviews: Set(t.version + 1),
        lapses: Set(m.as_ref().map_or(0, |m| m.lapses) + i32::from(input.rating == 1)),
        due_at: Set(result.due_at),
        reviewed_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            memory::Column::UserId,
            memory::Column::CardId,
            memory::Column::Direction,
        ])
        .update_columns([
            memory::Column::Identity,
            memory::Column::StudyKey,
            memory::Column::ParameterVersion,
            memory::Column::Stability,
            memory::Column::Difficulty,
            memory::Column::Step,
            memory::Column::Reviews,
            memory::Column::Lapses,
            memory::Column::DueAt,
            memory::Column::ReviewedAt,
        ])
        .to_owned(),
    )
    .exec(&txn)
    .await?;
    let mut saved: attempts::ActiveModel = t.into();
    saved.delta_days =
        Set(Some(m.as_ref().map_or(0, |m| {
            (now - m.reviewed_at).num_days().max(0) as i32
        })));
    saved.snapshot = Set(Some(
        json!({"word":c.word,"meaning":c.meaning,"ipa":c.ipa,"locale":c.locale,"part_of_speech":c.part_of_speech,"topics":c.topics,"examples":c.examples}),
    ));
    saved.reviewed_at = Set(Some(now));
    saved.rating = Set(Some(i32::from(input.rating)));
    saved.due_at = Set(Some(result.due_at));
    // Return the persisted timestamp: PostgreSQL stores microseconds, while
    // the scheduler's in-memory timestamp can contain nanoseconds.
    let saved = saved.update(&txn).await?;
    txn.commit().await?;
    Ok(ApiResponse::ok(
        json!({"rating":input.rating,"due_at":saved.due_at}),
    ))
}
