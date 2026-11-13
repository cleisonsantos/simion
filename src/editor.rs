//! The block editor: title, blocks, inline formatting and the gutter handles.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontFamily, FontId, Frame, Id, Key, Label, Layout,
    Margin, Modifiers, Rect, RichText, Sense, Shape, Stroke, TextEdit, Ui, Vec2, pos2,
    text::{ByteIndex, CCursor, CCursorRange, LayoutJob, LayoutSection, TextFormat},
    text_edit::{TextBuffer, TextEditState},
    vec2,
};

use crate::app::{Action, App, Focus, PendingFocus};
use crate::icons;
use crate::markdown::{self, InlineLayout};
use crate::model::BlockKind;
use crate::syntax;
use crate::theme::{
    FONT_BODY, FONT_ITALIC, FONT_MEDIUM, FONT_MONO, FONT_SEMIBOLD, FONT_SEMIBOLD_ITALIC, GUTTER,
    INDENT_WIDTH, MAX_CONTENT_WIDTH, Palette, family,
};
use crate::ui as widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSpan {
    pub anchor: (usize, usize),
    pub head: (usize, usize),
}

impl TextSpan {
    fn ordered(self) -> ((usize, usize), (usize, usize)) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BlockContextAction {
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Transform(BlockKind),
    Move(i32),
}

impl App {
    pub fn editor(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.palette;
        let mut reset_scroll = false;
        if self.reset_scroll {
            self.reset_scroll = false;
            reset_scroll = true;
        }

        self.header(ui, actions);
        widgets::hairline(ui, &p);

        let page_id = self.current.unwrap_or(0);
        let mut scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt(("simion-editor", page_id));
        if reset_scroll {
            scroll = scroll.vertical_scroll_offset(0.0);
        }

        scroll.show_viewport(ui, |ui, viewport| {
            self.editor_content(ui, viewport, actions);
        });
    }

