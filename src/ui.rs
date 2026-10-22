//! Small custom widgets that give Simion its look.

use eframe::egui::{
    self, Color32, FontId, Frame, Margin, Painter, Rect, Response, RichText, Sense, Shadow, Stroke,
    StrokeKind, Ui, Vec2, vec2,
};

use crate::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, Palette, family};

pub fn popup_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.elevated)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(12)
        .inner_margin(Margin::same(6))
        .shadow(Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: p.shadow,
        })
}

fn animated_fill(ui: &Ui, id: egui::Id, color: Color32, hovered: bool, active: bool) -> Color32 {
    let target = if active {
        1.0
    } else if hovered {
        0.85
    } else {
        0.0
    };
    let t = ui
        .ctx()
        .animate_bool_with_time(id.with("hover-anim"), target > 0.5, 0.09);
    let t = if active {
        1.0
    } else if hovered {
        t.max(0.75)
    } else {
        t * 0.8
    };
    if t <= 0.01 {
        Color32::TRANSPARENT
    } else {
        Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            (color.a() as f32 * t).round().clamp(0.0, 255.0) as u8,
        )
    }
}

/// A square, frameless icon button. `draw` paints inside a centered box.
pub fn icon_button(
    ui: &mut Ui,
    size: f32,
    tint: Color32,
    tooltip: &str,
    draw: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hovered = response.hovered();
    let active = response.is_pointer_button_down_on();
    let fill = animated_fill(
        ui,
        response.id,
        ui.visuals().widgets.hovered.weak_bg_fill,
        hovered,
        active,
    );
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 7, fill);
    }
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.62));
    let color = if hovered || active {
        ui.visuals().override_text_color.unwrap_or(tint)
    } else {
        tint
    };
    draw(ui.painter(), icon_rect, color);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// A row inside a popup menu.
pub fn menu_row(
    ui: &mut Ui,
    label: &str,
    hint: Option<&str>,
    danger: bool,
    selected: bool,
    p: &Palette,
) -> Response {
    let width = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let hovered = response.hovered();
    let active = response.is_pointer_button_down_on();
    let fill = animated_fill(ui, response.id, p.hover, hovered, active);
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 6, fill);
    }
    let color = if danger {
        p.danger
    } else if selected {
        p.accent_text
    } else {
        p.text
    };
    if selected {
        ui.painter().rect_filled(rect, 6, p.accent_soft);
    }
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, family(FONT_MEDIUM)),
        color,
    );
    if let Some(hint) = hint {
        ui.painter().text(
            rect.right_center() - vec2(10.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            hint,
            FontId::new(11.5, family(theme::FONT_BODY)),
            p.faint,
        );
    }
    response
}

pub fn section_label(ui: &mut Ui, text: &str, p: &Palette) {
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(text)
                .size(11.5)
                .font(FontId::new(11.5, family(FONT_SEMIBOLD)))
                .color(p.faint),
        );
    });
    ui.add_space(3.0);
}

pub fn key_hint(ui: &mut Ui, text: &str, p: &Palette) {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(11.0, family(FONT_MEDIUM)),
        p.faint,
    );
    let size = galley.size() + vec2(10.0, 4.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 4, p.hover);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.faint);
}

pub fn hairline(ui: &mut Ui, p: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, p.border);
}

pub fn primary_button(ui: &mut Ui, label: &str, p: &Palette) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(14.0, family(FONT_SEMIBOLD)),
        p.bg,
    );
    let size = vec2(galley.size().x + 26.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let fill = if hovered {
        p.text.gamma_multiply(0.86)
    } else {
        p.text
    };
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.bg);
    response
}

pub fn danger_button(ui: &mut Ui, label: &str, p: &Palette) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(14.0, family(FONT_SEMIBOLD)),
        Color32::WHITE,
    );
    let size = vec2(galley.size().x + 26.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        p.danger.gamma_multiply(0.88)
    } else {
        p.danger
    };
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    response
}

pub fn ghost_button(ui: &mut Ui, label: &str, p: &Palette) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(14.0, family(FONT_MEDIUM)),
        p.text,
    );
    let size = vec2(galley.size().x + 24.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 8, p.hover);
    } else {
        ui.painter()
            .rect_stroke(rect, 8, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.text);
    response
}
