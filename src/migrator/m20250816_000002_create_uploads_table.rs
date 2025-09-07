use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250816_000002_create_uploads_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Config table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Uploads::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Uploads::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Uploads::FileName).string().not_null())
                    .col(ColumnDef::new(Uploads::FileSize).big_integer().not_null())
                    .col(
                        ColumnDef::new(Uploads::FilePath)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Uploads::Status).string().not_null())
                    .col(ColumnDef::new(Uploads::CreatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    // Define how to rollback this migration: Drop the Config table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Uploads::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Uploads {
    Table,
    Id,
    FileName,
    FileSize,
    FilePath,
    Status,
    CreatedAt,
}
