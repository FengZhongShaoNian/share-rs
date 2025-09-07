use crate::backend::db::DatabaseSource;
use crate::backend::entities::shares;
use crate::backend::web::server::{ServerState, ShareServer};
use crate::gpui_tokio::Tokio;
use crate::mimes::detect_mime_type;
use crate::setting;
use crate::ui::file_list_page::FileListPage;
use crate::ui::root_view::Pages;
use crate::ui::settings_page::SettingsPage;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, Global, ImageSource, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, ReadGlobal, Render, RenderOnce,
    Resource, StatefulInteractiveElement, Styled, UpdateGlobal, WeakEntity, Window,
    WindowAppearance, WindowControlArea, img, transparent_black,
};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::switch::Switch;
use gpui_component::{Icon, IconName, Theme, h_flex, neutral_500, v_flex};
use log::{error, info};
use rfd::AsyncFileDialog;
use rust_i18n::t;
use sea_orm::{ActiveModelTrait, IntoActiveModel};
use sea_query::Iden;
use snowflaked::sync::Generator;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum WindowControlType {
    Minimize,
    Restore,
    Maximize,
    Close,
}

impl WindowControlType {
    pub fn name(&self) -> Arc<str> {
        match self {
            WindowControlType::Minimize => "minimize".into(),
            WindowControlType::Restore => "restore".into(),
            WindowControlType::Maximize => "maximize".into(),
            WindowControlType::Close => "close".into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum WindowControlState {
    Normal,
    Hover,
    Active,
    Disable,
}

impl WindowControlState {
    pub fn name(&self) -> Arc<str> {
        match self {
            WindowControlState::Normal => "normal".into(),
            WindowControlState::Hover => "hover".into(),
            WindowControlState::Active => "active".into(),
            WindowControlState::Disable => "disable".into(),
        }
    }
}

struct WindowControl {
    control_id: &'static str,
    control_type: WindowControlType,
    control_state: WindowControlState,
}

impl WindowControl {
    fn icon(&self, window_active: bool, appearance: WindowAppearance) -> String {
        let style = match appearance {
            WindowAppearance::Light => "light",
            WindowAppearance::VibrantLight => "light",
            WindowAppearance::Dark => "dark",
            WindowAppearance::VibrantDark => "dark",
        };
        if !window_active || self.control_state == WindowControlState::Disable {
            format!("icons/window_controls/backdrop-{}.svg", style)
        } else {
            let type_name = self.control_type.name();
            let state_name = self.control_state.name();
            format!(
                "icons/window_controls/{}-{}-{}.svg",
                type_name, state_name, style
            )
        }
    }
}

impl Render for WindowControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.control_type == WindowControlType::Maximize && window.is_maximized() {
            self.control_type = WindowControlType::Restore;
        } else if self.control_type == WindowControlType::Restore && !window.is_maximized() {
            self.control_type = WindowControlType::Maximize;
        }

        let icon_path = self.icon(window.is_window_active(), window.appearance());
        let icon = img(ImageSource::Resource(Resource::Embedded(icon_path.into())));

        h_flex()
            .id(self.control_id)
            .justify_center()
            .content_center()
            .cursor_pointer()
            .w_5()
            .h_5()
            .child(icon)
            .on_hover(cx.listener(|this, hover, _, cx| {
                cx.stop_propagation();
                this.control_state = match hover {
                    true => WindowControlState::Hover,
                    false => WindowControlState::Normal,
                };
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.control_state = WindowControlState::Active;
                    cx.notify();
                }),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.control_state = WindowControlState::Normal;
                cx.notify();

                match this.control_type {
                    WindowControlType::Minimize => {
                        window.minimize_window();
                    }
                    WindowControlType::Maximize | WindowControlType::Restore => {
                        window.zoom_window();
                    }
                    WindowControlType::Close => {
                        window.remove_window();
                    }
                }
            }))
    }
}

impl Global for ShareServer {}

#[derive(IntoElement)]
struct ServerControl {}

impl ServerControl {
    fn server_state_tooltip(server_state: &ServerState) -> String {
        match server_state {
            ServerState::On => t!("tooltip.server-switch-on").into_owned(),
            ServerState::Off => t!("tooltip.server-switch-off").into_owned(),
        }
    }

