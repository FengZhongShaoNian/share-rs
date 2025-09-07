use gpui::{ContentMask, Fill};
use std::ops::Range;

use gpui::{
    AbsoluteLength, App, Bounds, ClipboardItem, Context, CursorStyle, Edges, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div,
    fill, hsla, point, prelude::*, px, relative, rgba, size,
};
use gpui_component::ActiveTheme;
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Quit,
    ]
);

#[derive(Clone, Debug)]
pub struct TextInputStyle {
    /// How large should the padding be on each side?
    pub padding: Edges<AbsoluteLength>,

    /// The fill color of this element
    pub background: Fill,

    /// The line height of this element
    pub line_height: AbsoluteLength,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            padding: Edges {
                top: AbsoluteLength::Pixels(Pixels(4.)),
                right: AbsoluteLength::Pixels(Pixels(4.)),
                bottom: AbsoluteLength::Pixels(Pixels(0.)),
                left: AbsoluteLength::Pixels(Pixels(4.)),
            },
            background: Default::default(),
            line_height: AbsoluteLength::Pixels(Pixels(30.)),
        }
    }
}

pub struct TextInput {
    focus_handle: FocusHandle,
    pub content: SharedString,
    pub placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    pub style: TextInputStyle,
}

impl TextInput {
    pub fn new(
        content: Option<SharedString>,
        placeholder: Option<SharedString>,
        style: Option<TextInputStyle>,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
            content: content.unwrap_or("".into()),
            placeholder: placeholder.unwrap_or("Type here...".into()),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            style: style.unwrap_or(TextInputStyle::default()),
        })
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\n", " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                (&self.content[self.selected_range.clone()]).to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                (&self.content[self.selected_range.clone()]).to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        assert_eq!(last_layout.text, self.content);
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_offset: Pixels, // 文本向左滚动的距离
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            let text_color = if cx.theme().is_dark() {
                hsla(6., 6., 6., 0.1)
            } else {
                hsla(0., 0., 0., 0.3)
            };
            (input.placeholder.clone(), text_color)
        } else {
            (content.clone(), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run.clone()
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let rem_size = window.rem_size();
        let font_size = style.font_size.to_pixels(rem_size);
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);

        let style = input.style.clone();
        let mut scroll_offset = Pixels(0.);
        // If the cursor position exceeds the right border,
        // adjust the value of scroll_offset to bring the cursor back into the viewport.
        if cursor_pos
            > bounds.size.width
                - style.padding.left.to_pixels(rem_size)
                - style.padding.right.to_pixels(rem_size)
        {
            scroll_offset = cursor_pos
                - (bounds.size.width
                    - style.padding.left.to_pixels(rem_size)
                    - style.padding.right.to_pixels(rem_size));
        }

        // Draw cursors and selections
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(
                            bounds.left() + cursor_pos - scroll_offset,
                            bounds.top() + style.padding.top.to_pixels(rem_size),
                        ),
                        size(
                            px(2.),
                            bounds.size.height
                                - style.padding.top.to_pixels(rem_size)
                                - style.padding.bottom.to_pixels(rem_size),
                        ),
                    ),
                    gpui::blue(),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x3311ff30),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_offset,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let origin = Point::new(bounds.origin.x - prepaint.scroll_offset, bounds.origin.y);

        // Use window.with_content_mask to truncate text that exceeds the bounds.
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            line.paint(origin, window.line_height(), window, cx)
                .unwrap();
        });

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style.clone();
        let rem_size = window.rem_size();
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .bg(style.background.clone())
            .line_height(style.line_height)
            .child(
                div()
                    .h(style.line_height.to_pixels(rem_size)
                        + style.padding.top.to_pixels(rem_size)
                        + style.padding.bottom.to_pixels(rem_size))
                    .w_full()
                    .pl(style.padding.left)
                    .pr(style.padding.right)
                    .pt(style.padding.top)
                    .pb(style.padding.bottom)
                    .bg(style.background.clone())
                    .child(TextElement {
                        input: cx.entity().clone(),
                    }),
            )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// struct InputExample {
