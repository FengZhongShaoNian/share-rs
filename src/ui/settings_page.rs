use crate::gpui_tokio::Tokio;
use crate::setting::Settings;
use crate::ui::component::input::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
    SelectRight, ShowCharacterPalette, TextInput, TextInputStyle,
};
use crate::ui::root_view::Pages;
use gpui::prelude::FluentBuilder;
use gpui::{
    AbsoluteLength, App, AppContext, AsyncApp, Context, Edges, Entity, EntityInputHandler,
    FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, KeyBinding, ParentElement,
    Pixels, Render, SharedString, Styled, UpdateGlobal, WeakEntity, Window, black, div, px, red,
    rgb, transparent_black, white,
};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::{
    Icon, IconName, StyledExt, Theme, ThemeMode, gray_50, gray_100, h_flex, neutral_400,
    neutral_500, v_flex,
};
use log::{error, info};
use rfd::AsyncFileDialog;
use rust_i18n::t;

pub struct SettingsPage {
    port_input: Entity<TextInput>,
    upload_folder_input: Entity<TextInput>,
    focus_handle: FocusHandle,
}

impl SettingsPage {
    pub fn new(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let Settings {
            port,
            storage_folder,
        } = Settings::clone();
        let port = port.to_string();
        let port_input = TextInput::new(
            Some(port.into()),
            Some("Enter server port...".into()),
            None,
            cx,
        );

        let style = TextInputStyle {
            padding: Edges {
                top: AbsoluteLength::Pixels(Pixels(4.)),
                right: AbsoluteLength::Pixels(Pixels(4.)),
                bottom: AbsoluteLength::Pixels(Pixels(0.)),
                left: AbsoluteLength::Pixels(Pixels(4.)),
            },
            ..TextInputStyle::default()
        };
        let upload_folder_input = TextInput::new(
            Some(storage_folder.into()),
            Some("Enter upload folder...".into()),
            Some(style),
            cx,
        );

        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("cmd-v", Paste, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-x", Cut, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
        ]);

        cx.new(|cx| SettingsPage {
            port_input,
            upload_folder_input,
            focus_handle: cx.focus_handle(),
        })
    }

    pub fn handle_select_upload_folder(cx: &mut Context<Self>) {
        cx.spawn(async |this: WeakEntity<SettingsPage>, cx: &mut AsyncApp| {
            let result = Tokio::spawn(cx, async {
                AsyncFileDialog::pick_folder(Default::default()).await
            })
            .unwrap()
            .await
            .unwrap();

            if let Some(file_handle) = result {
                let folder = file_handle.clone();
                let folder = folder.path();
                let folder = folder.to_str();
                let folder = folder.unwrap().to_string();
                let folder = SharedString::from(folder);
                info!("Selected upload folder：{:?}", folder);
                let setting_page = this.upgrade().unwrap();
                setting_page
                    .update(cx, move |setting_page, cx| {
                        setting_page
                            .upload_folder_input
                            .update(cx, move |text_input, _cx| {
                                text_input.content = folder;
                            })
                    })
                    .unwrap();
            }
        })
        .detach();
    }

    fn handle_save_settings(&self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let port_input = self.port_input.read(cx);
        let port = port_input.content.to_string().parse::<u16>()?;

        let folder_input = self.upload_folder_input.read(cx);
        let upload_folder = folder_input.content.clone();

        let update_fn = move |settings: &mut Settings| {
            settings.port = port;
            settings.storage_folder = upload_folder.clone().to_string();
        };
        Settings::update(Box::new(update_fn))?;
        Ok(())
    }

    fn close_settings_page(cx: &mut Context<SettingsPage>) {
        Pages::set_global(cx, Pages::FileListPage);
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        h_flex()
            .size_full()
            .content_center()
            .justify_center()
            .track_focus(&self.focus_handle(cx))
            .child(
                v_flex()
                    .w(px(580.))
                    .h(px(320.))
                    .bg(theme.colors.background)
                    .text_color(neutral_400())
                    .rounded_2xl()
                    .border_1()
                    .border_color(theme.colors.border)
                    .child(
                        h_flex()
                            .h_8()
                            .pl_4()
                            .pr_4()
                            .justify_between()
                            .child(
                                div()
                                    .when(theme.mode == ThemeMode::Dark, |this| {
                                        this.text_color(gray_100())
                                    })
                                    .when(theme.mode == ThemeMode::Light, |this| {
                                        this.text_color(black())
                                    })
                                    .font_bold()
                                    .child(t!("label.settings").to_string()),
                            )
                            .child(
                                Button::new("close-settings-page")
                                    .icon(Icon::new(IconName::Close).text_color(neutral_500()))
                                    .with_variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        Self::close_settings_page(cx);
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_grow()
                            .justify_center()
                            .content_center()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .h_16()
                                    .pl_8()
                                    .pr_8()
                                    .gap_4()
                                    .child(t!("label.server-port").to_string())
                                    .child(
                                        div()
                                            .flex_grow()
                                            .border_b_1()
                                            .border_color(theme.colors.input)
                                            .child(self.port_input.clone()),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .h_16()
                                    .pl_8()
                                    .pr_8()
                                    .gap_4()
                                    .child(t!("label.upload-folder").to_string())
                                    .child(
                                        h_flex()
                                            .flex_grow()
                                            .border_b_1()
                                            .border_color(theme.colors.input)
                                            .justify_end()
                                            .child(
                                                div()
                                                    .flex_grow()
                                                    .child(self.upload_folder_input.clone()),
                                            )
                                            .child(
                                                Button::new("choose-upload-folder-button")
                                                    .text()
                                                    .flex_none()
                                                    .font_bold()
                                                    .child(Icon::new(IconName::Folder))
                                                    .on_click(cx.listener(
                                                        |_this, _ev, _window, cx| {
                                                            Self::handle_select_upload_folder(cx);
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .h_16()
                            .pl_8()
                            .pr_8()
                            .gap_4()
                            .child(
                                Button::new("cancel-button")
                                    .flex_grow()
                                    .text()
                                    .with_variant(ButtonVariant::Secondary)
                                    .child(t!("label.cancel").to_string()),
                            )
                            .child(
                                Button::new("save-button")
                                    .flex_grow()
                                    .text()
                                    .with_variant(ButtonVariant::Secondary)
                                    .child(t!("label.save").to_string())
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        match this.handle_save_settings(cx) {
                                            Ok(_) => {
                                                Self::close_settings_page(cx);
                                            }
                                            Err(e) => {
                                                error!("Failed to save settings: {}", e);
                                                // TODO 报错提示
                                            }
                                        }
                                    })),
                            ),
                    ),
            )
    }
}

impl Focusable for SettingsPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
