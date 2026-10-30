use std::str::FromStr;

use actix_files::NamedFile;
use actix_web::http::header::ContentType;
use actix_web::mime::Mime;
use actix_web::{Error, HttpResponse, get, mime, web};
use log::error;
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::assets::Assets;
use crate::backend::repository::shares;
use crate::mimes::get_icon_for_mime;

#[derive(Deserialize)]
struct DownloadOption {
    #[serde(default)]
    force_download: bool,
}

/// Stream a shared file.
///
/// `NamedFile` handles range requests, ETags and conditional requests.
#[get("/stream/{file_id}")]
pub async fn stream_download(
    path: web::Path<i64>,
    connection: web::Data<DatabaseConnection>,
    query: web::Query<DownloadOption>,
) -> Result<NamedFile, Error> {
    let file_id = path.into_inner();
    let share = shares::get(connection.get_ref(), file_id)
        .await
        .map_err(|error| {
            error!("Failed to find shared file {file_id}: {error:#}");
            actix_web::error::ErrorInternalServerError("Failed to query file")
        })?
        .ok_or_else(|| {
            error!("Shared file {file_id} does not exist");
            actix_web::error::ErrorNotFound("File not found")
        })?;

    let file = NamedFile::open(share.path)
        .map_err(|_| actix_web::error::ErrorNotFound("File not found"))?
        .use_last_modified(true);

    if query.force_download {
        Ok(file.set_content_type(Mime::from_str("application/octet-stream").unwrap()))
    } else {
        Ok(file)
    }
}

#[derive(Deserialize)]
struct IconForMimeTypeQuery {
    mime_type: String,
}

#[get("/icons")]
pub async fn get_icon_for_mime_type(query: web::Query<IconForMimeTypeQuery>) -> HttpResponse {
    let icon_path = get_icon_for_mime(&query.mime_type);
    let Some(data) = Assets::load_embedded(&icon_path) else {
        return HttpResponse::NotFound().finish();
    };

    HttpResponse::Ok()
        .content_type(ContentType(mime::Mime::from_str("image/svg+xml").unwrap()))
        .body(data)
}