//     text_input: Entity<TextInput>,
//     recent_keystrokes: Vec<Keystroke>,
//     focus_handle: FocusHandle,
// }
//
// impl Focusable for InputExample {
//     fn focus_handle(&self, _: &App) -> FocusHandle {
//         self.focus_handle.clone()
//     }
// }
//
// impl InputExample {
//     fn on_reset_click(&mut self, _: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
//         self.recent_keystrokes.clear();
//         self.text_input
//             .update(cx, |text_input, _cx| text_input.reset());
//         cx.notify();
//     }
// }
//
// impl Render for InputExample {
//     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//         div()
//             .bg(rgb(0xaaaaaa))
//             .track_focus(&self.focus_handle(cx))
//             .font_family(".SystemUIFont")
//             .flex()
//             .flex_col()
//             .size_full()
//             .child(
//                 div()
//                     .bg(white())
//                     .border_b_1()
//                     .border_color(black())
//                     .flex()
//                     .flex_row()
//                     .justify_between()
//                     .child(format!("Keyboard {}", cx.keyboard_layout().name()))
//                     .child(
//                         div()
//                             .border_1()
//                             .border_color(black())
//                             .px_2()
//                             .bg(yellow())
//                             .child("Reset")
//                             .hover(|style| {
//                                 style
//                                     .bg(yellow().blend(opaque_grey(0.5, 0.5)))
//                                     .cursor_pointer()
//                             })
//                             .on_mouse_up(MouseButton::Left, cx.listener(Self::on_reset_click)),
//                     ),
//             )
//             .child(div().w_80().pl_4().child(self.text_input.clone()))
//             .children(self.recent_keystrokes.iter().rev().map(|ks| {
//                 format!(
//                     "{:} {}",
//                     ks.unparse(),
//                     if let Some(key_char) = ks.key_char.as_ref() {
//                         format!("-> {:?}", key_char)
//                     } else {
//                         "".to_owned()
//                     }
//                 )
//             }))
//     }
// }
//
// fn main() {
//     Application::new().run(|cx: &mut App| {
//         let bounds = Bounds::centered(None, size(px(600.0), px(300.0)), cx);
//         cx.bind_keys([
//             KeyBinding::new("backspace", Backspace, None),
//             KeyBinding::new("delete", Delete, None),
//             KeyBinding::new("left", Left, None),
//             KeyBinding::new("right", Right, None),
//             KeyBinding::new("shift-left", SelectLeft, None),
//             KeyBinding::new("shift-right", SelectRight, None),
//             KeyBinding::new("cmd-a", SelectAll, None),
//             KeyBinding::new("cmd-v", Paste, None),
//             KeyBinding::new("cmd-c", Copy, None),
//             KeyBinding::new("cmd-x", Cut, None),
//             KeyBinding::new("home", Home, None),
//             KeyBinding::new("end", End, None),
//             KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
//         ]);
//
//         let window = cx
//             .open_window(
//                 WindowOptions {
//                     window_bounds: Some(WindowBounds::Windowed(bounds)),
//                     ..Default::default()
//                 },
//                 |_, cx| {
//                     let text_input = cx.new(|cx| TextInput {
//                         focus_handle: cx.focus_handle(),
//                         content: "".into(),
//                         placeholder: "Type here...".into(),
//                         selected_range: 0..0,
//                         selection_reversed: false,
//                         marked_range: None,
//                         last_layout: None,
//                         last_bounds: None,
//                         is_selecting: false,
//                         style: TextInputStyle::default(),
//                     });
//                     cx.new(|cx| InputExample {
//                         text_input,
//                         recent_keystrokes: vec![],
//                         focus_handle: cx.focus_handle(),
//                     })
//                 },
//             )
//             .unwrap();
//         let view = window.update(cx, |_, _, cx| cx.entity()).unwrap();
//         cx.observe_keystrokes(move |ev, _, cx| {
//             view.update(cx, |view, cx| {
//                 view.recent_keystrokes.push(ev.keystroke.clone());
//                 cx.notify();
//             })
//         })
//         .detach();
//         cx.on_keyboard_layout_change({
//             move |cx| {
//                 window.update(cx, |_, _, cx| cx.notify()).ok();
//             }
//         })
//         .detach();
//
//         window
//             .update(cx, |view, window, cx| {
//                 window.focus(&view.text_input.focus_handle(cx));
//                 cx.activate(true);
//             })
//             .unwrap();
//         cx.on_action(|_: &Quit, cx| cx.quit());
//         cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
//     });
// }
