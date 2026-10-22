//! Popups, menus, modals and transient UI.

use eframe::egui::{
    self, Align, Align2, Color32, Context, FontFamily, FontId, Id, Layout, Margin, Modal, Order,
    Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::app::{Action, App, Focus, PendingFocus};
use crate::markdown;
use crate::model::{Block, BlockKind};
use crate::theme::{FONT_BODY, FONT_MEDIUM, FONT_MONO, FONT_SEMIBOLD, GUTTER, Palette, family};
use crate::ui as widgets;

const EMOJIS: [&str; 48] = [
    "📄", "📝", "📚", "📌", "💡", "✅", "🚀", "⭐", "❤", "🎯", "🔥", "🌱", "🍀", "🌊", "☀", "🌙",
    "🎵", "🎨", "🏠", "💼", "📅", "🔍", "⚙", "🔒", "🎁", "🐱", "🐶", "🍎", "🌻", "🌈", "☕", "🏆",
    "📷", "✏", "📎", "🎓", "💬", "🛠", "🎉", "🔬", "🔖", "📊", "🗺", "⏰", "💎", "🐢", "🌸", "🍕",
];

impl App {
    pub fn overlays(&mut self, ctx: &Context) -> Vec<Action> {
        let mut actions = Vec::new();
        actions.extend(self.search_overlay(ctx));
        self.block_menu_overlay(ctx, &mut actions);
        self.page_menu_overlay(ctx, &mut actions);
        self.emoji_overlay(ctx, &mut actions);
        self.slash_overlay(ctx);
        self.shortcuts_overlay(ctx);
        self.markdown_overlay(ctx, &mut actions);
        self.delete_overlay(ctx, &mut actions);
        self.toast_overlay(ctx);
        self.drop_overlay(ctx);
        actions
    }

    // -----------------------------------------------------------------------
    // Slash commands
    // -----------------------------------------------------------------------

    pub fn slash_keys(&mut self, ctx: &Context) {
        let Some(slash) = self.slash.clone() else {
            return;
        };
        if self.consume_pub(ctx, egui::Key::Escape, egui::Modifiers::NONE) {
            self.slash = None;
            return;
        }
        let items = slash_items(&slash.query);
        if items.is_empty() {
            return;
        }
        if self.consume_pub(ctx, egui::Key::ArrowDown, egui::Modifiers::NONE) {
            if let Some(slash) = self.slash.as_mut() {
                slash.selected = (slash.selected + 1).min(items.len() - 1);
            }
            return;
        }
        if self.consume_pub(ctx, egui::Key::ArrowUp, egui::Modifiers::NONE) {
            if let Some(slash) = self.slash.as_mut() {
                slash.selected = slash.selected.saturating_sub(1);
            }
            return;
        }
        if self.consume_pub(ctx, egui::Key::Enter, egui::Modifiers::NONE) {
            let index = slash.selected.min(items.len() - 1);
            self.execute_slash(items[index]);
        }
    }

    pub(crate) fn execute_slash(&mut self, kind: BlockKind) {
        let Some(slash) = self.slash.take() else {
            return;
        };
        let Some(index) = self.index_of(slash.block) else {
            return;
        };
        let text = self.page.blocks[index].text.clone();
        let chars: Vec<char> = text.chars().collect();
        let start = slash.start.min(chars.len());
        let prefix: String = chars[..start].iter().collect();
        let prefix = prefix.trim_end().to_owned();

        self.begin_change();
        let id = {
            let block = &mut self.page.blocks[index];
            block.text = prefix;
            block.kind = kind;
            if kind == BlockKind::Todo {
                block.checked = false;
            }
            if kind == BlockKind::Callout && block.emoji.is_empty() {
                block.emoji = "💡".to_owned();
            }
            if !kind.is_list() {
                block.indent = 0;
            }
            if kind == BlockKind::Divider {
                block.text.clear();
            }
            block.id
        };
        self.end_change();

        if kind == BlockKind::Divider {
            let new_id = self.ids.next();
            self.begin_change();
            self.page
                .blocks
                .insert(index + 1, Block::new(new_id, BlockKind::Paragraph));
            self.end_change();
            self.focus = Focus::Block(new_id);
            self.pending = Some(PendingFocus {
                target: Focus::Block(new_id),
                range: Some((0, 0)),
                select_all: false,
            });
            self.scroll_to = Some(new_id);
        } else {
            let caret = self.page.blocks[index].text.chars().count();
            self.focus = Focus::Block(id);
            self.pending = Some(PendingFocus {
                target: Focus::Block(id),
                range: Some((caret, caret)),
                select_all: false,
            });
        }
    }

    fn slash_overlay(&mut self, ctx: &Context) {
        let Some(slash) = self.slash.clone() else {
            return;
        };
        let items = slash_items(&slash.query);
        let selected = slash.selected.min(items.len().saturating_sub(1));
        let p = self.palette;

        let anchor = self
            .block_rects
            .get(&slash.block)
            .map(|rect| rect.left_bottom() + vec2(GUTTER, 4.0))
            .or_else(|| ctx.input(|input| input.pointer.hover_pos()))
            .unwrap_or_else(|| ctx.content_rect().center());
        let height = (items.len().max(1) as f32 * 31.0 + 44.0).min(400.0);
        let pos = menu_pos(ctx, anchor, 340.0, height);
        let mut close = false;
        let mut clicked_kind: Option<BlockKind> = None;
        let area = egui::Area::new(Id::unique("simion-slash"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(5))
                    .show(ui, |ui| {
                        ui.set_width(330.0);
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(if slash.query.is_empty() {
                                    "Blocos"
                                } else {
                                    "Resultados"
                                })
                                .size(10.5)
                                .color(p.faint),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_space(4.0);
                                ui.label(RichText::new("↑↓ · Enter").size(10.0).color(p.faint));
                            });
                        });
                        ui.add_space(2.0);
                        ScrollArea::vertical()
                            .max_height(330.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if items.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Nenhum bloco encontrado")
                                            .size(12.5)
                                            .color(p.faint),
                                    );
                                    ui.add_space(8.0);
                                }
                                for (index, kind) in items.iter().enumerate() {
                                    let row = widgets::menu_row(
                                        ui,
                                        kind.label(),
                                        Some(kind.markdown_hint()),
                                        false,
                                        index == selected,
                                        &p,
                                    );
                                    if row.hovered() {
                                        if let Some(slash) = self.slash.as_mut() {
                                            slash.selected = index;
                                        }
                                    }
                                    if row.clicked() {
                                        clicked_kind = Some(*kind);
                                        close = true;
                                    }
                                }
                            });
                    });
            });

        if let Some(kind) = clicked_kind {
            self.execute_slash(kind);
        } else if close || self.outside_click(ctx, area.response.rect) {
            self.slash = None;
        }
    }

    // -----------------------------------------------------------------------
    // Block menu
    // -----------------------------------------------------------------------

    fn block_menu_overlay(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let Some((id, pos)) = self.block_menu else {
            return;
        };
        let Some(index) = self.index_of(id) else {
            self.block_menu = None;
            return;
        };
        let current_kind = self.page.blocks[index].kind;
        let p = self.palette;
        let transform = self.block_menu_transform;
        let height = if transform { 402.0 } else { 300.0 };
        let pos = menu_pos(ctx, pos, 252.0, height);
        let mut close = false;
        let mut open_transform = false;
        let mut go_back = false;

        let area = egui::Area::new(Id::unique("simion-block-menu"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                widgets::popup_frame(&p).show(ui, |ui| {
                    ui.set_width(240.0);
                    if transform {
                        let back = widgets::menu_row(ui, "← Voltar", None, false, false, &p);
                        if back.clicked() {
                            go_back = true;
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new("Transformar em").size(10.5).color(p.faint));
                        });
                        ui.add_space(2.0);
                        for kind in BlockKind::ALL {
                            let row = widgets::menu_row(
                                ui,
                                kind.label(),
                                Some(kind.markdown_hint()),
                                false,
                                kind == current_kind,
                                &p,
                            );
                            if row.clicked() {
                                actions.push(Action::Transform(id, kind));
                                close = true;
                            }
                        }
                    } else {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(current_kind.label())
                                    .size(10.5)
                                    .color(p.faint),
                            );
                        });
                        ui.add_space(2.0);
                        if widgets::menu_row(ui, "Transformar em", Some("›"), false, false, &p)
                            .clicked()
                        {
                            open_transform = true;
                        }
                        widgets::hairline(ui, &p);
                        ui.add_space(2.0);
                        if widgets::menu_row(ui, "Duplicar bloco", None, false, false, &p).clicked()
                        {
                            actions.push(Action::DuplicateBlock(id));
                            close = true;
                        }
                        if widgets::menu_row(ui, "Copiar texto", None, false, false, &p).clicked() {
                            actions.push(Action::CopyBlock(id));
                            close = true;
                        }
                        if widgets::menu_row(ui, "Mover para cima", Some("Alt ↑"), false, false, &p)
                            .clicked()
                        {
                            actions.push(Action::MoveBlock(id, -1));
                            close = true;
                        }
                        if widgets::menu_row(
                            ui,
                            "Mover para baixo",
                            Some("Alt ↓"),
                            false,
                            false,
                            &p,
                        )
                        .clicked()
                        {
                            actions.push(Action::MoveBlock(id, 1));
                            close = true;
                        }
                        widgets::hairline(ui, &p);
                        ui.add_space(2.0);
                        if widgets::menu_row(ui, "Excluir bloco", Some("Del"), true, false, &p)
                            .clicked()
                        {
                            actions.push(Action::DeleteBlock(id));
                            close = true;
                        }
                    }
                });
            });

        if open_transform {
            self.block_menu_transform = true;
        } else if go_back {
            self.block_menu_transform = false;
        } else if close || self.outside_click(ctx, area.response.rect) {
            self.block_menu = None;
            self.block_menu_transform = false;
        }
    }

    // -----------------------------------------------------------------------
    // Page menu
    // -----------------------------------------------------------------------

    fn page_menu_overlay(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let Some((page_id, pos)) = self.page_menu else {
            return;
        };
        let Some(meta) = self.meta_of(page_id).cloned() else {
            self.page_menu = None;
            return;
        };
        let p = self.palette;
        let pos = menu_pos(ctx, pos, 250.0, 360.0);
        let mut close = false;
        let area = egui::Area::new(Id::unique("simion-page-menu"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                widgets::popup_frame(&p).show(ui, |ui| {
                    ui.set_width(238.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(meta.display_title())
                                    .size(13.0)
                                    .font(FontId::new(13.0, family(FONT_SEMIBOLD)))
                                    .color(p.text),
                            );
                            ui.label(RichText::new("Página").size(10.5).color(p.faint));
                        });
                    });
                    ui.add_space(4.0);
                    widgets::hairline(ui, &p);
                    ui.add_space(2.0);

                    if widgets::menu_row(
                        ui,
                        if meta.favorite {
                            "Remover dos favoritos"
                        } else {
                            "Adicionar aos favoritos"
                        },
                        None,
                        false,
                        false,
                        &p,
                    )
                    .clicked()
                    {
                        actions.push(Action::ToggleFavorite(page_id));
                        close = true;
                    }
                    if widgets::menu_row(ui, "Renomear", None, false, false, &p).clicked() {
                        actions.push(Action::SelectPage(page_id));
                        actions.push(Action::FocusTitle);
                        close = true;
                    }
                    if widgets::menu_row(ui, "Nova subpágina", None, false, false, &p).clicked() {
                        actions.push(Action::NewPage {
                            parent: Some(page_id),
                        });
                        close = true;
                    }
                    widgets::hairline(ui, &p);
                    ui.add_space(2.0);
                    if widgets::menu_row(ui, "Duplicar página", None, false, false, &p).clicked() {
                        actions.push(Action::DuplicatePage(page_id));
                        close = true;
                    }
                    if widgets::menu_row(ui, "Exportar .md", None, false, false, &p).clicked() {
                        actions.push(Action::ExportMarkdown(page_id));
                        close = true;
                    }
                    if widgets::menu_row(ui, "Copiar Markdown", None, false, false, &p).clicked() {
                        actions.push(Action::CopyMarkdown(page_id));
                        close = true;
                    }
                    if widgets::menu_row(ui, "Ver Markdown", Some("Ctrl ⇧ M"), false, false, &p)
                        .clicked()
                    {
                        if page_id != self.current.unwrap_or(0) {
                            actions.push(Action::SelectPage(page_id));
                        }
                        actions.push(Action::OpenMarkdown);
                        close = true;
                    }
                    widgets::hairline(ui, &p);
                    ui.add_space(2.0);
                    if widgets::menu_row(ui, "Excluir página", None, true, false, &p).clicked() {
                        actions.push(Action::AskDelete(page_id));
                        close = true;
                    }
                });
            });
        if close || self.outside_click(ctx, area.response.rect) {
            self.page_menu = None;
        }
    }

    // -----------------------------------------------------------------------
    // Emoji picker
    // -----------------------------------------------------------------------

    fn emoji_overlay(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let Some(anchor) = self.emoji_picker else {
            return;
        };
        let p = self.palette;
        let width = 8.0 * 34.0 + 16.0;
        let height = 340.0;
        let pos = menu_pos(ctx, anchor, width, height);
        let mut close = false;
        let current = self.page.meta.icon.clone();
        let area = egui::Area::new(Id::unique("simion-emoji"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(width - 16.0);
                        ui.label(RichText::new("Ícone da página").size(10.5).color(p.faint));
                        ui.add_space(4.0);
                        for chunk in EMOJIS.chunks(8) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                for emoji in chunk {
                                    let (rect, response) =
                                        ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                                    let selected = current == *emoji;
                                    if response.hovered() || selected {
                                        ui.painter().rect_filled(
                                            rect,
                                            7,
                                            if selected { p.accent_soft } else { p.hover },
                                        );
                                    }
                                    ui.painter().text(
                                        rect.center(),
                                        Align2::CENTER_CENTER,
                                        emoji,
                                        FontId::new(20.0, FontFamily::Proportional),
                                        p.text,
                                    );
                                    if response.clicked() {
                                        actions.push(Action::SetPageIcon((*emoji).to_owned()));
                                        close = true;
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        widgets::hairline(ui, &p);
                        ui.add_space(2.0);
                        if widgets::menu_row(ui, "Remover ícone", None, false, false, &p).clicked()
                        {
                            actions.push(Action::SetPageIcon(String::new()));
                            close = true;
                        }
                    });
            });
        if close || self.outside_click(ctx, area.response.rect) {
            self.emoji_picker = None;
        }
    }

    // -----------------------------------------------------------------------
    // Modals
    // -----------------------------------------------------------------------

    fn shortcuts_overlay(&mut self, ctx: &Context) {
        if !self.shortcuts_open {
            return;
        }
        let p = self.palette;
        let mut close = false;
        let width = (ctx.content_rect().width() - 48.0).min(560.0);
        let modal = Modal::new(Id::unique("simion-shortcuts"))
            .backdrop_color(if p.dark {
                Color32::from_black_alpha(150)
            } else {
                Color32::from_black_alpha(40)
            })
            .frame(
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(18))
                    .corner_radius(14),
            );
        let response = modal.show(ctx, |ui| {
            ui.set_width(width);
            ui.label(
                RichText::new("Atalhos")
                    .size(19.0)
                    .font(FontId::new(19.0, family(FONT_SEMIBOLD)))
                    .color(p.text),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new("Feito para as mãos ficarem no teclado.")
                    .size(12.0)
                    .color(p.muted),
            );
            ui.add_space(6.0);
            ScrollArea::vertical()
                .max_height((ctx.content_rect().height() * 0.6).clamp(220.0, 520.0))
                .auto_shrink([false, true])
                .id_salt("simion-shortcuts-scroll")
                .show(ui, |ui| {
                    shortcut_group(
                        ui,
                        "Essencial",
                        &[
                            ("Ctrl K", "Buscar em todas as páginas"),
                            ("Ctrl N", "Nova subpágina"),
                            ("Ctrl S", "Salvar agora"),
                            ("Ctrl \\", "Mostrar ou ocultar a barra lateral"),
                            ("Ctrl /", "Esta lista de atalhos"),
                        ],
                        &p,
                    );
                    ui.add_space(8.0);
                    shortcut_group(
                        ui,
                        "Edição",
                        &[
                            ("Ctrl Z", "Desfazer"),
                            ("Ctrl ⇧ Z", "Refazer"),
                            ("Ctrl B", "Negrito"),
                            ("Ctrl I", "Itálico"),
                            ("Ctrl E", "Código inline"),
                            ("Ctrl ⇧ S", "Riscado"),
                            ("Ctrl ⇧ K", "Inserir link"),
                        ],
                        &p,
                    );
                    ui.add_space(8.0);
                    shortcut_group(
                        ui,
                        "Blocos",
                        &[
                            ("/", "Menu de comandos"),
                            ("Tab", "Aumentar recuo"),
                            ("Shift Tab", "Diminuir recuo"),
                            ("Alt ↑ ↓", "Mover bloco"),
                            ("Ctrl Alt 1-3", "Transformar em título"),
                            ("Ctrl Alt 0", "Voltar a texto"),
                        ],
                        &p,
                    );
                    ui.add_space(8.0);
                    shortcut_group(
                        ui,
                        "Navegação",
                        &[
                            ("↑ ↓", "Mover entre blocos"),
                            ("Ctrl ⇧ M", "Ver Markdown da página"),
                            ("Ctrl + -", "Zoom"),
                            ("Ctrl 0", "Zoom padrão"),
                        ],
                        &p,
                    );
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::primary_button(ui, "Fechar", &p).clicked() {
                        close = true;
                    }
                });
            });
        });
        if close || response.should_close() {
            self.shortcuts_open = false;
        }
    }

    fn markdown_overlay(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        if !self.markdown_open {
            return;
        }
        let p = self.palette;
        let page_id = self.current.unwrap_or(0);
        let source = markdown::page_markdown(&self.title(), &self.page.blocks);
        let mut close = false;
        let width = (ctx.content_rect().width() - 64.0).min(780.0);
        let modal = Modal::new(Id::unique("simion-markdown"))
            .backdrop_color(if p.dark {
                Color32::from_black_alpha(150)
            } else {
                Color32::from_black_alpha(40)
            })
            .frame(
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(18))
                    .corner_radius(14),
            );
        let response = modal.show(ctx, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Markdown desta página")
                        .size(17.0)
                        .font(FontId::new(17.0, family(FONT_SEMIBOLD)))
                        .color(p.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::key_hint(ui, "Ctrl ⇧ M", &p);
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new("Copie para qualquer lugar ou use como backup.")
                    .size(12.0)
                    .color(p.muted),
            );
            ui.add_space(10.0);
            egui::Frame::new()
                .fill(p.code_bg)
                .corner_radius(8)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() * 0.5).min(420.0))
                        .auto_shrink([false, false])
                        .id_salt("simion-markdown-scroll")
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&source)
                                        .font(FontId::new(11.5, family(FONT_MONO)))
                                        .color(p.text),
                                )
                                .wrap()
                                .selectable(true),
                            );
                        });
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Copiar Markdown", &p).clicked() {
                    actions.push(Action::CopyMarkdown(page_id));
                }
                ui.add_space(6.0);
                if widgets::ghost_button(ui, "Exportar .md", &p).clicked() {
                    actions.push(Action::ExportMarkdown(page_id));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::ghost_button(ui, "Fechar", &p).clicked() {
                        close = true;
                    }
                });
            });
        });
        if close || response.should_close() {
            self.markdown_open = false;
        }
    }

    fn delete_overlay(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let Some(page_id) = self.confirm_delete else {
            return;
        };
        let p = self.palette;
        let title = self
            .meta_of(page_id)
            .map(|meta| meta.display_title().to_owned())
            .unwrap_or_else(|| "Sem título".to_owned());
        let has_children = !self.descendants_public(page_id).is_empty();
        let mut close = false;
        let modal = Modal::new(Id::unique("simion-delete"))
            .backdrop_color(if p.dark {
                Color32::from_black_alpha(160)
            } else {
                Color32::from_black_alpha(50)
            })
            .frame(
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(20))
                    .corner_radius(14),
            );
        let response = modal.show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(
                RichText::new("Excluir página?")
                    .size(18.0)
                    .font(FontId::new(18.0, family(FONT_SEMIBOLD)))
                    .color(p.text),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("“{title}”"))
                    .size(13.0)
                    .font(FontId::new(13.0, family(FONT_MEDIUM)))
                    .color(p.text),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(if has_children {
                    "As subpáginas também serão excluídas. Não há como desfazer."
                } else {
                    "Esta ação não pode ser desfeita."
                })
                .size(12.0)
                .color(p.muted),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::danger_button(ui, "Excluir", &p).clicked() {
                        actions.push(Action::DeletePage(page_id));
                        close = true;
                    }
                    ui.add_space(6.0);
                    if widgets::ghost_button(ui, "Cancelar", &p).clicked() {
                        close = true;
                    }
                });
            });
        });
        if close || response.should_close() {
            self.confirm_delete = None;
        }
    }

    // -----------------------------------------------------------------------
    // Toast + drop indicator
    // -----------------------------------------------------------------------

    fn toast_overlay(&mut self, ctx: &Context) {
        let Some(toast) = self.toast.clone() else {
            return;
        };
        if self.now > toast.until + 0.4 {
            return;
        }
        let p = self.palette;
        let remaining = toast.until - self.now;
        let alpha = if remaining < 0.35 {
            (remaining / 0.35).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let area = egui::Area::new(Id::unique("simion-toast"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -42.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(alpha as f32);
                egui::Frame::new()
                    .fill(p.toast_bg)
                    .corner_radius(9)
                    .inner_margin(Margin::symmetric(14, 8))
                    .shadow(egui::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: p.shadow,
                    })
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&toast.text)
                                .size(12.5)
                                .font(FontId::new(12.5, family(FONT_MEDIUM)))
                                .color(p.toast_text),
                        );
                    });
            });
        let _ = area;
    }

    fn drop_overlay(&mut self, ctx: &Context) {
        if !self.file_hover {
            return;
        }
        let p = self.palette;
        let screen = ctx.content_rect();
        let _ = egui::Area::new(Id::unique("simion-drop"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();
                painter.rect_filled(screen, 0, Color32::from_black_alpha(70));
                let inner = screen.shrink(26.0);
                painter.rect_filled(inner, 16, Color32::from_rgba_unmultiplied(25, 25, 24, 220));
                painter.rect_stroke(inner, 16, Stroke::new(2.0, p.accent), StrokeKind::Inside);
                painter.text(
                    inner.center() - vec2(0.0, 10.0),
                    Align2::CENTER_CENTER,
                    "Solte para importar",
                    FontId::new(22.0, family(FONT_SEMIBOLD)),
                    Color32::WHITE,
                );
                painter.text(
                    inner.center() + vec2(0.0, 22.0),
                    Align2::CENTER_CENTER,
                    "O arquivo .md vira uma nova página",
                    FontId::new(13.0, family(FONT_BODY)),
                    Color32::from_rgb(200, 200, 198),
                );
            });
    }

    fn outside_click(&self, ctx: &Context, rect: Rect) -> bool {
        self.now > self.popup_guard && clicked_outside(ctx, rect)
    }

    fn descendants_public(&self, id: i64) -> Vec<i64> {
        let children = self.children_map();
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            if let Some(list) = children.get(&Some(current)) {
                for child in list {
                    out.push(*child);
                    stack.push(*child);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn slash_items(query: &str) -> Vec<BlockKind> {
    let needle = fold(query);
    BlockKind::ALL
        .iter()
        .copied()
        .filter(|kind| {
            if needle.is_empty() {
                return true;
            }
            fold(kind.label()).contains(&needle)
                || fold(kind.description()).contains(&needle)
                || fold(kind.markdown_hint()).contains(&needle)
                || fold(kind.keywords()).contains(&needle)
        })
        .collect()
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

fn menu_pos(ctx: &Context, pos: Pos2, width: f32, height: f32) -> Pos2 {
    let screen = ctx.content_rect();
    let margin = 8.0;
    let x = pos.x.clamp(
        screen.left() + margin,
        (screen.right() - width - margin).max(screen.left() + margin),
    );
    let y = if pos.y + height + margin > screen.bottom() {
        (pos.y - height).max(screen.top() + margin)
    } else {
        pos.y
    };
    pos2(x, y)
}

fn clicked_outside(ctx: &Context, rect: Rect) -> bool {
    ctx.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|position| !rect.contains(position))
    })
}

fn shortcut_group(ui: &mut Ui, title: &str, rows: &[(&str, &str)], p: &Palette) {
    ui.label(RichText::new(title).size(10.5).color(p.faint));
    ui.add_space(2.0);
    for (keys, description) in rows {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
        let pill =
            Rect::from_min_size(pos2(rect.left() + 4.0, rect.top() + 4.0), vec2(122.0, 18.0));
        ui.painter().rect_filled(pill, 4, p.hover);
        ui.painter().text(
            pill.center(),
            Align2::CENTER_CENTER,
            *keys,
            FontId::new(10.5, family(FONT_MONO)),
            p.muted,
        );
        ui.painter().text(
            pos2(rect.left() + 140.0, rect.center().y),
            Align2::LEFT_CENTER,
            *description,
            FontId::new(12.5, family(FONT_BODY)),
            p.text,
        );
    }
}
