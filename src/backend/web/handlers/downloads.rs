use crate::assets::Assets;
use crate::backend::entities::shares;
use crate::mimes::get_icon_for_mime;
use actix_files::NamedFile;
use actix_web::http::header::ContentType;
use actix_web::mime::Mime;
use actix_web::{Error, HttpResponse, get, mime, web};
use log::error;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Deserialize)]
struct DownloadOption {
    force_download: bool,
}

#[get("/stream/{file_id}")]
pub async fn stream_download(
    path: web::Path<i64>,
    connection: web::Data<DatabaseConnection>,
    query: web::Query<DownloadOption>,
) -> Result<NamedFile, Error> {
    let file_id = path.into_inner();
    let force_download = query.force_download;
    let result = shares::Entity::find_by_id(file_id)
        .all(connection.get_ref())
        .await;

    if let Err(e) = result {
        error!("Failed to find shares by id, {e}");
        return Err(actix_web::error::ErrorNotFound("File not found"));
    }

    let result = result.unwrap();
    let share_info = result.first();
    if share_info.is_none() {
        error!("Failed to find shares by id, file_id={file_id}");
        return Err(actix_web::error::ErrorNotFound("File not found"));
    }
    let file_path = share_info.unwrap().file_path.clone();
    let file_path = PathBuf::from(file_path);

    // 使用 NamedFile 会自动处理范围请求、ETag 等
    match NamedFile::open(file_path) {
        Ok(file) => {
            if force_download {
                Ok(file
                    .use_last_modified(true)
                    .set_content_type(Mime::from_str("application/octet-stream").unwrap()))
            } else {
                Ok(file.use_last_modified(true))
            }
        }
        Err(_) => Err(actix_web::error::ErrorNotFound("File not found")),
    }
}

#[derive(Deserialize)]
struct IconForMimeTypeQuery {
    mime_type: String,
}

#[get("/icons")]
pub async fn get_icon_for_mime_type(query: web::Query<IconForMimeTypeQuery>) -> HttpResponse {
    let mime_type = &query.mime_type;

    let icon_path = get_icon_for_mime(mime_type);
    let data = Assets::get(&*icon_path);
    if data.is_none() {
        return HttpResponse::NotFound().finish();
    }
    let data = data.unwrap();

    HttpResponse::Ok()
        .content_type(ContentType(mime::Mime::from_str("image/svg+xml").unwrap()))
        .body(data.data)
}
