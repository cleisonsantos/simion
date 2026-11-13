//! The left sidebar: search, favorites and the page tree.

use std::collections::{HashMap, HashSet};

use eframe::egui::{
    self, Align, Color32, Context, CursorIcon, FontFamily, FontId, Key, Layout, Modifiers, Order,
    Painter, Pos2, Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2,
    vec2,
};

use crate::app::{Action, App};
use crate::icons;
use crate::model::PageId;
use crate::theme::{FONT_BODY, FONT_MEDIUM, FONT_SEMIBOLD, Palette, family};
use crate::ui as widgets;

#[derive(Clone, Copy)]
struct PageRowHit {
    id: PageId,
    rect: Rect,
    has_children: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DropZone {
    Before,
    Inside,
    After,
}

#[derive(Clone, Copy)]
struct DropTarget {
    id: PageId,
    zone: DropZone,
}

impl App {
    pub fn sidebar(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.palette;
        let current = self.current;
        let pages = self.pages.clone();
        let collapsed = self.collapsed.clone();
        let children = self.children_map();

        let mut by_id: HashMap<PageId, crate::model::PageMeta> = HashMap::new();
        for page in &pages {
            by_id.insert(page.id, page.clone());
        }

        let mut rows = Vec::new();
        let mut drag_started = None;
        let scroll_target = self.sidebar_scroll_to;

        ui.take_available_width();
        ui.set_min_width(ui.available_width());

        // Header.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            icons::logo(ui.painter(), rect);
            ui.add_space(8.0);
            ui.label(
                RichText::new("S")
                    .font(FontId::new(15.0, family(FONT_SEMIBOLD)))
                    .color(icons::BRAND),
            );
            ui.label(
                RichText::new("imion")
                    .font(FontId::new(15.0, family(FONT_SEMIBOLD)))
                    .color(p.text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let collapse = widgets::icon_button(
                    ui,
                    26.0,
                    p.faint,
                    "Ocultar barra lateral  Ctrl+\\",
                    |painter, rect, color| icons::panel_left(painter, rect, color, 1.5),
                );
                if collapse.clicked() {
                    actions.push(Action::ToggleSidebar);
                }
            });
        });

        ui.add_space(10.0);

        // Search.
        let search = action_row(
            ui,
            "Buscar",
            Some("Ctrl K"),
            None,
            &p,
            |painter, rect, color| {
                icons::search(painter, rect, color, 1.5);
            },
        );
        if search.clicked() {
            actions.push(Action::OpenSearch);
        }

        // New page.
        let new_page = action_row(
            ui,
            "Nova página",
            None,
            None,
            &p,
            |painter, rect, color| {
                icons::plus(painter, rect, color, 1.5);
            },
        );
        if new_page.clicked() {
            actions.push(Action::NewPage { parent: None });
        }

        // New folder.
        let new_folder = action_row(ui, "Nova pasta", None, None, &p, |painter, rect, color| {
            icons::folder(painter, rect, color, 1.5);
        });
        if new_folder.clicked() {
            actions.push(Action::NewFolder { parent: None });
        }

        ui.add_space(8.0);

        let favorites: Vec<_> = pages.iter().filter(|page| page.favorite).collect();
        if !favorites.is_empty() {
            widgets::section_label(ui, "Favoritos", &p);
            for meta in favorites {
                page_row(
                    ui,
                    meta,
                    &p,
                    current,
                    0,
                    false,
                    false,
                    &mut rows,
                    &mut drag_started,
                    actions,
                );
            }
            ui.add_space(8.0);
        }

        widgets::section_label(ui, "Páginas", &p);

