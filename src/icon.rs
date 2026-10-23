//! Generates the Simion window icon at startup (no image files needed).

use eframe::egui::IconData;

const SIZE: u32 = 128;

pub fn app_icon() -> IconData {
    let n = SIZE as usize;
    let s = SIZE as f32;
    let mut rgba = vec![0u8; n * n * 4];

    // Rounded-square mask.
    let radius = s * 0.235;
    // The S is a thick cubic Bézier stroke.
    let curve = s_curve_points(s);
    let stroke = s * 0.104;

    let ss = 2; // 2x2 supersampling
    for py in 0..n {
        for px in 0..n {
            let mut coverage = 0.0f32;
            let mut color_acc = [0.0f32; 3];
            let mut ink_acc = 0.0f32;
            let mut dot_acc = 0.0f32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = px as f32 + (sx as f32 + 0.5) / ss as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / ss as f32;
                    if !inside_rounded_rect(x, y, s, radius) {
                        continue;
                    }
                    coverage += 1.0;
                    // Diagonal indigo → violet gradient.
                    let t = (x / s * 0.45 + y / s * 0.55).clamp(0.0, 1.0);
                    let base = mix([0x0A, 0x5C, 0x4A], [0x17, 0xAE, 0x8B], t);
                    color_acc[0] += base[0];
                    color_acc[1] += base[1];
                    color_acc[2] += base[2];

                    let distance = distance_to_polyline((x, y), &curve);
                    let ink = smoothstep(
                        stroke * 0.5 - 0.6,
                        stroke * 0.5 + 0.6,
                        stroke * 0.5 - distance,
                    );
                    ink_acc += ink;
                    let dot_dx = x - s * 0.79;
                    let dot_dy = y - s * 0.79;
                    let dot = 1.0
                        - smoothstep(
                            s * 0.044 - 0.8,
                            s * 0.044 + 0.8,
                            (dot_dx * dot_dx + dot_dy * dot_dy).sqrt(),
                        );
                    dot_acc += dot;
                }
            }
            if coverage == 0.0 {
                continue;
            }
            let samples = (ss * ss) as f32;
            let base = [
                color_acc[0] / coverage,
                color_acc[1] / coverage,
                color_acc[2] / coverage,
            ];
            let ink = (ink_acc / coverage).clamp(0.0, 1.0);
            let mut final_color = [
                base[0] + (0xF8 as f32 - base[0]) * ink,
                base[1] + (0xF5 as f32 - base[1]) * ink,
                base[2] + (0xEA as f32 - base[2]) * ink,
            ];
            let dot = (dot_acc / coverage).clamp(0.0, 1.0);
            final_color = [
                final_color[0] + (0xA7 as f32 - final_color[0]) * dot,
                final_color[1] + (0xF3 as f32 - final_color[1]) * dot,
                final_color[2] + (0xD0 as f32 - final_color[2]) * dot,
            ];
            let alpha = (coverage / samples * 255.0).round() as u8;
            let index = (py * n + px) * 4;
            rgba[index] = final_color[0].round().clamp(0.0, 255.0) as u8;
            rgba[index + 1] = final_color[1].round().clamp(0.0, 255.0) as u8;
            rgba[index + 2] = final_color[2].round().clamp(0.0, 255.0) as u8;
            rgba[index + 3] = alpha;
        }
    }

    IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

fn inside_rounded_rect(x: f32, y: f32, s: f32, radius: f32) -> bool {
    if x < 0.0 || y < 0.0 || x > s || y > s {
        return false;
    }
    let rx = if x < radius {
        radius - x
    } else if x > s - radius {
        x - (s - radius)
    } else {
        0.0
    };
    let ry = if y < radius {
        radius - y
    } else if y > s - radius {
        y - (s - radius)
    } else {
        0.0
    };
    if rx > 0.0 && ry > 0.0 {
        rx * rx + ry * ry <= radius * radius
    } else {
        true
    }
}

/// Sampled cubic Bézier "S", in pixel coordinates.
fn s_curve_points(s: f32) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    // Top bowl.
    bezier(
        (0.70 * s, 0.30 * s),
        (0.40 * s, 0.18 * s),
        (0.28 * s, 0.34 * s),
        (0.48 * s, 0.47 * s),
        &mut points,
    );
    // Bottom bowl.
    bezier(
        (0.48 * s, 0.47 * s),
        (0.68 * s, 0.58 * s),
        (0.63 * s, 0.78 * s),
        (0.30 * s, 0.72 * s),
        &mut points,
    );
    points
}

fn bezier(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    out: &mut Vec<(f32, f32)>,
) {
    const STEPS: usize = 80;
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let u = 1.0 - t;
        let x =
            u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
        let y =
            u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
        out.push((x, y));
    }
}

fn distance_to_polyline(point: (f32, f32), line: &[(f32, f32)]) -> f32 {
    let mut best = f32::INFINITY;
    for window in line.windows(2) {
        let d = distance_to_segment(point, window[0], window[1]);
        if d < best {
            best = d;
        }
    }
    best
}

fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = p;
    let (ax, ay) = a;
    let (bx, by) = b;
    let dx = bx - ax;
    let dy = by - ay;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= f32::EPSILON {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / length_sq).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [f32; 3] {
    [
        a[0] as f32 + (b[0] as f32 - a[0] as f32) * t,
        a[1] as f32 + (b[1] as f32 - a[1] as f32) * t,
        a[2] as f32 + (b[2] as f32 - a[2] as f32) * t,
    ]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
