use std::net::IpAddr;
use std::sync::Arc;

use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Icon, IconName, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, ClipboardItem, Context, Entity, Image, ImageSource, IntoElement, Render, SharedString,
    Window, div, img, px,
};
use local_ip_address::local_ip;
use log::{error, info};
use rust_i18n::t;

use crate::settings::Settings;
use crate::ui::qr_code::qr_code_image;

/// Modal that shows the LAN URL and a QR code for the sharing page.
pub struct ServerInfoDialog {
    local_ip: SharedString,
    qr_code: Option<(SharedString, Arc<Image>)>,
}

impl ServerInfoDialog {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let local_ip = match local_ip() {
            Ok(ip) => {
                info!("Local IP address: {ip}");
                match ip {
                    IpAddr::V4(ip) => ip.to_string(),
                    IpAddr::V6(ip) => format!("[{ip}]"),
                }
            }
            Err(error) => {
                error!("Failed to detect local IP address: {error}");
                "127.0.0.1".to_string()
            }
        };

        cx.new(|_| Self {
            local_ip: local_ip.into(),
            qr_code: None,
        })
    }

    /// Open the server information as a modal dialog.
    pub fn open(dialog: Entity<Self>, window: &mut Window, cx: &mut App) {
        let view = dialog.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(t!("label.server-info").to_string())
                .width(px(520.))
                .child(view.clone())
                .footer(
                    DialogFooter::new().child(
                        Button::new("close-server-info")
                            .label(t!("label.close").to_string())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }

    fn url(&self) -> SharedString {
        let port = Settings::get().port;
        format!("http://{}:{port}/web/index.html", self.local_ip).into()
    }

    fn image_for(&mut self, url: &str) -> Option<Arc<Image>> {
        if let Some((cached_url, image)) = &self.qr_code
            && cached_url.as_ref() == url
        {
            return Some(image.clone());
        }

        match qr_code_image(url) {
            Ok(image) => {
                self.qr_code = Some((url.to_string().into(), image.clone()));
                Some(image)
            }
            Err(error) => {
                error!("Failed to generate QR code: {error:#}");
                None
            }
        }
    }
}

impl Render for ServerInfoDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let url = self.url();
        let image = self.image_for(&url);

        let qr_content = match image {
            Some(image) => img(ImageSource::Image(image))
                .size(px(320.))
                .into_any_element(),
            None => div()
                .size(px(320.))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(t!("label.qr-code-unavailable").to_string())
                .into_any_element(),
        };

        div()
            .w_full()
            .py_2()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(qr_content)
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(url.clone()),
            )
            .child(
                Button::new("copy-server-url")
                    .label(t!("label.copy-url").to_string())
                    .icon(Icon::new(IconName::Copy))
                    .on_click(cx.listener(move |_this, _, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
                        window.push_notification(
                            Notification::success(t!("notification.url-copied").to_string()),
                            cx,
                        );
                    })),
            )
    }
}
