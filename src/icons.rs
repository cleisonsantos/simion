//! Hand-drawn vector icons (crisp at any scale, no icon-font needed).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};

fn geometry(rect: Rect) -> (Pos2, f32) {
    let s = rect.width().min(rect.height());
    (rect.center(), s)
}

pub fn chevron_right(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let x = s * 0.17;
    let y = s * 0.22;
    p.line_segment(
        [c + vec2(-x, -y), c + vec2(x * 0.5, 0.0)],
        Stroke::new(w, color),
    );
    p.line_segment(
        [c + vec2(x * 0.5, 0.0), c + vec2(-x, y)],
        Stroke::new(w, color),
    );
}

pub fn chevron_down(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let x = s * 0.22;
    let y = s * 0.17;
    p.line_segment(
        [c + vec2(-x, -y * 0.5), c + vec2(0.0, y)],
        Stroke::new(w, color),
    );
    p.line_segment(
        [c + vec2(0.0, y), c + vec2(x, -y * 0.5)],
        Stroke::new(w, color),
    );
}

pub fn plus(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = s * 0.26;
    p.line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], Stroke::new(w, color));
    p.line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], Stroke::new(w, color));
}

pub fn grip(p: &Painter, rect: Rect, color: Color32) {
    let (c, s) = geometry(rect);
    let r = (s * 0.07).max(1.15);
    let dx = s * 0.13;
    let dy = s * 0.19;
    for row in -1..=1 {
        for col in [-1.0f32, 1.0] {
            p.circle_filled(c + vec2(col * dx, row as f32 * dy), r, color);
        }
    }
}

pub fn dots_h(p: &Painter, rect: Rect, color: Color32) {
    let (c, s) = geometry(rect);
    let r = (s * 0.055).max(1.0);
    for i in -1..=1 {
        p.circle_filled(c + vec2(i as f32 * s * 0.17, 0.0), r, color);
    }
}

pub fn search(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let radius = s * 0.24;
    let center = c + vec2(-s * 0.07, -s * 0.07);
    p.circle_stroke(center, radius, Stroke::new(w, color));
    p.line_segment(
        [
            center + vec2(radius * 0.72, radius * 0.72),
            center + vec2(radius * 1.55, radius * 1.55),
        ],
        Stroke::new(w, color),
    );
}

pub fn check(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let points = vec![
        c + vec2(-s * 0.26, s * 0.0),
        c + vec2(-s * 0.07, s * 0.18),
        c + vec2(s * 0.26, -s * 0.19),
    ];
    p.add(Shape::line(points, Stroke::new(w, color)));
}

pub fn sun(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = s * 0.16;
    p.circle_stroke(c, r, Stroke::new(w, color));
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = vec2(angle.cos(), angle.sin());
        p.line_segment(
            [c + dir * (r * 1.7), c + dir * (r * 2.5)],
            Stroke::new(w, color),
        );
    }
}

pub fn moon(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = s * 0.3;
    // Outer arc (the lit side) plus a shorter inner arc: a crescent outline.
    let outer = (0..17)
        .map(|i| {
            let t = i as f32 / 16.0;
            let angle = 0.7 + t * (std::f32::consts::TAU - 1.4);
            c + vec2(angle.cos(), angle.sin()) * r
        })
        .collect::<Vec<_>>();
    p.add(Shape::line(outer, Stroke::new(w, color)));
    let inner = (0..13)
        .map(|i| {
            let t = i as f32 / 12.0;
            let angle = 0.7 + (1.0 - t) * (std::f32::consts::TAU - 1.4);
            c + vec2(s * 0.16, 0.0) + vec2(angle.cos(), angle.sin()) * r * 0.78
        })
        .collect::<Vec<_>>();
    p.add(Shape::line(inner, Stroke::new(w, color)));
}

pub fn panel_left(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = Rect::from_center_size(c, vec2(s * 0.68, s * 0.56));
    p.rect_stroke(r, 3, Stroke::new(w, color), StrokeKind::Inside);
    let x = r.left() + r.width() * 0.36;
    p.line_segment(
        [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
        Stroke::new(w, color),
    );
}

pub fn star(p: &Painter, rect: Rect, color: Color32, filled: bool, w: f32) {
    let (c, s) = geometry(rect);
    let outer = s * 0.33;
    let inner = outer * 0.45;
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { outer } else { inner };
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(c + vec2(angle.cos(), angle.sin()) * radius);
    }
    if filled {
        p.add(Shape::convex_polygon(points, color, Stroke::new(w, color)));
    } else {
        p.add(Shape::closed_line(points, Stroke::new(w, color)));
    }
}

pub fn download(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    p.line_segment(
        [c + vec2(0.0, -s * 0.28), c + vec2(0.0, s * 0.12)],
        Stroke::new(w, color),
    );
    let tip = c + vec2(0.0, s * 0.12);
    p.line_segment(
        [tip + vec2(-s * 0.14, -s * 0.13), tip],
        Stroke::new(w, color),
    );
    p.line_segment(
        [tip + vec2(s * 0.14, -s * 0.13), tip],
        Stroke::new(w, color),
    );
    p.line_segment(
        [c + vec2(-s * 0.24, s * 0.25), c + vec2(s * 0.24, s * 0.25)],
        Stroke::new(w, color),
    );
}

