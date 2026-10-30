use actix_web::http::header::ContentType;
use actix_web::{HttpResponse, Responder, get, mime, web};
use log::info;
use mime_guess2::MimeGuess;
use std::str::FromStr;

use crate::assets::Assets;

/// Serve the embedded web frontend from `assets/web`.
#[get("/web/{path:.*}")]
pub async fn index(path: web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    info!("Accept request: GET /web/{path}");

    let asset_path = format!("web/{path}");
    let Some(data) = Assets::load_embedded(&asset_path) else {
        return HttpResponse::NotFound().finish();
    };

    match MimeGuess::from_path(&asset_path).first() {
        Some(mime) => match mime::Mime::from_str(mime.as_ref()) {
            Ok(content_type) => HttpResponse::Ok()
                .content_type(ContentType(content_type))
                .body(data),
            Err(_) => HttpResponse::Ok().body(data),
        },
        None => HttpResponse::Ok().body(data),
    }
}