    fn editor_content(&mut self, ui: &mut Ui, viewport: Rect, actions: &mut Vec<Action>) {
        let full_width = ui.available_width();
        let inner_width = (full_width - 64.0).clamp(280.0, MAX_CONTENT_WIDTH);
        let left = ((full_width - inner_width) / 2.0).max(22.0);
        let editor_left = ui.cursor().left();

        // Page keys move the caret as well as revealing its destination.
        if !self.search.open && self.slash.is_none() && self.code_lang_menu.is_none() {
            let page = viewport.height() * 0.8;
            if self.consume_pub(ui.ctx(), Key::PageDown, Modifiers::NONE) {
                if !self.focus_page(ui.ctx(), true, page, inner_width) {
                    ui.scroll_with_delta(vec2(0.0, -page));
                }
            }
            if self.consume_pub(ui.ctx(), Key::PageUp, Modifiers::NONE) {
                if !self.focus_page(ui.ctx(), false, page, inner_width) {
                    ui.scroll_with_delta(vec2(0.0, page));
                }
            }
        }

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(left);
            ui.vertical(|ui| {
                ui.set_width(inner_width);
                ui.add_space(22.0 * self.zoom);
                self.page_icon(ui, actions);
                self.page_title(ui, actions);
                ui.add_space(6.0);
                self.blocks(ui, viewport, editor_left, actions);
            });
        });
    }

    fn header(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.palette;
        let zoom = self.zoom;
        let page_id = self.current;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            if !self.sidebar_visible {
                let resp = widgets::icon_button(
                    ui,
                    26.0,
                    p.muted,
                    "Mostrar barra lateral  Ctrl+\\",
                    |painter, rect, color| {
                        icons::panel_left(painter, rect, color, 1.5);
                    },
                );
                if resp.clicked() {
                    actions.push(Action::ToggleSidebar);
                }
                ui.add_space(4.0);
            }

            if let Some(current) = page_id {
                let path = self.path_to(current);
                let start = path.len().saturating_sub(3);
                if start > 0 {
                    ui.label(RichText::new("…").size(12.0).color(p.faint));
                    ui.label(RichText::new("/").size(12.0).color(p.faint));
                }
                for (position, id) in path.iter().enumerate().skip(start) {
                    let Some(meta) = self.meta_of(*id).cloned() else {
                        continue;
                    };
                    let last = *id == current;
                    let label = meta.display_title();
                    let text = if last {
                        RichText::new(label)
                            .size(12.5)
                            .color(p.muted)
                            .font(FontId::new(12.5, family(FONT_MEDIUM)))
                    } else {
                        RichText::new(label).size(12.5).color(p.faint)
                    };
                    let resp = ui.add(Label::new(text).selectable(false).sense(Sense::click()));
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if resp.clicked() && !last {
                        actions.push(Action::SelectPage(*id));
                    }
                    if position + 1 < path.len() {
                        ui.label(RichText::new("/").size(12.0).color(p.faint));
                    }
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if let Some(current) = page_id {
                    let more = widgets::icon_button(
                        ui,
                        28.0,
                        p.muted,
                        "Mais opções",
                        |painter, rect, color| {
                            icons::dots_h(painter, rect, color);
                        },
                    );
                    if more.clicked() {
                        actions.push(Action::OpenPageMenu(current, more.rect.left_bottom()));
                    }

                    let export = widgets::icon_button(
                        ui,
                        28.0,
                        p.muted,
                        "Exportar como Markdown",
                        |painter, rect, color| {
                            icons::download(painter, rect, color, 1.5);
                        },
                    );
                    if export.clicked() {
                        actions.push(Action::ExportMarkdown(current));
                    }

                    let favorite = self.meta_of(current).is_some_and(|meta| meta.favorite);
                    let star = widgets::icon_button(
                        ui,
                        28.0,
                        if favorite { p.accent } else { p.muted },
                        if favorite {
                            "Remover dos favoritos"
                        } else {
                            "Adicionar aos favoritos"
                        },
                        move |painter, rect, color| {
                            icons::star(painter, rect, color, favorite, 1.4);
                        },
                    );
                    if star.clicked() {
                        actions.push(Action::ToggleFavorite(current));
                    }

                    let emoji = widgets::icon_button(
                        ui,
                        28.0,
                        p.muted,
                        "Ícone da página",
                        |painter, rect, color| {
                            icons::smile(painter, rect, color, 1.4);
                        },
                    );
                    if emoji.clicked() {
                        actions.push(Action::OpenEmojiPicker(emoji.rect.left_bottom()));
                    }

                    let _ = zoom;
                }
            });
        });
        ui.add_space(4.0);
    }

    fn page_icon(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.palette;
        let zoom = self.zoom;
        let icon = self.page.meta.icon.clone();
        if icon.is_empty() {
            return;
        }
        let size = 52.0 * zoom;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(GUTTER - 4.0);
            let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
            let hover = ui.ctx().animate_bool_with_time(
                response.id.with("icon-hover"),
                response.hovered(),
                0.1,
            );
            if hover > 0.01 {
                let fill = Color32::from_rgba_unmultiplied(
                    p.hover.r(),
                    p.hover.g(),
                    p.hover.b(),
                    (p.hover.a() as f32 * hover) as u8,
                );
                ui.painter().rect_filled(rect, 10, fill);
            }
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                &icon,
                FontId::new(size * 0.82, FontFamily::Proportional),
                p.text,
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if response.clicked() {
                actions.push(Action::OpenEmojiPicker(rect.left_bottom()));
            }
        });
        ui.add_space(4.0);
    }

    fn page_title(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let _ = actions;
        let p = self.palette;
        let zoom = self.zoom;
        let page_id = self.current.unwrap_or(0);
        let te_id = Id::unique(("simion-title", page_id));
        let font = FontId::new(38.0 * zoom, family(FONT_SEMIBOLD));
        let line_height = 46.0 * zoom;
        let mut title = self.page.meta.title.clone();
        let text_width = (ui.available_width() - GUTTER).max(80.0);

        let mut layouter = |ui: &Ui, buf: &dyn TextBuffer, wrap: f32| -> Arc<egui::Galley> {
            let job = plain_job(buf.as_str(), font.clone(), p.text, wrap, line_height);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };

        let output = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(GUTTER);
                TextEdit::multiline(&mut title)
                    .id(te_id)
                    .font(font.clone())
                    .text_color(p.text)
                    .frame(Frame::NONE.inner_margin(Margin {
                        left: 0,
                        right: 0,
                        top: 2,
                        bottom: 4,
                    }))
                    .desired_width(text_width)
                    .desired_rows(1)
                    .hint_text(
                        RichText::new("Sem título")
                            .font(font.clone())
                            .color(p.faint),
                    )
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;

        if title != self.page.meta.title {
            self.page.meta.title = title;
            if let Some(current) = self.current {
                if let Some(meta) = self.pages.iter_mut().find(|page| page.id == current) {
                    meta.title = self.page.meta.title.clone();
                }
            }
            self.dirty = true;
            self.last_edit = self.now;
        }

        if output.response.has_focus() {
            self.focus = Focus::Title;
        }
        self.title_galley = Some((output.galley.clone(), output.galley_pos));
        self.title_cursor = output
            .cursor_range
            .map(|range| (range.primary.index.0, range.secondary.index.0));

        let pending_title = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.target == Focus::Title);
        if pending_title {
            let chars = self.page.meta.title.chars().count();
            let pending = self.pending.take().unwrap_or_default();
            let (start, end) = if pending.select_all {
                (0, chars)
            } else {
                pending.range.unwrap_or((chars, chars))
            };
            output.response.request_focus();
            ui.scroll_to_rect(output.response.rect, None);
            if let Some(mut state) = TextEditState::load(ui.ctx(), te_id) {
                state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(start.min(chars)),
                    CCursor::new(end.min(chars)),
                )));
                state.store(ui.ctx(), te_id);
            } else {
                let mut state = TextEditState::default();
                state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(start.min(chars)),
                    CCursor::new(end.min(chars)),
                )));
                state.store(ui.ctx(), te_id);
            }
            self.focus = Focus::Title;
        }
    }

    fn blocks(&mut self, ui: &mut Ui, viewport: Rect, editor_left: f32, actions: &mut Vec<Action>) {
        let text_width = (ui.available_width()).max(120.0);
        let column_left = ui.cursor().left();
        ui.spacing_mut().item_spacing.y = 0.0;
        self.hover_block = None;
        self.block_rects.clear();

        // Any click in the editor drops the block selection.
        if self.block_selection.is_some()
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input(|input| input.pointer.primary_pressed())
        {
            self.block_selection = None;
        }

        let count = self.page.blocks.len();
        let mut index = 0;
        let mut any_changed = false;
        // `viewport` is in content coordinates (0 = top of the document), so we
        // track the document offset ourselves instead of using screen-space
        // cursor positions (which would break virtualization as soon as the
        // user scrolls).
        let mut content_top = 0.0f32;

        while index < count {
            let (id, slot) = {
                let block = &self.page.blocks[index];
                let height = self
                    .block_heights
                    .get(&block.id)
                    .copied()
                    .unwrap_or_else(|| block.estimated_height(text_width - GUTTER, self.zoom));
                (block.id, (height + 2.0).max(10.0))
            };

            let forced = self.focus == Focus::Block(id)
                || self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.target == Focus::Block(id));
            let visible = (content_top + slot + 700.0 >= viewport.top()
                && content_top - 700.0 <= viewport.bottom())
                || forced;

            if !visible {
                ui.add_space(slot);
                content_top += slot;
                index += 1;
                continue;
            }

            let selection_shape = ui.painter().add(Shape::Noop);
            let outcome = self.block_row(ui, index, text_width, column_left, editor_left, actions);
            if self.block_selected(index) {
                let rect = Rect::from_min_max(
                    pos2(column_left, outcome.rect.top()),
                    pos2(column_left + text_width, outcome.rect.bottom()),
                );
                ui.painter().set(
                    selection_shape,
                    Shape::rect_filled(rect, 4, self.palette.selection),
                );
            }
            if outcome.changed {
                any_changed = true;
                let caret = outcome.cursor.map(|(a, b)| a.max(b));
                self.after_text_change(index, caret, actions);
            }
            if outcome.focused && self.block_selection.is_none() {
                self.focus = Focus::Block(id);
            }
            if let Some(range) = outcome.cursor {
                self.cursors.insert(id, range);
            }
            let measured = outcome.rect.height().max(8.0);
            self.block_heights.insert(id, measured);
            self.block_rows.insert(id, outcome.rows);
            self.block_rects.insert(id, outcome.rect);
            match outcome.galley {
                Some(galley) => self.block_galleys.insert(id, galley),
                None => self.block_galleys.remove(&id),
            };
            self.paint_span(
                ui,
                index,
                column_left,
                text_width,
                outcome.rect,
                outcome.span_shape,
            );
            content_top += measured + 2.0;
            if self.scroll_to == Some(id) {
                if !viewport.contains_rect(outcome.rect) {
                    ui.scroll_to_rect(outcome.rect.expand2(vec2(0.0, 40.0)), Some(Align::Center));
                }
                self.scroll_to = None;
            }
            ui.add_space(2.0);
            index += 1;
        }

        if self.scroll_to.is_some() {
            self.scroll_to = None;
        }

        // Clicking the empty area under the last block adds a new block.
        // Keep this area independent of the scroll offset, otherwise the
        // measured content height (and thus the scroll range) grows every frame.
        let remaining = (viewport.height() - content_top).max(0.0);
        let click_height = 220.0 + remaining;
        let (click_rect, click_response) =
            ui.allocate_exact_size(vec2(text_width, click_height), Sense::click());
        if click_response.clicked() {
            // An empty paragraph at the end is reused instead of piling up
            // empty blocks on every click.
            let id = match self.page.blocks.last() {
                Some(last) if last.kind == BlockKind::Paragraph && last.text.is_empty() => last.id,
                _ => {
                    let id = self.ids.next();
                    self.begin_change();
                    self.page
                        .blocks
                        .push(crate::model::Block::new(id, BlockKind::Paragraph));
                    self.end_change();
                    id
                }
            };
            self.focus = Focus::Block(id);
            self.pending = Some(PendingFocus {
                target: Focus::Block(id),
                range: Some((0, 0)),
                select_all: false,
            });
            self.scroll_to = Some(id);
            actions.push(Action::FocusBlock { id, range: (0, 0) });
        }
        let _ = click_rect;

        if any_changed {
            self.dirty = true;
            self.last_edit = self.now;
        }

        if let Some(pending) = &self.pending {
            if let Focus::Block(id) = pending.target {
                if self.index_of(id).is_none() {
                    self.pending = None;
                }
            }
        }
    }

    fn block_row(
        &mut self,
        ui: &mut Ui,
        index: usize,
        text_width: f32,
        column_left: f32,
        editor_left: f32,
        actions: &mut Vec<Action>,
    ) -> BlockOutcome {
        let p = self.palette;
        let zoom = self.zoom;
        let (id, kind, indent, checked, text, lang) = {
            let block = &self.page.blocks[index];
            (
                block.id,
                block.kind,
                block.indent,
                block.checked,
                block.text.clone(),
                block.lang.clone(),
            )
        };

        if kind == BlockKind::Divider {
            return self.divider_row(ui, text_width, column_left, id, actions);
        }

        let base_size = kind.font_size() * zoom;
        let line_factor = kind.line_height_factor();
        let line_height = base_size * line_factor;
        let (top_margin, bottom_margin) = kind.vertical_margins();
        let top_margin = top_margin * zoom;
        let bottom_margin = bottom_margin * zoom;
        let indent_px = indent as f32 * INDENT_WIDTH;
        let base_family = if kind.is_heading() {
            FONT_SEMIBOLD
        } else {
            FONT_BODY
        };
        let base_color = if checked { p.muted } else { p.text };
        let active = self.cursors.get(&id).copied();
        let focused_before = self.focus == Focus::Block(id);
        let is_code = kind == BlockKind::Code;
        let is_callout = kind == BlockKind::Callout;
        let is_quote = kind == BlockKind::Quote;

        let show_hint =
            focused_before || (self.page.blocks.len() == 1 && kind == BlockKind::Paragraph);
        let hint = if text.is_empty() && show_hint {
            Some(hint_for(kind))
        } else {
            None
        };
        let hint_font = FontId::new(base_size, family(base_family));

        let shape_idx = ui.painter().add(Shape::Noop);
        let span_shape = ui.painter().add(Shape::Noop);
        let row_start = ui.cursor().min;
        let mut new_text = text.clone();
        let mut changed = false;
        let mut focused = false;
        let mut cursor_range = None;
        let mut te_rect = Rect::NOTHING;
        let mut marker_rect = Rect::NOTHING;
        let mut te_id = Id::unique(("simion-block", id));
        let mut rows = 1usize;
        let mut galley = None;
        let mut toggle_todo = false;

        let base_family_owned = base_family.to_owned();
        let code_lang = lang.clone();
        let mut layouter = move |ui: &Ui, buf: &dyn TextBuffer, wrap: f32| -> Arc<egui::Galley> {
            let source = buf.as_str();
            let job = if is_code {
                let spans = syntax::highlight(source, &code_lang, &p);
                code_job(
                    source,
                    &spans,
                    FontId::new(base_size, family(FONT_MONO)),
                    p.text,
                    wrap,
                    line_height,
                )
            } else {
                let layout = markdown::parse_inline(source);
                inline_job(
                    source,
                    &layout,
                    base_size,
                    &base_family_owned,
                    line_factor,
                    base_color,
                    checked,
                    active,
                    &p,
                    wrap,
                )
            };
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };

        let margin_left: i8 = if is_code {
            12
        } else if is_callout {
            10
        } else {
            0
        };
        let margin_right: i8 = if is_code {
            36
        } else if is_callout {
            12
        } else {
            0
        };
        let margin_top: i8 = if is_code {
            10
        } else {
            top_margin.round() as i8
        };
        let margin_bottom: i8 = if is_code {
            10
        } else {
            bottom_margin.round() as i8
        };

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if indent_px > 0.0 {
                ui.add_space(indent_px);
            }

            let mark_gutter = !is_code;
            if mark_gutter {
                let marker_width = GUTTER;
                let marker_height = (top_margin + line_height).max(14.0);
                let sense = if kind == BlockKind::Todo {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (rect, response) =
                    ui.allocate_exact_size(vec2(marker_width, marker_height), sense);
                marker_rect = rect;
                if kind == BlockKind::Todo {
                    if response.clicked() {
                        toggle_todo = true;
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                }
                let center_y = rect.top() + top_margin + line_height / 2.0;
                match kind {
                    BlockKind::Bullet => {
                        let center = pos2(rect.left() + 12.0, center_y);
                        ui.painter().circle_filled(
                            center,
                            3.2 * zoom,
                            if checked { p.faint } else { p.text },
                        );
                    }
                    BlockKind::Numbered => {
                        let number = self.list_number(index).to_string();
                        ui.painter().text(
                            pos2(rect.right() - 8.0, center_y),
                            Align2::RIGHT_CENTER,
                            format!("{number}."),
                            FontId::new(base_size * 0.95, family(FONT_BODY)),
                            p.muted,
                        );
                    }
                    BlockKind::Todo => {
                        let size = 17.0 * zoom;
                        let box_rect = Rect::from_center_size(
                            pos2(rect.left() + 8.0 + size / 2.0, center_y),
                            Vec2::splat(size),
                        );
                        if checked {
                            ui.painter().rect_filled(box_rect, 4, p.accent);
                            icons::check(
                                ui.painter(),
                                Rect::from_center_size(box_rect.center(), Vec2::splat(size * 0.8)),
                                Color32::WHITE,
                                1.9,
                            );
                        } else {
                            ui.painter().rect_stroke(
                                box_rect,
                                4,
                                Stroke::new(1.5, p.faint),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    BlockKind::Callout => {
                        let emoji = {
                            let emoji = self.page.blocks[index].emoji.clone();
                            if emoji.is_empty() {
                                "💡".to_owned()
                            } else {
                                emoji
                            }
                        };
                        ui.painter().text(
                            pos2(rect.left() + 10.0, center_y),
                            Align2::LEFT_CENTER,
                            emoji,
                            FontId::new(base_size * 1.25, FontFamily::Proportional),
                            p.text,
                        );
                    }
                    _ => {}
                }
            }

            te_id = Id::unique(("simion-block", id));
            let available = ui.available_width().max(40.0);
            // Pending focus / selection for this block. Applied before the
            // text field runs so keys typed in this same frame land at the
            // new caret instead of being moved by it afterwards.
            let pending_here = self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.target == Focus::Block(id));
            if pending_here {
                let chars = new_text.chars().count();
                let pending = self.pending.take().unwrap_or_default();
                let (start, end) = pending.range.unwrap_or((chars, chars));
                ui.memory_mut(|memory| memory.request_focus(te_id));
                let mut state = TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
                state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(start.min(chars)),
                    CCursor::new(end.min(chars)),
                )));
                state.store(ui.ctx(), te_id);
                focused = true;
            }

            // Tab presses `block_keys` did not take (an overlay was open) must
            // not insert "\t" either.
            if ui.memory(|memory| memory.has_focus(te_id)) {
                while self.consume_key(ui.ctx(), Key::Tab, Modifiers::NONE)
                    || self.consume_key(ui.ctx(), Key::Tab, Modifiers::SHIFT)
                {}
            }

            let te = TextEdit::multiline(&mut new_text)
                .id(te_id)
                .font(FontId::new(base_size, family(base_family)))
                .text_color(base_color)
                .frame(Frame::NONE.inner_margin(Margin {
                    left: margin_left,
                    right: margin_right,
                    top: margin_top,
                    bottom: margin_bottom,
                }))
                .desired_width(available)
                .desired_rows(1)
                // The document selection owns painting and clipboard events.
                .interactive(self.text_span.is_none())
                // Tab indents the block (see `block_keys`); without the lock
                // egui would move keyboard focus to the next widget.
                .lock_focus(true);

            let te = if let Some(hint) = hint {
                te.hint_text(RichText::new(hint).font(hint_font.clone()).color(p.faint))
            } else {
                te
            };

            let output = te.layouter(&mut layouter).show(ui);
            // Keep the native text widget as the drag target. The document-level
            // controller uses its cached rect/galley before TextEdit handles events,
            // so a drag continues beyond this widget's bounds.
            let response = output.response.clone().interact(Sense::click_and_drag());
            self.block_context_menu(ui, &response, id, actions);
            te_rect = output.response.rect;
            rows = output.galley.rows.len().max(1);
            // Anchor to this frame's widget geometry, including the text inset.
            // A detached/cached galley origin must not turn screen-space y into
            // a position below every wrapped row (and select the last row).
            let text_origin = te_rect.min + vec2(margin_left as f32, margin_top as f32);
            galley = Some((output.galley.clone(), text_origin));
            if output.response.changed() {
                changed = true;
            }
            if output.response.has_focus() {
                focused = true;
            }
            if let Some(range) = output.cursor_range {
                cursor_range = Some((range.primary.index.0, range.secondary.index.0));
            }

            // Ctrl/Cmd+click opens links.
            let command = ui.input(|input| input.modifiers.command || input.modifiers.ctrl);
            if command && output.response.hovered() {
                if let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) {
                    let local = pointer - text_origin;
                    let ccursor = output.galley.cursor_from_pos(local);
                    let byte = markdown::char_to_byte(&new_text, ccursor.index.0);
                    let layout = markdown::parse_inline(&new_text);
                    if let Some((_, url)) = layout
                        .links
                        .iter()
                        .find(|(range, _)| range.start <= byte && byte < range.end)
                    {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        if output.response.clicked() {
                            open_url(url);
                        }
                    }
                }
            }
        });

        if toggle_todo {
            actions.push(Action::ToggleTodo(id));
        }

        let row_rect = Rect::from_min_max(
            pos2(row_start.x, row_start.y),
            pos2(
                te_rect.right().max(marker_rect.right()),
                te_rect.bottom().max(marker_rect.bottom()),
            ),
        )
        .expand2(vec2(0.0, 0.0));

        if new_text != text {
            if let Some(block) = self.page.blocks.get_mut(index) {
                block.text = new_text;
            }
            changed = true;
        }

        // Backgrounds.
        if is_code {
            ui.painter()
                .set(shape_idx, Shape::rect_filled(te_rect, 9, p.code_bg));
        } else if is_callout {
            let rect = Rect::from_min_max(
                pos2(row_start.x + indent_px, te_rect.top()),
                pos2(te_rect.right(), te_rect.bottom()),
            );
            ui.painter()
                .set(shape_idx, Shape::rect_filled(rect, 9, p.callout_bg));
        } else if is_quote {
            let rect = Rect::from_min_size(
                pos2(row_start.x + indent_px + 2.0, row_rect.top() + 1.0),
                vec2(3.0, (row_rect.height() - 2.0).max(6.0)),
            );
            ui.painter()
                .set(shape_idx, Shape::rect_filled(rect, 2, p.quote_bar));
        }

        let margin_right = if kind == BlockKind::Todo {
            marker_rect.left()
        } else {
            te_rect.left()
        };
        let margin_rect = Rect::from_min_max(
            pos2(editor_left, row_rect.top()),
            pos2(margin_right.max(editor_left), row_rect.bottom()),
        );
        let margin_response = if margin_rect.width() > 0.0 && margin_rect.height() > 0.0 {
            Some(ui.interact(
                margin_rect,
                Id::unique(("simion-row-margin", id)),
                Sense::click(),
            ))
        } else {
            None
        };

        let handle_clicked = self.hover_handles(ui, id, column_left, row_rect, actions);
        if !handle_clicked
            && !toggle_todo
            && margin_response.is_some_and(|response| response.clicked())
        {
            self.focus = Focus::Block(id);
            self.pending = Some(PendingFocus {
                target: Focus::Block(id),
                range: Some((0, 0)),
                select_all: false,
            });
            ui.memory_mut(|memory| memory.request_focus(te_id));
        }

        // Copy button for code blocks, on hover.
        if is_code && self.hover_block == Some(id) {
            let button_rect = Rect::from_min_size(
                pos2(te_rect.right() - 30.0, te_rect.top() + 8.0),
                Vec2::splat(22.0),
            );
            let response = ui.interact(
                button_rect,
                Id::unique(("simion-code-copy", id)),
                Sense::click(),
            );
            if response.hovered() {
                ui.painter().rect_filled(button_rect, 5, p.active);
            }
            icons::copy(
                ui.painter(),
                Rect::from_center_size(button_rect.center(), Vec2::splat(16.0)),
                p.muted,
                1.4,
            );
            if response.clicked() {
                actions.push(Action::CopyBlock(id));
            }
            let _ = response.on_hover_text("Copiar código");
        }

        // Language picker, left of the copy button.
        let lang_menu_here = self.code_lang_menu.is_some_and(|(menu, _)| menu == id);
        if is_code && (self.hover_block == Some(id) || lang_menu_here) {
            let galley = ui.painter().layout_no_wrap(
                format!("{} ▾", lang_label(&lang)),
                FontId::new(11.5 * zoom, family(FONT_BODY)),
                p.muted,
            );
            let width = galley.size().x + 14.0;
            let button_rect = Rect::from_min_size(
                pos2(te_rect.right() - 34.0 - width, te_rect.top() + 8.0),
                vec2(width, 22.0),
            );
            let response = ui.interact(
                button_rect,
                Id::unique(("simion-code-lang", id)),
                Sense::click(),
            );
            if response.hovered() || lang_menu_here {
                ui.painter().rect_filled(button_rect, 5, p.active);
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            ui.painter().galley(
                pos2(
                    button_rect.left() + 7.0,
                    button_rect.center().y - galley.size().y / 2.0,
                ),
                galley,
                p.muted,
            );
            if response.clicked() {
                actions.push(Action::OpenCodeLangMenu(
                    id,
                    pos2(button_rect.left(), button_rect.bottom() + 4.0),
                ));
            }
            let _ = response.on_hover_text("Linguagem");
        }

        // Keep a recognizable language tag for code visually.
        if is_code && !lang.is_empty() && self.hover_block != Some(id) && !lang_menu_here {
            let rect = Rect::from_min_size(
                pos2(te_rect.right() - 12.0, te_rect.bottom() - 2.0),
                vec2(0.0, 0.0),
            );
            ui.painter().text(
                rect.left_top() - vec2(0.0, 18.0),
                Align2::RIGHT_TOP,
                lang_label(&lang),
                FontId::new(10.5 * zoom, family(FONT_MONO)),
                p.faint,
            );
        }

        let _ = base_size;
        if focused && self.block_selection.is_none() {
            self.focus = Focus::Block(id);
        }

        BlockOutcome {
            changed,
            focused,
            span_shape,
            cursor: cursor_range,
            rect: row_rect,
            rows,
            galley,
        }
    }

    fn divider_row(
        &mut self,
        ui: &mut Ui,
        text_width: f32,
        column_left: f32,
        id: u64,
        actions: &mut Vec<Action>,
    ) -> BlockOutcome {
        let p = self.palette;
        let zoom = self.zoom;
        let row_start = ui.cursor().min;
        let (rect, response) =
            ui.allocate_exact_size(vec2(text_width, 24.0 * zoom), Sense::click());
        self.block_context_menu(ui, &response, id, actions);
        let y = rect.center().y;
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(rect.left(), y - 0.75), vec2(rect.width(), 1.5)),
            0,
            p.border,
        );
        let row_rect = Rect::from_min_max(row_start, rect.max);
        self.hover_handles(ui, id, column_left, row_rect, actions);
        BlockOutcome {
            changed: false,
            focused: false,
            span_shape: ui.painter().add(Shape::Noop),
            cursor: None,
            rect: row_rect,
            rows: 1,
            galley: None,
        }
    }

    fn hover_handles(
        &mut self,
        ui: &Ui,
        id: u64,
        column_left: f32,
        row_rect: Rect,
        actions: &mut Vec<Action>,
    ) -> bool {
        let p = self.palette;
        let overlays_open = self.slash.is_some()
            || self.block_menu.is_some()
            || self.page_menu.is_some()
            || self.emoji_picker.is_some()
            || self.search.open
            || self.shortcuts_open
            || self.markdown_open
            || self.confirm_delete.is_some();
        if overlays_open {
            return false;
        }
        let zone = Rect::from_min_max(
            pos2(column_left - 46.0, row_rect.top() - 3.0),
            pos2(row_rect.right() + 6.0, row_rect.bottom() + 3.0),
        );
        if !ui.rect_contains_pointer(zone) {
            return false;
        }
        self.hover_block = Some(id);

        let plus_rect = Rect::from_min_size(
            pos2(column_left - 26.0, row_rect.top() + 1.0),
            Vec2::splat(20.0),
        );
        let grip_rect = Rect::from_min_size(
            pos2(column_left - 46.0, row_rect.top() + 1.0),
            Vec2::splat(20.0),
        );

        let plus = ui.interact(plus_rect, Id::unique(("simion-plus", id)), Sense::click());
        let grip = ui.interact(grip_rect, Id::unique(("simion-grip", id)), Sense::click());

        for (rect, response, icon, tip) in [
            (&plus_rect, &plus, 0u8, "Inserir bloco abaixo"),
            (&grip_rect, &grip, 1u8, "Opções do bloco"),
        ] {
            if response.hovered() {
                ui.painter().rect_filled(*rect, 5, p.hover);
            }
            let color = if response.hovered() { p.text } else { p.muted };
            let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(15.0));
            if icon == 0 {
                icons::plus(ui.painter(), icon_rect, color, 1.4);
            } else {
                icons::grip(ui.painter(), icon_rect, color);
            }
            let _ = tip;
        }

        let mut handle_clicked = false;
        if plus.clicked() {
            actions.push(Action::InsertBelow(id));
            handle_clicked = true;
        }
        if plus.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if grip.clicked() {
            actions.push(Action::OpenBlockMenu(id, grip_rect.left_bottom()));
            handle_clicked = true;
        }
        if grip.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        handle_clicked
    }

    fn list_number(&self, index: usize) -> usize {
        let current = &self.page.blocks[index];
        let mut number = 1;
        let mut i = index;
        while i > 0 {
            i -= 1;
            let block = &self.page.blocks[i];
            if block.kind == BlockKind::Numbered && block.indent == current.indent {
                number += 1;
            } else {
                break;
            }
        }
        number
    }

    // -----------------------------------------------------------------------
    // Text change side effects: markdown shortcuts and the slash menu
    // -----------------------------------------------------------------------

    fn after_text_change(&mut self, index: usize, caret: Option<usize>, actions: &mut Vec<Action>) {
        let Some(block) = self.page.blocks.get(index) else {
            return;
        };
        let id = block.id;
        let text = block.text.clone();
        let kind = block.kind;
        let caret = caret.unwrap_or_else(|| text.chars().count());
        // Markdown block shortcuts ("# ", "- ", "```" …), typed at the start
        // of the block. Text after the caret is kept, like in Notion.
        if let Some((new_kind, prefix_chars)) = markdown::shortcut_for_before_caret(&text, caret) {
            let rest: String = text.chars().skip(prefix_chars).collect();
            let applies = kind.accepts_text()
                && kind != BlockKind::Code
                && kind != new_kind
                && (new_kind != BlockKind::Divider || rest.is_empty());
            if applies {
                self.begin_change();
                if let Some(block) = self.page.blocks.get_mut(index) {
                    block.kind = new_kind;
                    block.text = rest;
                    if new_kind == BlockKind::Todo {
                        block.checked = false;
                    }
                    if new_kind == BlockKind::Callout {
                        block.emoji = "💡".to_owned();
                    }
                }
                self.end_change();
                self.pending = Some(PendingFocus {
                    target: Focus::Block(id),
                    range: Some((0, 0)),
                    select_all: false,
                });
                if new_kind == BlockKind::Divider {
                    let new_id = self.ids.next();
                    self.begin_change();
                    self.page.blocks.insert(
                        index + 1,
                        crate::model::Block::new(new_id, BlockKind::Paragraph),
                    );
                    self.end_change();
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(new_id),
                        range: Some((0, 0)),
                        select_all: false,
                    });
                    // The divider has no text field; the caret goes below it.
                    actions.push(Action::FocusBlock {
                        id: new_id,
                        range: (0, 0),
                    });
                    return;
                }
                actions.push(Action::FocusBlock { id, range: (0, 0) });
                return;
            }
        }

        // Slash menu: only for a "/" at a word boundary right before the caret.
        if let Some((start, query)) = markdown::slash_query_before_caret(&text, caret) {
            self.slash = Some(crate::app::SlashState {
                block: id,
                start,
                query,
                selected: 0,
            });
            self.popup_guard = self.now + 0.2;
            return;
        }
        if self.slash.as_ref().is_some_and(|slash| slash.block == id) {
            self.slash = None;
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard handling for blocks
    // -----------------------------------------------------------------------

    pub fn editor_keys(&mut self, ctx: &egui::Context) {
        self.span_pointer(ctx);
        if self.span_keys(ctx) {
            return;
        }
        if self.block_selection.is_some() && self.selection_keys(ctx) {
            return;
        }
        if self.consume_pub(ctx, Key::Escape, Modifiers::NONE) {
            // Esc on a block selects it, like Notion; arrows then move the
            // selection and Esc / Enter go back to editing.
            match self.focus {
                Focus::Block(id) if self.index_of(id).is_some() => {
                    if let Some((a, b)) = self.cursor_range_for(ctx, id)
                        && a != b
                    {
                        self.focus_block(id, b);
                    } else {
                        self.start_block_selection(ctx, id);
                    }
                }
                _ => self.focus = Focus::None,
            }
            return;
        }

        match self.focus {
            Focus::Title => self.title_keys(ctx),
            Focus::Block(id) => self.block_keys(ctx, id),
            Focus::None => {}
        }
    }

    fn title_keys(&mut self, ctx: &egui::Context) {
        let title = self.page.meta.title.clone();
        let chars = title.chars().count();
        let (a, b) = self.title_cursor.unwrap_or((chars, chars));
        let (start, end) = (a.min(b).min(chars), a.max(b).min(chars));

        if self.consume_pub(ctx, Key::Enter, Modifiers::NONE) {
            if start >= chars {
                self.focus_first_block(None);
                return;
            }
            // Enter in the middle splits the title: the rest becomes the
            // first block.
            let head = title[..markdown::char_to_byte(&title, start)].to_owned();
            let tail = title[markdown::char_to_byte(&title, end)..].to_owned();
            self.begin_change();
            self.set_title(head);
            let id = self.ids.next();
            let mut block = crate::model::Block::new(id, BlockKind::Paragraph);
            block.text = tail;
            self.page.blocks.insert(0, block);
            self.end_change();
            self.focus_block(id, 0);
            return;
        }

        if start != end {
            return;
        }
        let caret = TextEditState::load(ctx, title_te_id(self.current.unwrap_or(0)))
            .and_then(|state| state.cursor.char_range())
            .map_or(CCursor::new(start), |range| range.primary);
        let galley = self.title_galley.clone();
        let on_last_row = galley
            .as_ref()
            .is_none_or(|(galley, _)| caret_row(galley, caret) + 1 >= galley.rows.len());
        if on_last_row && self.consume_pub(ctx, Key::ArrowDown, Modifiers::NONE) {
            self.focus_first_block(None);
        }
    }

    fn set_title(&mut self, title: String) {
        self.page.meta.title = title;
        if let Some(current) = self.current {
            if let Some(meta) = self.pages.iter_mut().find(|page| page.id == current) {
                meta.title = self.page.meta.title.clone();
            }
        }
        self.dirty = true;
        self.last_edit = self.now;
    }

    /// From the title into the first text block: on its first row at `x`
    /// (screen space), or at its start. An empty page gets a paragraph.
    fn focus_first_block(&mut self, x: Option<f32>) {
        let first = self
            .page
            .blocks
            .iter()
            .position(|block| block.kind.accepts_text());
        let id = match first {
            Some(index) => self.page.blocks[index].id,
            None => {
                let id = self.ids.next();
                self.begin_change();
                self.page
                    .blocks
                    .insert(0, crate::model::Block::new(id, BlockKind::Paragraph));
                self.end_change();
                id
            }
        };
        let caret = x
            .zip(self.block_galleys.get(&id))
            .map_or(0, |(x, (galley, pos))| caret_at_row_x(galley, 0, x - pos.x));
        self.focus_block(id, caret);
    }

    fn focus_block(&mut self, id: u64, caret: usize) {
        self.focus = Focus::Block(id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((caret, caret)),
            select_all: false,
        });
        self.scroll_to = Some(id);
    }

    fn focus_title(&mut self, caret: usize) {
        self.focus = Focus::Title;
        self.pending = Some(PendingFocus {
            target: Focus::Title,
            range: Some((caret, caret)),
            select_all: false,
        });
    }

    /// Nearest block above `index` that takes text (dividers are skipped).
    fn prev_text_block(&self, index: usize) -> Option<usize> {
        (0..index)
            .rev()
            .find(|i| self.page.blocks[*i].kind.accepts_text())
    }

    /// Nearest block below `index` that takes text (dividers are skipped).
    fn next_text_block(&self, index: usize) -> Option<usize> {
        (index + 1..self.page.blocks.len()).find(|i| self.page.blocks[*i].kind.accepts_text())
    }

    /// ↑ on the first row (or ← at the start): into the block above, on its
    /// last row at `x`, else at its end. Above the first block is the title.
    fn focus_above(&mut self, index: usize, x: Option<f32>) {
        let last_row_at = |x: f32, (galley, pos): &(Arc<egui::Galley>, egui::Pos2)| {
            caret_at_row_x(galley, galley.rows.len().saturating_sub(1), x - pos.x)
        };
        match self.prev_text_block(index) {
            Some(previous) => {
                let id = self.page.blocks[previous].id;
                let length = self.page.blocks[previous].text.chars().count();
                let caret = x
                    .zip(self.block_galleys.get(&id))
                    .map_or(length, |(x, laid)| last_row_at(x, laid));
                self.focus_block(id, caret);
            }
            None => {
                let length = self.page.meta.title.chars().count();
                self.focus_title(length);
            }
        }
    }

    /// ↓ on the last row (or → at the end): into the block below, on its
    /// first row at `x`, else at its start. Nothing below: stay.
    fn focus_below(&mut self, index: usize, x: Option<f32>) {
        if let Some(next) = self.next_text_block(index) {
            let id = self.page.blocks[next].id;
            let caret = x
                .zip(self.block_galleys.get(&id))
                .map_or(0, |(x, (galley, pos))| caret_at_row_x(galley, 0, x - pos.x));
            self.focus_block(id, caret);
        }
    }

    fn block_keys(&mut self, ctx: &egui::Context, id: u64) {
        let Some(index) = self.index_of(id) else {
            self.focus = Focus::None;
            return;
        };
        let kind = self.page.blocks[index].kind;
        let text_len = self.page.blocks[index].text.chars().count();
        let (selection_start, selection_end) = self
            .cursor_range_for(ctx, id)
            .or_else(|| self.cursors.get(&id).copied())
            .unwrap_or((text_len, text_len));
        let has_selection = selection_start != selection_end;
        let caret = selection_end.min(text_len);

        for (key, forward) in [(Key::ArrowRight, true), (Key::ArrowLeft, false)] {
            let range = self.cursor_range_for(ctx, id).unwrap_or((caret, caret));
            let head = range.0;
            if ((forward && head == text_len && index + 1 < self.page.blocks.len())
                || (!forward && head == 0 && index > 0))
                && self.consume_pub(ctx, key, Modifiers::SHIFT)
            {
                let next = if forward { index + 1 } else { index - 1 };
                let offset = if forward {
                    0
                } else {
                    self.page.blocks[next].text.chars().count()
                };
                self.activate_span(
                    ctx,
                    TextSpan {
                        anchor: (index, range.1),
                        head: (next, offset),
                    },
                );
                return;
            }
        }
        if kind.accepts_text() {
            if self.consume_pub(ctx, Key::B, Modifiers::COMMAND) {
                self.inline_toggle("**", id, (selection_start, selection_end));
                return;
            }
            if self.consume_pub(ctx, Key::I, Modifiers::COMMAND) {
                self.inline_toggle("*", id, (selection_start, selection_end));
                return;
            }
            if self.consume_pub(ctx, Key::E, Modifiers::COMMAND) {
                self.inline_toggle("`", id, (selection_start, selection_end));
                return;
            }
            if self.consume_pub(ctx, Key::S, Modifiers::COMMAND | Modifiers::SHIFT) {
                self.inline_toggle("~~", id, (selection_start, selection_end));
                return;
            }
            if self.consume_pub(ctx, Key::K, Modifiers::COMMAND | Modifiers::SHIFT) {
                self.insert_link(id, (selection_start, selection_end));
                return;
            }
            if self.consume_pub(ctx, Key::Num1, Modifiers::COMMAND | Modifiers::ALT) {
                self.transform_block(id, BlockKind::Heading1);
                return;
            }
            if self.consume_pub(ctx, Key::Num2, Modifiers::COMMAND | Modifiers::ALT) {
                self.transform_block(id, BlockKind::Heading2);
                return;
            }
            if self.consume_pub(ctx, Key::Num3, Modifiers::COMMAND | Modifiers::ALT) {
                self.transform_block(id, BlockKind::Heading3);
                return;
            }
            if self.consume_pub(ctx, Key::Num0, Modifiers::COMMAND | Modifiers::ALT) {
                self.transform_block(id, BlockKind::Paragraph);
                return;
            }
        }

        if self.consume_pub(ctx, Key::Tab, Modifiers::NONE) {
            self.adjust_indent(id, 1);
            return;
        }
        if self.consume_pub(ctx, Key::Tab, Modifiers::SHIFT) {
            self.adjust_indent(id, -1);
            return;
        }
        // Notion's move shortcut. Left to the text field it would select up
        // to the start and the next key would replace the text.
        if self.consume_pub(ctx, Key::ArrowUp, Modifiers::COMMAND | Modifiers::SHIFT) {
            self.move_block(id, -1);
            return;
        }
        if self.consume_pub(ctx, Key::ArrowDown, Modifiers::COMMAND | Modifiers::SHIFT) {
            self.move_block(id, 1);
            return;
        }
        if self.consume_pub(ctx, Key::D, Modifiers::COMMAND) {
            self.duplicate_block(id);
            return;
        }
        // Ctrl+A selects the text; with all of it already selected (or
        // nothing to select), the next one selects every block.
        if kind.accepts_text() && self.consume_pub(ctx, Key::A, Modifiers::COMMAND) {
            if text_len == 0 || (selection_start == 0 && selection_end == text_len) {
                let first = self.page.blocks.first().map(|block| block.id);
                let last = self.page.blocks.last().map(|block| block.id);
                if let Some(range) = first.zip(last) {
                    self.start_block_selection(ctx, id);
                    self.block_selection = Some(range);
                }
            } else {
                self.pending = Some(PendingFocus {
                    target: Focus::Block(id),
                    range: Some((0, text_len)),
                    select_all: false,
                });
            }
            return;
        }
        // Shift+↑/↓ selects this block when no text is selected, or when the
        // text selection already reaches the start / end of the block. The
        // range comes from the text field's own state, not `self.cursors`.
        let cursor_range = self.cursor_range_for(ctx, id);
        let empty_selection = cursor_range.is_none_or(|(a, b)| a == b);
        if (empty_selection || cursor_range.is_some_and(|(start, _)| start == 0))
            && self.consume_pub(ctx, Key::ArrowUp, Modifiers::SHIFT)
        {
            self.start_block_selection(ctx, id);
            return;
        }
        if (empty_selection || cursor_range.is_some_and(|(_, end)| end >= text_len))
            && self.consume_pub(ctx, Key::ArrowDown, Modifiers::SHIFT)
        {
            self.start_block_selection(ctx, id);
            return;
        }
        if self.consume_pub(ctx, Key::ArrowUp, Modifiers::ALT) {
            self.move_block(id, -1);
            return;
        }
        if self.consume_pub(ctx, Key::ArrowDown, Modifiers::ALT) {
            self.move_block(id, 1);
            return;
        }

        if kind != BlockKind::Code && self.consume_pub(ctx, Key::Enter, Modifiers::NONE) {
            let indent = self.page.blocks[index].indent;
            if text_len == 0 && kind.is_list() && indent > 0 {
                // Notion first moves an empty nested item one level out.
                self.adjust_indent(id, -1);
            } else if text_len == 0 && (kind.is_list() || kind == BlockKind::Quote) {
                // An empty list item (or quote) leaves the list. An empty
                // heading stays and a paragraph opens below it.
                self.transform_block(id, BlockKind::Paragraph);
            } else if !has_selection && caret == 0 && text_len > 0 {
                self.insert_block_above(id);
            } else {
                self.split_block(id, (selection_start, selection_end));
            }
            return;
        }

        if !has_selection && caret == 0 && self.consume_pub(ctx, Key::Backspace, Modifiers::NONE) {
            self.backspace_block(id);
            return;
        }
        if !has_selection
            && caret == text_len
            && self.consume_pub(ctx, Key::Delete, Modifiers::NONE)
        {
            self.delete_merge_forward(id);
            return;
        }

        // With text selected the text field adjusts the selection itself.
        if has_selection || !kind.accepts_text() {
            return;
        }

        // ←/→ (and Ctrl+←/→) at the edge of the block cross into the next one.
        if caret == 0
            && (self.consume_pub(ctx, Key::ArrowLeft, Modifiers::NONE)
                || self.consume_pub(ctx, Key::ArrowLeft, Modifiers::COMMAND))
        {
            self.focus_above(index, None);
            return;
        }
        if caret == text_len
            && (self.consume_pub(ctx, Key::ArrowRight, Modifiers::NONE)
                || self.consume_pub(ctx, Key::ArrowRight, Modifiers::COMMAND))
        {
            self.focus_below(index, None);
            return;
        }

        // ↑/↓ leave the block from its first / last visual row, keeping the
        // caret's x. Inside the block the text field moves between rows.
        let Some((galley, galley_pos)) = self.block_galleys.get(&id).cloned() else {
            // Not laid out yet: fall back to the start / end of the text.
            if caret == 0 && self.consume_pub(ctx, Key::ArrowUp, Modifiers::NONE) {
                self.focus_above(index, None);
            } else if caret == text_len && self.consume_pub(ctx, Key::ArrowDown, Modifiers::NONE) {
                self.focus_below(index, None);
            }
            return;
        };
        let cursor = TextEditState::load(ctx, block_te_id(id))
            .and_then(|state| state.cursor.char_range())
            .map_or(CCursor::new(caret), |range| range.primary);
        let row = caret_row(&galley, cursor);
        let x = galley_pos.x + caret_x(&galley, cursor);
        if row == 0 && self.consume_pub(ctx, Key::ArrowUp, Modifiers::NONE) {
            self.focus_above(index, Some(x));
            return;
        }
        if row + 1 >= galley.rows.len() && self.consume_pub(ctx, Key::ArrowDown, Modifiers::NONE) {
            self.focus_below(index, Some(x));
        }
    }

    fn inline_toggle(&mut self, marker: &str, id: u64, sel: (usize, usize)) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let text = self.page.blocks[index].text.clone();
        let chars = text.chars().count();
        let a = sel.0.min(sel.1).min(chars);
        let b = sel.0.max(sel.1).min(chars);
        let (new_text, new_sel) = markdown::toggle_wrap(&text, (a, b), marker);
        if new_text == text {
            return;
        }
        self.begin_change();
        self.page.blocks[index].text = new_text;
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some(new_sel),
            select_all: false,
        });
    }

    fn insert_link(&mut self, id: u64, sel: (usize, usize)) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let text = self.page.blocks[index].text.clone();
        let (new_text, new_sel) = markdown::wrap_link(&text, sel, "https://");
        self.begin_change();
        self.page.blocks[index].text = new_text;
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some(new_sel),
            select_all: false,
        });
    }

    /// Enter: the text before the selection stays, the text after it moves
    /// to a new block below. Lists continue the list; anything else (headings
    /// included) continues as a paragraph at the same indent.
    fn split_block(&mut self, id: u64, (start, end): (usize, usize)) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let kind = self.page.blocks[index].kind;
        let indent = self.page.blocks[index].indent;
        let text = self.page.blocks[index].text.clone();
        let chars = text.chars().count();
        let left = text[..markdown::char_to_byte(&text, start.min(end).min(chars))].to_owned();
        let right = text[markdown::char_to_byte(&text, start.max(end).min(chars))..].to_owned();

        let new_kind = if kind.is_list() {
            kind
        } else {
            BlockKind::Paragraph
        };

        self.begin_change();
        self.page.blocks[index].text = left;
        let new_id = self.ids.next();
        let mut block = crate::model::Block::new(new_id, new_kind);
        block.text = right;
        block.indent = indent;
        block.checked = false;
        self.page.blocks.insert(index + 1, block);
        self.end_change();
        self.focus = Focus::Block(new_id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(new_id),
            range: Some((0, 0)),
            select_all: false,
        });
        self.scroll_to = Some(new_id);
    }

    /// Enter at the very start of a non-empty block: Notion pushes the block
    /// down, leaving an empty one above, and the caret stays with the text.
    fn insert_block_above(&mut self, id: u64) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let kind = self.page.blocks[index].kind;
        let new_kind = if kind.is_list() {
            kind
        } else {
            BlockKind::Paragraph
        };
        self.begin_change();
        let mut block = crate::model::Block::new(self.ids.next(), new_kind);
        block.indent = self.page.blocks[index].indent;
        self.page.blocks.insert(index, block);
        self.end_change();
        self.focus = Focus::Block(id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((0, 0)),
            select_all: false,
        });
        self.scroll_to = Some(id);
    }

    fn backspace_block(&mut self, id: u64) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let indent = self.page.blocks[index].indent;
        let kind = self.page.blocks[index].kind;

        // Notion's order: outdent, then drop the block type, then merge.
        if indent > 0 {
            self.adjust_indent(id, -1);
            return;
        }
        if kind != BlockKind::Paragraph {
            // A code block with content keeps its type (its text is not
            // prose that should flow into the previous block).
            if kind == BlockKind::Code && !self.page.blocks[index].text.is_empty() {
                return;
            }
            self.transform_block(id, BlockKind::Paragraph);
            return;
        }
        if index == 0 {
            return;
        }

        let previous = &self.page.blocks[index - 1];
        let previous_kind = previous.kind;
        let previous_len = previous.text.chars().count();
        let current_text = self.page.blocks[index].text.clone();

        if previous_kind == BlockKind::Divider {
            self.begin_change();
            self.page.blocks.remove(index - 1);
            let new_index = index - 1;
            let target = self.page.blocks[new_index].id;
            self.end_change();
            self.focus = Focus::Block(target);
            self.pending = Some(PendingFocus {
                target: Focus::Block(target),
                range: Some((0, 0)),
                select_all: false,
            });
            return;
        }

        self.begin_change();
        {
            let previous = &mut self.page.blocks[index - 1];
            if !previous.text.is_empty()
                && !current_text.is_empty()
                && previous_kind == BlockKind::Code
            {
                previous.text.push('\n');
            }
            previous.text.push_str(&current_text);
        }
        self.page.blocks.remove(index);
        let target = self.page.blocks[index - 1].id;
        self.end_change();
        self.focus = Focus::Block(target);
        self.pending = Some(PendingFocus {
            target: Focus::Block(target),
            range: Some((previous_len, previous_len)),
            select_all: false,
        });
    }

    fn delete_merge_forward(&mut self, id: u64) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        if index + 1 >= self.page.blocks.len() {
            return;
        }
        let next_kind = self.page.blocks[index + 1].kind;
        let next_text = self.page.blocks[index + 1].text.clone();
        // The caret stays at the seam, where the removed break was.
        let caret = self.page.blocks[index].text.chars().count();
        self.begin_change();
        if next_kind == BlockKind::Divider {
            self.page.blocks.remove(index + 1);
        } else {
            if !self.page.blocks[index].text.is_empty()
                && !next_text.is_empty()
                && self.page.blocks[index].kind == BlockKind::Code
            {
                self.page.blocks[index].text.push('\n');
            }
            self.page.blocks[index].text.push_str(&next_text);
            self.page.blocks.remove(index + 1);
        }
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((caret, caret)),
            select_all: false,
        });
    }

    fn block_context_menu(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        id: u64,
        actions: &mut Vec<Action>,
    ) {
        if response.secondary_clicked() {
            if !self.index_of(id).is_some_and(|i| self.block_selected(i)) {
                self.start_block_selection(ui.ctx(), id);
            }
            self.slash = None;
        }
        let p = self.palette;
        let mut chosen = None;
        egui::Popup::context_menu(response)
            .frame(widgets::popup_frame(&p))
            .show(|ui| {
                ui.set_width(240.0);
                for (label, action) in [
                    ("Recortar", BlockContextAction::Cut),
                    ("Copiar", BlockContextAction::Copy),
                    ("Colar", BlockContextAction::Paste),
                ] {
                    if widgets::menu_row(ui, label, None, false, false, &p).clicked() {
                        chosen = Some(action);
                    }
                }
                ui.separator();
                for (label, action) in [
                    ("Duplicar", BlockContextAction::Duplicate),
                    ("Excluir", BlockContextAction::Delete),
                ] {
                    if widgets::menu_row(ui, label, None, label == "Excluir", false, &p).clicked() {
                        chosen = Some(action);
                    }
                }
                ui.separator();
                let common = self.block_selection_range().and_then(|(lo, hi)| {
                    let kind = self.page.blocks[lo].kind;
                    self.page.blocks[lo..=hi]
                        .iter()
                        .all(|b| b.kind == kind)
                        .then_some(kind)
                });
                ui.menu_button("Transformar em", |ui| {
                    ui.set_width(240.0);
                    for kind in BlockKind::ALL {
                        if widgets::menu_row(
                            ui,
                            &format!(
                                "{}{}",
                                if common == Some(kind) { "● " } else { "" },
                                kind.label()
                            ),
                            None,
                            false,
                            common == Some(kind),
                            &p,
                        )
                        .clicked()
                        {
                            chosen = Some(BlockContextAction::Transform(kind));
                        }
                    }
                    if chosen.is_some() {
                        ui.close();
                    }
                });
                ui.separator();
                for (label, delta) in [("Mover para cima", -1), ("Mover para baixo", 1)] {
                    if widgets::menu_row(ui, label, None, false, false, &p).clicked() {
                        chosen = Some(BlockContextAction::Move(delta));
                    }
                }
                if chosen.is_some() {
                    ui.close();
                }
            });
        if let Some(action) = chosen {
            actions.push(Action::BlockContext(action));
        }
    }

    pub fn apply_block_context(&mut self, action: BlockContextAction, ctx: &egui::Context) {
        let Some((lo, hi)) = self.block_selection_range() else {
            return;
        };
        match action {
            BlockContextAction::Copy | BlockContextAction::Cut => {
                ctx.copy_text(markdown::to_markdown(&self.page.blocks[lo..=hi]));
                if matches!(action, BlockContextAction::Cut) {
                    self.delete_block_selection();
                }
            }
            BlockContextAction::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            BlockContextAction::Delete => self.delete_block_selection(),
            BlockContextAction::Transform(kind) => self.transform_selection(kind),
            BlockContextAction::Duplicate => {
                self.begin_change();
                let mut blocks = self.page.blocks[lo..=hi].to_vec();
                for block in &mut blocks {
                    block.id = self.ids.next();
                }
                self.page.blocks.splice(hi + 1..hi + 1, blocks);
                self.end_change();
            }
            BlockContextAction::Move(delta) => {
                if delta < 0 && lo > 0 {
                    self.begin_change();
                    self.page.blocks[lo - 1..=hi].rotate_left(1);
                    self.end_change();
                } else if delta > 0 && hi + 1 < self.page.blocks.len() {
                    self.begin_change();
                    self.page.blocks[lo..=hi + 1].rotate_right(1);
                    self.end_change();
                }
            }
        }
        if let Some((anchor, _)) = self.block_selection {
            self.restore_selection_focus(anchor);
        }
    }

    fn restore_selection_focus(&mut self, id: u64) {
        let (a, b) = self.cursors.get(&id).copied().unwrap_or((0, 0));
        self.focus = Focus::Block(id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((a, b)),
            select_all: false,
        });
    }

    pub fn paste_block_selection(&mut self, text: &str) {
        let Some((lo, hi)) = self.block_selection_range() else {
            return;
        };
        let drafts = markdown::parse_blocks(text, true);
        if drafts.is_empty() {
            return;
        }
        self.begin_change();
        let blocks: Vec<_> = drafts
            .into_iter()
            .map(|draft| draft.into_block(self.ids.next()))
            .collect();
        let last = blocks.last().unwrap();
        let (id, caret) = (last.id, last.text.chars().count());
        self.page.blocks.splice(lo..=hi, blocks);
        self.block_selection = None;
        self.end_change();
        self.focus_block(id, caret);
    }

    fn transform_selection(&mut self, kind: BlockKind) {
        let Some((lo, hi)) = self.block_selection_range() else {
            return;
        };
        if !self.page.blocks[lo..=hi]
            .iter()
            .any(|b| b.kind != BlockKind::Divider && (b.kind != kind || kind == BlockKind::Todo))
        {
            return;
        }
        self.begin_change();
        for block in &mut self.page.blocks[lo..=hi] {
            if block.kind == BlockKind::Divider {
                continue;
            }
            block.kind = kind;
            if kind == BlockKind::Divider {
                block.text.clear();
            }
            if kind == BlockKind::Todo {
                block.checked = false;
            }
            if kind == BlockKind::Callout && block.emoji.is_empty() {
                block.emoji = "💡".to_owned();
            }
            if !kind.is_list() {
                block.indent = 0;
            }
        }
        self.end_change();
        if let Some((anchor, _)) = self.block_selection {
            self.restore_selection_focus(anchor);
        }
    }

    fn transform_block(&mut self, id: u64, kind: BlockKind) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        if self.page.blocks[index].kind == kind {
            return;
        }
        self.begin_change();
        let block = &mut self.page.blocks[index];
        block.kind = kind;
        if kind == BlockKind::Divider {
            block.text.clear();
        }
        if kind == BlockKind::Todo {
            block.checked = false;
        }
        if kind == BlockKind::Callout && block.emoji.is_empty() {
            block.emoji = "💡".to_owned();
        }
        if !kind.is_list() {
            block.indent = 0;
        }
        // Keep the caret where it was (Backspace at the start stays at 0).
        let chars = block.text.chars().count();
        let (a, b) = self.cursors.get(&id).copied().unwrap_or((chars, chars));
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((a.min(chars), b.min(chars))),
            select_all: false,
        });
    }

    fn adjust_indent(&mut self, id: u64, delta: i32) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let current = self.page.blocks[index].indent as i32;
        let next = (current + delta).clamp(0, 6) as u8;
        if next == self.page.blocks[index].indent {
            return;
        }
        let sel = self.cursors.get(&id).copied().unwrap_or((0, 0));
        self.begin_change();
        self.page.blocks[index].indent = next;
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some(sel),
            select_all: false,
        });
    }

    fn move_block(&mut self, id: u64, delta: i32) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let target = index as i32 + delta;
        if target < 0 || target as usize >= self.page.blocks.len() {
            return;
        }
        let sel = self.cursors.get(&id).copied().unwrap_or((0, 0));
        self.begin_change();
        let block = self.page.blocks.remove(index);
        self.page.blocks.insert(target as usize, block);
        self.end_change();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some(sel),
            select_all: false,
        });
        self.scroll_to = Some(id);
    }

    // -----------------------------------------------------------------------
    // Block selection (Shift+↑/↓)
    // -----------------------------------------------------------------------

    /// Collapse both the persistent TextEdit cursor and our cached range.
    fn clear_block_text_selection(&mut self, ctx: &egui::Context, id: u64) {
        let mut state = TextEditState::load(ctx, block_te_id(id)).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(0))));
        state.store(ctx, block_te_id(id));
        self.cursors.insert(id, (0, 0));
    }

    fn focus_page(&mut self, ctx: &egui::Context, down: bool, distance: f32, width: f32) -> bool {
        let (mut index, caret) = match self.focus {
            Focus::Block(id) => {
                let Some(index) = self.index_of(id) else {
                    return false;
                };
                let caret = TextEditState::load(ctx, block_te_id(id))
                    .and_then(|state| state.cursor.char_range())
                    .map_or(0, |range| range.primary.index.0);
                (index, caret)
            }
            Focus::Title if down => {
                self.focus_first_block(None);
                return true;
            }
            _ => return false,
        };
        let source = self.page.blocks[index].id;
        let column = self
            .block_galleys
            .get(&source)
            .map(|(galley, _)| caret_x(galley, CCursor::new(caret)));
        let mut traveled = 0.0;
        loop {
            let next = if down {
                self.next_text_block(index)
            } else {
                self.prev_text_block(index)
            };
            let Some(next) = next else {
                break;
            };
            let (lo, hi) = (index.min(next), index.max(next));
            traveled += self.page.blocks[lo..hi]
                .iter()
                .map(|block| {
                    self.block_heights
                        .get(&block.id)
                        .copied()
                        .unwrap_or_else(|| block.estimated_height(width - GUTTER, self.zoom))
                        + 2.0
                })
                .sum::<f32>();
            index = next;
            if traveled >= distance {
                break;
            }
        }
        let block = &self.page.blocks[index];
        let target = column
            .zip(self.block_galleys.get(&block.id))
            .map_or(caret.min(block.text.chars().count()), |(x, (galley, _))| {
                caret_at_row_x(galley, 0, x)
            });
        self.focus_block(block.id, target);
        true
    }

    fn point_in_text(&self, pos: egui::Pos2, dragging: bool) -> Option<(usize, usize)> {
        let (index, block) = self
            .page
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| self.block_rects.contains_key(&b.id))
            .filter(|(_, b)| dragging || self.block_rects[&b.id].contains(pos))
            .min_by(|(_, a), (_, b)| {
                let distance = |id| {
                    let r = self.block_rects[&id];
                    (pos.y - pos.y.clamp(r.top(), r.bottom())).abs()
                };
                distance(a.id).total_cmp(&distance(b.id))
            })?;
        // Non-text blocks have a row rect but no galley; their only valid
        // text endpoint is offset zero.
        let offset = self.block_galleys.get(&block.id).map_or(0, |(g, origin)| {
            g.cursor_from_pos(pos - *origin)
                .index
                .0
                .min(block.text.chars().count())
        });
        Some((index, offset))
    }

    fn activate_span(&mut self, ctx: &egui::Context, span: TextSpan) {
        self.block_selection = None;
        self.text_span = Some(span);
        self.text_anchor = Some(span.anchor);
        self.pending = None;
        self.slash = None;
        for block in &self.page.blocks {
            ctx.memory_mut(|m| m.surrender_focus(block_te_id(block.id)));
        }
        self.focus = Focus::None;
        self.scroll_to = self.page.blocks.get(span.head.0).map(|b| b.id);
    }

    fn span_pointer(&mut self, ctx: &egui::Context) {
        let (press, down, released, pos) = ctx.input(|i| {
            (
                i.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } => Some((*pos, modifiers.shift)),
                    _ => None,
                }),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
            )
        });
        // A press and movement can arrive in the same frame. The anchor must
        // use the button event position, not the final pointer position.
        if let Some((press_pos, shift)) = press {
            self.text_dragging = false;
            self.native_drag_origin = None;
            if let Some(point) = self.point_in_text(press_pos, false) {
                if shift {
                    let anchor = self
                        .text_span
                        .map(|s| s.anchor)
                        .or_else(|| match self.focus {
                            Focus::Block(id) => self
                                .index_of(id)
                                .zip(self.cursor_range_for(ctx, id).map(|r| r.1)),
                            _ => None,
                        })
                        .or(self.text_anchor)
                        .unwrap_or(point);
                    self.activate_span(
                        ctx,
                        TextSpan {
                            anchor,
                            head: point,
                        },
                    );
                    self.text_dragging = true;
                } else {
                    self.text_span = None;
                    self.text_anchor = Some(point);
                    self.native_drag_origin = Some(point);
                }
            } else {
                self.text_span = None;
                self.text_anchor = None;
            }
        }
        // TextEdit owns the entire gesture until the pointer leaves its block.
        // In particular, wrapped vertical drags must keep their native cursor
        // range, focus, clipboard handling, and selection painting.
        let left_origin = self.native_drag_origin.is_some_and(|(index, _)| {
            self.page.blocks.get(index).is_some_and(|block| {
                self.block_rects
                    .get(&block.id)
                    .is_some_and(|rect| pos.is_some_and(|point| !rect.contains(point)))
            })
        });
        if (down || released) && (self.text_dragging || left_origin) {
            if let Some((anchor, head)) = self
                .text_anchor
                .zip(pos.and_then(|p| self.point_in_text(p, true)))
            {
                if anchor != head || self.text_dragging {
                    self.text_dragging = true;
                    self.activate_span(ctx, TextSpan { anchor, head });
                }
            }
        }
        if self.text_dragging {
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            ..
                        } | egui::Event::PointerMoved(_)
                    )
                })
            });
        }
        if released {
            self.text_dragging = false;
            self.native_drag_origin = None;
        }
    }

    fn replace_span(&mut self, replacement: &str) {
        let Some(span) = self.text_span.take() else {
            return;
        };
        let ((lo, a), (hi, b)) = span.ordered();
        if hi >= self.page.blocks.len() {
            return;
        }
        self.begin_change();
        let first = self.page.blocks[lo].clone();
        let last = self.page.blocks[hi].clone();
        let prefix = &first.text[..markdown::char_to_byte(&first.text, a)];
        let suffix = &last.text[markdown::char_to_byte(&last.text, b)..];
        let mut merged = first.clone();
        if !merged.kind.accepts_text() {
            merged.kind = BlockKind::Paragraph;
        }
        merged.text = format!("{prefix}{replacement}{suffix}");
        let caret = prefix.chars().count() + replacement.chars().count();
        // Different textual kinds retain their respective formatting at the seam.
        if lo != hi && first.kind != last.kind && last.kind.accepts_text() {
            merged.text = format!("{prefix}{replacement}");
            let mut tail = last.clone();
            tail.text = suffix.to_owned();
            self.page.blocks.splice(lo..=hi, [merged, tail]);
        } else {
            self.page.blocks.splice(lo..=hi, [merged]);
        }
        self.text_anchor = None;
        self.end_change();
        self.focus_block(first.id, caret);
    }

    fn span_keys(&mut self, ctx: &egui::Context) -> bool {
        let Some(span) = self.text_span else {
            return false;
        };
        let ((lo, a), (hi, b)) = span.ordered();
        if hi >= self.page.blocks.len() {
            self.text_span = None;
            return false;
        }
        if self.consume_pub(ctx, Key::Escape, Modifiers::NONE) {
            // Collapse at the document-order end, including for a reverse drag.
            self.text_span = None;
            self.text_anchor = Some((hi, b));
            self.focus_block(self.page.blocks[hi].id, b);
            return true;
        }
        let copy = take_event(ctx, |e| matches!(e, egui::Event::Copy))
            || self.consume_pub(ctx, Key::C, Modifiers::COMMAND);
        let cut = take_event(ctx, |e| matches!(e, egui::Event::Cut))
            || self.consume_pub(ctx, Key::X, Modifiers::COMMAND);
        if copy || cut {
            let mut blocks = self.page.blocks[lo..=hi].to_vec();
            let end = markdown::char_to_byte(&blocks[hi - lo].text, b);
            blocks[hi - lo].text.truncate(end);
            let start = markdown::char_to_byte(&blocks[0].text, a);
            blocks[0].text.drain(..start);
            ctx.copy_text(markdown::to_markdown(&blocks));
            if cut {
                self.replace_span("");
            }
            return true;
        }
        if self.consume_pub(ctx, Key::Backspace, Modifiers::NONE)
            || self.consume_pub(ctx, Key::Delete, Modifiers::NONE)
        {
            self.replace_span("");
            return true;
        }
        let mut replacement = String::new();
        let mut typed = false;
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                if let egui::Event::Text(text) | egui::Event::Paste(text) = e {
                    replacement.push_str(text);
                    typed = true;
                    false
                } else {
                    true
                }
            })
        });
        if typed {
            self.replace_span(&replacement);
            return true;
        }
        for (key, forward) in [(Key::ArrowRight, true), (Key::ArrowLeft, false)] {
            if self.consume_pub(ctx, key, Modifiers::SHIFT) {
                let (mut index, mut offset) = span.head;
                let len = self.page.blocks[index].text.chars().count();
                if forward {
                    if offset < len {
                        offset += 1;
                    } else if index + 1 < self.page.blocks.len() {
                        index += 1;
                        offset = 0;
                    }
                } else if offset > 0 {
                    offset -= 1;
                } else if index > 0 {
                    index -= 1;
                    offset = self.page.blocks[index].text.chars().count();
                }
                self.activate_span(
                    ctx,
                    TextSpan {
                        head: (index, offset),
                        ..span
                    },
                );
                return true;
            }
        }
        for (key, down) in [(Key::ArrowUp, false), (Key::ArrowDown, true)] {
            if self.consume_pub(ctx, key, Modifiers::SHIFT) {
                let (mut index, offset) = span.head;
                let mut target = offset;
                if let Some((g, origin)) = self.block_galleys.get(&self.page.blocks[index].id) {
                    let cursor = CCursor::new(offset);
                    let row = caret_row(g, cursor);
                    let x = origin.x + caret_x(g, cursor);
                    if down && row + 1 < g.rows.len() {
                        target = caret_at_row_x(g, row + 1, x - origin.x);
                    } else if !down && row > 0 {
                        target = caret_at_row_x(g, row - 1, x - origin.x);
                    } else if (down && index + 1 < self.page.blocks.len()) || (!down && index > 0) {
                        index = if down { index + 1 } else { index - 1 };
                        target = self.block_galleys.get(&self.page.blocks[index].id).map_or(
                            0,
                            |(g, origin)| {
                                caret_at_row_x(
                                    g,
                                    if down {
                                        0
                                    } else {
                                        g.rows.len().saturating_sub(1)
                                    },
                                    x - origin.x,
                                )
                            },
                        );
                    }
                }
                self.activate_span(
                    ctx,
                    TextSpan {
                        head: (index, target),
                        ..span
                    },
                );
                return true;
            }
        }
        true
    }

    fn paint_span(
        &self,
        ui: &Ui,
        index: usize,
        left: f32,
        width: f32,
        rect: Rect,
        shape: egui::layers::ShapeIdx,
    ) {
        let Some(span) = self.text_span else {
            return;
        };
        let ((lo, a), (hi, b)) = span.ordered();
        if index < lo || index > hi {
            return;
        }
        let color = self.palette.selection;
        if index > lo && index < hi || !self.page.blocks[index].kind.accepts_text() {
            ui.painter().set(
                shape,
                Shape::rect_filled(
                    Rect::from_min_size(pos2(left, rect.top()), vec2(width, rect.height())),
                    4,
                    color,
                ),
            );
            return;
        }
        let Some((g, origin)) = self.block_galleys.get(&self.page.blocks[index].id) else {
            return;
        };
        let start = if index == lo { a } else { 0 };
        let end = if index == hi {
            b
        } else {
            self.page.blocks[index].text.chars().count()
        };
        let mut shapes = Vec::new();
        for col in start..end {
            let cursor = g.pos_from_cursor(CCursor::new(col));
            let next = g.pos_from_cursor(CCursor::new(col + 1));
            let right = if next.top() == cursor.top() {
                next.left()
            } else {
                g.size().x
            };
            let r = Rect::from_min_max(
                cursor.min + origin.to_vec2(),
                pos2(right.max(cursor.left() + 2.0), cursor.bottom()) + origin.to_vec2(),
            );
            shapes.push(Shape::rect_filled(r, 0, color));
        }
        if span.head.0 == index {
            let caret = g.pos_from_cursor(CCursor::new(span.head.1));
            shapes.push(Shape::line_segment(
                [
                    caret.min + origin.to_vec2(),
                    caret.left_bottom() + origin.to_vec2(),
                ],
                Stroke::new(1.0, self.palette.text),
            ));
        }
        ui.painter().set(shape, Shape::Vec(shapes));
    }

    fn start_block_selection(&mut self, ctx: &egui::Context, id: u64) {
        // The text field lets go of the keyboard so typing does not edit a
        // block while several are selected.
        self.clear_block_text_selection(ctx, id);
        ctx.memory_mut(|memory| memory.surrender_focus(block_te_id(id)));
        self.focus = Focus::None;
        self.pending = None;
        self.slash = None;
        self.text_span = None;
        self.text_anchor = None;
        self.block_selection = Some((id, id));
    }

    /// Index range (inclusive) of the selected blocks, if the selection is
    /// still valid for the current page.
    pub fn block_selection_range(&self) -> Option<(usize, usize)> {
        let (anchor, extent) = self.block_selection?;
        let a = self.index_of(anchor)?;
        let b = self.index_of(extent)?;
        Some((a.min(b), a.max(b)))
    }

    fn block_selected(&self, index: usize) -> bool {
        self.block_selection_range()
            .is_some_and(|(lo, hi)| lo <= index && index <= hi)
    }

    /// Keys while blocks are selected. Returns true when the key was handled.
    fn selection_keys(&mut self, ctx: &egui::Context) -> bool {
        let Some((anchor, extent)) = self.block_selection else {
            return false;
        };
        let Some(extent_index) = self.index_of(extent) else {
            self.block_selection = None;
            return false;
        };
        if self.index_of(anchor).is_none() {
            self.block_selection = None;
            return false;
        }
        let extent_kind = self.page.blocks[extent_index].kind;
        for (key, kind) in [
            (Key::Num1, BlockKind::Heading1),
            (Key::Num2, BlockKind::Heading2),
            (Key::Num3, BlockKind::Heading3),
            (Key::Num0, BlockKind::Paragraph),
        ] {
            if self.consume_pub(ctx, key, Modifiers::COMMAND | Modifiers::ALT) {
                self.transform_selection(kind);
                return true;
            }
        }

        // Escape restores the anchor; Enter edits the moving end.
        if self.consume_pub(ctx, Key::Escape, Modifiers::NONE) {
            self.block_selection = None;
            self.clear_block_text_selection(ctx, anchor);
            self.clear_block_text_selection(ctx, extent);
            self.focus_block(anchor, 0);
            return true;
        }
        if self.consume_pub(ctx, Key::Enter, Modifiers::NONE) {
            self.block_selection = None;
            if extent_kind.accepts_text() {
                let length = self.page.blocks[extent_index].text.chars().count();
                self.focus_block(extent, length);
            }
            return true;
        }
        if self.consume_pub(ctx, Key::Backspace, Modifiers::NONE)
            || self.consume_pub(ctx, Key::Delete, Modifiers::NONE)
        {
            self.delete_block_selection();
            return true;
        }
        // egui-winit turns Ctrl+C / Ctrl+X into `Event::Copy` / `Event::Cut`.
        let copy = take_event(ctx, |event| matches!(event, egui::Event::Copy))
            || self.consume_pub(ctx, Key::C, Modifiers::COMMAND);
        let cut = !copy
            && (take_event(ctx, |event| matches!(event, egui::Event::Cut))
                || self.consume_pub(ctx, Key::X, Modifiers::COMMAND));
        if copy || cut {
            if let Some((lo, hi)) = self.block_selection_range() {
                ctx.copy_text(markdown::to_markdown(&self.page.blocks[lo..=hi]));
            }
            if cut {
                self.delete_block_selection();
            }
            return true;
        }
        for (key, delta) in [(Key::ArrowUp, -1i32), (Key::ArrowDown, 1)] {
            let shift = self.consume_pub(ctx, key, Modifiers::SHIFT);
            if !shift && !self.consume_pub(ctx, key, Modifiers::NONE) {
                continue;
            }
            let next =
                (extent_index as i32 + delta).clamp(0, self.page.blocks.len() as i32 - 1) as usize;
            let next_id = self.page.blocks[next].id;
            // Shift extends the selection; a plain arrow moves it.
            let new_anchor = if shift { anchor } else { next_id };
            self.block_selection = Some((new_anchor, next_id));
            self.scroll_to = Some(next_id);
            return true;
        }
        false
    }

    fn delete_block_selection(&mut self) {
        let Some((lo, hi)) = self.block_selection_range() else {
            self.block_selection = None;
            return;
        };
        self.block_selection = None;
        self.begin_change();
        self.page.blocks.drain(lo..=hi);
        if self.page.blocks.is_empty() {
            self.page.blocks.push(crate::model::Block::new(
                self.ids.next(),
                BlockKind::Paragraph,
            ));
        }
        self.end_change();

        // Caret at the end of the block above the hole, else at the start of
        // the block below it.
        let above = lo
            .checked_sub(1)
            .filter(|i| self.page.blocks[*i].kind.accepts_text())
            .map(|i| (i, self.page.blocks[i].text.chars().count()));
        let below = (lo < self.page.blocks.len() && self.page.blocks[lo].kind.accepts_text())
            .then_some((lo, 0));
        if let Some((index, caret)) = above.or(below) {
            let id = self.page.blocks[index].id;
            self.focus = Focus::Block(id);
            self.pending = Some(PendingFocus {
                target: Focus::Block(id),
                range: Some((caret, caret)),
                select_all: false,
            });
            self.scroll_to = Some(id);
        }
    }

    fn cursor_range_for(&self, ctx: &egui::Context, id: u64) -> Option<(usize, usize)> {
        let te_id = Id::unique(("simion-block", id));
        let state = TextEditState::load(ctx, te_id)?;
        let range = state.cursor.char_range()?;
        let a = range.primary.index.0;
        let b = range.secondary.index.0;
        Some((a.min(b), a.max(b)))
    }

    pub fn consume_pub(&self, ctx: &egui::Context, key: Key, modifiers: Modifiers) -> bool {
        self.consume_key(ctx, key, modifiers)
    }
}