pub fn copy(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let back = Rect::from_min_size(c + vec2(-s * 0.22, -s * 0.22), vec2(s * 0.34, s * 0.34));
    let front = Rect::from_min_size(c + vec2(-s * 0.06, -s * 0.06), vec2(s * 0.34, s * 0.34));
    p.rect_stroke(back, 2, Stroke::new(w, color), StrokeKind::Inside);
    p.rect_stroke(front, 2, Stroke::new(w, color), StrokeKind::Inside);
}

pub fn file(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = Rect::from_center_size(c, vec2(s * 0.52, s * 0.66));
    p.rect_stroke(r, 2, Stroke::new(w, color), StrokeKind::Inside);
    let fold = s * 0.16;
    p.line_segment(
        [
            r.right_top() + vec2(-fold, 0.0),
            r.right_top() + vec2(0.0, fold),
        ],
        Stroke::new(w, color),
    );
    p.line_segment(
        [
            r.right_top() + vec2(-fold, 0.0),
            r.right_top() + vec2(0.0, fold),
        ],
        Stroke::new(w, color),
    );
    for i in 0..2 {
        let y = r.top() + r.height() * (0.55 + i as f32 * 0.18);
        p.line_segment(
            [
                Pos2::new(r.left() + s * 0.1, y),
                Pos2::new(r.right() - s * 0.12, y),
            ],
            Stroke::new(w * 0.9, color),
        );
    }
}

pub fn smile(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = s * 0.3;
    p.circle_stroke(c, r, Stroke::new(w, color));
    p.circle_filled(c + vec2(-s * 0.1, -s * 0.06), w * 0.9, color);
    p.circle_filled(c + vec2(s * 0.1, -s * 0.06), w * 0.9, color);
    let arc = (0..11)
        .map(|i| {
            let t = i as f32 / 10.0;
            let angle = 0.35 + t * (std::f32::consts::PI - 0.7);
            c + vec2(angle.cos(), angle.sin()) * r * 0.62
        })
        .collect::<Vec<_>>();
    p.add(Shape::line(arc, Stroke::new(w, color)));
}

pub fn keyboard(p: &Painter, rect: Rect, color: Color32, w: f32) {
    let (c, s) = geometry(rect);
    let r = Rect::from_center_size(c, vec2(s * 0.7, s * 0.46));
    p.rect_stroke(r, 3, Stroke::new(w, color), StrokeKind::Inside);
    let mut x = r.left() + r.width() * 0.14;
    let step = r.width() * 0.16;
    while x < r.right() - r.width() * 0.1 {
        p.circle_filled(Pos2::new(x, r.top() + r.height() * 0.3), w * 0.7, color);
        x += step;
    }
    p.line_segment(
        [
            Pos2::new(r.left() + r.width() * 0.22, r.bottom() - r.height() * 0.22),
            Pos2::new(r.right() - r.width() * 0.22, r.bottom() - r.height() * 0.22),
        ],
        Stroke::new(w, color),
    );
}

/// The Simion mark: a rounded square with a white S.
pub const BRAND: Color32 = Color32::from_rgb(0x0F, 0x83, 0x6B);
pub const BRAND_CREAM: Color32 = Color32::from_rgb(0xF8, 0xF5, 0xEA);
pub const BRAND_MINT: Color32 = Color32::from_rgb(0xA7, 0xF3, 0xD0);

/// The Simion mark: a jade rounded square with a cream S and a mint note dot.
pub fn logo(p: &Painter, rect: Rect) {
    let (c, s) = geometry(rect);
    let r = Rect::from_center_size(c, vec2(s * 0.94, s * 0.94));
    p.rect_filled(r, (s * 0.26).round() as u8, BRAND);

    // A quiet top highlight, like the sheen on the app icon.
    let sheen = Rect::from_min_size(
        r.min + vec2(0.0, r.height() * 0.04),
        vec2(r.width(), r.height() * 0.42),
    );
    p.rect_filled(
        sheen,
        (s * 0.22).round() as u8,
        Color32::from_white_alpha(10),
    );

    let path = s_path(r);
    let stroke = (r.width() * 0.104).max(1.4);
    p.add(Shape::line(path.clone(), Stroke::new(stroke, BRAND_CREAM)));
    if let (Some(first), Some(last)) = (path.first(), path.last()) {
        p.circle_filled(*first, stroke / 2.0, BRAND_CREAM);
        p.circle_filled(*last, stroke / 2.0, BRAND_CREAM);
    }
    p.circle_filled(
        r.min + vec2(r.width() * 0.78, r.height() * 0.78),
        r.width() * 0.044,
        BRAND_MINT,
    );
}

fn s_path(rect: Rect) -> Vec<Pos2> {
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * rect.width();
    let mut points = Vec::with_capacity(50);
    cubic(
        at(0.70, 0.30),
        at(0.40, 0.18),
        at(0.28, 0.34),
        at(0.48, 0.47),
        &mut points,
    );
    cubic(
        at(0.48, 0.47),
        at(0.68, 0.58),
        at(0.63, 0.78),
        at(0.30, 0.72),
        &mut points,
    );
    points
}

fn cubic(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, out: &mut Vec<Pos2>) {
    const STEPS: usize = 18;
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let u = 1.0 - t;
        let point = p0.to_vec2() * (u * u * u)
            + p1.to_vec2() * (3.0 * u * u * t)
            + p2.to_vec2() * (3.0 * u * t * t)
            + p3.to_vec2() * (t * t * t);
        out.push(point.to_pos2());
    }
}
