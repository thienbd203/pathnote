use crate::{access_token::CurrentUser, response::ApiResponse, state::AppState, AppError};
use axum::extract::{Path, State};
use sea_orm::{DbBackend, FromQueryResult, Statement};
use serde::Serialize;

#[derive(FromQueryResult, Serialize)]
pub struct SessionStats {
    id: i64,
    words: i64,
    exercises: i64,
}
#[derive(Serialize)]
pub struct CourseStats {
    sessions: Vec<SessionStats>,
    words: i64,
    exercises: i64,
}

// Count blocks in the current note, including realtime saves. Stale extracted cards
// or exercises which were created but never inserted in a note are not counted.
pub async fn overview(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<ApiResponse<CourseStats>, AppError> {
    super::service::model(state.db.as_ref(), user.id, id).await?;
    let rows = SessionStats::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"SELECT s.id,
            (SELECT COUNT(DISTINCT node #>> '{attrs,id}')
             FROM jsonb_path_query(n.content, '$.** ? (@.type == "vocabularyEntry")') AS node
             WHERE btrim(COALESCE(node #>> '{attrs,word}', '')) <> '') AS words,
            (SELECT COUNT(DISTINCT node #>> '{attrs,exerciseId}')
             FROM jsonb_path_query(n.content, '$.** ? (@.type == "exercise")') AS node) AS exercises
           FROM sessions s
           JOIN courses c ON c.id = s.course_id AND c.deleted_at IS NULL
           JOIN places p ON p.id = s.place_id AND p.deleted_at IS NULL
           LEFT JOIN notes n ON n.session_id = s.id
           WHERE s.course_id = $1 AND p.user_id = $2 AND s.deleted_at IS NULL
           ORDER BY s.starts_at, s.id"#,
        [id.into(), user.id.into()],
    ))
    .all(state.db.as_ref())
    .await?;
    Ok(ApiResponse::ok(CourseStats {
        words: rows.iter().map(|row| row.words).sum(),
        exercises: rows.iter().map(|row| row.exercises).sum(),
        sessions: rows,
    }))
}