fn block_te_id(id: u64) -> Id {
    Id::unique(("simion-block", id))
}

fn title_te_id(page_id: i64) -> Id {
    Id::unique(("simion-title", page_id))
}

/// Removes the first input event matching `wanted`.
fn take_event(ctx: &egui::Context, wanted: impl Fn(&egui::Event) -> bool) -> bool {
    ctx.input_mut(|input| {
        let position = input.events.iter().position(wanted);
        position.is_some_and(|position| {
            input.events.remove(position);
            true
        })
    })
}

/// `caret` clamped to the text of `galley` (it may come from an older frame).
fn clamp_cursor(galley: &egui::Galley, caret: CCursor) -> CCursor {
    let chars = galley.text().chars().count();
    CCursor {
        prefer_next_row: caret.prefer_next_row,
        ..CCursor::new(caret.index.0.min(chars))
    }
}

/// Visual row of `galley` the caret is on.
fn caret_row(galley: &egui::Galley, caret: CCursor) -> usize {
    let row = galley.layout_from_cursor(clamp_cursor(galley, caret)).row;
    row.min(galley.rows.len().saturating_sub(1))
}

/// x of the caret, relative to the galley.
fn caret_x(galley: &egui::Galley, caret: CCursor) -> f32 {
    galley
        .pos_from_cursor(clamp_cursor(galley, caret))
        .center()
        .x
}

