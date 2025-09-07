use crate::migrator::m20250816_000002_create_uploads_table::Uploads;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250816_000002_create_chunks_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Define how to apply this migration: Create the Config table.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Chunks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Chunks::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Chunks::UploadId).string().not_null())
                    .col(ColumnDef::new(Chunks::ChunkNumber).integer().not_null())
                    .col(ColumnDef::new(Chunks::ChunkSize).big_integer().not_null())
                    .col(ColumnDef::new(Chunks::ChunkHash).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_upload_id")
                            .from(Chunks::Table, Chunks::UploadId)
                            .to(Uploads::Table, Uploads::Id),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx_upload_id_chunk_number")
                            .col(Chunks::UploadId)
                            .col(Chunks::ChunkNumber),
                    )
                    .to_owned(),
            )
            .await
    }

    // Define how to rollback this migration: Drop the Config table.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Chunks::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Chunks {
    Table,
    Id,
    UploadId,
    ChunkNumber,
    ChunkSize,
    ChunkHash,
}
