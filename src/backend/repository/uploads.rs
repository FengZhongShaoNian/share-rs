use crate::backend::entities::prelude::{Chunks, Uploads};
use crate::backend::entities::{chunks, uploads};
use anyhow::Context;
pub use chunks::Model as Chunk;
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, QueryFilter};
use sea_orm::{ColumnTrait, EntityTrait};
pub use uploads::ActiveModel as UploadItemActiveModel;
pub use uploads::Model as UploadItem;

/// Find an upload by its id (the SHA-256 hash of the file).
pub async fn get_upload_item(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<Option<UploadItem>> {
    Uploads::find_by_id(upload_id)
        .one(connection)
        .await
        .context("Failed to query upload item")
}

/// Insert a new upload item.
pub async fn insert_upload_item(
    connection: &DatabaseConnection,
    upload: UploadItem,
) -> anyhow::Result<UploadItem> {
    let upload_item: uploads::ActiveModel = upload.into();
    upload_item
        .insert(connection)
        .await
        .context("Failed to insert upload item")
}

/// Update an upload item.
pub async fn update_upload_item(
    connection: &DatabaseConnection,
    upload_item: UploadItemActiveModel,
) -> anyhow::Result<UploadItem> {
    upload_item
        .update(connection)
        .await
        .context("Failed to update upload item")
}

/// Insert an uploaded chunk.
pub async fn insert_chunk(connection: &DatabaseConnection, chunk: Chunk) -> anyhow::Result<Chunk> {
    let mut chunk: chunks::ActiveModel = chunk.into();
    chunk.not_set(chunks::Column::Id);
    chunk
        .insert(connection)
        .await
        .context("Failed to insert chunk")
}

/// Return all chunks belonging to an upload.
pub async fn get_upload_chunks(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<Vec<Chunk>> {
    Chunks::find()
        .filter(chunks::Column::UploadId.eq(upload_id))
        .all(connection)
        .await
        .with_context(|| format!("Failed to query chunks with upload_id {upload_id}"))
}

/// Return a single chunk by upload id and chunk number.
pub async fn get_chunk_by_number(
    connection: &DatabaseConnection,
    upload_id: &str,
    chunk_number: i32,
) -> anyhow::Result<Option<Chunk>> {
    Chunks::find()
        .filter(chunks::Column::UploadId.eq(upload_id))
        .filter(chunks::Column::ChunkNumber.eq(chunk_number))
        .one(connection)
        .await
        .with_context(|| {
            format!("Failed to query chunk with upload_id {upload_id} and number {chunk_number}")
        })
}

/// Delete an upload and all of its chunks.
pub async fn delete_upload_item(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<()> {
    Chunks::delete_many()
        .filter(chunks::Column::UploadId.eq(upload_id))
        .exec(connection)
        .await
        .context("Failed to delete existing chunks")?;

    Uploads::delete_many()
        .filter(uploads::Column::Id.eq(upload_id))
        .exec(connection)
        .await
        .context("Failed to delete existing upload item")?;

    Ok(())
}

/// Delete a single chunk record.
pub async fn delete_chunk_by_id(
    connection: &DatabaseConnection,
    chunk_id: i32,
) -> anyhow::Result<()> {
    Chunks::delete_by_id(chunk_id)
        .exec(connection)
        .await
        .with_context(|| format!("Failed to delete chunk {chunk_id}"))?;
    Ok(())
}