    fn handle_add_files_to_share_list(cx: &mut App) {
        let db = DatabaseSource::global(cx);
        let sqlite = db.instance.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            Tokio::spawn(cx, async move {
                let files = AsyncFileDialog::pick_files(Default::default()).await;

                if let Some(files) = files {
                    let connection = sqlite.clone().connection().await.unwrap();
                    let generator = Generator::new(0);

                    for file in files {
                        let file_path = file.path().to_str().unwrap().to_string();
                        let file_name = file.file_name();
                        let model: shares::ActiveModel = shares::Model {
                            id: generator.generate(),
                            file_name,
                            mime_type: detect_mime_type(&file_path),
                            file_path,
                        }
                        .into_active_model();
                        match model.insert(&connection).await {
                            Ok(_) => (),
                            Err(e) => {
                                error!("Failed to insert share file: {}", e);
                            }
                        }
                    }
                }
            })
            .unwrap()
            .await
            .unwrap();
            cx.update(|cx| {
                FileListPage::reload(cx);
            })
        })
        .detach();
    }
}

impl RenderOnce for ServerControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let server_state = ShareServer::global(cx).state();
        h_flex()
            .size_full()
            .px_3()
            .gap_2()
            .justify_start()
            .content_center()
            .child(
                Switch::new("server-switch")
                    .tooltip(Self::server_state_tooltip(&server_state))
                    .checked(match server_state {
                        ServerState::On => true,
                        ServerState::Off => false,
                    })
                    .on_click(|is_on, _, cx| {
                        let datasource = DatabaseSource::global(cx).instance.clone();
                        let share_server = cx.global_mut::<ShareServer>();
                        if *is_on {
                            share_server.start(setting::Settings::clone(), datasource);
                        } else {
                            share_server.stop();
                        }
                    }),
            )
            .child(
                h_flex()
                    .size_full()
                    .justify_end()
                    .content_center()
                    .child(
                        Button::new("add-file-button")
                            .icon(Icon::new(IconName::Plus).text_color(neutral_500()))
                            .tooltip(t!("tooltip.add-file-button"))
                            .with_variant(ButtonVariant::Ghost)
                            .on_click(|_, _, cx| {
                                cx.stop_propagation();
                                println!("add-file-button clicked");
                                Self::handle_add_files_to_share_list(cx);
                            }),
                    )
                    .child(
                        Button::new("server-info-button")
                            .icon(Icon::new(IconName::Info).text_color(neutral_500()))
                            .tooltip(t!("tooltip.server-info-button"))
                            .with_variant(ButtonVariant::Ghost)
                            .on_click(|_, _, cx| {
                                cx.stop_propagation();
                                info!("server-info-button clicked");
                                Pages::set_global(cx, Pages::ServerInfoPage);
                            }),
                    )
                    .child(
                        Button::new("setting-button")
                            .icon(Icon::new(IconName::Settings).text_color(neutral_500()))
                            .with_variant(ButtonVariant::Ghost)
                            .tooltip(t!("tooltip.setting-button"))
                            .on_click(|_, _window, cx| {
                                cx.stop_propagation();
                                info!("setting-button clicked");
                                Pages::set_global(cx, Pages::SettingsPage);
                            }),
                    ),
            )
    }
}

struct WindowControls {
    minimize: Entity<WindowControl>,
    maximize_or_restore: Entity<WindowControl>,
    close: Entity<WindowControl>,
}

impl WindowControls {
    fn new(cx: &mut App) -> Entity<Self> {
        let minimize = cx.new(|_| WindowControl {
            control_id: "minimize",
            control_type: WindowControlType::Minimize,
            control_state: WindowControlState::Normal,
        });

        let maximize_or_restore = cx.new(|_| WindowControl {
            control_id: "maximize_or_restore",
            control_type: WindowControlType::Maximize,
            control_state: WindowControlState::Normal,
        });

        let close = cx.new(|_| WindowControl {
            control_id: "close",
            control_type: WindowControlType::Close,
            control_state: WindowControlState::Normal,
        });

        cx.new(|_| WindowControls {
            minimize,
            maximize_or_restore,
            close,
        })
    }
}

impl Render for WindowControls {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("window-controls")
            .px_3()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _ev, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.minimize.clone())
            .child(self.maximize_or_restore.clone())
            .child(self.close.clone())
    }
}

pub struct TitleBar {
    window_controls: Entity<WindowControls>,
}

impl TitleBar {
    pub fn new(cx: &mut App) -> Entity<TitleBar> {
        let window_controls = WindowControls::new(cx);
        cx.new(|_| TitleBar { window_controls })
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        h_flex()
            .id("title-bar")
            .w_full()
            .justify_end()
            .h_11()
            .bg(transparent_black())
            .border_b_1()
            .border_color(theme.colors.title_bar_border)
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, ev: &MouseDownEvent, window, _cx| {
                    if ev.click_count == 1 {
                        window.start_window_move();
                    } else if ev.click_count == 2 {
                        window.zoom_window();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_this, ev: &MouseDownEvent, window, _cx| {
                    window.show_window_menu(ev.position);
                }),
            )
            .child(ServerControl {})
            .child(self.window_controls.clone())
    }
}