/// Char index closest to `x` (relative to the galley) on visual row `row`.
fn caret_at_row_x(galley: &egui::Galley, row: usize, x: f32) -> usize {
    let Some(placed) = galley
        .rows
        .get(row.min(galley.rows.len().saturating_sub(1)))
    else {
        return 0;
    };
    let start: usize = galley.rows[..row.min(galley.rows.len() - 1)]
        .iter()
        .map(|row| row.char_count_including_newline().0)
        .sum();
    (start + placed.char_at(x - placed.pos.x).0).min(galley.text().chars().count())
}

pub struct BlockOutcome {
    pub span_shape: egui::layers::ShapeIdx,
    pub changed: bool,
    pub focused: bool,
    pub cursor: Option<(usize, usize)>,
    pub rect: Rect,
    pub rows: usize,
    /// The laid-out text and its screen position (none for dividers).
    pub galley: Option<(Arc<egui::Galley>, egui::Pos2)>,
}

fn hint_for(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "Digite '/' para comandos",
        BlockKind::Heading1 => "Título 1",
        BlockKind::Heading2 => "Título 2",
        BlockKind::Heading3 => "Título 3",
        BlockKind::Bullet | BlockKind::Numbered => "Item",
        BlockKind::Todo => "Tarefa",
        BlockKind::Quote => "Citação",
        BlockKind::Callout => "Destaque",
        BlockKind::Code => "Código",
        BlockKind::Divider => "",
    }
}

