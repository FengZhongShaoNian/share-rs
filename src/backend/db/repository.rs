use crate::backend::entities::prelude::{Chunks, Uploads};
use crate::backend::entities::{chunks, uploads};
use anyhow::Context;
pub use chunks::Model as Chunk;
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, QueryFilter};
use sea_orm::{ColumnTrait, EntityTrait};
pub use uploads::ActiveModel as UploadItemActiveModel;
pub use uploads::Model as UploadItem;

/// 根据upload_id获取对应的上传项
pub async fn get_upload_item(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<Option<UploadItem>> {
    let result = Uploads::find_by_id(upload_id).one(connection).await?;
    Ok(result)
}

/// 插入一个上传项
pub async fn insert_upload_item(
    connection: &DatabaseConnection,
    upload: UploadItem,
) -> anyhow::Result<UploadItem> {
    let upload_item: uploads::ActiveModel = upload.into();
    let result = upload_item.insert(connection).await?;
    Ok(result)
}

/// 更新上传项
pub async fn update_upload_item(
    connection: &DatabaseConnection,
    upload_item: UploadItemActiveModel,
) -> anyhow::Result<UploadItem> {
    let result = upload_item.update(connection).await?;
    Ok(result)
}

/// 插入一个上传分片
pub async fn insert_chunk(connection: &DatabaseConnection, chunk: Chunk) -> anyhow::Result<Chunk> {
    let mut chunk: chunks::ActiveModel = chunk.into();
    chunk.not_set(chunks::Column::Id);
    let result = chunk.insert(connection).await?;
    Ok(result)
}

/// 根据上传ID获取所有上传分片记录
pub async fn get_upload_chunks(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<Vec<Chunk>> {
    let result = Chunks::find()
        .filter(chunks::Column::UploadId.eq(upload_id))
        .all(connection)
        .await
        .context(format!(
            "Failed to query chunks with upload_id {}",
            upload_id
        ))?;
    Ok(result)
}

/// 根据上传ID和分片序号来获取分片
pub async fn get_chunk_by_number(
    connection: &DatabaseConnection,
    upload_id: &str,
    chunk_number: i32,
) -> anyhow::Result<Option<Chunk>> {
    let result = Chunks::find()
        .filter(chunks::Column::UploadId.eq(upload_id))
        .filter(chunks::Column::ChunkNumber.eq(chunk_number))
        .one(connection)
        .await
        .context(format!(
            "Failed to query chunks with upload_id {} and chunk_number {}",
            upload_id, chunk_number
        ))?;
    Ok(result)
}

/// 删除上传项
pub async fn delete_upload_item(
    connection: &DatabaseConnection,
    upload_id: &str,
) -> anyhow::Result<()> {
    // 删除相关的chunks记录
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

pub async fn delete_chunk_by_id(
    connection: &DatabaseConnection,
    chunk_id: i32,
) -> anyhow::Result<()> {
    let _ = Chunks::delete_by_id(chunk_id)
        .exec(connection)
        .await
        .context(format!("Failed to delete chunk {}", chunk_id))?;
    Ok(())
}
