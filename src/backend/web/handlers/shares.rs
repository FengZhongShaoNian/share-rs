use actix_web::error::ErrorInternalServerError;
use actix_web::{Responder, post, web};
use log::info;
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::backend::repository::shares;

#[derive(Debug, Serialize)]
pub struct ShareItem {
    id: String,
    file_name: String,
    mime_type: String,
}

/// Return the list of shared files for the web frontend.
#[post("/shares")]
pub async fn get_shares(conn: web::Data<DatabaseConnection>) -> actix_web::Result<impl Responder> {
    info!("Accept request: POST /shares");

    let share_list = shares::list(conn.get_ref())
        .await
        .map_err(ErrorInternalServerError)?;

    let share_list: Vec<ShareItem> = share_list
        .into_iter()
        .map(|file| ShareItem {
            id: file.id.to_string(),
            file_name: file.name,
            mime_type: file.mime_type,
        })
        .collect();

    Ok(web::Json(share_list))
}
