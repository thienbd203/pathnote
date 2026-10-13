pub use sea_orm_migration::prelude::*;

mod m20261012_101052_create_users_table;
mod m20261013_043011_create_places;
mod m20261013_045811_create_courses;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261012_101052_create_users_table::Migration),
            Box::new(m20261013_043011_create_places::Migration),
            Box::new(m20261013_045811_create_courses::Migration),
        ]
    }
}
