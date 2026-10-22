//! Full-text search overlay (Ctrl+K).

use eframe::egui::{
    self, Align2, Color32, Context, FontFamily, FontId, Id, Margin, Modal, Rect, RichText,
    ScrollArea, Sense, TextEdit, Ui, pos2,
    text::{ByteIndex, LayoutJob, LayoutSection, TextFormat},
    vec2,
};

use crate::app::{Action, App};
use crate::icons;
use crate::store::SearchHit;
use crate::theme::{FONT_BODY, FONT_SEMIBOLD, Palette, family};
use crate::ui as widgets;

#[derive(Default)]
pub struct SearchState {
    pub open: bool,
    pub query: String,
    pub results: Vec<SearchHit>,
    pub selected: usize,
    pub focus_pending: bool,
    /// Scroll the selected result into view after keyboard navigation.
    pub scroll_selected: bool,
}

impl App {
    pub fn search_recent(&mut self) {
        let mut pages = self.pages.clone();
        pages.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.search.results = pages
            .into_iter()
            .take(8)
            .map(|meta| SearchHit {
                page_id: meta.id,
                title: meta.display_title().to_owned(),
                icon: meta.icon,
                snippet: String::new(),
            })
            .collect();
        self.search.selected = 0;
    }

    pub fn run_search(&mut self) {
        let query = self.search.query.trim().to_owned();
        if query.is_empty() {
            self.search_recent();
            return;
        }
        let results = self
            .store
            .as_ref()
            .and_then(|store| store.search(&query, 50).ok())
            .unwrap_or_default();
        self.search.results = results;
        self.search.selected = self
            .search
            .selected
            .min(self.search.results.len().saturating_sub(1));
    }

    pub fn search_keys(&mut self, ctx: &Context) {
        if self.consume_pub(ctx, egui::Key::Escape, egui::Modifiers::NONE)
            || self.consume_pub(ctx, egui::Key::K, egui::Modifiers::COMMAND)
        {
            self.search.open = false;
            return;
        }
        let len = self.search.results.len();
        if len == 0 {
            return;
        }
        if self.consume_pub(ctx, egui::Key::ArrowDown, egui::Modifiers::NONE) {
            self.search.selected = (self.search.selected + 1).min(len - 1);
            self.search.scroll_selected = true;
        }
        if self.consume_pub(ctx, egui::Key::ArrowUp, egui::Modifiers::NONE) {
            self.search.selected = self.search.selected.saturating_sub(1);
            self.search.scroll_selected = true;
        }
        if self.consume_pub(ctx, egui::Key::Enter, egui::Modifiers::NONE) {
            if let Some(hit) = self.search.results.get(self.search.selected) {
                let page_id = hit.page_id;
                self.search.open = false;
                self.open_page(page_id);
            }
        }
    }

    pub fn search_overlay(&mut self, ctx: &Context) -> Vec<Action> {
        let mut actions = Vec::new();
        if !self.search.open {
            return actions;
        }
        let p = self.palette;
        let width = (ctx.content_rect().width() - 48.0).min(640.0).max(320.0);
        let results = self.search.results.clone();
        let selected = self.search.selected;
        let scroll_selected = self.search.scroll_selected;
        let focus_pending = self.search.focus_pending;
        let mut query = std::mem::take(&mut self.search.query);
        let new_query;
        let mut open_page: Option<i64> = None;
        let mut new_selected = selected;

        let modal = Modal::new(Id::unique("simion-search-modal"))
            .backdrop_color(if p.dark {
                Color32::from_black_alpha(150)
            } else {
                Color32::from_black_alpha(40)
            })
            .frame(
                widgets::popup_frame(&p)
                    .inner_margin(Margin::same(10))
                    .corner_radius(14),
            )
            .area(
                Modal::default_area(Id::unique("simion-search-area"))
                    .anchor(Align2::CENTER_TOP, vec2(0.0, 76.0)),
            );

        let response = modal.show(ctx, |ui| {
            ui.set_width(width);
            let input_id = Id::unique("simion-search-input");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add_space(2.0);
                let (icon_rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icons::search(ui.painter(), icon_rect, p.faint, 1.6);
                let available = (ui.available_width() - 54.0).max(80.0);
                let input = TextEdit::singleline(&mut query)
                    .id(input_id)
                    .frame(egui::Frame::NONE)
                    .margin(Margin::symmetric(2, 8))
                    .font(FontId::new(16.0, family(FONT_BODY)))
                    .text_color(p.text)
                    .desired_width(available)
                    .hint_text(
                        RichText::new("Buscar em todas as páginas…")
                            .size(16.0)
                            .color(p.faint),
                    );
                let output = input.show(ui);
                if focus_pending {
                    output.response.request_focus();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::key_hint(ui, "Esc", &p);
                });
            });

            ui.add_space(6.0);
            widgets::hairline(ui, &p);
            ui.add_space(4.0);

            if results.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(if query.trim().is_empty() {
                            "Comece a digitar para buscar"
                        } else {
                            "Nenhum resultado encontrado"
                        })
                        .size(13.0)
                        .color(p.faint),
                    );
                    ui.add_space(20.0);
                });
            } else {
                let list_height = (ctx.content_rect().height() * 0.55).clamp(120.0, 420.0);
                ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(list_height)
                    .id_salt("simion-search-results")
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (index, hit) in results.iter().enumerate() {
                            let is_selected = index == selected;
                            let (rect, row_response) =
                                result_row(ui, hit, is_selected, &query, &p, width - 20.0);
                            if row_response.clicked() {
                                open_page = Some(hit.page_id);
                                new_selected = index;
                            }
                            if scroll_selected && is_selected && !ui.is_rect_visible(rect) {
                                ui.scroll_to_rect(rect, None);
                            }
                        }
                    });
            }

            ui.add_space(6.0);
            widgets::hairline(ui, &p);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("↑ ↓ navegar").size(11.0).color(p.faint));
                ui.label(RichText::new("·").size(11.0).color(p.faint));
                ui.label(RichText::new("Enter abrir").size(11.0).color(p.faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} resultados", results.len()))
                            .size(11.0)
                            .color(p.faint),
                    );
                });
            });
        });

        if response.should_close() {
            self.search.open = false;
        }

        new_query = query.clone();
        if self.search.query != new_query {
            self.search.query = new_query;
            self.run_search();
            self.search.selected = 0;
        }
        self.search.selected = new_selected.min(self.search.results.len().saturating_sub(1));
        self.search.focus_pending = false;
        self.search.scroll_selected = false;

        if let Some(page_id) = open_page {
            self.search.open = false;
            actions.push(Action::SelectPage(page_id));
        }

        actions
    }
}

