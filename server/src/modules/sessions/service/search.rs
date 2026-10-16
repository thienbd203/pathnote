use crate::{
    modules::sessions::dto::{self, SearchOrder, SearchPeriod, SearchQuery, SessionPage},
    pagination::ListQuery,
    AppError,
};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use sea_orm::{
    sea_query::{Expr, Func, LikeExpr},
    ColumnTrait, Condition, DatabaseConnection, QueryFilter,
};
fn calendar_day(term: &str, year: i32) -> Option<NaiveDate> {
    let parts = term.split('/').collect::<Vec<_>>();
    if ![2, 3].contains(&parts.len())
        || parts[0].len() > 2
        || parts[1].len() > 2
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let year = if parts.len() == 3 {
        if parts[2].len() != 4 {
            return None;
        }
        parts[2].parse().ok()?
    } else {
        year
    };
    if year < 1 {
        return None;
    }
    NaiveDate::from_ymd_opt(year, parts[1].parse().ok()?, parts[0].parse().ok()?)
}
pub async fn search(
    db: &DatabaseConnection,
    user_id: i64,
    input: SearchQuery,
) -> Result<SessionPage, AppError> {
    let tz = dto::timezone(&input.timezone)?;
    let paging = ListQuery {
        q: input.q.clone(),
        page: input.page.unwrap_or(1),
        per_page: input.per_page,
    };
    paging.page_size()?;
    let mut query = super::owned(user_id);
    if let Some(id) = input.place_id {
        if id <= 0 {
            return Err(AppError::validation("place_id", "Nơi học không hợp lệ."));
        }
        crate::modules::places::service::get(db, user_id, id).await?;
        query = query.filter(crate::entities::sessions::Column::PlaceId.eq(id));
    }
    if input.independent == Some(true) {
        if input.course_id.is_some() {
            return Err(AppError::validation(
                "course_id",
                "Buổi học độc lập không thuộc khóa học.",
            ));
        }
        query = query.filter(crate::entities::sessions::Column::CourseId.is_null());
    }
    if let Some(period) = input.period {
        let now = Utc::now().fixed_offset();
        query = query.filter(match period {
            SearchPeriod::Past => crate::entities::sessions::Column::StartsAt.lte(now),
            SearchPeriod::Upcoming => crate::entities::sessions::Column::StartsAt.gt(now),
        });
    }
    if let Some(id) = input.course_id {
        if id <= 0 {
            return Err(AppError::validation("course_id", "Khóa học không hợp lệ."));
        }
        let course = crate::modules::courses::service::model(db, user_id, id).await?;
        if input.place_id.is_some_and(|p| p != course.place_id) {
            return Err(AppError::NotFound("Không tìm thấy khóa học.".into()));
        }
        query = query.filter(crate::entities::sessions::Column::CourseId.eq(id));
    }
    let term = input.q.as_deref().unwrap_or("").trim();
    if !term.is_empty() {
        let pattern = crate::pagination::like_contains(term);
        let title = Expr::expr(Func::lower(Expr::col((
            crate::entities::sessions::Entity,
            crate::entities::sessions::Column::Title,
        ))))
        .like(LikeExpr::new(pattern).escape('!'));
        let mut matching = Condition::any().add(title);
        if let Some(day) = calendar_day(term, Utc::now().with_timezone(&tz).year()) {
            let next = day
                .succ_opt()
                .ok_or_else(|| AppError::validation("q", "Ngày tìm kiếm không hợp lệ."))?;
            let start = tz
                .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
                .single()
                .ok_or_else(|| {
                    AppError::validation(
                        "timezone",
                        "Ngày này không có mốc bắt đầu duy nhất tại múi giờ đã chọn.",
                    )
                })?;
            let end = tz
                .from_local_datetime(&next.and_hms_opt(0, 0, 0).unwrap())
                .single()
                .ok_or_else(|| {
                    AppError::validation(
                        "timezone",
                        "Ngày kế tiếp không có mốc bắt đầu duy nhất tại múi giờ đã chọn.",
                    )
                })?;
            matching = matching.add(
                Condition::all()
                    .add(
                        crate::entities::sessions::Column::StartsAt
                            .gte(start.with_timezone(&Utc).fixed_offset()),
                    )
                    .add(
                        crate::entities::sessions::Column::StartsAt
                            .lt(end.with_timezone(&Utc).fixed_offset()),
                    ),
            );
        }
        query = query.filter(matching);
    }
    super::ordered_page(
        db,
        query,
        paging,
        matches!(input.order, Some(SearchOrder::Desc)),
    )
    .await
}