        let avail = ui.available_height();
        let footer_space = 130.0;
        let list_height = (avail - footer_space).max(80.0);
        let dragging = self.drag_page.is_some();
        let scroll = ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(list_height)
            .id_salt("simion-sidebar-tree")
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if dragging {
                    if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                        let viewport = ui.clip_rect();
                        if viewport.contains(pos) {
                            if pos.y < viewport.top() + 24.0 {
                                ui.scroll_with_delta(vec2(0.0, -10.0));
                            } else if pos.y > viewport.bottom() - 24.0 {
                                ui.scroll_with_delta(vec2(0.0, 10.0));
                            }
                        }
                    }
                }
                if pages.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Nenhuma página ainda")
                            .size(12.5)
                            .color(p.faint),
                    );
                    return;
                }
                let mut budget = 500;
                render_tree(
                    ui,
                    &children,
                    &by_id,
                    &collapsed,
                    current,
                    None,
                    0,
                    &p,
                    &mut rows,
                    &mut drag_started,
                    actions,
                    &mut budget,
                );
                if let Some(target) = scroll_target {
                    if let Some(row) = rows.iter().find(|row| row.id == target) {
                        ui.scroll_to_rect(row.rect, Some(Align::Center));
                    }
                }
                ui.add_space(4.0);
            });
        let list_rect = scroll.inner_rect;

        if scroll_target.is_some() {
            self.sidebar_scroll_to = None;
        }

        if let Some(id) = drag_started {
            self.drag_page = Some(id);
        }

        let pointer = ui.ctx().input(|i| i.pointer.interact_pos());
        let released = ui.ctx().input(|i| i.pointer.any_released());
        let escaped = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));

        if let Some(id) = self.drag_page {
            let hit = pointer.and_then(|pos| row_under_pointer(&rows, pos));

            if !released {
                if let Some((row, target)) = hit {
                    draw_drop_indicator(ui.painter(), &row, target.zone, &p);
                }
                if let Some(pos) = pointer {
                    if let Some(meta) = by_id.get(&id) {
                        draw_drag_ghost(ui, pos, meta.display_title(), &p);
                    }
                }
            }

            if released {
                if let Some((row, target)) = hit {
                    if target.id != id {
                        if let Some((parent, index)) =
                            drop_position(id, &row, target.zone, &by_id, &children)
                        {
                            let old_parent = by_id.get(&id).and_then(|meta| meta.parent);
                            let old_index = sibling_index(&children, old_parent, id);
                            if old_parent != parent || old_index != Some(index) {
                                actions.push(Action::MovePage { id, parent, index });
                            }
                        }
                    }
                } else if let Some(pos) = pointer {
                    let below_last = rows.last().map_or(true, |row| pos.y >= row.rect.bottom());
                    if list_rect.contains(pos) && below_last {
                        let new_parent: Option<PageId> = None;
                        let mut roots = children.get(&new_parent).cloned().unwrap_or_default();
                        roots.retain(|sibling| *sibling != id);
                        let index = roots.len();
                        let old_parent = by_id.get(&id).and_then(|meta| meta.parent);
                        let old_index = sibling_index(&children, old_parent, id);
                        if old_parent != new_parent || old_index != Some(index) {
                            actions.push(Action::MovePage {
                                id,
                                parent: new_parent,
                                index,
                            });
                        }
                    }
                }
            }

            if released || escaped {
                self.drag_page = None;
            }
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(2.0);
            let db_path = self
                .store
                .as_ref()
                .map(|store| store.path().display().to_string())
                .unwrap_or_default();
            let version = ui.add(
                egui::Label::new(
                    RichText::new(format!("Simion {} · local", env!("CARGO_PKG_VERSION")))
                        .size(10.5)
                        .color(p.faint),
                )
                .sense(Sense::click()),
            );
            if version.clicked() {
                actions.push(Action::OpenSettings);
            }
            version.on_hover_text(format!(
                "Banco de dados local em SQLite:\n{db_path}\nSuas anotações nunca saem deste computador."
            ));

            let check = ui.small_button(
                RichText::new("Verificar atualizações")
                    .size(10.5)
                    .color(p.faint),
            );
            if check.clicked() {
                actions.push(Action::CheckUpdates);
            }

            ui.add_space(2.0);
            let shortcuts = action_row(
                ui,
                "Atalhos",
                Some("Ctrl /"),
                None,
                &p,
                |painter, rect, color| icons::keyboard(painter, rect, color, 1.3),
            );
            if shortcuts.clicked() {
                actions.push(Action::OpenShortcuts);
            }

            let settings = action_row(
                ui,
                "Configurações",
                Some("Ctrl ,"),
                None,
                &p,
                |painter, rect, color| icons::gear(painter, rect, color, 1.3),
            );
            if settings.clicked() {
                actions.push(Action::OpenSettings);
            }

            let theme_label = if self.mode == crate::theme::Mode::Light {
                "Tema escuro"
            } else {
                "Tema claro"
            };
            let theme = action_row(
                ui,
                theme_label,
                None,
                None,
                &p,
                move |painter, rect, color| {
                    if self.mode == crate::theme::Mode::Light {
                        icons::moon(painter, rect, color, 1.4);
                    } else {
                        icons::sun(painter, rect, color, 1.4);
                    }
                },
            );
            if theme.clicked() {
                actions.push(Action::ToggleTheme);
            }
        });
    }

    /// Gestos de pagina da arvore. O gate fica em
    /// [`App::sidebar_hotkeys_allowed`]: ponteiro sobre a sidebar ou nenhum
    /// campo de texto com o teclado (blocos com caret sempre vencem). Consome
    /// a tecla mesmo quando o gesto e no-op (borda da arvore), para o evento
    /// nao vazar para o editor.
    pub fn sidebar_keys(&mut self, ctx: &Context) -> bool {
        let Some(id) = self.current else {
            return false;
        };
        if self.consume_key(ctx, Key::ArrowUp, Modifiers::ALT | Modifiers::SHIFT) {
            self.apply_page_gesture(id, |store| store.indent_page(id));
            return true;
        }
        if self.consume_key(ctx, Key::ArrowDown, Modifiers::ALT | Modifiers::SHIFT) {
            self.apply_page_gesture(id, |store| store.outdent_page(id));
            return true;
        }
        if self.consume_key(ctx, Key::ArrowUp, Modifiers::ALT) {
            self.apply_page_gesture(id, |store| store.move_page_up(id));
            return true;
        }
        if self.consume_key(ctx, Key::ArrowDown, Modifiers::ALT) {
            self.apply_page_gesture(id, |store| store.move_page_down(id));
            return true;
        }
        false
    }
}

