//! Hocuspocus owns CRDT synchronization. These endpoints only bridge it to our data and permissions.
use super::{dto::NoteResource, service};
use crate::{
    access_token::CurrentUser,
    entities::{notes, sessions, users},
    modules::places::service as places,
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::{
    extract::{DefaultBodyLimit, Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/sessions/{id}/collaboration", get(config))
        .merge(
            Router::new()
                .route("/sessions/{id}/collaboration/authorize", post(authorize))
                .route("/internal/notes/{id}", get(fetch).put(store))
                .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
                .layer(middleware::from_fn_with_state(state, bridge_secret)),
        )
}
async fn bridge_secret(
    State(s): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let expected = s
        .config
        .collaboration_secret
        .as_deref()
        .filter(|v| v.len() >= 32)
        .ok_or(AppError::Forbidden)?;
    if s.config.collaboration_url.is_none() {
        return Err(AppError::Forbidden);
    }
    let provided = request
        .headers()
        .get("x-collaboration-secret")
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    if !bool::from(expected.as_bytes().ct_eq(provided)) {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}
async fn config(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<Value>, AppError> {
    service::access(s.db.as_ref(), u.id, id, false).await?;
    // Persisted CRDT documents must never fall back to JSON autosave: that loses their history.
    if s.config.collaboration_url.is_none()
        && notes::Entity::find_by_id(id)
            .one(s.db.as_ref())
            .await?
            .is_some_and(|n| n.yjs_state.is_some())
    {
        return Err(AppError::BadRequest(
            "Realtime chưa được cấu hình cho ghi chép này.".into(),
        ));
    }
    Ok(ApiResponse::ok(
        json!({"enrichment_enabled":s.learning_services.enabled(), "enabled":s.config.collaboration_url.is_some(), "url":s.config.collaboration_url, "document_name":format!("session-{id}")}),
    ))
}
async fn authorize(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<Value>, AppError> {
    service::access(s.db.as_ref(), u.id, id, false).await?;
    let user = users::Entity::find_by_id(u.id)
        .one(s.db.as_ref())
        .await?
        .ok_or(AppError::Unauthenticated)?;
    Ok(ApiResponse::ok(json!({"id":user.id, "name":user.name})))
}
async fn active<C: sea_orm::ConnectionTrait>(db: &C, id: i64) -> Result<sessions::Model, AppError> {
    service::active()
        .filter(sessions::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy ghi chép.".into()))
}
async fn snapshot<C: sea_orm::ConnectionTrait>(db: &C, id: i64) -> Result<Value, AppError> {
    if let Some(n) = notes::Entity::find_by_id(id).one(db).await? {
        Ok(
            json!({"session_id":id,"content":n.content,"schema_version":n.schema_version,"revision":n.revision,"saved_at":n.saved_at,"state":n.yjs_state.map(|bytes| STANDARD.encode(bytes))}),
        )
    } else {
        let mut result = serde_json::to_value(service::note(db, id).await?)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        result["state"] = Value::Null;
        Ok(result)
    }
}
async fn fetch(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<ApiResponse<Value>, AppError> {
    active(s.db.as_ref(), id).await?;
    Ok(ApiResponse::ok(snapshot(s.db.as_ref(), id).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveState {
    state: String,
    content: Value,
    base_revision: i32,
}
async fn store(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<SaveState>,
) -> Result<Response, AppError> {
    let session = active(s.db.as_ref(), id).await?;
    let txn = s.db.begin().await?;
    let place = crate::entities::places::Entity::find_by_id(session.place_id)
        .one(&txn)
        .await?
        .ok_or(AppError::Forbidden)?;
    places::lock_for_write(&txn, place.user_id, place.id).await?;
    active(&txn, id).await?;
    if !(0..i32::MAX).contains(&input.base_revision) || input.state.len() > 2_800_000 {
        return Err(AppError::validation(
            "state",
            "Trạng thái đồng bộ không hợp lệ.",
        ));
    }
    let bytes = STANDARD
        .decode(&input.state)
        .map_err(|_| AppError::validation("state", "Trạng thái đồng bộ không hợp lệ."))?;
    if bytes.is_empty() || bytes.len() > 2_100_000 {
        return Err(AppError::validation(
            "state",
            "Trạng thái đồng bộ quá lớn hoặc rỗng.",
        ));
    }
    let current = service::note(&txn, id).await?;
    // Only the trusted Hocuspocus bridge supplies binary state; Rust still validates the JSON and media ownership.
    let version = current.schema_version.max(3);
    service::validate_content(&txn, id, &input.content, version).await?;
    let old = notes::Entity::find_by_id(id).one(&txn).await?;
    if old
        .as_ref()
        .is_some_and(|n| n.yjs_state.as_ref() == Some(&bytes) && n.content == input.content)
    {
        return Ok(ApiResponse::ok(current).into_response());
    }
    if current.revision != input.base_revision {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({"message":"Ghi chép đã thay đổi.","data":current})),
        )
            .into_response());
    }
    let now = chrono::Utc::now().fixed_offset();
    let next = notes::ActiveModel {
        session_id: Set(id),
        content: Set(input.content),
        yjs_state: Set(Some(bytes)),
        schema_version: Set(version),
        revision: Set(current.revision + 1),
        saved_at: Set(now),
    };
    if old.is_some() {
        next.update(&txn).await?;
    } else {
        next.insert(&txn).await?;
    }
    sessions::Entity::update_many()
        .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
        .filter(sessions::Column::Id.eq(id))
        .exec(&txn)
        .await?;
    let note: NoteResource = service::note(&txn, id).await?;
    crate::modules::vocabulary::sync::sync(&txn, id, &note.content).await?;
    txn.commit().await?;
    Ok(ApiResponse::ok(note).into_response())
}