/// Label for a block's language: the pretty name, or the stored name when the
/// language is unknown (e.g. `mermaid` pasted from Markdown).
pub fn lang_label(lang: &str) -> &str {
    if syntax::normalize(lang) == "plain" && !syntax::canonical(lang).is_empty() {
        lang
    } else {
        syntax::label(lang)
    }
}

/// Code text with one section per highlighted span. Falls back to a single
/// color if the spans do not cover the text exactly.
fn code_job(
    text: &str,
    spans: &[(Range<usize>, Color32)],
    font: FontId,
    color: Color32,
    wrap: f32,
    line_height: f32,
) -> LayoutJob {
    let covers = spans.first().is_some_and(|(range, _)| range.start == 0)
        && spans
            .last()
            .is_some_and(|(range, _)| range.end == text.len())
        && spans
            .windows(2)
            .all(|pair| pair[0].0.end == pair[1].0.start)
        && spans.iter().all(|(range, _)| {
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
        });
    if !covers {
        return plain_job(text, font, color, wrap, line_height);
    }
    let mut job = plain_job(text, font, color, wrap, line_height);
    let Some(base) = job.sections.first().map(|section| section.format.clone()) else {
        return job;
    };
    job.sections = spans
        .iter()
        .map(|(range, color)| LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(range.start)..ByteIndex(range.end),
            format: TextFormat {
                color: *color,
                ..base.clone()
            },
        })
        .collect();
    job
}

