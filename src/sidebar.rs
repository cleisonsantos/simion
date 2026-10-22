//! The left sidebar: search, favorites and the page tree.

use std::collections::{HashMap, HashSet};

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontFamily, FontId, Layout, Painter, Rect, Response,
    RichText, ScrollArea, Sense, Ui, Vec2, pos2, vec2,
};

use crate::app::{Action, App};
use crate::icons;
use crate::model::PageId;
use crate::theme::{FONT_BODY, FONT_MEDIUM, FONT_SEMIBOLD, Palette, family};
use crate::ui as widgets;

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

        ui.take_available_width();
        ui.set_min_width(ui.available_width());

        // Header.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            icons::logo(ui.painter(), rect, p.accent);
            ui.label(
                RichText::new("Simion")
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

        ui.add_space(8.0);

        let favorites: Vec<_> = pages.iter().filter(|page| page.favorite).collect();
        if !favorites.is_empty() {
            widgets::section_label(ui, "Favoritos", &p);
            for meta in favorites {
                page_row(ui, meta, &p, current, 0, false, false, actions);
            }
            ui.add_space(8.0);
        }

        widgets::section_label(ui, "Páginas", &p);

        let avail = ui.available_height();
        let footer_space = 78.0;
        let list_height = (avail - footer_space).max(80.0);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(list_height)
            .id_salt("simion-sidebar-tree")
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
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
                    actions,
                    &mut budget,
                );
                ui.add_space(4.0);
            });

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(2.0);
            let version = ui.label(
                RichText::new("Simion 0.1 · local")
                    .size(10.5)
                    .color(p.faint),
            );
            version.on_hover_text(
                "Banco de dados local em SQLite.\nSuas anotações nunca saem deste computador.",
            );

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
    actions: &mut Vec<Action>,
) {
    let active = current == Some(meta.id);
    let height = 28.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let hovered = response.hovered();

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

    if response.clicked() {
        actions.push(Action::SelectPage(meta.id));
    }
    if hovered {
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
