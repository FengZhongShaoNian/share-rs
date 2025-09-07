use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250816_000002_create_share_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Config table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Shares::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Shares::Id)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Shares::FileName).string().not_null())
                    .col(ColumnDef::new(Shares::FilePath).string().not_null())
                    .col(ColumnDef::new(Shares::MimeType).string().not_null())
                    .to_owned(),
            )
            .await
    }

    // Define how to rollback this migration: Drop the Config table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Shares::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Shares {
    Table,
    Id,
    FileName,
    FilePath,
    MimeType,
}
