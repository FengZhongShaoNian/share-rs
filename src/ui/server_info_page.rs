use crate::setting::Settings;
use crate::ui::root_view::Pages;
use clipboard_rs::{Clipboard, ClipboardContext};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, Focusable, Image, ImageFormat, ImageSource, Img,
    InteractiveElement, IntoElement, ParentElement, ReadGlobal, Render, Resource, SharedString,
    Styled, UpdateGlobal, Window, black, div, img, px, red,
};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::{
    Icon, IconName, StyledExt, Theme, ThemeMode, gray_100, h_flex, neutral_400, neutral_500, v_flex,
};
use image::Luma;
use local_ip_address::local_ip;
use log::{error, info};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode, Version};
use rust_i18n::t;
use std::io::{BufWriter, Cursor};
use std::sync::Arc;

pub struct ServerInfoPage {
    local_ip: SharedString,
}

impl ServerInfoPage {
    pub fn new(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let local_ip = local_ip().unwrap();

        info!("My local IP address: {:?}", local_ip);

        cx.new(move |_cx| ServerInfoPage {
            local_ip: local_ip.to_string().into(),
        })
    }

    fn create_qr_code(url: &String) -> Img {
        let code = QrCode::new(url.clone().into_bytes()).unwrap();
        let mut buffer = Vec::new();
        let mut writer = Cursor::new(&mut buffer);
        code.render::<Luma<u8>>()
            .min_dimensions(300, 300)
            .build()
            .write_to(&mut writer, image::ImageFormat::Png)
            .unwrap();
        let image = Image::from_bytes(ImageFormat::Png, buffer);
        img(ImageSource::Image(image.into()))
    }
}

impl Render for ServerInfoPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let settings = Settings::clone();
        let url = format!("http://{}:{}/web/index.html", self.local_ip, settings.port);
        let image = Self::create_qr_code(&url);
        let url = SharedString::from(url);
        h_flex()
            .size_full()
            .content_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(700.))
                    .h(px(480.))
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
                                    .child(t!("label.server-info").to_string()),
                            )
                            .child(
                                Button::new("close-server-info-page")
                                    .icon(Icon::new(IconName::Close).text_color(neutral_500()))
                                    .with_variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        Pages::set_global(cx, Pages::FileListPage);
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_grow()
                            .justify_center()
                            .items_center()
                            .child(image),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .h_16()
                            .gap_4()
                            .items_center()
                            .justify_center()
                            .child(url.clone())
                            .child(
                                Button::new("copy-url-button")
                                    .icon(Icon::new(IconName::Copy).text_color(neutral_500()))
                                    .with_variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                        match ClipboardContext::new() {
                                            Ok(ctx) => {
                                                if let Err(e) =
                                                    ctx.set_text(url.clone().to_string())
                                                {
                                                    error!("Failed to write url to clipboard, {e}");
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to write url to clipboard, {e}");
                                            }
                                        }
                                    })),
                            ),
                    ),
            )
    }
}