fn action_row(
    ui: &mut Ui,
    label: &str,
    hint: Option<&str>,
    trailing: Option<&str>,
    p: &Palette,
    icon: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let height = 30.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let fill = if response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 6, fill);
    }
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(16.0));
    icon(ui.painter(), icon_rect, p.muted);
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::new(13.5, family(FONT_MEDIUM)),
        p.text,
    );
    if let Some(hint) = hint {
        ui.painter().text(
            pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            hint,
            FontId::new(10.5, family(FONT_BODY)),
            p.faint,
        );
    }
    if let Some(trailing) = trailing {
        ui.painter().text(
            pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            trailing,
            FontId::new(10.5, family(FONT_BODY)),
            p.faint,
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

#[allow(clippy::too_many_arguments)]
fn render_tree(
    ui: &mut Ui,
    children: &HashMap<Option<PageId>, Vec<PageId>>,
    by_id: &HashMap<PageId, crate::model::PageMeta>,
    collapsed: &HashSet<PageId>,
    current: Option<PageId>,
    parent: Option<PageId>,
    depth: usize,
    p: &Palette,
    rows: &mut Vec<PageRowHit>,
    drag_started: &mut Option<PageId>,
    actions: &mut Vec<Action>,
    budget: &mut usize,
) {
    if depth > 12 || *budget == 0 {
        return;
    }
    let Some(ids) = children.get(&parent) else {
        return;
    };
    for id in ids {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let Some(meta) = by_id.get(id) else {
            continue;
        };
        let has_children = children.contains_key(&Some(*id));
        let is_collapsed = collapsed.contains(id);
        page_row(
            ui,
            meta,
            p,
            current,
            depth,
            has_children,
            is_collapsed,
            rows,
            drag_started,
            actions,
        );
        if has_children && !is_collapsed {
            render_tree(
                ui,
                children,
                by_id,
                collapsed,
                current,
                Some(*id),
                depth + 1,
                p,
                rows,
                drag_started,
                actions,
                budget,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn page_row(
    ui: &mut Ui,
    meta: &crate::model::PageMeta,
    p: &Palette,
    current: Option<PageId>,
    depth: usize,
    has_children: bool,
    is_collapsed: bool,
    rows: &mut Vec<PageRowHit>,
    drag_started: &mut Option<PageId>,
    actions: &mut Vec<Action>,
) {
    let active = current == Some(meta.id);
    let height = 28.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click_and_drag());
    let hovered = response.hovered();

    rows.push(PageRowHit {
        id: meta.id,
        rect,
        has_children,
    });
    if response.drag_started() {
        *drag_started = Some(meta.id);
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }

    let fill = if active {
        p.active
    } else if hovered {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 5, fill);
    }

    let mut x = 4.0 + depth as f32 * 14.0;
    let chevron_rect = Rect::from_center_size(
        pos2(rect.left() + x + 8.0, rect.center().y),
        Vec2::splat(16.0),
    );
    if has_children {
        if is_collapsed {
            icons::chevron_right(ui.painter(), chevron_rect, p.faint, 1.4);
        } else {
            icons::chevron_down(ui.painter(), chevron_rect, p.faint, 1.4);
        }
    }
    x += 18.0;

    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + x + 8.0, rect.center().y),
        Vec2::splat(16.0),
    );
    if meta.icon.trim().is_empty() {
        icons::file(ui.painter(), icon_rect, p.faint, 1.2);
    } else {
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            &meta.icon,
            FontId::new(14.0, FontFamily::Proportional),
            p.text,
        );
    }
    x += 22.0;

    let title = meta.display_title();
    let text_rect = Rect::from_min_max(
        pos2(rect.left() + x, rect.top()),
        pos2(rect.right() - 26.0, rect.bottom()),
    );
    let color = if active { p.text } else { p.text };
    let galley = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            title.to_owned(),
            FontId::new(13.5, family(FONT_BODY)),
            color,
        )
    });
    let clipped = ui.painter().with_clip_rect(text_rect);
    clipped.galley(
        pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );

    if meta.favorite {
        icons::star(
            ui.painter(),
            Rect::from_center_size(
                pos2(rect.right() - 14.0, rect.center().y),
                Vec2::splat(13.0),
            ),
            p.accent,
            true,
            1.0,
        );
    }

    let chevron = ui.interact(
        chevron_rect,
        ui.make_persistent_id(("page-chevron", meta.id)),
        Sense::click(),
    );
    if chevron.hovered() {
        ui.painter()
            .rect_filled(chevron_rect.shrink(1.0), 4, p.hover);
    }
    if chevron.clicked() {
        actions.push(Action::ToggleCollapsed(meta.id));
        return;
    }

    if response.clicked() && !response.dragged() && !response.drag_stopped() {
        actions.push(Action::SelectPage(meta.id));
    }
    if response.dragged() || response.drag_stopped() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let id = meta.id;
    let favorite = meta.favorite;
    let title = meta.display_title().to_owned();
    response.context_menu(|ui| {
        ui.set_min_width(200.0);
        if ui.button("Nova subpágina").clicked() {
            actions.push(Action::NewPage { parent: Some(id) });
        }
        if ui.button("Nova pasta").clicked() {
            actions.push(Action::NewFolder { parent: Some(id) });
        }
        if ui.button("Renomear").clicked() {
            actions.push(Action::SelectPage(id));
            actions.push(Action::FocusTitle);
        }
        if ui
            .button(if favorite {
                "Remover dos favoritos"
            } else {
                "Adicionar aos favoritos"
            })
            .clicked()
        {
            actions.push(Action::ToggleFavorite(id));
        }
        ui.separator();
        if ui.button("Duplicar").clicked() {
            actions.push(Action::DuplicatePage(id));
        }
        if ui.button("Exportar .md").clicked() {
            actions.push(Action::ExportMarkdown(id));
        }
        if ui.button("Copiar Markdown").clicked() {
            actions.push(Action::CopyMarkdown(id));
        }
        ui.separator();
        if ui
            .button(egui::RichText::new("Excluir").color(p.danger))
            .clicked()
        {
            actions.push(Action::AskDelete(id));
        }
        let _ = &title;
    });
}

