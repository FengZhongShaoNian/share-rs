use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::form::{Field, Form};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, IconName, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::{AnyWindowHandle, App, Context, Entity, Render, SharedString, Window, div, px};
use log::error;
use rust_i18n::t;

use crate::settings::Settings;

/// Modal editor for the server settings.
pub struct SettingsDialog {
    port_input: Entity<InputState>,
    upload_folder_input: Entity<InputState>,
    window_handle: AnyWindowHandle,
}

impl SettingsDialog {
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let settings = Settings::get();
        let port_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("12345")
                .default_value(settings.port.to_string())
        });
        let upload_folder_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("label.upload-folder-placeholder").into_owned())
                .default_value(settings.storage_folder)
        });

        cx.new(|_| Self {
            port_input,
            upload_folder_input,
            window_handle: window.window_handle(),
        })
    }

    /// Open the settings as a modal dialog.
    pub fn open(dialog: Entity<Self>, window: &mut Window, cx: &mut App) {
        let view = dialog.clone();
        let weak = dialog.downgrade();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let save_weak = weak.clone();
            let enter_weak = weak.clone();

            dialog
                .title(t!("label.settings").to_string())
                .width(px(560.))
                .child(view.clone())
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancel-settings")
                                .label(t!("label.cancel").to_string())
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("save-settings")
                                .label(t!("label.save").to_string())
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let saved = save_weak
                                        .update(cx, |dialog, cx| dialog.save(window, cx))
                                        .unwrap_or(false);
                                    if saved {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    enter_weak
                        .update(cx, |dialog, cx| dialog.save(window, cx))
                        .unwrap_or(false)
                })
        });
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let port_text = self.port_input.read(cx).value();
        let port = match port_text.parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => {
                window.push_notification(
                    Notification::error(t!("notification.invalid-port").to_string()),
                    cx,
                );
                return false;
            }
        };

        let storage_folder = self.upload_folder_input.read(cx).value().to_string();
        if storage_folder.trim().is_empty() {
            window.push_notification(
                Notification::error(t!("notification.empty-storage-folder").to_string()),
                cx,
            );
            return false;
        }

        let result = Settings::update(|settings| {
            settings.port = port;
            settings.storage_folder = storage_folder;
        });

        match result {
            Ok(()) => {
                window.push_notification(
                    Notification::success(t!("notification.settings-saved").to_string()),
                    cx,
                );
                true
            }
            Err(error) => {
                error!("Failed to save settings: {error:#}");
                window.push_notification(
                    Notification::error(t!("notification.settings-save-failed").to_string()),
                    cx,
                );
                false
            }
        }
    }

    fn choose_upload_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("label.choose-upload-folder").into_owned().into()),
        });
        let window_handle = self.window_handle;

        cx.spawn(async move |this, cx| {
            let folder = match paths.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    error!("Failed to open folder picker: {error}");
                    None
                }
                Err(error) => {
                    error!("Folder picker channel closed: {error}");
                    None
                }
            };

            let Some(folder) = folder else {
                return;
            };
            let folder: SharedString = folder.to_string_lossy().into_owned().into();

            cx.update(|cx| {
                window_handle
                    .update(cx, |_, window, cx| {
                        let _ = this.update(cx, |dialog, cx| {
                            dialog.upload_folder_input.update(cx, |state, cx| {
                                state.set_value(folder, window, cx);
                            });
                            cx.notify();
                        });
                    })
                    .ok();
            });
        })
        .detach();
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().py_2().child(
            Form::vertical()
                .child(
                    Field::new()
                        .label(t!("label.server-port").to_string())
                        .child(Input::new(&self.port_input).cleanable(true)),
                )
                .child(
                    Field::new()
                        .label(t!("label.upload-folder").to_string())
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().flex_1().child(Input::new(&self.upload_folder_input)))
                                .child(
                                    Button::new("choose-upload-folder")
                                        .icon(Icon::new(IconName::FolderOpen))
                                        .label(t!("label.browse").to_string())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.choose_upload_folder(cx);
                                        })),
                                ),
                        ),
                ),
        )
    }
}
