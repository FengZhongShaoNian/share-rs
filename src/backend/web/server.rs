use crate::assets::Assets;
use crate::backend::db::SqliteDatabaseSource;
use crate::backend::entities::prelude::Shares;
use crate::backend::web::handlers::downloads::{get_icon_for_mime_type, stream_download};
use crate::backend::web::handlers::uploads::{complete_upload, init_upload, upload_chunk};
use crate::backend::web::server::ServerState::{Off, On};
use crate::setting::Settings;
use actix_web::error::ErrorInternalServerError;
use actix_web::http::header::ContentType;
use actix_web::{App, HttpResponse, HttpServer, Responder, Result, get, mime, post, web};
use log::{error, info, warn};
use mime_guess2::MimeGuess;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Serialize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use tokio::sync::oneshot;
use tokio::sync::oneshot::{Receiver, Sender};

#[derive(Debug, Serialize)]
struct ShareItem {
    id: String,
    file_name: String,
    mime_type: String,
}

#[get("/web/{path:.*}")]
async fn index(path: web::Path<(String)>) -> impl Responder {
    let path = path.into_inner();
    info!("Accept request: GET /web/{}", path);

    let file_path = format!("web/{}", path);
    let data = Assets::get(&file_path);
    if data.is_none() {
        return HttpResponse::NotFound().finish();
    }
    let buf = data.unwrap().data;
    let mime = MimeGuess::from_path(file_path)
        .first()
        .map(|m| m.to_string());
    if let Some(mime_type) = mime {
        return HttpResponse::Ok()
            .content_type(ContentType(mime::Mime::from_str(&mime_type).unwrap()))
            .body(buf);
    }
    HttpResponse::Ok().body(buf)
}

#[post("/shares")]
async fn get_shares(conn: web::Data<DatabaseConnection>) -> Result<impl Responder> {
    info!("Accept request: POST /shares");

    let result = Shares::find().all(conn.get_ref()).await;
    if let Err(e) = result {
        error!("Failed to query shares, {e}");
        return Err(ErrorInternalServerError(e));
    }
    let share_list = result.unwrap();
    let share_list: Vec<ShareItem> = share_list
        .iter()
        .map(|item| ShareItem {
            id: item.id.to_string(),
            file_name: item.file_name.clone(),
            mime_type: item.mime_type.clone(),
        })
        .collect();
    Ok(web::Json(share_list))
}

type ShutdownSignalSender = Sender<()>;
type ShutdownSignalReceiver = Receiver<()>;

struct ShutdownToken {
    sender: ShutdownSignalSender,
}

impl ShutdownToken {
    fn new() -> (Self, ShutdownSignalReceiver) {
        let (sender, receiver) = oneshot::channel();
        (Self { sender }, receiver)
    }

    fn shutdown(self) {
        info!("Sending shutdown signal...");
        if let Err(e) = self.sender.send(()) {
            error!("Failed to send shutdown signal to backend, {:?}", e);
        }
    }
}

async fn receive_shutdown_signal(shutdown_signal_receiver: ShutdownSignalReceiver) {
    match shutdown_signal_receiver.await {
        Ok(_) => {
            info!("Received shutdown signal");
        }
        Err(e) => {
            error!("The shutdown signal sender dropped, {e}");
        }
    }
}

async fn start_server(
    settings: Settings,
    shutdown_signal_receiver: ShutdownSignalReceiver,
    datasource: Arc<SqliteDatabaseSource>,
) -> std::io::Result<()> {
    let connection = datasource.clone().connection().await.unwrap();
    let settings = Arc::new(settings);
    let port = settings.port;
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(connection.clone()))
            .app_data(web::Data::new(settings.clone()))
            .service(index)
            .service(get_shares)
            .service(stream_download)
            .service(get_icon_for_mime_type)
            .service(init_upload)
            .service(upload_chunk)
            .service(complete_upload)
    })
    .shutdown_signal(receive_shutdown_signal(shutdown_signal_receiver))
    .bind(format!("[::]:{}", port))?
    .run()
    .await
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum ServerState {
    On,
    Off,
}

pub struct ShareServer {
    runtime: tokio::runtime::Handle,
    server_state: Arc<Mutex<ServerState>>,
    shutdown_token: Option<ShutdownToken>,
}

impl ShareServer {
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            runtime,
            server_state: Arc::new(Mutex::new(Off)),
            shutdown_token: None,
        }
    }

    pub fn start(&mut self, settings: Settings, datasource: Arc<SqliteDatabaseSource>) {
        let mut server_state = self.server_state.lock().unwrap();
        if *server_state == On {
            warn!("The backend is already up, do not start again!");
            return;
        }

        let (shutdown_token, shutdown_signal_receiver) = ShutdownToken::new();
        self.shutdown_token = Some(shutdown_token);
        *server_state = On;

        let server_state = self.server_state.clone();
        let runtime = self.runtime.clone();
        spawn(move || {
            if let Err(e) = runtime.block_on(async {
                start_server(settings, shutdown_signal_receiver, datasource).await
            }) {
                error!("Failed to start backend, {e}");
                let mut server_state = server_state.lock().unwrap();
                *server_state = Off;
            }
        });
    }

    pub fn state(&self) -> ServerState {
        let server_state = self.server_state.lock().unwrap();
        *server_state
    }

    pub fn stop(&mut self) {
        let mut server_state = self.server_state.lock().unwrap();
        if *server_state == Off {
            warn!("The backend is already down, do not stop again!");
            return;
        }

        if let Some(shutdown_token) = self.shutdown_token.take() {
            shutdown_token.shutdown();
            *server_state = Off;
        }
    }
}
