mod duplicates;
use super::{dto::*, phonemes};
use crate::{
    entities::{ipa_library as library, users},
    AppError,
};
pub use duplicates::{pairs_duplicates, words_duplicates};
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
struct Prepared {
    key: String,
    label: String,
    content: Value,
}
fn hash(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
pub async fn list(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    q: Query,
    kind: Kind,
) -> Result<ListResource, AppError> {
    if q.page == 0 || q.page > i32::MAX as u64 {
        return Err(AppError::validation("page", "Trang không hợp lệ."));
    }
    let locale = locale(&q.locale);
    let page = library::Entity::find()
        .filter(library::Column::UserId.eq(user))
        .filter(library::Column::Locale.eq(locale))
        .filter(library::Column::Kind.eq(kind.as_str()))
        .order_by_asc(library::Column::Id)
        .paginate(db, 100);
    let total = page.num_items().await?;
    let rows = page.fetch_page(q.page - 1).await?;
    let data = if kind == Kind::Word {
        LibraryData::Words {
            version: 1,
            locale: locale.into(),
            entries: rows
                .into_iter()
                .map(|m| serde_json::from_value(m.content))
                .collect::<Result<_, _>>()
                .map_err(|e| AppError::Internal(e.to_string()))?,
        }
    } else {
        let mut sets: BTreeMap<String, PairSet> = BTreeMap::new();
        for m in rows {
            let sounds: [String; 2] = serde_json::from_value(m.content["sounds"].clone())
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let set = sets
                .entry(
                    serde_json::to_string(&sounds)
                        .map_err(|e| AppError::Internal(e.to_string()))?,
                )
                .or_insert_with(|| PairSet {
                    sounds,
                    pairs: Vec::new(),
                });
            set.pairs.push(PairContent {
                left: serde_json::from_value(m.content["left"].clone())
                    .map_err(|e| AppError::Internal(e.to_string()))?,
                right: serde_json::from_value(m.content["right"].clone())
                    .map_err(|e| AppError::Internal(e.to_string()))?,
            });
        }
        LibraryData::Pairs {
            version: 1,
            locale: locale.into(),
            sets: sets.into_values().collect(),
        }
    };
    Ok(ListResource {
        data,
        meta: crate::pagination::PageMeta::new(q.page, 100, total),
    })
}

async fn persist(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    locale: &str,
    kind: Kind,
    entries: Vec<Prepared>,
) -> Result<ImportResource, AppError> {
    let txn = db.begin().await?;
    users::Entity::find_by_id(user)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    let mut keys: HashSet<_> = library::Entity::find()
        .filter(library::Column::UserId.eq(user))
        .filter(library::Column::Locale.eq(locale))
        .filter(library::Column::Kind.eq(kind.as_str()))
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
            kind: Set(kind.as_str().into()),
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
    Ok(ImportResource { added, skipped })
}
pub async fn import_words(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: Words,
) -> Result<ImportResource, AppError> {
    let (locale, entries) = prepare_words(input)?;
    persist(db, user, &locale, Kind::Word, entries).await
}
fn prepare_words(mut input: Words) -> Result<(String, Vec<Prepared>), AppError> {
    if input.version != 1 || input.entries.is_empty() {
        return Err(invalid());
    }
    let mut entries = Vec::new();
    for e in &mut input.entries {
        e.validate(&input.locale)?;
        entries.push(Prepared {
            key: hash(&value(&key(&e.word))?),
            label: e.word.clone(),
            content: value(e)?,
        });
    }
    Ok((locale(&input.locale).into(), entries))
}
pub async fn import_pairs(
    db: &sea_orm::DatabaseConnection,
    user: i64,
    input: Pairs,
) -> Result<ImportResource, AppError> {
    let (locale, entries) = prepare_pairs(input)?;
    persist(db, user, &locale, Kind::Pair, entries).await
}
fn prepare_pairs(mut input: Pairs) -> Result<(String, Vec<Prepared>), AppError> {
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
            key: hash(&value(&(&sounds, &names))?),
            label,
            content: value(&StoredPair {
                sounds: &sounds,
                left,
                right,
            })?,
        });
    }
    Ok((locale(&input.locale).into(), entries))
}