fn plain_job(text: &str, font: FontId, color: Color32, wrap: f32, line_height: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text = text.to_owned();
    job.wrap.max_width = wrap;
    let mut format = TextFormat {
        font_id: font,
        color,
        line_height: Some(line_height),
        ..Default::default()
    };
    format.valign = Align::Center;
    if !text.is_empty() {
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(0)..ByteIndex(text.len()),
            format,
        });
    }
    job
}

fn inline_job(
    text: &str,
    layout: &InlineLayout,
    base_size: f32,
    base_family: &str,
    line_factor: f32,
    base_color: Color32,
    checked: bool,
    active: Option<(usize, usize)>,
    p: &Palette,
    wrap: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text = text.to_owned();
    job.wrap.max_width = wrap;
    job.break_on_newline = true;
    let line_height = base_size * line_factor;
    let active_bytes = active.map(|(start, end)| {
        (
            markdown::char_to_byte(text, start),
            markdown::char_to_byte(text, end),
        )
    });

    for piece in &layout.pieces {
        let style = piece.style;
        let revealed = piece.marker
            && active_bytes.is_some_and(|(start, end)| {
                piece.group_range.start <= end && piece.group_range.end >= start
            });

        let font_size = if piece.marker && !revealed {
            1.0
        } else if style.code {
            base_size * 0.92
        } else {
            base_size
        };

        let font_family = if style.code {
            FONT_MONO
        } else if style.bold && style.italic {
            FONT_SEMIBOLD_ITALIC
        } else if style.bold {
            FONT_SEMIBOLD
        } else if style.italic {
            FONT_ITALIC
        } else {
            base_family
        };

        let color = if piece.marker {
            if revealed {
                p.faint
            } else {
                Color32::TRANSPARENT
            }
        } else if style.link {
            p.accent_text
        } else if checked {
            p.muted
        } else {
            base_color
        };

        let underline = if style.link && !piece.marker {
            Stroke::new(1.0, p.accent_text.gamma_multiply(0.7))
        } else {
            Stroke::NONE
        };
        let strikethrough = if style.strike || checked {
            Stroke::new(1.2, color)
        } else {
            Stroke::NONE
        };

        let format = TextFormat {
            font_id: FontId::new(font_size, family(font_family)),
            color,
            background: if style.code {
                p.code_bg
            } else {
                Color32::TRANSPARENT
            },
            expand_bg: if style.code { 1.5 } else { 0.0 },
            underline,
            strikethrough,
            line_height: Some(line_height),
            valign: Align::Center,
            ..Default::default()
        };

        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(piece.range.start)..ByteIndex(piece.range.end),
            format,
        });
    }

    job
}

fn open_url(url: &str) {
    let url = if url.contains("://") {
        url.to_owned()
    } else {
        format!("https://{url}")
    };
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }
}

