use crate::backend::db::repository::{
    Chunk, UploadItem, delete_chunk_by_id, delete_upload_item, get_chunk_by_number,
    get_upload_chunks, get_upload_item, insert_chunk, insert_upload_item, update_upload_item,
};
use crate::setting::Settings;
use crate::util;
use crate::util::{
    check_file, check_file_hash, delete_file_if_exists, exists_file, get_available_filename,
};
use actix_multipart::form::{MultipartForm, json::Json, tempfile::TempFile};
use actix_web::{HttpResponse, Responder, post, web};
use anyhow::{Context, anyhow};
use chrono::Local;
use futures::AsyncWriteExt;
use futures_util::AsyncReadExt;
use log::{error, info, warn};
use sea_orm::strum::{Display as StrumDisplay, EnumString};
use sea_orm::{DatabaseConnection, IntoActiveModel, Set};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

// 常量定义
const CHUNK_FILE_PREFIX: &str = "chunk_";

/// 上传状态枚举
#[derive(EnumString, StrumDisplay, Debug, PartialEq, Serialize, Deserialize, Clone)]
enum UploadStatus {
    Uploading,
    Completed,
}

/// 初始化上传请求结构
#[derive(Deserialize)]
pub struct InitUploadRequest {
    file_name: String,
    file_size: i64,
    file_hash: String,
}

/// 初始化上传响应结构
#[derive(Serialize)]
pub struct InitUploadResponse {
    file_id: String,
    status: UploadStatus,
    uploaded_chunks: Vec<i32>,
    uploaded_size: i64,
}