fn row_under_pointer(rows: &[PageRowHit], pos: Pos2) -> Option<(PageRowHit, DropTarget)> {
    rows.iter().rev().find_map(|row| {
        row.rect.expand(3.0).contains(pos).then(|| {
            let zone = if pos.y < row.rect.top() + row.rect.height() * 0.25 {
                DropZone::Before
            } else if pos.y > row.rect.bottom() - row.rect.height() * 0.25 {
                DropZone::After
            } else {
                DropZone::Inside
            };
            (*row, DropTarget { id: row.id, zone })
        })
    })
}

fn draw_drop_indicator(painter: &Painter, row: &PageRowHit, zone: DropZone, p: &Palette) {
    match zone {
        DropZone::Before => {
            let y = row.rect.top();
            painter.line_segment(
                [
                    pos2(row.rect.left() + 6.0, y),
                    pos2(row.rect.right() - 6.0, y),
                ],
                Stroke::new(2.0, p.accent),
            );
        }
        DropZone::After => {
            let y = row.rect.bottom();
            painter.line_segment(
                [
                    pos2(row.rect.left() + 6.0, y),
                    pos2(row.rect.right() - 6.0, y),
                ],
                Stroke::new(2.0, p.accent),
            );
        }
        DropZone::Inside => {
            let inset = if row.has_children { 2.0 } else { 1.0 };
            let rect = row.rect.shrink(inset);
            painter.rect_filled(rect, 5, p.accent_soft);
            painter.rect_stroke(rect, 5, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
    }
}

fn draw_drag_ghost(ui: &Ui, pos: Pos2, title: &str, p: &Palette) {
    let galley = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            title.to_owned(),
            FontId::new(13.0, family(FONT_MEDIUM)),
            p.text,
        )
    });
    let rect = Rect::from_min_size(pos + vec2(14.0, 12.0), galley.size() + vec2(16.0, 10.0));
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        Order::Tooltip,
        egui::Id::unique("simion-page-drag-ghost"),
    ));
    painter.rect_filled(rect.translate(vec2(0.0, 2.0)), 6, p.shadow);
    painter.rect_filled(rect, 6, p.elevated);
    painter.rect_stroke(rect, 6, Stroke::new(1.0, p.border), StrokeKind::Inside);
    painter.galley(rect.min + vec2(8.0, 5.0), galley, p.text);
}

fn drop_position(
    id: PageId,
    row: &PageRowHit,
    zone: DropZone,
    by_id: &HashMap<PageId, crate::model::PageMeta>,
    children: &HashMap<Option<PageId>, Vec<PageId>>,
) -> Option<(Option<PageId>, usize)> {
    let target = by_id.get(&row.id)?;
    match zone {
        DropZone::Inside => {
            let parent = Some(row.id);
            let mut siblings = children.get(&parent).cloned().unwrap_or_default();
            siblings.retain(|sibling| *sibling != id);
            Some((parent, siblings.len()))
        }
        DropZone::Before | DropZone::After => {
            let parent = target.parent;
            let mut siblings = children.get(&parent).cloned().unwrap_or_default();
            siblings.retain(|sibling| *sibling != id);
            let mut index = siblings.iter().position(|sibling| *sibling == row.id)?;
            if zone == DropZone::After {
                index += 1;
            }
            Some((parent, index))
        }
    }
}

fn sibling_index(
    children: &HashMap<Option<PageId>, Vec<PageId>>,
    parent: Option<PageId>,
    id: PageId,
) -> Option<usize> {
    children
        .get(&parent)?
        .iter()
        .position(|sibling| *sibling == id)
}