pub fn status_bar(app: &mut App, ui: &mut Ui) {
    let p = app.palette;
    let words: usize = std::iter::once(app.page.meta.title.as_str())
        .chain(app.page.blocks.iter().map(|block| block.text.as_str()))
        .map(|text| text.split_whitespace().count())
        .sum();
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let (dot, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
        let color = if app.dirty { p.faint } else { p.accent };
        ui.painter().circle_filled(dot.center(), 3.0, color);
        let label = if app.dirty { "Editando…" } else { "Salvo" };
        ui.label(RichText::new(label).size(11.5).color(p.faint));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            let zoom = (app.zoom * 100.0).round() as i32;
            ui.label(RichText::new(format!("{zoom}%")).size(11.5).color(p.faint));
            ui.label(RichText::new("·").size(11.5).color(p.faint));
            ui.label(
                RichText::new(format!("{words} palavras"))
                    .size(11.5)
                    .color(p.faint),
            );
            ui.label(RichText::new("·").size(11.5).color(p.faint));
            let label = ui.label(RichText::new("SQLite local").size(11.5).color(p.faint));
            if let Some(store) = app.store.as_ref() {
                label.on_hover_text(format!("Arquivo: {}", store.path().display()));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Block;
    use egui::{Event, RawInput};

    /// Runs the real editor headless, so text fields, galleys and key
    /// handling behave as in the app.
    struct Harness {
        app: App,
        ctx: egui::Context,
        copied: Vec<String>,
        last_actions: Vec<Action>,
        selection_rects: Vec<Rect>,
    }

    impl Harness {
        fn new(title: &str, blocks: &[(BlockKind, &str)]) -> Self {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let mut app = App::blank();
            crate::theme::apply(&ctx, &app.palette, 1.0);
            app.page.meta.title = title.to_owned();
            app.page.blocks = blocks
                .iter()
                .map(|(kind, text)| {
                    let mut block = Block::new(app.ids.next(), *kind);
                    block.text = (*text).to_owned();
                    block
                })
                .collect();
            let mut harness = Self {
                app,
                ctx,
                copied: Vec::new(),
                last_actions: Vec::new(),
                selection_rects: Vec::new(),
            };
            harness.idle();
            harness
        }

        fn frame(&mut self, events: Vec<Event>) {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0))),
                events,
                ..Default::default()
            };
            let app = &mut self.app;
            let mut actions = Vec::new();
            let mut output = self.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                app.editor_keys(&ctx);
                app.editor(ui, &mut actions);
            });
            self.last_actions = actions;
            fn selection_rects(shape: &Shape, color: Color32, rects: &mut Vec<Rect>) {
                match shape {
                    Shape::Vec(shapes) => {
                        for shape in shapes {
                            selection_rects(shape, color, rects);
                        }
                    }
                    Shape::Rect(rect) if rect.fill == color => rects.push(rect.rect),
                    _ => {}
                }
            }
            self.selection_rects.clear();
            for shape in &output.shapes {
                selection_rects(
                    &shape.shape,
                    self.app.palette.selection,
                    &mut self.selection_rects,
                );
            }
            // No renderer here: the font atlas updates are dropped on purpose.
            output.textures_delta.clear();
            for command in output.platform_output.commands {
                if let egui::OutputCommand::CopyText(text) = command {
                    self.copied.push(text);
                }
            }
        }

        fn idle(&mut self) {
            self.frame(Vec::new());
            self.frame(Vec::new());
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }]);
            self.idle();
        }

        fn event(&mut self, event: Event) {
            self.frame(vec![event]);
            self.idle();
        }

        fn type_text(&mut self, text: &str) {
            self.event(Event::Text(text.to_owned()));
        }

        fn click(&mut self, pos: egui::Pos2) -> Vec<Action> {
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }]);
            self.frame(vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
            let actions = std::mem::take(&mut self.last_actions);
            self.idle();
            actions
        }

        fn id(&self, index: usize) -> u64 {
            self.app.page.blocks[index].id
        }

        /// Puts the caret in block `index`, as a click would.
        fn caret_in(&mut self, index: usize, caret: usize) {
            let id = self.id(index);
            self.app.focus_block(id, caret);
            self.idle();
        }

        fn click_in(&mut self, index: usize, caret: usize) {
            let (galley, origin) = self.app.block_galleys[&self.id(index)].clone();
            let pos = origin
                + galley
                    .pos_from_cursor(CCursor::new(caret))
                    .center()
                    .to_vec2();
            self.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]);
            self.frame(vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
            self.idle();
        }

        fn texts(&self) -> Vec<&str> {
            self.app
                .page
                .blocks
                .iter()
                .map(|block| block.text.as_str())
                .collect()
        }
    }

    fn long_text() -> String {
        (1..=40)
            .map(|i| format!("word{i:02}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn drag_text(h: &mut Harness, a: (usize, usize), b: (usize, usize)) {
        let point = |h: &Harness, (index, col)| {
            let (g, origin) = &h.app.block_galleys[&h.id(index)];
            *origin + g.pos_from_cursor(CCursor::new(col)).center().to_vec2()
        };
        let start = point(h, a);
        let end = point(h, b);
        h.frame(vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.frame(vec![Event::PointerMoved(end)]);
        h.frame(vec![Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
        h.idle();
    }

    #[test]
    fn context_menu_keeps_inside_selection_and_retargets_outside() {
        let mut h = Harness::new(
            "",
            &[
                (BlockKind::Paragraph, "A"),
                (BlockKind::Paragraph, "B"),
                (BlockKind::Paragraph, "C"),
            ],
        );
        let selection = Some((h.id(0), h.id(1)));
        h.app.block_selection = selection;
        for index in [1, 2] {
            let pos = h.app.block_rects[&h.id(index)].center();
            h.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]);
            h.frame(vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
            assert!(egui::Popup::is_any_open(&h.ctx));
            assert_eq!(
                h.app.block_selection,
                if index == 1 {
                    selection
                } else {
                    Some((h.id(2), h.id(2)))
                }
            );
            egui::Popup::close_all(&h.ctx);
            h.idle();
        }
    }

    #[test]
    fn batch_transform_preserves_structure_selection_and_undo() {
        let mut h = Harness::new(
            "",
            &[
                (BlockKind::Paragraph, "A"),
                (BlockKind::Divider, ""),
                (BlockKind::Bullet, "B"),
                (BlockKind::Todo, "C"),
            ],
        );
        h.app.page.blocks[2].indent = 2;
        h.app.page.blocks[3].checked = true;
        let selection = Some((h.id(0), h.id(3)));
        h.app.block_selection = selection;
        h.app.begin_change();
        let before = h.app.page.blocks.clone();
        let undo_count = h.app.history.undo.len();
        h.app.transform_selection(BlockKind::Todo);
        assert_eq!(h.app.history.undo.len(), undo_count + 1);
        assert_eq!(h.app.block_selection, selection);
        assert_eq!(h.app.page.blocks[1].kind, BlockKind::Divider);
        assert_eq!(h.app.page.blocks[2].indent, 2);
        for i in [0, 2, 3] {
            assert_eq!(h.app.page.blocks[i].kind, BlockKind::Todo);
            assert!(!h.app.page.blocks[i].checked);
            assert_eq!(h.app.page.blocks[i].text, before[i].text);
        }
        h.app.undo();
        assert_eq!(
            markdown::to_markdown(&h.app.page.blocks),
            markdown::to_markdown(&before)
        );
    }

    #[test]
    fn selection_duplicate_move_and_paste() {
        let mut h = Harness::new(
            "",
            &[
                (BlockKind::Paragraph, "A"),
                (BlockKind::Bullet, "B"),
                (BlockKind::Paragraph, "C"),
            ],
        );
        h.app.block_selection = Some((h.id(0), h.id(1)));
        let selection = h.app.block_selection;
        h.app
            .apply_block_context(BlockContextAction::Duplicate, &h.ctx);
        assert_eq!(h.texts(), vec!["A", "B", "A", "B", "C"]);
        let ids: std::collections::HashSet<_> = h.app.page.blocks.iter().map(|b| b.id).collect();
        assert_eq!(ids.len(), 5);
        h.app
            .apply_block_context(BlockContextAction::Move(1), &h.ctx);
        assert_eq!(h.texts(), vec!["A", "A", "B", "B", "C"]);
        assert_eq!(h.app.block_selection, selection);
        h.app.paste_block_selection("replacement");
        assert_eq!(h.texts(), vec!["A", "replacement", "B", "C"]);
        assert!(h.app.block_selection.is_none());
    }

    #[test]
    fn selection_heading_shortcut_and_divider_only_noop() {
        let mut h = Harness::new(
            "",
            &[(BlockKind::Paragraph, "A"), (BlockKind::Paragraph, "B")],
        );
        h.app.block_selection = Some((h.id(0), h.id(1)));
        h.key(Key::Num2, Modifiers::COMMAND | Modifiers::ALT);
        assert!(
            h.app
                .page
                .blocks
                .iter()
                .all(|b| b.kind == BlockKind::Heading2)
        );
        h.app.transform_selection(BlockKind::Divider);
        let count = h.app.history.undo.len();
        h.app.transform_selection(BlockKind::Paragraph);
        assert_eq!(h.app.history.undo.len(), count);
        assert!(
            h.app
                .page
                .blocks
                .iter()
                .all(|b| b.kind == BlockKind::Divider)
        );
    }

    #[test]
    fn batched_drag_paints_and_copies_both_endpoints() {
        for reverse in [false, true] {
            let mut h = Harness::new(
                "",
                &[
                    (BlockKind::Paragraph, "alpha one"),
                    (BlockKind::Paragraph, "bravo two"),
                ],
            );
            batched_drag(&mut h, (0, 5), (1, 2), reverse);
            assert_eq!(h.app.text_span.unwrap().ordered(), ((0, 5), (1, 2)));
            for index in 0..2 {
                let (g, origin) = &h.app.block_galleys[&h.id(index)];
                let col = if index == 0 { 6 } else { 1 };
                let caret = g
                    .pos_from_cursor(CCursor::new(col))
                    .translate(origin.to_vec2());
                assert!(h.selection_rects.iter().any(|r| r.intersects(caret)));
            }
            h.event(Event::Copy);
            assert_eq!(h.copied.last().unwrap(), " one\n\nbr\n");
            h.key(Key::Backspace, Modifiers::NONE);
            assert_eq!(h.texts(), vec!["alphaavo two"]);
        }
    }

    fn batched_drag(h: &mut Harness, a: (usize, usize), b: (usize, usize), reverse: bool) {
        let point = |(index, col)| {
            let (g, origin) = &h.app.block_galleys[&h.id(index)];
            *origin + g.pos_from_cursor(CCursor::new(col)).center().to_vec2()
        };
        let (start, end) = if reverse {
            (point(b), point(a))
        } else {
            (point(a), point(b))
        };
        h.frame(vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.idle();
    }

    #[test]
    fn native_drag_within_single_and_wrapped_paragraphs() {
        for text in [
            "bravo two".to_owned(),
            (1..=60)
                .map(|i| format!("w{i:02}"))
                .collect::<Vec<_>>()
                .join(" "),
        ] {
            let mut h = Harness::new("", &[(BlockKind::Paragraph, &text)]);
            let end = if text.len() > 20 {
                let (g, origin) = &h.app.block_galleys[&h.id(0)];
                assert!(g.rows.len() > 1);
                // Same x, next visual row: exercise y -> row -> character.
                let start = g.pos_from_cursor(CCursor::new(2));
                h.app
                    .point_in_text(
                        *origin + vec2(start.center().x, g.rows[1].min_y() + 2.0),
                        true,
                    )
                    .unwrap()
                    .1
            } else {
                5
            };
            drag_text(&mut h, (0, 2), (0, end));
            assert!(h.app.text_span.is_none());
            h.event(Event::Copy);
            assert_eq!(h.copied.last().unwrap(), &text[2..end]);
            assert_eq!(h.app.cursor_range_for(&h.ctx, h.id(0)), Some((2, end)));
            h.type_text("Q");
            assert_eq!(h.texts(), vec![format!("{}Q{}", &text[..2], &text[end..])]);
        }
    }

    #[test]
    fn wrapped_drag_uses_pointer_row_and_column() {
        let text = (1..=60)
            .map(|i| format!("w{i:02}"))
            .collect::<Vec<_>>()
            .join(" ");
        for reverse in [false, true] {
            for x in [80.0, 160.0, 500.0] {
                for target_row in [1, 2, usize::MAX] {
                    let mut h = Harness::new(
                        "",
                        &[(BlockKind::Paragraph, &text), (BlockKind::Paragraph, "fim")],
                    );
                    let g = h.app.block_galleys[&h.id(0)].0.clone();
                    // Use visible block geometry, independently of the cached
                    // galley origin under test. Keep the paragraph unfocused.
                    let rect = h.app.block_rects[&h.id(0)];
                    let origin =
                        rect.min + vec2(GUTTER, BlockKind::Paragraph.vertical_margins().0.round());
                    assert!(origin.y > g.rows.last().unwrap().max_y());
                    assert!(g.rows.len() > 1);
                    let row = target_row.min(g.rows.len() - 1);
                    let column = g.rows[0].char_at(x - g.rows[0].pos.x).0;
                    let end = g.rows[..row]
                        .iter()
                        .map(|r| r.char_count_including_newline().0)
                        .sum::<usize>()
                        + g.rows[row].char_at(x - g.rows[row].pos.x).0;
                    assert!(end > column);
                    if row < g.rows.len() - 1 {
                        assert!(end < text.len());
                    }
                    let a = origin + vec2(x, g.rows[0].rect().center().y);
                    let b = origin + vec2(x, g.rows[row].rect().center().y);
                    assert_eq!(h.app.point_in_text(a, false), Some((0, column)));
                    assert_eq!(h.app.point_in_text(b, true), Some((0, end)));
                    let (start, finish) = if reverse { (b, a) } else { (a, b) };
                    h.frame(vec![
                        Event::PointerMoved(start),
                        Event::PointerButton {
                            pos: start,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: Modifiers::NONE,
                        },
                    ]);
                    h.frame(vec![Event::PointerMoved(finish)]);
                    h.frame(vec![Event::PointerButton {
                        pos: finish,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    }]);
                    h.idle();
                    assert!(h.app.text_span.is_none());
                    h.event(Event::Copy);
                    assert_eq!(h.copied.last().unwrap(), &text[column..end]);
                    assert_eq!(h.app.cursor_range_for(&h.ctx, h.id(0)), Some((column, end)));
                    h.type_text("Q");
                    assert_eq!(
                        h.texts(),
                        vec![
                            format!("{}Q{}", &text[..column], &text[end..]),
                            "fim".to_owned()
                        ]
                    );
                }
            }
        }
    }

    #[test]
    fn drag_span_partial_endpoints_both_directions_and_copy() {
        for reverse in [false, true] {
            let mut h = Harness::new(
                "",
                &[
                    (BlockKind::Paragraph, "alpha one"),
                    (BlockKind::Divider, ""),
                    (BlockKind::Paragraph, "bravo two"),
                ],
            );
            let (a, b) = ((0, 6), (2, 6));
            drag_text(
                &mut h,
                if reverse { b } else { a },
                if reverse { a } else { b },
            );
            assert_eq!(h.app.text_span.unwrap().ordered(), (a, b));
            h.event(Event::Copy);
            assert_eq!(h.copied.last().unwrap(), "one\n\n---\n\nbravo\n");
            h.key(Key::Backspace, Modifiers::NONE);
            assert_eq!(h.texts(), vec!["alpha two"]);
        }
    }

    #[test]
    fn drag_span_wrapped_text_replacement_and_escape() {
        let text = "a long paragraph with many words ".repeat(12);
        let mut h = Harness::new("", &[(BlockKind::Paragraph, &text)]);
        assert!(h.app.block_galleys[&h.id(0)].0.rows.len() > 1);
        drag_text(&mut h, (0, 5), (0, 100));
        h.type_text("Q");
        assert_eq!(h.texts(), vec![format!("{}Q{}", &text[..5], &text[100..])]);
        drag_text(&mut h, (0, 20), (0, 3));
        h.key(Key::Escape, Modifiers::NONE);
        h.type_text("Z");
        assert_eq!(h.app.page.blocks[0].text.chars().nth(20), Some('Z'));
    }

    #[test]
    fn shift_click_extends_anchor_and_cut_removes_unicode_span() {
        let mut h = Harness::new(
            "",
            &[
                (BlockKind::Paragraph, "áλpha"),
                (BlockKind::Paragraph, "brávo"),
            ],
        );
        h.click_in(0, 2);
        let (g, origin) = &h.app.block_galleys[&h.id(1)];
        let pos = *origin + g.pos_from_cursor(CCursor::new(2)).center().to_vec2();
        for pressed in [true, false] {
            h.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::SHIFT,
                },
            ]);
        }
        assert_eq!(h.app.text_span.unwrap().ordered(), ((0, 2), (1, 2)));
        h.event(Event::Cut);
        assert_eq!(h.copied.last().unwrap(), "pha\n\nbr\n");
        assert_eq!(h.texts(), vec!["áλávo"]);
    }

    #[test]
    fn shift_right_crosses_block_and_replaces() {
        let mut h = Harness::new(
            "",
            &[
                (BlockKind::Paragraph, "alpha"),
                (BlockKind::Paragraph, "bravo"),
            ],
        );
        h.caret_in(0, 5);
        h.key(Key::ArrowRight, Modifiers::SHIFT);
        assert_eq!(h.app.text_span.unwrap().head, (1, 0));
        h.key(Key::ArrowRight, Modifiers::SHIFT);
        h.type_text("Q");
        assert_eq!(h.texts(), vec!["alphaQravo"]);
        h.app.undo();
        assert_eq!(h.texts(), vec!["alpha", "bravo"]);
    }

    #[test]
    fn caret_helpers_follow_visual_rows() {
        let text = long_text();
        let h = Harness::new("t", &[(BlockKind::Paragraph, text.as_str())]);
        let laid = h.app.block_galleys.get(&h.id(0)).cloned();
        assert!(laid.is_some(), "block was not laid out");
        let Some((galley, _)) = laid else {
            return;
        };
        let rows = galley.rows.len();
        assert!(rows >= 2, "test text should wrap, got {rows} rows");
        assert_eq!(caret_row(&galley, CCursor::new(0)), 0);
        assert_eq!(caret_row(&galley, CCursor::new(text.len())), rows - 1);
        // Out-of-range carets (stale state) are clamped, not a panic.
        assert_eq!(caret_row(&galley, CCursor::new(text.len() + 50)), rows - 1);
        let x = caret_x(&galley, CCursor::new(3));
        assert_eq!(caret_at_row_x(&galley, 0, x), 3);
        let below = caret_at_row_x(&galley, 1, x);
        assert_eq!(caret_row(&galley, CCursor::new(below)), 1);
        assert_eq!(caret_at_row_x(&galley, 99, 1.0e6), text.len());
    }

    #[test]
    fn down_and_up_cross_blocks_in_one_press_keeping_the_column() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "aaaaa one"),
                (BlockKind::Paragraph, "aaaaa two"),
            ],
        );
        h.caret_in(0, 5);
        h.key(Key::ArrowDown, Modifiers::NONE);
        h.type_text("Q");
        assert_eq!(h.texts(), ["aaaaa one", "aaaaaQ two"]);
        h.caret_in(1, 5);
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.type_text("R");
        assert_eq!(h.texts(), ["aaaaaR one", "aaaaaQ two"]);
    }

    #[test]
    fn up_and_down_move_by_visual_row_inside_a_wrapped_block() {
        let text = long_text();
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "above"),
                (BlockKind::Paragraph, text.as_str()),
                (BlockKind::Paragraph, "below"),
            ],
        );
        let id = h.id(1);
        h.caret_in(1, 2);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(
            h.app.focus,
            Focus::Block(id),
            "Down on row 0 stays in the block"
        );
        // From the last row, Down leaves; Up from `below` lands on the last row.
        h.caret_in(1, text.chars().count());
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(2)));
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.type_text("Q");
        let merged = &h.app.page.blocks[1].text;
        let q = merged.find('Q');
        assert!(
            q.is_some_and(|q| q > text.len() / 2),
            "Up should land on the last row, got {merged}"
        );
    }

    #[test]
    fn up_from_the_first_block_goes_to_the_title_and_down_comes_back() {
        let mut h = Harness::new(
            "aaaaa",
            &[
                (BlockKind::Paragraph, "aaaaaaaaaa"),
                (BlockKind::Paragraph, "x"),
            ],
        );
        h.caret_in(0, 0);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Title);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(0)));
    }

    #[test]
    fn arrows_skip_dividers() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "one"),
                (BlockKind::Divider, ""),
                (BlockKind::Paragraph, "two"),
            ],
        );
        h.caret_in(0, 3);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(2)));
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(0)));
    }

    #[test]
    fn left_and_right_cross_block_edges() {
        let mut h = Harness::new(
            "title",
            &[
                (BlockKind::Paragraph, "alpha"),
                (BlockKind::Paragraph, "bravo"),
            ],
        );
        h.caret_in(0, 5);
        h.key(Key::ArrowRight, Modifiers::NONE);
        h.type_text("Q");
        assert_eq!(h.texts(), ["alpha", "Qbravo"]);
        h.caret_in(1, 0);
        h.key(Key::ArrowLeft, Modifiers::COMMAND);
        h.type_text("R");
        assert_eq!(h.texts(), ["alphaR", "Qbravo"]);
        h.caret_in(0, 0);
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.type_text("S");
        assert_eq!(h.app.page.meta.title, "titleS");
    }

    #[test]
    fn enter_in_the_middle_of_the_title_splits_it() {
        let mut h = Harness::new("titulo", &[(BlockKind::Paragraph, "alpha")]);
        h.app.focus_title(3);
        h.idle();
        h.key(Key::Enter, Modifiers::NONE);
        assert_eq!(h.app.page.meta.title, "tit");
        assert_eq!(h.texts(), ["ulo", "alpha"]);
        h.type_text("Q");
        assert_eq!(h.texts(), ["Qulo", "alpha"]);
    }

    #[test]
    fn escape_selects_the_block_and_arrows_move_the_selection() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "a"),
                (BlockKind::Paragraph, "b"),
                (BlockKind::Paragraph, "c"),
            ],
        );
        h.caret_in(1, 1);
        h.key(Key::Escape, Modifiers::NONE);
        assert_eq!(h.app.block_selection, Some((h.id(1), h.id(1))));
        assert_eq!(h.app.focus, Focus::None);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.block_selection, Some((h.id(2), h.id(2))));
        h.key(Key::Escape, Modifiers::NONE);
        assert_eq!(h.app.block_selection, None);
        h.type_text("Q");
        assert_eq!(h.texts(), ["a", "b", "Qc"]);
    }

    #[test]
    fn escape_restores_anchor_and_collapses_text_selection() {
        let mut h = Harness::new(
            "title",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
                (BlockKind::Paragraph, "charlie three"),
            ],
        );
        h.click_in(1, 4);
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        assert_eq!(h.app.block_selection, Some((h.id(1), h.id(1))));
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        h.key(Key::Escape, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(1)));
        assert_eq!(h.app.cursor_range_for(&h.ctx, h.id(1)), Some((0, 0)));
        assert!(
            h.ctx
                .memory(|memory| memory.has_focus(block_te_id(h.id(1))))
        );
        h.type_text("Q");
        assert_eq!(h.texts(), ["alpha one", "Qbravo two", "charlie three"]);
    }

    #[test]
    fn first_shift_arrow_selects_and_deletes_only_the_current_block() {
        for key in [Key::ArrowUp, Key::ArrowDown] {
            let mut h = Harness::new(
                "title",
                &[
                    (BlockKind::Paragraph, "alpha one"),
                    (BlockKind::Paragraph, "bravo two"),
                    (BlockKind::Paragraph, "charlie three"),
                ],
            );
            h.click_in(1, 4);
            h.key(key, Modifiers::SHIFT);
            h.key(Key::Backspace, Modifiers::NONE);
            assert_eq!(h.texts(), ["alpha one", "charlie three"]);
        }
    }

    #[test]
    fn first_row_up_reaches_title_end_and_down_reaches_block_start() {
        let mut h = Harness::new("title", &[(BlockKind::Paragraph, "alpha one")]);
        h.click_in(0, 5);
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.type_text("Q");
        assert_eq!(h.app.page.meta.title, "titleQ");
        h.key(Key::End, Modifiers::NONE);
        h.key(Key::ArrowDown, Modifiers::NONE);
        h.type_text("R");
        assert_eq!(h.texts(), ["Ralpha one"]);
    }

    #[test]
    fn page_keys_move_focus_and_preserve_column() {
        let blocks = vec![(BlockKind::Paragraph, "alpha one"); 60];
        let mut h = Harness::new("title", &blocks);
        h.caret_in(0, 3);
        h.key(Key::PageDown, Modifiers::NONE);
        let Focus::Block(target) = h.app.focus else {
            panic!("missing block focus")
        };
        assert_ne!(target, h.id(0));
        h.type_text("Q");
        assert_eq!(
            h.app.page.blocks[h.app.index_of(target).unwrap()].text,
            "alpQha one"
        );
        h.key(Key::PageUp, Modifiers::NONE);
        assert_eq!(h.app.focus, Focus::Block(h.id(0)));
    }

    #[test]
    fn copy_and_cut_selected_blocks_as_markdown() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "a"),
                (BlockKind::Bullet, "b"),
                (BlockKind::Paragraph, "c"),
            ],
        );
        h.caret_in(0, 1);
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        h.event(Event::Copy);
        let expected = markdown::to_markdown(&h.app.page.blocks[0..2]);
        assert!(expected.contains("- b"));
        assert_eq!(h.copied.last(), Some(&expected));
        h.event(Event::Cut);
        assert_eq!(h.texts(), ["c"]);
    }

    #[test]
    fn copied_paragraphs_paste_back_as_separate_blocks() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
                (BlockKind::Paragraph, "delta"),
            ],
        );
        h.caret_in(0, 1);
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        h.key(Key::ArrowDown, Modifiers::SHIFT);
        h.event(Event::Copy);
        let copied = h.copied.last().cloned().unwrap_or_default();
        let drafts = markdown::parse_blocks(&copied, true);
        assert_eq!(drafts.len(), 2, "copied {copied:?}");
    }

    #[test]
    fn ctrl_shift_arrows_move_the_block_without_touching_text() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
            ],
        );
        h.caret_in(1, 5);
        h.key(Key::ArrowUp, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(h.texts(), ["bravo two", "alpha one"]);
        h.type_text("Q");
        assert_eq!(h.texts(), ["bravoQ two", "alpha one"]);
        h.key(Key::ArrowDown, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(h.texts(), ["alpha one", "bravoQ two"]);
    }

    #[test]
    fn ctrl_a_twice_selects_every_block() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha"),
                (BlockKind::Paragraph, "bravo"),
            ],
        );
        h.caret_in(1, 2);
        h.key(Key::A, Modifiers::COMMAND);
        assert_eq!(h.app.block_selection, None);
        h.key(Key::A, Modifiers::COMMAND);
        assert_eq!(h.app.block_selection, Some((h.id(0), h.id(1))));
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.texts(), [""]);
    }

    #[test]
    fn delete_at_the_end_leaves_the_caret_at_the_seam() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha"),
                (BlockKind::Paragraph, "bravo"),
            ],
        );
        h.caret_in(0, 5);
        h.key(Key::Delete, Modifiers::NONE);
        h.type_text("Q");
        assert_eq!(h.texts(), ["alphaQbravo"]);
    }

    #[test]
    fn ctrl_d_duplicates_the_block() {
        let mut h = Harness::new("t", &[(BlockKind::Bullet, "alpha")]);
        h.caret_in(0, 2);
        h.key(Key::D, Modifiers::COMMAND);
        assert_eq!(h.texts(), ["alpha", "alpha"]);
        assert_eq!(h.app.page.blocks[1].kind, BlockKind::Bullet);
        assert_eq!(h.app.focus, Focus::Block(h.id(1)));
    }

    #[test]
    fn left_margin_click_focuses_block_at_offset_zero() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
            ],
        );
        let id1 = h.id(1);
        let row_rect = h.app.block_rects[&id1];
        let y = row_rect.center().y;

        // Click in the gutter/margin between column_left and TextEdit.
        h.click(pos2(row_rect.left() + 10.0, y));
        assert_eq!(h.app.focus, Focus::Block(id1));
        h.type_text("Q");
        assert_eq!(h.texts(), ["alpha one", "Qbravo two"]);

        // Click in the outer left margin (to the left of the hover handles).
        h.caret_in(0, 2);
        h.click(pos2(10.0, y));
        assert_eq!(h.app.focus, Focus::Block(id1));
        h.type_text("R");
        assert_eq!(h.texts(), ["alpha one", "RQbravo two"]);
    }

    #[test]
    fn clicking_inside_text_keeps_pointer_caret_position() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
            ],
        );
        let id1 = h.id(1);
        let (galley, galley_pos) = h.app.block_galleys[&id1].clone();
        let x = galley_pos.x + caret_x(&galley, CCursor::new(5));
        let y = galley_pos.y + galley.rows[0].rect().center().y;
        h.click(pos2(x, y));
        assert_eq!(h.app.focus, Focus::Block(id1));
        h.type_text("Q");
        assert_eq!(h.texts(), ["alpha one", "bravoQ two"]);
    }

    #[test]
    fn clicking_hover_handles_does_not_focus_block_or_move_caret() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
            ],
        );
        h.caret_in(0, 3);
        let id1 = h.id(1);
        let row_rect = h.app.block_rects[&id1];

        // Click on the grip handle (column_left - 46..26, top + 1..21).
        let grip_pos = pos2(row_rect.left() - 36.0, row_rect.top() + 10.0);
        let actions = h.click(grip_pos);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::OpenBlockMenu(id, _) if *id == id1)),
            "expected OpenBlockMenu for block {id1}, got {actions:?}"
        );
        assert_eq!(h.app.focus, Focus::Block(h.id(0)));

        // Click on the '+' handle (column_left - 26..6, top + 1..21).
        let plus_pos = pos2(row_rect.left() - 16.0, row_rect.top() + 10.0);
        let actions = h.click(plus_pos);
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::InsertBelow(id) if *id == id1)),
            "expected InsertBelow for block {id1}, got {actions:?}"
        );
        assert_eq!(h.app.focus, Focus::Block(h.id(0)));
    }

    #[test]
    fn left_margin_click_clears_block_selection() {
        let mut h = Harness::new(
            "t",
            &[
                (BlockKind::Paragraph, "alpha one"),
                (BlockKind::Paragraph, "bravo two"),
            ],
        );
        h.caret_in(0, 1);
        h.key(Key::Escape, Modifiers::NONE);
        assert_eq!(h.app.block_selection, Some((h.id(0), h.id(0))));

        let id1 = h.id(1);
        let row_rect = h.app.block_rects[&id1];
        h.click(pos2(row_rect.left() + 10.0, row_rect.center().y));
        assert_eq!(h.app.block_selection, None);
        assert_eq!(h.app.focus, Focus::Block(id1));
        h.type_text("Q");
        assert_eq!(h.texts(), ["alpha one", "Qbravo two"]);
    }
}
