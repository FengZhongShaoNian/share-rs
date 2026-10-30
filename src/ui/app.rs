use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Theme, ThemeMode, TitleBar, WindowExt, h_flex, window_border,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, IntoElement, Render, Window, div};
use rust_i18n::t;

use crate::backend::db::DatabaseSource;
use crate::backend::web::server::ShareServer;
use crate::settings::Settings;
use crate::ui::dialogs::server_info::ServerInfoDialog;
use crate::ui::dialogs::settings::SettingsDialog;
use crate::ui::file_list::FileListPage;

/// Application root view: custom title bar, share list and modal dialogs.
pub struct AppRoot {
    file_list: Entity<FileListPage>,
}

impl AppRoot {
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let file_list = FileListPage::new(window, cx);
        FileListPage::reload(cx);

        cx.new(|cx| {
            cx.observe_window_appearance(window, |_this, window, cx| {
                Theme::change(ThemeMode::from(window.appearance()), None, cx);
                cx.notify();
            })
            .detach();
            Theme::change(ThemeMode::from(window.appearance()), None, cx);

            Self { file_list }
        })
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut App) {
        SettingsDialog::open(SettingsDialog::new(window, cx), window, cx);
    }

    fn open_server_info(&mut self, window: &mut Window, cx: &mut App) {
        ServerInfoDialog::open(ServerInfoDialog::new(cx), window, cx);
    }

    fn set_server_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        let database = cx.global::<DatabaseSource>().instance.clone();
        let server = cx.global_mut::<ShareServer>();

        if enabled {
            server.start(Settings::get(), database);
        } else {
            server.stop();
        }

        cx.notify();

        let message = if enabled {
            t!("notification.server-started")
        } else {
            t!("notification.server-stopped")
        };
        window.push_notification(Notification::success(message.to_string()), cx);
    }

    fn render_title_bar(&self, server_on: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let add_file_list = self.file_list.clone();

        TitleBar::new()
            .h_11()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .gap_3()
                    .px_3()
                    .items_center()
                    .child(
                        Switch::new("server-switch")
                            .checked(server_on)
                            .tooltip(if server_on {
                                t!("tooltip.server-switch-on").to_string()
                            } else {
                                t!("tooltip.server-switch-off").to_string()
                            })
                            .on_click(cx.listener(|this, enabled, window, cx| {
                                this.set_server_enabled(*enabled, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if server_on {
                                t!("label.server-running").to_string()
                            } else {
                                t!("label.server-stopped").to_string()
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .px_3()
                    .items_center()
                    .child(
                        Button::new("add-file-button")
                            .icon(Icon::new(IconName::Plus))
                            .tooltip(t!("tooltip.add-file-button"))
                            .ghost()
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                add_file_list.update(cx, |page, cx| page.add_files(cx));
                            })),
                    )
                    .child(
                        Button::new("server-info-button")
                            .icon(Icon::new(IconName::Info))
                            .tooltip(t!("tooltip.server-info-button"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_server_info(window, cx);
                            })),
                    )
                    .child(
                        Button::new("setting-button")
                            .icon(Icon::new(IconName::Settings))
                            .tooltip(t!("tooltip.setting-button"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_settings(window, cx);
                            })),
                    ),
            )
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let server_on = cx.global::<ShareServer>().state().is_on();

        window_border().child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(self.render_title_bar(server_on, cx))
                .child(div().flex_1().min_h_0().child(self.file_list.clone())),
        )
    }
}
