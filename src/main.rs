use std::env::args;
use std::path::PathBuf;
use std::process;

use futures::StreamExt;
use gpui_kit::component::TitleBar;
use gpui_kit::{
    App, AsyncApp, Bounds, Global, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowOptions, px, size,
};
use log::{error, info};
use sea_orm::DatabaseConnection;

use crate::assets::Assets;
use crate::backend::db::DatabaseSource;
use crate::backend::repository::shares;
use crate::backend::web::server::ShareServer;
use crate::gpui_tokio::Tokio;
use crate::settings::{Settings, configuration_dir};
use crate::single_instance::NextStep::{Abort, Continue};
use crate::single_instance::{OpenRequest, check_single_instance};
use crate::ui::app::AppRoot;
use crate::ui::file_list::FileListPage;

mod assets;
mod backend;
mod gpui_tokio;
mod migrator;
mod mimes;
mod settings;
mod single_instance;
mod ui;
mod util;

rust_i18n::i18n!("locales", fallback = "en");

// Backend services are stored in GPUI globals so views can reach them without
// threading handles through every constructor.
impl Global for DatabaseSource {}
impl Global for ShareServer {}

/// Add files passed on the command line (or forwarded by another instance) to
/// the share list, then refresh the desktop view.
fn add_files_to_share(paths: impl IntoIterator<Item = String>, cx: &mut App) {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    if paths.is_empty() {
        return;
    }

    let database = cx.global::<DatabaseSource>().instance.clone();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let result = Tokio::spawn(cx, async move {
            let connection: DatabaseConnection = database.connection().await?;
            let added = shares::add_many(&connection, paths).await;
            Ok::<_, anyhow::Error>(added)
        })
        .await;

        match result {
            Ok(Ok(added)) => info!("Added {} file(s) to the share list", added.len()),
            Ok(Err(error)) => error!("Failed to add files to the share list: {error:#}"),
            Err(error) => error!("File import task failed: {error}"),
        }

        cx.update(FileListPage::reload);
    })
    .detach();
}

fn handle_open_args(args: Vec<String>, cx: &mut App) {
    // The first argument is the executable path.
    add_files_to_share(args.into_iter().skip(1), cx);
}

fn main() {
    env_logger::init();

    let mut open_request_receiver = match check_single_instance() {
        Ok(Continue(receiver)) => receiver,
        Ok(Abort) => {
            info!("share-rs is already running");
            process::exit(0);
        }
        Err(error) => panic!("Failed to check single instance: {error}"),
    };

    let db_file = configuration_dir().join("data.db");
    let database = DatabaseSource::new(db_file.to_string_lossy().into_owned());

    gpui_kit::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            gpui_tokio::init(cx);

            cx.set_global(ShareServer::new(Tokio::handle(cx)));
            cx.set_global(database);

            let bounds = Bounds::centered(None, size(px(900.0), px(620.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Opaque,
                window_min_size: Some(size(px(640.0), px(420.0))),
                window_decorations: Some(WindowDecorations::Server),
                ..TitleBar::window_options()
            };

            gpui_kit::open_window(options, cx, AppRoot::new)
                .expect("Failed to open the main window");

            // Load settings once so the first render and the HTTP server use a
            // stable snapshot.
            let _ = Settings::get();

            handle_open_args(args().collect(), cx);

            cx.spawn(async move |cx: &mut AsyncApp| {
                while let Some(OpenRequest { args }) = open_request_receiver.next().await {
                    info!("Received open request: {args:?}");
                    cx.update(|cx: &mut App| handle_open_args(args, cx));
                }
            })
            .detach();
        });
}
