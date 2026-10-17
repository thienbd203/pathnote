use super::{prepare_pairs, prepare_words, Prepared};
use crate::{
    entities::ipa_library as library,
    modules::ipa::dto::{Duplicates, Pairs, Words},
    AppError,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use std::collections::HashSet;
async fn lookup(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    locale: String,
    kind: &str,
    entries: Vec<Prepared>,
) -> Result<Duplicates, AppError> {
    let mut existing: HashSet<String> = library::Entity::find()
        .filter(library::Column::UserId.eq(user))
        .filter(library::Column::Locale.eq(locale))
        .filter(library::Column::Kind.eq(kind))
        .filter(library::Column::Key.is_in(entries.iter().map(|e| e.key.clone())))
        .select_only()
        .column(library::Column::Key)
        .into_tuple()
        .all(db)
        .await?
        .into_iter()
        .collect();
    Ok(Duplicates {
        duplicates: entries
            .into_iter()
            .filter_map(|e| {
                if existing.insert(e.key) {
                    None
                } else {
                    Some(e.label)
                }
            })
            .collect(),
    })
}
pub async fn words_duplicates(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: Words,
) -> Result<Duplicates, AppError> {
    let (locale, entries) = prepare_words(input)?;
    lookup(db, user, locale, "word", entries).await
}
pub async fn pairs_duplicates(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: Pairs,
) -> Result<Duplicates, AppError> {
    let (locale, entries) = prepare_pairs(input)?;
    lookup(db, user, locale, "pair", entries).await
}
