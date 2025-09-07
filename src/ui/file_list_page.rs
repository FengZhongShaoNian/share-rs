use crate::backend::db::DatabaseSource;
use crate::backend::entities::shares;
use crate::backend::entities::shares::Model;
use crate::gpui_tokio::Tokio;
use crate::mimes::get_icon_for_mime;
use crate::util::open_file_in_file_manager;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, Fill, Global, ImageSource, InteractiveElement,
    IntoElement, ParentElement, ReadGlobal, Render, Resource, StatefulInteractiveElement, Styled,
    TextOverflow, Window, div, img, px, uniform_list,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariant, ButtonVariants};
use gpui_component::{Icon, IconName, StyledExt, Theme, h_flex, neutral_500};
use rust_i18n::t;
use sea_orm::EntityTrait;

pub struct ShareList {
    data: Vec<Model>,
}

impl Default for ShareList {
    fn default() -> Self {
        Self { data: vec![] }
    }
}

impl Global for ShareList {}

pub struct FileListPage {}

impl FileListPage {
    pub fn new(cx: &mut App) -> Entity<FileListPage> {
        cx.new(|_cx| FileListPage {})
    }

    pub fn reload(cx: &mut App) {
        let db = DatabaseSource::global(cx).instance.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let data = Tokio::spawn(cx, async move {
                let connection = db.connection().await.unwrap();
                shares::Entity::find().all(&connection).await.unwrap()
            })
            .unwrap()
            .await
            .unwrap();

            let share_list = ShareList { data };
            cx.update(move |cx: &mut App| {
                cx.set_global::<ShareList>(share_list);
                cx.refresh_windows();
            })
        })
        .detach();
    }

    pub fn remove_item(share_id: i64, cx: &mut App) {
        let db = DatabaseSource::global(cx).instance.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let data = Tokio::spawn(cx, async move {
                let connection = db.connection().await.unwrap();
                shares::Entity::delete_by_id(share_id)
                    .exec(&connection)
                    .await
                    .unwrap();
                shares::Entity::find().all(&connection).await.unwrap()
            })
            .unwrap()
            .await
            .unwrap();

            let share_list = ShareList { data };
            cx.update(move |cx: &mut App| {
                cx.set_global::<ShareList>(share_list);
                cx.refresh_windows();
            })
        })
        .detach();
    }
}

impl Render for FileListPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = Theme::global(cx).background;
        let share_list_size = cx.default_global::<ShareList>().data.len();

        div().size_full().bg(background).child(
            uniform_list(
                "entries",
                share_list_size,
                cx.processor(|_this, range, _window, cx| {
                    let mut items = Vec::new();
                    let theme = Theme::global(cx);
                    let share_list = ShareList::global(cx);

                    for ix in range {
                        let item = share_list.data.get(ix);
                        items.push(
                            div()
                                .id(ix)
                                .h_full()
                                .pl(px(16.))
                                .pr(px(16.))
                                .h(px(48.))
                                .bg(theme.colors.list)
                                .text_color(theme.colors.foreground)
                                .hover(|mut style| {
                                    style.background = Some(Fill::from(theme.colors.list_hover));

                                    style
                                })
                                .active(|mut style| {
                                    style.background = Some(Fill::from(theme.colors.list_active));
                                    style
                                })
                                .on_click(move |_event, _window, _cx| {
                                    println!("clicked Item {ix:?}");
                                })
                                .when_some(item, |this, item: &Model| {
                                    let share_id = item.id;
                                    let file_name = item.file_name.clone();
                                    let file_path = item.file_path.clone();
                                    let icon_file_mime = get_icon_for_mime(&item.mime_type);
                                    this.child(
                                        h_flex()
                                            .h_full()
                                            .gap_4()
                                            .items_center()
                                            .justify_between()
                                            .flex_nowrap()
                                            .child(
                                                img(ImageSource::Resource(Resource::Embedded(
                                                    icon_file_mime.into(),
                                                )))
                                                .size_8()
                                                .flex_none(),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_grow()
                                                    .flex_shrink()
                                                    .overflow_x_hidden()
                                                    .text_overflow(TextOverflow::Truncate(
                                                        "...".into(),
                                                    ))
                                                    .child(file_name),
                                            )
                                            .child(
                                                Button::new("remove-button")
                                                    .icon(
                                                        Icon::new(IconName::Delete)
                                                            .text_color(theme.colors.danger),
                                                    )
                                                    .with_variant(ButtonVariant::Custom(
                                                        ButtonCustomVariant::new(cx)
                                                            .hover(theme.colors.primary_hover)
                                                            .active(theme.colors.primary_active),
                                                    ))
                                                    .tooltip(t!(
                                                        "tooltip.remove-file-from-share-list"
                                                    ))
                                                    .on_click(move |_ev, _window, cx| {
                                                        cx.stop_propagation();
                                                        FileListPage::remove_item(share_id, cx);
                                                    }),
                                            )
                                            .child(
                                                Button::new("open-location-button")
                                                    .icon(
                                                        Icon::new(IconName::Folder)
                                                            .text_color(neutral_500()),
                                                    )
                                                    .with_variant(ButtonVariant::Custom(
                                                        ButtonCustomVariant::new(cx)
                                                            .hover(theme.colors.primary_hover)
                                                            .active(theme.colors.primary_active),
                                                    ))
                                                    .tooltip(t!(
                                                        "tooltip.open-file-in-file-manager"
                                                    ))
                                                    .on_click(move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        open_file_in_file_manager(&file_path);
                                                    }),
                                            ),
                                    )
                                }),
                        );
                    }
                    items
                }),
            )
            .h_full(),
        )
    }
}
