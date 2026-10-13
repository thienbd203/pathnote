use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Notes::Table)
                .col(
                    ColumnDef::new(Notes::SessionId)
                        .big_integer()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Notes::Content).json_binary().not_null())
                .col(ColumnDef::new(Notes::SchemaVersion).integer().not_null())
                .col(ColumnDef::new(Notes::Revision).integer().not_null())
                .col(
                    ColumnDef::new(Notes::SavedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Notes::Table, Notes::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::col(Notes::Revision).gte(0))
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(NoteCollaborators::Table)
                .col(
                    ColumnDef::new(NoteCollaborators::SessionId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(NoteCollaborators::UserId)
                        .big_integer()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(NoteCollaborators::SessionId)
                        .col(NoteCollaborators::UserId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(NoteCollaborators::Table, NoteCollaborators::SessionId)
                        .to(Sessions::Table, Sessions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(NoteCollaborators::Table, NoteCollaborators::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("note_collaborators_user")
                .table(NoteCollaborators::Table)
                .col(NoteCollaborators::UserId)
                .col(NoteCollaborators::SessionId)
                .to_owned(),
        )
        .await
    }
    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(NoteCollaborators::Table).to_owned())
            .await?;
        m.drop_table(Table::drop().table(Notes::Table).to_owned())
            .await
    }
}
#[derive(DeriveIden)]
enum Notes {
    Table,
    SessionId,
    Content,
    SchemaVersion,
    Revision,
    SavedAt,
}
#[derive(DeriveIden)]
enum NoteCollaborators {
    Table,
    SessionId,
    UserId,
}
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
