use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::list::{List, ListDelegate, ListItem, ListState};
use gpui_kit::component::{ActiveTheme, Icon, IconName, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, Global, ImageSource, IntoElement, PathPromptOptions, Render, Resource,
    Styled, TextOverflow, WeakEntity, Window, div, img, px,
};
use log::{error, warn};
use rust_i18n::t;

use crate::backend::db::DatabaseSource;
use crate::backend::repository::shares::{self, ShareFile};
use crate::gpui_tokio::Tokio;
use crate::mimes::get_icon_for_mime;

/// Points at the active file list so non-view code (the single-instance
/// listener and command-line file opener) can ask it to refresh.
struct FileListView(WeakEntity<FileListPage>);

impl Global for FileListView {}

/// The desktop share list.
pub struct FileListPage {
    list_state: Entity<ListState<ShareListDelegate>>,
    request_id: u64,
}

impl FileListPage {
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let page = cx.new(|cx| {
            let delegate = ShareListDelegate {
                items: Vec::new(),
                loading: true,
                page: cx.weak_entity(),
            };
            let list_state = cx.new(|cx| ListState::new(delegate, window, cx).selectable(false));

            Self {
                list_state,
                request_id: 0,
            }
        });

        cx.set_global(FileListView(page.downgrade()));
        page
    }

    /// Reload the list from the database.
    pub fn reload(cx: &mut App) {
        let Some(view) = cx.try_global::<FileListView>() else {
            return;
        };
        let view = view.0.clone();
        let _ = view.update(cx, |page, cx| page.reload_from_database(cx));
    }

    fn reload_from_database(&mut self, cx: &mut Context<Self>) {
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        let database = cx.global::<DatabaseSource>().instance.clone();

        self.set_loading(true, cx);
        cx.spawn(async move |this, cx| {
            let result = Tokio::spawn(cx, async move {
                let connection = database.connection().await?;
                shares::list(&connection).await
            })
            .await;

            let _ = this.update(cx, |page, cx| {
                if page.request_id != request_id {
                    return;
                }

                page.set_loading(false, cx);
                match result {
                    Ok(Ok(items)) => page.set_items(items, cx),
                    Ok(Err(error)) => error!("Failed to load shared files: {error:#}"),
                    Err(error) => error!("Shared file query task failed: {error}"),
                }
            });
        })
        .detach();
    }

    /// Open the native file picker and add the selected files.
    pub fn add_files(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t!("label.choose-files").into_owned().into()),
        });

        let database = cx.global::<DatabaseSource>().instance.clone();

        cx.spawn(async move |this, cx| {
            let paths = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    error!("Failed to open file picker: {error}");
                    return;
                }
                Err(error) => {
                    error!("File picker channel closed: {error}");
                    return;
                }
            };

            let result = Tokio::spawn(cx, async move {
                let connection = database.connection().await?;
                Ok::<_, anyhow::Error>(shares::add_many(&connection, paths).await)
            })
            .await;

            match result {
                Ok(Ok(items)) => {
                    if items.is_empty() {
                        warn!("No files were added from the picker");
                    }
                }
                Ok(Err(error)) => error!("Failed to add selected files: {error:#}"),
                Err(error) => error!("Add files task failed: {error}"),
            }

            let _ = this.update(cx, |page, cx| page.reload_from_database(cx));
        })
        .detach();
    }

    fn remove(&mut self, share_id: i64, cx: &mut Context<Self>) {
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        let database = cx.global::<DatabaseSource>().instance.clone();

        self.set_loading(true, cx);
        cx.spawn(async move |this, cx| {
            let result = Tokio::spawn(cx, async move {
                let connection = database.connection().await?;
                shares::remove(&connection, share_id).await?;
                shares::list(&connection).await
            })
            .await;

            let _ = this.update(cx, |page, cx| {
                if page.request_id != request_id {
                    return;
                }

                page.set_loading(false, cx);
                match result {
                    Ok(Ok(items)) => page.set_items(items, cx),
                    Ok(Err(error)) => error!("Failed to remove shared file: {error:#}"),
                    Err(error) => error!("Remove shared file task failed: {error}"),
                }
            });
        })
        .detach();
    }

    fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.list_state.update(cx, |state, cx| {
            state.delegate_mut().loading = loading;
            cx.notify();
        });
    }

    fn set_items(&mut self, items: Vec<ShareFile>, cx: &mut Context<Self>) {
        self.list_state.update(cx, |state, cx| {
            state.delegate_mut().items = items;
            cx.notify();
        });
    }
}

impl Render for FileListPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .child(List::new(&self.list_state).size_full())
    }
}

/// Delegate that renders the virtualized share list.
pub struct ShareListDelegate {
    items: Vec<ShareFile>,
    loading: bool,
    page: WeakEntity<FileListPage>,
}

impl ListDelegate for ShareListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.items.get(ix.row)?.clone();
        let icon = get_icon_for_mime(&item.mime_type);
        let page = self.page.clone();
        let share_id = item.id;
        let open_path = item.path.clone();
        let remove_page = page.clone();

        Some(
            ListItem::new(("share-file", ix.row))
                .accessibility_label(item.name.clone())
                .h(px(52.))
                .px_4()
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .child(
                            img(ImageSource::Resource(Resource::Embedded(icon.into())))
                                .size_8()
                                .flex_none(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_overflow(TextOverflow::Truncate("…".into()))
                                .child(item.name.clone()),
                        )
                        .child(
                            Button::new(("open-file", ix.row))
                                .icon(
                                    Icon::new(IconName::FolderOpen)
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .tooltip(t!("tooltip.open-file-in-file-manager"))
                                .ghost()
                                .on_click(move |_, _, cx| {
                                    cx.reveal_path(&open_path);
                                }),
                        )
                        .child(
                            Button::new(("remove-file", ix.row))
                                .icon(Icon::new(IconName::Delete).text_color(cx.theme().danger))
                                .tooltip(t!("tooltip.remove-file-from-share-list"))
                                .ghost()
                                .on_click(move |_, _, cx| {
                                    let page = remove_page.clone();
                                    let _ = page.update(cx, |page, cx| page.remove(share_id, cx));
                                }),
                        ),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        Empty::new().header(
            EmptyHeader::new()
                .media(
                    EmptyMedia::new()
                        .with_variant(EmptyMediaVariant::Icon)
                        .child(Icon::new(IconName::Inbox)),
                )
                .title(EmptyTitle::new().child(t!("label.empty-share-list").to_string()))
                .description(
                    EmptyDescription::new().child(t!("label.empty-share-list-hint").to_string()),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }
}
