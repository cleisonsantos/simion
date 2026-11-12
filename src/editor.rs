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

        let full_height = ui.available_height();
        let _ = full_height;

        scroll.show_viewport(ui, |ui, viewport| {
            self.editor_content(ui, viewport, actions);
        });
    }

    fn editor_content(&mut self, ui: &mut Ui, viewport: Rect, actions: &mut Vec<Action>) {
        let full_width = ui.available_width();
        let inner_width = (full_width - 64.0).clamp(280.0, MAX_CONTENT_WIDTH);
        let left = ((full_width - inner_width) / 2.0).max(22.0);

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(left);
            ui.vertical(|ui| {
                ui.set_width(inner_width);
                ui.add_space(22.0 * self.zoom);
                self.page_icon(ui, actions);
                self.page_title(ui, actions);
                ui.add_space(6.0);
                self.blocks(ui, viewport, actions);
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

    fn blocks(&mut self, ui: &mut Ui, viewport: Rect, actions: &mut Vec<Action>) {
        let text_width = (ui.available_width()).max(120.0);
        let column_left = ui.cursor().left();
        ui.spacing_mut().item_spacing.y = 0.0;
        self.hover_block = None;

        // Any click in the editor drops the block selection.
        if self.block_selection.is_some() && ui.input(|input| input.pointer.any_pressed()) {
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
            let outcome = self.block_row(ui, index, text_width, column_left, actions);
            if self.block_selected(index) {
                let rect = Rect::from_min_max(
                    pos2(column_left, outcome.rect.top()),
                    pos2(column_left + text_width, outcome.rect.bottom()),
                );
                ui.painter().set(
                    selection_shape,
                    Shape::rect_filled(rect, 4, self.palette.hover),
                );
            }
            if outcome.changed {
                any_changed = true;
                let caret = outcome.cursor.map(|(a, b)| a.max(b));
                self.after_text_change(index, caret, actions);
            }
            if outcome.focused {
                self.focus = Focus::Block(id);
            }
            if let Some(range) = outcome.cursor {
                self.cursors.insert(id, range);
            }
            let measured = outcome.rect.height().max(8.0);
            self.block_heights.insert(id, measured);
            self.block_rows.insert(id, outcome.rows);
            self.block_rects.insert(id, outcome.rect);
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
            let id = self.ids.next();
            self.begin_change();
            self.page
                .blocks
                .push(crate::model::Block::new(id, BlockKind::Paragraph));
            self.end_change();
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
        let row_start = ui.cursor().min;
        let mut new_text = text.clone();
        let mut changed = false;
        let mut focused = false;
        let mut cursor_range = None;
        let mut te_rect = Rect::NOTHING;
        let mut marker_rect = Rect::NOTHING;
        let mut te_id = Id::unique(("simion-block", id));
        let mut rows = 1usize;
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
                // Tab indents the block (see `block_keys`); without the lock
                // egui would move keyboard focus to the next widget.
                .lock_focus(true);

            let te = if let Some(hint) = hint {
                te.hint_text(RichText::new(hint).font(hint_font.clone()).color(p.faint))
            } else {
                te
            };

            let output = te.layouter(&mut layouter).show(ui);
            te_rect = output.response.rect;
            rows = output.galley.rows.len().max(1);
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
                    let local = pointer - output.galley_pos;
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

        self.hover_handles(ui, id, column_left, row_rect, actions);

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
        if focused {
            self.focus = Focus::Block(id);
        }

        BlockOutcome {
            changed,
            focused,
            cursor: cursor_range,
            rect: row_rect,
            rows,
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
        let (rect, _response) =
            ui.allocate_exact_size(vec2(text_width, 24.0 * zoom), Sense::click());
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
            cursor: None,
            rect: row_rect,
            rows: 1,
        }
    }

    fn hover_handles(
        &mut self,
        ui: &Ui,
        id: u64,
        column_left: f32,
        row_rect: Rect,
        actions: &mut Vec<Action>,
    ) {
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
            return;
        }
        let zone = Rect::from_min_max(
            pos2(column_left - 46.0, row_rect.top() - 3.0),
            pos2(row_rect.right() + 6.0, row_rect.bottom() + 3.0),
        );
        if !ui.rect_contains_pointer(zone) {
            return;
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

        if plus.clicked() {
            actions.push(Action::InsertBelow(id));
        }
        if plus.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if grip.clicked() {
            actions.push(Action::OpenBlockMenu(id, grip_rect.left_bottom()));
        }
        if grip.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
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
        if self.block_selection.is_some() && self.selection_keys(ctx) {
            return;
        }
        if self.consume_pub(ctx, Key::Escape, Modifiers::NONE) {
            if let Focus::Block(id) = self.focus {
                ctx.memory_mut(|memory| memory.surrender_focus(Id::unique(("simion-block", id))));
            }
            self.focus = Focus::None;
            return;
        }

        match self.focus {
            Focus::Title => {
                if self.consume_pub(ctx, Key::Enter, Modifiers::NONE) {
                    if self.page.blocks.is_empty() {
                        self.begin_change();
                        self.page.blocks.push(crate::model::Block::new(
                            self.ids.next(),
                            BlockKind::Paragraph,
                        ));
                        self.end_change();
                    }
                    let id = self.page.blocks[0].id;
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(id),
                        range: Some((0, 0)),
                        select_all: false,
                    });
                    self.scroll_to = Some(id);
                }
            }
            Focus::Block(id) => self.block_keys(ctx, id),
            Focus::None => {}
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
        // Shift+↑/↓ selects this block when no text is selected, or when the
        // text selection already reaches the start / end of the block. The
        // range comes from the text field's own state, not `self.cursors`.
        let text_selection = self.cursor_range_for(ctx, id).filter(|(a, b)| a != b);
        if text_selection.is_none_or(|(start, _)| start == 0)
            && self.consume_pub(ctx, Key::ArrowUp, Modifiers::SHIFT)
        {
            self.start_block_selection(ctx, id);
            return;
        }
        if text_selection.is_none_or(|(_, end)| end >= text_len)
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

        if !has_selection && caret == 0 && self.consume_pub(ctx, Key::ArrowUp, Modifiers::NONE) {
            if index > 0 {
                let previous = self.page.blocks[index - 1].id;
                let length = self.page.blocks[index - 1].text.chars().count();
                self.pending = Some(PendingFocus {
                    target: Focus::Block(previous),
                    range: Some((length, length)),
                    select_all: false,
                });
                self.scroll_to = Some(previous);
            }
            return;
        }
        if !has_selection
            && caret == text_len
            && self.consume_pub(ctx, Key::ArrowDown, Modifiers::NONE)
        {
            if index + 1 < self.page.blocks.len() {
                let next = self.page.blocks[index + 1].id;
                self.pending = Some(PendingFocus {
                    target: Focus::Block(next),
                    range: Some((0, 0)),
                    select_all: false,
                });
                self.scroll_to = Some(next);
            }
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
        let caret = self.page.blocks[index].text.chars().count();
        self.pending = Some(PendingFocus {
            target: Focus::Block(id),
            range: Some((caret, caret)),
            select_all: false,
        });
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

    fn start_block_selection(&mut self, ctx: &egui::Context, id: u64) {
        // The text field lets go of the keyboard so typing does not edit a
        // block while several are selected.
        ctx.memory_mut(|memory| memory.surrender_focus(Id::unique(("simion-block", id))));
        self.focus = Focus::None;
        self.pending = None;
        self.slash = None;
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
        if self.consume_pub(ctx, Key::Escape, Modifiers::NONE) {
            self.block_selection = None;
            return true;
        }
        if self.consume_pub(ctx, Key::Backspace, Modifiers::NONE)
            || self.consume_pub(ctx, Key::Delete, Modifiers::NONE)
        {
            self.delete_block_selection();
            return true;
        }
        for (key, delta) in [(Key::ArrowUp, -1i32), (Key::ArrowDown, 1)] {
            if self.consume_pub(ctx, key, Modifiers::SHIFT) {
                let next = (extent_index as i32 + delta).clamp(0, self.page.blocks.len() as i32 - 1)
                    as usize;
                let next_id = self.page.blocks[next].id;
                self.block_selection = Some((anchor, next_id));
                self.scroll_to = Some(next_id);
                return true;
            }
            if self.consume_pub(ctx, key, Modifiers::NONE) {
                // Plain arrows leave the selection and put the caret back.
                self.block_selection = None;
                if self.page.blocks[extent_index].kind.accepts_text() {
                    let caret = if delta < 0 {
                        0
                    } else {
                        self.page.blocks[extent_index].text.chars().count()
                    };
                    self.focus = Focus::Block(extent);
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(extent),
                        range: Some((caret, caret)),
                        select_all: false,
                    });
                }
                return true;
            }
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

pub struct BlockOutcome {
    pub changed: bool,
    pub focused: bool,
    pub cursor: Option<(usize, usize)>,
    pub rect: Rect,
    pub rows: usize,
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
