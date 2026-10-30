use std::sync::{Arc, Mutex};

use actix_web::{App, HttpServer, web};
use log::{error, info, warn};
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use crate::backend::db::SqliteDatabaseSource;
use crate::backend::web::handlers::downloads::{get_icon_for_mime_type, stream_download};
use crate::backend::web::handlers::pages::index;
use crate::backend::web::handlers::shares::get_shares;
use crate::backend::web::handlers::uploads::{complete_upload, init_upload, upload_chunk};
use crate::backend::web::server::ServerState::{Off, On};
use crate::settings::Settings;

type ShutdownSignalSender = oneshot::Sender<()>;
type ShutdownSignalReceiver = oneshot::Receiver<()>;

struct ShutdownToken {
    sender: ShutdownSignalSender,
}

impl ShutdownToken {
    fn new() -> (Self, ShutdownSignalReceiver) {
        let (sender, receiver) = oneshot::channel();
        (Self { sender }, receiver)
    }

    fn shutdown(self) {
        info!("Sending HTTP server shutdown signal");
        if let Err(error) = self.sender.send(()) {
            error!("Failed to send HTTP server shutdown signal: {error:?}");
        }
    }
}

async fn receive_shutdown_signal(receiver: ShutdownSignalReceiver) {
    match receiver.await {
        Ok(()) => info!("Received HTTP server shutdown signal"),
        Err(error) => error!("HTTP server shutdown signal sender dropped: {error}"),
    }
}

async fn start_server(
    settings: Settings,
    shutdown_signal_receiver: ShutdownSignalReceiver,
    datasource: Arc<SqliteDatabaseSource>,
) -> anyhow::Result<()> {
    let connection = datasource.connection().await?;
    let settings = Arc::new(settings);
    let port = settings.port;

    info!("Starting HTTP server on 0.0.0.0:{port}");

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
    .bind(("0.0.0.0", port))?
    .run()
    .await?;

    info!("HTTP server stopped");
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ServerState {
    On,
    Off,
}

impl ServerState {
    pub fn is_on(self) -> bool {
        matches!(self, On)
    }
}

/// Controls the embedded Actix HTTP server from the GPUI application.
pub struct ShareServer {
    runtime: Handle,
    server_state: Arc<Mutex<ServerState>>,
    shutdown_token: Option<ShutdownToken>,
}

impl ShareServer {
    pub fn new(runtime: Handle) -> Self {
        Self {
            runtime,
            server_state: Arc::new(Mutex::new(Off)),
            shutdown_token: None,
        }
    }

    /// Start the server unless it is already running.
    pub fn start(&mut self, settings: Settings, datasource: Arc<SqliteDatabaseSource>) {
        let mut server_state = match self.server_state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };

        if *server_state == On {
            warn!("The HTTP server is already running");
            return;
        }

        let (shutdown_token, shutdown_signal_receiver) = ShutdownToken::new();
        self.shutdown_token = Some(shutdown_token);
        *server_state = On;
        drop(server_state);

        let state = self.server_state.clone();
        let runtime = self.runtime.clone();
        // Actix's server future is not `Send`, so it runs on a dedicated
        // thread with the shared Tokio runtime rather than `Handle::spawn`.
        std::thread::spawn(move || {
            if let Err(error) =
                runtime.block_on(start_server(settings, shutdown_signal_receiver, datasource))
            {
                error!("Failed to start HTTP server: {error:#}");
                *state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Off;
            }
        });
    }

    pub fn state(&self) -> ServerState {
        *self
            .server_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Stop the server if it is running.
    pub fn stop(&mut self) {
        let mut server_state = match self.server_state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };

        if *server_state == Off {
            warn!("The HTTP server is already stopped");
            return;
        }

        if let Some(shutdown_token) = self.shutdown_token.take() {
            shutdown_token.shutdown();
        }
        *server_state = Off;
    }
}
