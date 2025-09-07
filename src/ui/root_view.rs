use crate::ui::file_list_page::FileListPage;
use crate::ui::server_info_page::ServerInfoPage;
use crate::ui::settings_page::SettingsPage;
use crate::ui::title_bar::TitleBar;
use gpui::prelude::FluentBuilder;
use gpui::{
    Bounds, Context, CursorStyle, Decorations, Entity, Global, HitboxBehavior, Hsla,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, ReadGlobal, Render,
    ResizeEdge, Size, Styled, Window, canvas, div, point, px, transparent_black,
};
use gpui_component::Theme;
use std::cmp::PartialEq;

#[derive(Debug, Eq, PartialEq)]
pub enum Pages {
    FileListPage,
    SettingsPage,
    ServerInfoPage,
}

impl Default for Pages {
    fn default() -> Self {
        Pages::FileListPage
    }
}

impl Global for Pages {}

pub struct WindowRootView {
    pub title_bar: Entity<TitleBar>,
    pub file_list_page: Entity<FileListPage>,
    pub settings_page: Entity<SettingsPage>,
    pub server_info_page: Entity<ServerInfoPage>,
}

impl Render for WindowRootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let decorations = window.window_decorations();
        let rounding = px(12.0);
        let shadow_size = px(10.0);
        let border_size = px(1.0);
        window.set_client_inset(shadow_size);

        let theme = Theme::global(cx);

        div()
            .id("window-backdrop")
            .bg(transparent_black())
            .map(|div| match decorations {
                Decorations::Server => div,
                Decorations::Client { tiling, .. } => div
                    .bg(transparent_black())
                    .child(
                        canvas(
                            |_bounds, window, _cx| {
                                window.insert_hitbox(
                                    Bounds::new(point(px(0.0), px(0.0)), window.viewport_size()),
                                    HitboxBehavior::Normal,
                                )
                            },
                            move |_bounds, hitbox, window, _cx| {
                                let mouse = window.mouse_position();
                                let viewport_size = window.viewport_size();
                                if window.is_maximized() {
                                    // Since the window.start_window_resize(edge) method is ineffective when the window is maximized,
                                    // we do not intend to proceed with the subsequent operations.
                                    return;
                                }
                                let Some(edge) = resize_edge(mouse, shadow_size, viewport_size)
                                else {
                                    return;
                                };
                                window.set_cursor_style(
                                    match edge {
                                        ResizeEdge::Top | ResizeEdge::Bottom => {
                                            CursorStyle::ResizeUpDown
                                        }
                                        ResizeEdge::Left | ResizeEdge::Right => {
                                            CursorStyle::ResizeLeftRight
                                        }
                                        ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                            CursorStyle::ResizeUpLeftDownRight
                                        }
                                        ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                            CursorStyle::ResizeUpRightDownLeft
                                        }
                                    },
                                    &hitbox,
                                );
                            },
                        )
                        .size_full()
                        .absolute(),
                    )
                    .when(!(tiling.top || tiling.right), |div| {
                        div.rounded_tr(rounding)
                    })
                    .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                    .when(!tiling.top, |div| div.pt(shadow_size))
                    .when(!tiling.bottom, |div| div.pb(shadow_size))
                    .when(!tiling.left, |div| div.pl(shadow_size))
                    .when(!tiling.right, |div| div.pr(shadow_size))
                    .on_mouse_move(|_e, window, _cx| window.refresh())
                    .on_mouse_down(MouseButton::Left, move |e, window, _cx| {
                        let pos = e.position;
                        let viewport_size = window.viewport_size();

                        if window.is_maximized() {
                            // Since the window.start_window_resize(edge) method is ineffective when the window is maximized,
                            // we do not intend to proceed with the subsequent operations.
                            return;
                        }

                        if let Some(edge) = resize_edge(pos, shadow_size, viewport_size) {
                            println!("edge: {:?}", edge);
                            window.start_window_resize(edge);
                        };
                    }),
            })
            .size_full()
            .child(
                div()
                    .cursor(CursorStyle::Arrow)
                    .map(|div| match decorations {
                        Decorations::Server => div,
                        Decorations::Client { tiling } => div
                            .border_color(theme.colors.border)
                            .when(!(tiling.top || tiling.right), |div| {
                                div.rounded_tr(rounding)
                            })
                            .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                            .when(!tiling.top, |div| div.border_t(border_size))
                            .when(!tiling.bottom, |div| div.border_b(border_size))
                            .when(!tiling.left, |div| div.border_l(border_size))
                            .when(!tiling.right, |div| div.border_r(border_size))
                            .when(!tiling.is_tiled(), |div| {
                                div.shadow(vec![gpui::BoxShadow {
                                    color: Hsla {
                                        h: 0.,
                                        s: 0.,
                                        l: 0.,
                                        a: 0.4,
                                    },
                                    blur_radius: shadow_size / 2.,
                                    spread_radius: px(0.),
                                    offset: point(px(0.0), px(0.0)),
                                }])
                            }),
                    })
                    .on_mouse_move(|_e, _, cx| {
                        cx.stop_propagation();
                    })
                    .bg(theme.colors.background)
                    // .bg(gpui::rgb(0xFF0000))
                    .size_full()
                    .flex()
                    .flex_col()
                    .justify_start()
                    .child(self.title_bar.clone())
                    .when(Pages::global(cx) == &Pages::FileListPage, |this| {
                        this.child(self.file_list_page.clone())
                    })
                    .when(Pages::global(cx) == &Pages::SettingsPage, |this| {
                        this.child(self.settings_page.clone())
                    })
                    .when(Pages::global(cx) == &Pages::ServerInfoPage, |this| {
                        this.child(self.server_info_page.clone())
                    }),
            )
    }
}

fn resize_edge(
    pos: Point<Pixels>,
    shadow_size: Pixels,
    viewport_size: Size<Pixels>,
) -> Option<ResizeEdge> {
    let edge = if pos.y < shadow_size && pos.x < shadow_size {
        ResizeEdge::TopLeft
    } else if pos.y < shadow_size && pos.x > viewport_size.width - shadow_size {
        ResizeEdge::TopRight
    } else if pos.y < shadow_size {
        ResizeEdge::Top
    } else if pos.y > viewport_size.height - shadow_size && pos.x < shadow_size {
        ResizeEdge::BottomLeft
    } else if pos.y > viewport_size.height - shadow_size
        && pos.x > viewport_size.width - shadow_size
    {
        ResizeEdge::BottomRight
    } else if pos.y > viewport_size.height - shadow_size {
        ResizeEdge::Bottom
    } else if pos.x < shadow_size {
        ResizeEdge::Left
    } else if pos.x > viewport_size.width - shadow_size {
        ResizeEdge::Right
    } else {
        return None;
    };
    Some(edge)
}
