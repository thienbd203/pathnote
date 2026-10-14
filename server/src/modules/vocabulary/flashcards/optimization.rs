use super::{history, personal};
use crate::{
    access_token::CurrentUser,
    entities::{flashcard_attempts as attempts, flashcard_models as models, users},
    response::ApiResponse,
    state::AppState,
    AppError,
};
use axum::extract::State;
use fsrs::{compute_parameters, CombinedProgressState, ComputeParametersInput, FSRS};
use sea_orm::{
    sea_query::{Expr, ExprTrait, LockType, OnConflict},
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
    TransactionTrait,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
fn metadata(profile: &personal::Profile, data: &history::Dataset) -> Value {
    let samples = data.sample_count();
    let state = if samples < history::MIN_SAMPLES {
        "collecting"
    } else if profile.version() > 0 {
        "personal"
    } else if profile
        .model
        .as_ref()
        .is_some_and(|m| m.training_samples >= samples as i32)
    {
        "current"
    } else {
        "pending"
    };
    json!({"automatic":true,"state":state,"personalized":profile.version()>0,"version":profile.version(),"reviews":data.review_count(),"samples":samples,"minimum_samples":history::MIN_SAMPLES,"eligible":samples>=history::MIN_SAMPLES,"trained_reviews":profile.model.as_ref().map_or(0,|m|m.trained_reviews),"optimized_at":profile.model.as_ref().and_then(|m|m.optimized_at)})
}
pub async fn status(
    State(s): State<AppState>,
    user: CurrentUser,
) -> Result<ApiResponse<Value>, AppError> {
    let profile = personal::load(s.db.as_ref(), user.id).await?;
    let data = history::load(s.db.as_ref(), user.id).await?;
    Ok(ApiResponse::ok(metadata(&profile, &data)))
}
struct CancelTraining(Arc<Mutex<CombinedProgressState>>);
impl Drop for CancelTraining {
    fn drop(&mut self) {
        if let Ok(mut p) = self.0.lock() {
            p.want_abort = true;
        }
    }
}
struct Fit {
    parameters: Vec<f32>,
    baseline_loss: f32,
    fitted_loss: f32,
}
async fn fit(data: &history::Dataset, parameters: Vec<f32>) -> Result<Fit, AppError> {
    let (train_set, card_ids) = data.training();
    let progress = CombinedProgressState::new_shared();
    let _cancellation = CancelTraining(progress.clone());
    let mut work = tokio::task::spawn_blocking(move || {
        let fitted = compute_parameters(ComputeParametersInput {
            train_set: train_set.clone(),
            card_ids: Some(card_ids),
            progress: Some(progress),
            num_relearning_steps: Some(1),
            ..Default::default()
        })
        .map_err(|_| AppError::Unavailable("Không tối ưu được FSRS.".into()))?;
        let baseline_loss = FSRS::new(&parameters)
            .map_err(|_| AppError::Internal("Tham số FSRS không hợp lệ.".into()))?
            .evaluate(train_set.clone(), |_| true)
            .map_err(|_| AppError::Unavailable("Không đánh giá được lịch sử ôn.".into()))?
            .log_loss;
        let fitted_loss = FSRS::new(&fitted)
            .map_err(|_| AppError::Internal("Tham số FSRS không hợp lệ.".into()))?
            .evaluate(train_set, |_| true)
            .map_err(|_| AppError::Unavailable("Không đánh giá được kết quả tối ưu.".into()))?
            .log_loss;
        if !baseline_loss.is_finite()
            || !fitted_loss.is_finite()
            || fitted.iter().any(|p| !p.is_finite())
        {
            return Err(AppError::Unavailable("Kết quả tối ưu không hợp lệ.".into()));
        }
        Ok::<_, AppError>(Fit {
            parameters: fitted,
            baseline_loss,
            fitted_loss,
        })
    });
    // Neither a transaction nor a row lock is held while fitting.
    tokio::time::timeout(std::time::Duration::from_secs(90), &mut work)
        .await
        .map_err(|_| AppError::Unavailable("Tối ưu FSRS vượt thời gian cho phép.".into()))?
        .map_err(|_| AppError::Unavailable("Lượt tối ưu FSRS bị gián đoạn.".into()))?
}
async fn optimize(db: &DatabaseConnection, user: i64) -> Result<bool, AppError> {
    let profile = personal::load(db, user).await?;
    let now = chrono::Utc::now().fixed_offset();
    if profile
        .model
        .as_ref()
        .is_some_and(|m| now - m.checked_at < chrono::Duration::hours(1))
    {
        return Ok(false);
    }
    let data = history::load(db, user).await?;
    let samples = data.sample_count();
    if samples < history::MIN_SAMPLES {
        return Ok(false);
    }
    if profile.model.as_ref().is_some_and(|m| {
        samples < (m.training_samples as usize + history::MIN_SAMPLES)
            && (samples <= m.training_samples as usize
                || now - m.checked_at < chrono::Duration::days(30))
    }) {
        return Ok(false);
    }
    // Record the attempt before CPU work: a crash/error retries after an hour, not every tick.
    let txn = db.begin().await?;
    users::Entity::find_by_id(user)
        .lock(LockType::Update)
        .one(&txn)
        .await?;
    models::Entity::insert(models::ActiveModel {
        user_id: Set(user),
        parameters: Set(json!(profile.parameters)),
        version: Set(profile.version()),
        trained_reviews: Set(profile.model.as_ref().map_or(0, |m| m.trained_reviews)),
        training_samples: Set(profile.model.as_ref().map_or(0, |m| m.training_samples)),
        baseline_loss: Set(profile.model.as_ref().map_or(0.0, |m| m.baseline_loss)),
        fitted_loss: Set(profile.model.as_ref().map_or(0.0, |m| m.fitted_loss)),
        optimized_at: Set(profile.model.as_ref().and_then(|m| m.optimized_at)),
        checked_at: Set(now),
    })
    .on_conflict(
        OnConflict::column(models::Column::UserId)
            .update_column(models::Column::CheckedAt)
            .to_owned(),
    )
    .exec(&txn)
    .await?;
    txn.commit().await?;
    let fitted = fit(&data, profile.parameters.clone()).await?;
    let changed =
        fitted.fitted_loss < fitted.baseline_loss && fitted.parameters != profile.parameters;
    let txn = db.begin().await?;
    users::Entity::find_by_id(user)
        .lock(LockType::Update)
        .one(&txn)
        .await?;
    if personal::load(&txn, user).await?.version() != profile.version() {
        return Ok(false);
    }
    let finished = chrono::Utc::now().fixed_offset();
    models::Entity::insert(models::ActiveModel {
        user_id: Set(user),
        parameters: Set(json!(if changed {
            &fitted.parameters
        } else {
            &profile.parameters
        })),
        version: Set(profile.version() + i32::from(changed)),
        trained_reviews: Set(data.review_count() as i32),
        training_samples: Set(samples as i32),
        baseline_loss: Set(fitted.baseline_loss),
        fitted_loss: Set(fitted.fitted_loss),
        optimized_at: Set(if changed {
            Some(finished)
        } else {
            profile.model.as_ref().and_then(|m| m.optimized_at)
        }),
        checked_at: Set(finished),
    })
    .on_conflict(
        OnConflict::column(models::Column::UserId)
            .update_columns([
                models::Column::Parameters,
                models::Column::Version,
                models::Column::TrainedReviews,
                models::Column::TrainingSamples,
                models::Column::BaselineLoss,
                models::Column::FittedLoss,
                models::Column::OptimizedAt,
                models::Column::CheckedAt,
            ])
            .to_owned(),
    )
    .exec(&txn)
    .await?;
    txn.commit().await?;
    Ok(changed)
}
/// DB review logs are the durable work source. Restarting the server does not lose pending work.
pub async fn run_once(db: &DatabaseConnection) -> Result<u32, AppError> {
    use sea_orm::sqlx::{postgres::PgAdvisoryLock, Either};
    let mut connection = db
        .get_postgres_connection_pool()
        .acquire()
        .await
        .map_err(|_| AppError::Unavailable("Không lấy được kết nối cho optimizer.".into()))?;
    let namespace: String =
        sea_orm::sqlx::query_scalar("SELECT current_database() || ':' || current_schema()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| AppError::Unavailable("Không xác định được schema optimizer.".into()))?;
    let lock = PgAdvisoryLock::new(format!("pathnote.fsrs.optimizer:{namespace}"));
    let guard = match lock
        .try_acquire(connection)
        .await
        .map_err(|_| AppError::Unavailable("Không khóa được optimizer.".into()))?
    {
        Either::Left(guard) => guard,
        Either::Right(_) => return Ok(0),
    };
    let users: Vec<i64> = attempts::Entity::find()
        .select_only()
        .column(attempts::Column::UserId)
        .filter(attempts::Column::Rating.is_not_null())
        .filter(attempts::Column::ReviewedAt.is_not_null())
        .filter(
            Condition::any()
                .add(attempts::Column::DeltaDays.gt(0))
                .add(attempts::Column::DeltaDays.is_null()),
        )
        .group_by(attempts::Column::UserId)
        .having(
            Expr::col(attempts::Column::Id)
                .count()
                .gte(history::MIN_SAMPLES as u64),
        )
        .into_tuple()
        .all(db)
        .await?;
    let mut updated = 0;
    for user in users {
        match optimize(db, user).await {
            Ok(true) => updated += 1,
            Ok(false) => {}
            Err(error) => tracing::error!(
                user,
                ?error,
                "Tự tối ưu FSRS lỗi; giữ lịch hiện tại và thử lại sau"
            ),
        }
    }
    guard
        .release_now()
        .await
        .map_err(|_| AppError::Unavailable("Không giải phóng được optimizer.".into()))?;
    Ok(updated)
}
pub async fn run(db: Arc<DatabaseConnection>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = run_once(db.as_ref()).await {
            tracing::error!(?error, "Không kiểm tra được lịch ôn cần tối ưu; sẽ thử lại");
        }
    }
}
