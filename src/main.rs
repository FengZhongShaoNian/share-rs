use std::env::args;
use std::process;

mod assets;
mod backend;
mod gpui_tokio;
mod migrator;
mod mimes;
mod setting;
mod single_instance;
mod ui;
mod util;

use crate::assets::Assets;
use crate::backend::db::DatabaseSource;
use crate::backend::entities::shares;
use crate::backend::web::server::ShareServer;
use crate::gpui_tokio::Tokio;
use crate::mimes::detect_mime_type;
use crate::setting::configuration_dir;
use crate::single_instance::NextStep::{Abort, Continue};
use crate::single_instance::{OpenRequest, check_single_instance};
use crate::ui::file_list_page::FileListPage;
use crate::ui::root_view::{Pages, WindowRootView};
use crate::ui::server_info_page::ServerInfoPage;
use crate::ui::settings_page::SettingsPage;
use crate::ui::title_bar::TitleBar;
use futures::StreamExt;
use gpui::{
    App, Application, AsyncApp, Bounds, ReadGlobal, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, prelude::*, px, size,
};
use gpui_component::{Theme, ThemeMode};
use log::{error, info};
use sea_orm::{ActiveModelTrait, IntoActiveModel};
use snowflaked::sync::Generator;

rust_i18n::i18n!("locales", fallback = "en");

fn handle_open_args(args: Vec<String>, cx: &mut App) {
    if args.len() > 1 {
        let db = DatabaseSource::global(cx);
        let sqlite = db.instance.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            Tokio::spawn(cx, async move {
                // args[1] is program name
                for file in args.iter().skip(1) {
                    match async_fs::canonicalize(&file).await {
                        Ok(path) => {
                            let connection = sqlite.clone().connection().await.unwrap();

                            let generator = Generator::new(0);

                            let file_path = path.to_str().unwrap().to_string();
                            let model: shares::ActiveModel = shares::Model {
                                id: generator.generate(),
                                file_name: path.file_name().unwrap().to_str().unwrap().to_string(),
                                mime_type: detect_mime_type(&file_path),
                                file_path,
                            }
                            .into_active_model();
                            model.insert(&connection).await.unwrap();
                        }
                        Err(_) => {
                            error!("Failed to canonicalize path: {}", file);
                        }
                    };
                }
            })
            .unwrap()
            .await
            .unwrap();

            cx.update(|cx: &mut App| {
                FileListPage::reload(cx);
            })
        })
        .detach();
    }
}

fn main() {
    env_logger::init();

    let mut open_request_receiver = match check_single_instance() {
        Ok(next_step) => match next_step {
            Continue(rx) => rx,

            Abort => {
                info!("share-rs is already running");
                process::exit(0);
            }
        },
        Err(e) => {
            panic!("Failed to check_single_instance, {e}");
        }
    };

    let db_file = configuration_dir().join("data.db");
    let db_source = DatabaseSource::new(db_file.to_str().unwrap());

    Application::new().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        gpui_tokio::init(cx);
        cx.set_global(ShareServer::new(Tokio::handle(cx)));

        setting::Settings::init();
        cx.set_global::<DatabaseSource>(db_source);

        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Opaque,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    cx.observe_window_appearance(window, |_, window, cx| {
                        Theme::change(ThemeMode::from(window.appearance()), None, cx);
                        window.refresh();
                    })
                    .detach();

                    let title_bar = TitleBar::new(cx);
                    let file_list_page = FileListPage::new(cx);
                    FileListPage::reload(cx);
                    let settings_page = SettingsPage::new(window, cx);
                    cx.set_global::<Pages>(Pages::FileListPage);
                    let server_info_page = ServerInfoPage::new(window, cx);

                    WindowRootView {
                        title_bar,
                        file_list_page,
                        settings_page,
                        server_info_page,
                    }
                })
            },
        )
        .unwrap();

        handle_open_args(args().collect(), cx);
        cx.spawn(async move |cx| {
            while let Some(open_request) = open_request_receiver.next().await {
                cx.update(|cx: &mut App| {
                    let OpenRequest { args } = open_request;
                    info!("open_request.args: {:?}", &args);
                    handle_open_args(args, cx);
                })
                .ok();
            }
        })
        .detach();
    });
}