fn result_row(
    ui: &mut Ui,
    hit: &SearchHit,
    selected: bool,
    query: &str,
    p: &Palette,
    width: f32,
) -> (Rect, egui::Response) {
    let height = if hit.snippet.is_empty() { 38.0 } else { 54.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let fill = if selected {
        p.accent_soft
    } else if response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 8, fill);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let icon_center = pos2(rect.left() + 20.0, rect.top() + 19.0);
    if hit.icon.trim().is_empty() {
        icons::file(
            ui.painter(),
            Rect::from_center_size(icon_center, vec2(17.0, 17.0)),
            p.muted,
            1.3,
        );
    } else {
        ui.painter().text(
            icon_center,
            Align2::CENTER_CENTER,
            &hit.icon,
            FontId::new(15.0, FontFamily::Proportional),
            p.text,
        );
    }

    let title = hit.title.trim();
    let title = if title.is_empty() {
        "Sem título"
    } else {
        title
    };
    let title_galley = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            title.to_owned(),
            FontId::new(13.5, family(FONT_SEMIBOLD)),
            p.text,
        )
    });
    let title_clip = ui.painter().with_clip_rect(Rect::from_min_max(
        pos2(rect.left() + 38.0, rect.top()),
        pos2(rect.right() - 8.0, rect.top() + 30.0),
    ));
    title_clip.galley(
        pos2(rect.left() + 38.0, rect.top() + 9.0),
        title_galley,
        p.text,
    );

    if !hit.snippet.is_empty() {
        let job = snippet_job(&hit.snippet, query, p, width - 46.0);
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let clip = ui.painter().with_clip_rect(Rect::from_min_max(
            pos2(rect.left() + 38.0, rect.top() + 26.0),
            pos2(rect.right() - 8.0, rect.bottom() - 2.0),
        ));
        clip.galley(pos2(rect.left() + 38.0, rect.top() + 27.0), galley, p.muted);
    }

    (rect, response)
}

fn match_ranges(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    use crate::markdown;
    use crate::store::fold;
    let haystack = fold(text);
    let needle = fold(query.trim());
    if needle.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < haystack.len() {
        let Some(position) = haystack[start..].find(&needle) else {
            break;
        };
        let byte = start + position;
        let char_start = haystack[..byte].chars().count();
        let char_end = char_start + needle.chars().count();
        let from = markdown::char_to_byte(text, char_start);
        let to = markdown::char_to_byte(text, char_end);
        out.push(from..to);
        start = byte + needle.len();
        if out.len() >= 8 {
            break;
        }
    }
    out
}

fn snippet_job(snippet: &str, query: &str, p: &Palette, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text = snippet.to_owned();
    job.wrap.max_width = wrap;
    job.wrap.max_rows = 2;
    job.break_on_newline = false;

    let matches = match_ranges(snippet, query);
    let _ = &matches;

    let normal_format = || TextFormat {
        font_id: FontId::new(12.0, family(FONT_BODY)),
        color: p.muted,
        ..Default::default()
    };
    let match_format = || TextFormat {
        font_id: FontId::new(12.0, family(FONT_SEMIBOLD)),
        color: p.accent_text,
        ..Default::default()
    };

    let mut covered = 0usize;
    let text_len = snippet.len();
    for range in matches {
        if range.start > covered {
            job.sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: ByteIndex(covered)..ByteIndex(range.start),
                format: normal_format(),
            });
        }
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(range.start)..ByteIndex(range.end),
            format: match_format(),
        });
        covered = range.end;
        if covered >= text_len {
            break;
        }
    }
    if covered < text_len {
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(covered)..ByteIndex(text_len),
            format: normal_format(),
        });
    }
    job
}
