pub use sea_orm_migration::prelude::*;

mod m20261012_101052_create_users_table;
mod m20261013_043011_create_places;
mod m20261013_045811_create_courses;
mod m20261013_064853_create_sessions;

mod m20261013_090001_create_notes;

mod m20261013_100001_create_media;

mod m20261013_120001_add_note_yjs_state;

mod m20261014_000001_create_vocabulary_cards;

mod m20261014_010001_create_vocabulary_reviews;

mod m20261014_020001_create_exercises;

mod m20261014_030001_create_ai_imports;
mod m20261014_040001_create_ipa_library;
mod m20261014_050001_create_flashcards;
mod m20261014_060001_add_personal_fsrs;
mod m20261014_070001_add_review_snapshot;
mod m20261015_000001_create_user_settings;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261012_101052_create_users_table::Migration),
            Box::new(m20261013_043011_create_places::Migration),
            Box::new(m20261013_045811_create_courses::Migration),
            Box::new(m20261013_064853_create_sessions::Migration),
            Box::new(m20261013_090001_create_notes::Migration),
            Box::new(m20261013_100001_create_media::Migration),
            Box::new(m20261013_120001_add_note_yjs_state::Migration),
            Box::new(m20261014_000001_create_vocabulary_cards::Migration),
            Box::new(m20261014_010001_create_vocabulary_reviews::Migration),
            Box::new(m20261014_020001_create_exercises::Migration),
            Box::new(m20261014_030001_create_ai_imports::Migration),
            Box::new(m20261014_040001_create_ipa_library::Migration),
            Box::new(m20261014_050001_create_flashcards::Migration),
            Box::new(m20261014_060001_add_personal_fsrs::Migration),
            Box::new(m20261014_070001_add_review_snapshot::Migration),
            Box::new(m20261015_000001_create_user_settings::Migration),
        ]
    }
}