/// 初始化上传端点
#[post("/upload/init")]
pub async fn init_upload(
    connection: web::Data<DatabaseConnection>,
    settings: web::Data<Arc<Settings>>,
    info: web::Json<InitUploadRequest>,
) -> impl Responder {
    let file_hash = info.file_hash.clone();
    let storage_folder = &settings.get_ref().storage_folder;
    let connection_ref = connection.get_ref();

    // 查找文件元数据
    match get_upload_item(connection_ref, &file_hash).await {
        Ok(Some(upload_item)) => {
            let status = match UploadStatus::from_str(&upload_item.status) {
                Ok(status) => status,
                Err(e) => {
                    error!("Invalid upload status in database: {}", e);
                    return HttpResponse::InternalServerError().body("Invalid upload status");
                }
            };

            match status {
                UploadStatus::Completed => {
                    handle_completed_upload(upload_item, connection_ref, storage_folder, &info)
                        .await
                }
                UploadStatus::Uploading => {
                    handle_uploading_upload(upload_item, connection_ref, storage_folder).await
                }
            }
        }
        Ok(None) => {
            // 没有找到现有记录，创建新的上传项
            match create_upload_item(connection_ref, storage_folder, &info).await {
                Ok(upload_item) => HttpResponse::Ok().json(InitUploadResponse {
                    file_id: upload_item.id,
                    status: UploadStatus::Uploading,
                    uploaded_chunks: vec![],
                    uploaded_size: 0,
                }),
                Err(e) => {
                    error!("Failed to create new upload item: {}", e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(e) => {
            error!("Failed to query uploads: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 创建上传项
async fn create_upload_item<P: AsRef<Path>>(
    connection: &DatabaseConnection,
    storage_folder: P,
    info: &InitUploadRequest,
) -> anyhow::Result<UploadItem> {
    let file_path = get_available_filename(storage_folder.as_ref().join(&info.file_name))
        .await
        .context("Failed to get available filename")?;

    // 创建新的上传记录
    let upload_item = UploadItem {
        id: info.file_hash.clone(),
        file_name: info.file_name.clone(),
        file_size: info.file_size,
        file_path: file_path.to_string_lossy().into_owned(),
        status: UploadStatus::Uploading.to_string(),
        created_at: Local::now().naive_local(),
    };

    insert_upload_item(connection, upload_item)
        .await
        .context("Failed to insert upload item")
}

type ValidChunks = Vec<Chunk>;

/// 清理无效的分片并返回剩下的有效的分片
async fn check_and_clean_invalid_chunks<P: AsRef<Path>>(
    connection: &DatabaseConnection,
    upload_dir: P,
    chunks: Vec<Chunk>,
) -> anyhow::Result<ValidChunks> {
    let mut valid_chunks = vec![];
    for chunk in chunks {
        let chunk_path = get_chunk_file(&upload_dir, chunk.chunk_number);
        match check_file(&chunk_path, &chunk.chunk_hash).await? {
            util::CheckFileResult::Valid => {
                valid_chunks.push(chunk);
            }
            util::CheckFileResult::Invalid(msg) => {
                info!(
                    "Chunk {} is invalid since {}, remove it...",
                    chunk.chunk_number, msg
                );
                delete_file_if_exists(&chunk_path)
                    .await
                    .context("Failed to delete invalid chunk")?;
                delete_chunk_by_id(connection, chunk.id).await?;
            }
        }
    }
    Ok(valid_chunks)
}

/// 处理已完成状态的上传项
async fn handle_completed_upload(
    upload_item: UploadItem,
    connection: &DatabaseConnection,
    storage_folder: &str,
    info: &InitUploadRequest,
) -> HttpResponse {
    match check_file(upload_item.file_path, &upload_item.id).await {
        Ok(util::CheckFileResult::Valid) => HttpResponse::Ok().json(InitUploadResponse {
            file_id: upload_item.id,
            status: UploadStatus::Completed,
            uploaded_chunks: vec![],
            uploaded_size: upload_item.file_size,
        }),
        Ok(util::CheckFileResult::Invalid(msg)) => {
            // 文件完整性校验不通过，重置上传项
            info!(
                "File {} is invalid since {}, resetting upload item",
                upload_item.id, msg
            );
            if let Err(e) = delete_upload_item(connection, &upload_item.id).await {
                error!("Failed to delete invalid upload item: {}", e);
                return HttpResponse::InternalServerError()
                    .body("Failed to delete invalid upload item");
            }
            match create_upload_item(connection, Path::new(storage_folder), info).await {
                Ok(new_upload_item) => HttpResponse::Ok().json(InitUploadResponse {
                    file_id: new_upload_item.id,
                    status: UploadStatus::Uploading,
                    uploaded_chunks: vec![],
                    uploaded_size: 0,
                }),
                Err(e) => {
                    error!("Failed to reset upload item: {}", e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(e) => {
            error!("Failed to check completed upload: {}", e);
            HttpResponse::InternalServerError().body("Failed to check file status")
        }
    }
}

/// 处理上传中状态的上传项
async fn handle_uploading_upload(
    upload_item: UploadItem,
    connection: &DatabaseConnection,
    storage_folder: &str,
) -> HttpResponse {
    let upload_id = upload_item.id;
    let upload_dir = Path::new(storage_folder).join(&upload_id);

    match get_upload_chunks(connection, &upload_id).await {
        Ok(chunks) => match check_and_clean_invalid_chunks(connection, upload_dir, chunks).await {
            Ok(valid_chunks) => {
                let uploaded_chunks: Vec<i32> =
                    valid_chunks.iter().map(|c| c.chunk_number).collect();
                let uploaded_size = valid_chunks.iter().map(|c| c.chunk_size).sum::<i64>();
                HttpResponse::Ok().json(InitUploadResponse {
                    file_id: upload_id,
                    status: UploadStatus::Uploading,
                    uploaded_chunks,
                    uploaded_size,
                })
            }
            Err(e) => {
                error!("Failed to query chunks: {}", e);
                HttpResponse::InternalServerError().body("Failed to check and clean invalid chunks")
            }
        },
        Err(e) => {
            error!("Failed to get upload chunks: {}", e);
            HttpResponse::InternalServerError().body("Failed to query chunks")
        }
    }
}

/// 上传元数据结构
#[derive(Debug, Deserialize)]
struct Metadata {
    file_id: String,
    chunk_number: i32, // 切片序号，从1开始
    chunk_hash: String,
}

/// 上传表单结构
#[derive(Debug, MultipartForm)]
struct UploadForm {
    #[multipart(limit = "100MB")]
    file: TempFile,
    json: Json<Metadata>,
}

/// 处理文件切片上传端点
#[post("/upload/chunk")]
pub async fn upload_chunk(
    connection: web::Data<DatabaseConnection>,
    settings: web::Data<Arc<Settings>>,
    MultipartForm(form): MultipartForm<UploadForm>,
) -> impl Responder {
    let metadata = form.json.0;
    let upload_id = metadata.file_id;
    let chunk_number = metadata.chunk_number;
    let chunk_hash = metadata.chunk_hash;
    let chunk_file = form.file;
    let storage_folder = &settings.get_ref().storage_folder;
    let connection = connection.get_ref();

    // 验证输入参数
    if upload_id.is_empty() || chunk_number <= 0 {
        return HttpResponse::BadRequest().body("Missing or invalid required fields");
    }

    // 检查上传项目是否存在
    let upload_item = match get_upload_item(connection, &upload_id).await {
        Ok(Some(item)) => item,
        Ok(None) => {
            return HttpResponse::NotFound().body("Upload item not found");
        }
        Err(e) => {
            error!("Failed to query upload item: {}", e);
            return HttpResponse::InternalServerError().body("Failed to query upload item");
        }
    };

    // 检查文件是否已完成上传
    if let Ok(UploadStatus::Completed) = UploadStatus::from_str(&upload_item.status) {
        return HttpResponse::BadRequest().body("File already completed");
    }

    // 创建上传目录
    let upload_dir = Path::new(storage_folder).join(&upload_id);
    if let Err(e) = async_fs::create_dir_all(&upload_dir).await {
        error!("Failed to create directory: {}", e);
        return HttpResponse::InternalServerError().body("Failed to create directory");
    }

    let chunk_path = get_chunk_file(&upload_dir, chunk_number);

    // 检查是否已存在相同切片
    match get_chunk_by_number(connection, &upload_id, chunk_number).await {
        Ok(Some(chunk)) => match check_file(&chunk_path, &chunk.chunk_hash).await {
            Ok(util::CheckFileResult::Valid) => {
                info!("Chunk {} already uploaded", chunk_number);
                return HttpResponse::Ok().body("Chunk already uploaded");
            }
            Ok(util::CheckFileResult::Invalid(msg)) => {
                info!(
                    "Existing chunk {} is invalid since {}, remove it",
                    chunk_number, msg
                );
                if let Err(e) = delete_file_if_exists(&chunk_path).await {
                    warn!("Failed to delete invalid chunk file: {}", e);
                }
                if let Err(e) = delete_chunk_by_id(connection, chunk.id).await {
                    error!("Failed to delete chunk record: {}", e);
                    return HttpResponse::InternalServerError()
                        .body("Failed to delete invalid chunk");
                }
            }
            Err(e) => {
                error!("Failed to check existing chunk: {}", e);
                return HttpResponse::InternalServerError().body("Failed to check existing chunk");
            }
        },
        Err(e) => {
            error!("Failed to get chunk: {}", e);
        }
        Ok(None) => {
            if let Err(e) = delete_file_if_exists(&chunk_path).await {
                warn!("Failed to delete invalid chunk file: {}", e);
            }
        }
    }

    // 保存切片文件
    let temp_file_path = chunk_file.file.path();
    if let Err(e) = async_fs::copy(temp_file_path, &chunk_path).await {
        error!("Failed to save chunk: {}", e);
        return HttpResponse::InternalServerError().body("Failed to save chunk");
    }

    // 计算切片哈希并验证
    if let Err(e) = check_file_hash(&chunk_path, &chunk_hash).await {
        // 清理无效文件
        if let Err(e) = async_fs::remove_file(&chunk_path).await {
            warn!("Failed to delete chunk file: {}", e);
        }
        error!("Chunk hash verification failed: {}", e);
        return HttpResponse::BadRequest().body("Chunk hash verification failed");
    }

    let chunk_size = chunk_file.size as i64;

    // 保存切片信息
    let chunk = Chunk {
        id: 0,
        upload_id: upload_id.to_string(),
        chunk_number,
        chunk_size,
        chunk_hash: chunk_hash.to_string(),
    };

    info!("Saving chunk {:?} to db", chunk);

    if let Err(e) = insert_chunk(connection, chunk).await {
        error!("Failed to insert chunk: {}", e);
        return HttpResponse::InternalServerError().body(format!("Failed to insert chunk: {}", e));
    }

    HttpResponse::Ok().body("Chunk uploaded successfully")
}

/// 获取分片文件路径
fn get_chunk_file<P: AsRef<Path>>(upload_dir: P, chunk_number: i32) -> PathBuf {
    upload_dir
        .as_ref()
        .join(format!("{}{}", CHUNK_FILE_PREFIX, chunk_number))
}

/// 完成上传请求结构
#[derive(Deserialize)]
pub struct CompleteUploadRequest {
    file_id: String,
}

/// 完成文件上传并验证完整性端点
#[post("/upload/complete")]
pub async fn complete_upload(
    connection: web::Data<DatabaseConnection>,
    settings: web::Data<Arc<Settings>>,
    info: web::Json<CompleteUploadRequest>,
) -> impl Responder {
    let upload_id = &info.file_id;
    let storage_folder = &settings.get_ref().storage_folder;
    let connection_ref = connection.get_ref();

    // 获取文件元数据
    let mut upload_item = match get_upload_item(connection_ref, &upload_id).await {
        Ok(Some(meta)) => meta,
        Ok(None) => return HttpResponse::NotFound().body("File not found"),
        Err(e) => {
            error!("Failed to query file metadata: {}", e);
            return HttpResponse::InternalServerError().body("Failed to query file metadata");
        }
    };

    let upload_dir = Path::new(storage_folder).join(&upload_item.id);
    match merge_chunks(connection_ref, &upload_item, &upload_dir).await {
        Ok(output_file) => {
            info!("Successfully completed upload {:?}", output_file);
        }
        Err(e) => {
            error!("Failed to merge chunks: {}", e);
            return HttpResponse::InternalServerError().body("Failed to merge chunks");
        }
    }

    // 更新文件状态为已完成
    let mut upload_item = upload_item.into_active_model();
    upload_item.status = Set(UploadStatus::Completed.to_string());
    if let Err(e) = update_upload_item(connection_ref, upload_item).await {
        error!("Failed to update upload item status: {}", e);
        return HttpResponse::InternalServerError().body("Failed to update upload item status");
    }

    // 清理临时切片文件
    if let Err(e) = async_fs::remove_dir_all(&upload_dir).await {
        warn!("Failed to remove chunk directory: {}", e);
    }

    HttpResponse::Ok().body("File uploaded and verified successfully")
}

/// 合并所有分片文件
async fn merge_chunks<P: AsRef<Path>>(
    connection: &DatabaseConnection,
    upload_item: &UploadItem,
    upload_dir: P,
) -> anyhow::Result<PathBuf> {
    let output_path = Path::new(&upload_item.file_path);
    let expected_size = upload_item.file_size;

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        async_fs::create_dir_all(parent)
            .await
            .context("Failed to create output directory")?;
    }

    let mut output_file = async_fs::File::create(output_path)
        .await
        .context("Failed to create output file")?;

    let mut chunks = get_upload_chunks(connection, &upload_item.id).await?;
    chunks.sort_by(|x, y| x.chunk_number.cmp(&y.chunk_number));

    // 计算已上传切片的大小并校验
    let total_size: i64 = chunks.iter().map(|c| c.chunk_size).sum();
    if total_size != expected_size {
        return Err(anyhow!(
            "File size mismatch, expected: {}, actual: {}",
            expected_size,
            total_size
        ));
    }

    let mut total_size = 0;
    for chunk in chunks {
        let chunk_path = get_chunk_file(&upload_dir, chunk.chunk_number);
        if !exists_file(&chunk_path).await? {
            error!("Chunk file not found: {chunk_path:?}");
            return Err(anyhow!("Chunk file not found"));
        }

        let mut chunk_file = async_fs::File::open(&chunk_path)
            .await
            .context("Failed to open chunk file")?;

        let mut chunk_data = Vec::new();
        chunk_file
            .read_to_end(&mut chunk_data)
            .await
            .context("Failed to read chunk data")?;

        total_size += chunk_data.len() as i64;
        if total_size > expected_size {
            return Err(anyhow!("File size exceeds expected size"));
        }

        output_file
            .write_all(&chunk_data)
            .await
            .context("Failed to write chunk data")?;
    }

    if total_size != expected_size {
        return Err(anyhow!(
            "File size mismatch, expected: {}, actual: {}",
            expected_size,
            total_size
        ));
    }

    // 计算SHA256并验证文件哈希
    check_file_hash(&output_path, &upload_item.id).await?;

    Ok(output_path.to_path_buf())
}
