//! Top view of the M6980X, drawn with shapes after the product photos (no product images are shipped): glossy rim
//! with a thumb and a finger recess, matte shell with the two main buttons, glossy centre strip with tilt wheel,
//! three LED bars, DPI and profile button, and two silver side buttons in the thumb recess.
//! Works in a 100 x 160 design space that is scaled into the given rect.

use std::f32::consts::PI;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, TextFormat, Ui, pos2, vec2,
};

use crate::theme::ACCENT;
use crate::widgets::centered_text;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Region {
    Left,
    Right,
    Middle,
    WheelUp,
    WheelDown,
    TiltLeft,
    TiltRight,
    Forward,
    Back,
    Dpi,
    Profile,
}

impl Region {
    /// Device button number (`names::BUTTON_IDS`); DPI and profile button cannot be assigned.
    pub fn button(self) -> Option<u8> {
        Some(match self {
            Region::Left => 0,
            Region::Right => 1,
            Region::Middle => 2,
            Region::Back => 3,
            Region::Forward => 4,
            Region::TiltLeft => 6,
            Region::TiltRight => 7,
            Region::WheelUp => 8,
            Region::WheelDown => 9,
            Region::Dpi | Region::Profile => return None,
        })
    }

    pub fn from_button(b: u8) -> Option<Region> {
        ALL.into_iter().find(|r| r.button() == Some(b))
    }
}

const ALL: [Region; 11] = [
    Region::Left,
    Region::Right,
    Region::Middle,
    Region::WheelUp,
    Region::WheelDown,
    Region::TiltLeft,
    Region::TiltRight,
    Region::Forward,
    Region::Back,
    Region::Dpi,
    Region::Profile,
];

pub struct Art {
    /// LED colour of the shown profile
    pub led: Color32,
    /// LEDs lit (= active DPI stage), counted from the bottom
    pub lit: usize,
    pub selected: Option<Region>,
}

const W: f32 = 100.0;
const H: f32 = 160.0;
const TOP: f32 = 4.0;
const LEN: f32 = 152.0;
const CX: f32 = 50.0;
/// glossy rim between the outline and the matte shell
const RIM: f32 = 2.2;
/// thumb recess (left) and finger recess (right): depth and range along the length
const THUMB: (f32, f32, f32) = (14.0, 0.22, 0.92);
const FINGER: (f32, f32, f32) = (15.0, 0.34, 0.96);
/// centre strip from the front edge to its rounded end (design y)
const STRIP_TOP: f32 = TOP;
const STRIP_END: f32 = 92.5;
/// seam between main buttons and palm: leaves the strip at the first fraction, reaches the shell edge at the second
const SEAM: (f32, f32) = (0.44, 0.50);

const WHEEL: Rect = Rect { min: pos2(44.2, 22.0), max: pos2(55.8, 46.0) };
const DPI: Rect = Rect { min: pos2(44.0, 57.5), max: pos2(56.0, 65.0) };
const PROFILE: Rect = Rect { min: pos2(45.0, 75.0), max: pos2(55.0, 85.0) };
const TILT_L: Pos2 = pos2(38.0, 34.0);
const TILT_R: Pos2 = pos2(62.0, 34.0);
const FORWARD: (f32, f32) = (0.31, 0.455);
const BACK: (f32, f32) = (0.47, 0.60);

const RIM_FILL: Color32 = Color32::from_rgb(0x11, 0x12, 0x15);
const MATTE_DARK: Color32 = Color32::from_rgb(0x1C, 0x1E, 0x22);
const MATTE_LIGHT: Color32 = Color32::from_rgb(0x34, 0x37, 0x3D);
const SILVER_DARK: Color32 = Color32::from_rgb(0x3A, 0x3D, 0x44);
const SILVER: Color32 = Color32::from_rgb(0xB4, 0xBA, 0xC3);
const SILVER_LIGHT: Color32 = Color32::from_rgb(0xC4, 0xC9, 0xD0);

fn y(u: f32) -> f32 {
    TOP + LEN * u
}

fn steps(from: f32, to: f32, n: usize) -> impl Iterator<Item = f32> {
    (0..=n).map(move |k| from + (to - from) * k as f32 / n as f32)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Rounded profile along the length: 0 at both ends, 1 in between; boxier at the front, rounder at the back.
fn envelope(u: f32) -> f32 {
    let p = if u < 0.5 { 4.0 } else { 2.6 };
    (1.0 - (2.0 * u - 1.0).abs().powf(p)).max(0.0).powf(1.0 / p)
}

/// Half-sine hump between `from` and `to` (pointed ends like the recesses on the photos).
fn hump(u: f32, (depth, from, to): (f32, f32, f32)) -> f32 {
    if u <= from || u >= to { 0.0 } else { depth * (PI * (u - from) / (to - from)).sin() }
}

/// Half width before the rounding of the ends: narrower at the front, the thumb rest flares out a little.
fn base_width(u: f32, side: f32) -> f32 {
    let flare = if side < 0.0 { hump(u, (1.5, 0.55, 1.0)) } else { 0.0 };
    37.5 + 8.0 * smooth(u) + flare
}

/// Outline x at fraction `u` of the length; `side` -1 = left, +1 = right.
fn outer(u: f32, side: f32) -> f32 {
    CX + side * base_width(u, side) * envelope(u)
}

/// Edge of the matte shell: the outline moved in by the rim, minus the recess on that side.
fn shell(u: f32, side: f32) -> f32 {
    let v = ((u - 0.014) / 0.972).clamp(0.0, 1.0);
    let recess = hump(u, if side < 0.0 { THUMB } else { FINGER });
    CX + side * ((base_width(u, side) - RIM) * envelope(v) - recess).max(0.0)
}

/// Half width of the centre strip at design height `y`: runs out at the front edge, tapers slightly, rounded end.
fn strip_half(yy: f32) -> f32 {
    if !(STRIP_TOP..=STRIP_END).contains(&yy) {
        return 0.0;
    }
    let w = 8.6 - 0.8 * (yy - STRIP_TOP) / (STRIP_END - STRIP_TOP);
    let round = |d: f32, r: f32| if d > 0.0 { (1.0 - (d / r).powi(2)).max(0.0).sqrt() } else { 1.0 };
    let body = (outer((yy - TOP) / LEN, 1.0) - CX - 0.6).max(0.0);
    (w * round(yy - (STRIP_END - w), w)).min(body)
}

/// A shape given as rows (design y, left x, right x) from front to back.
type Rows = Vec<(f32, f32, f32)>;

fn body_rows() -> Rows {
    steps(0.0, 1.0, 120).map(|u| (y(u), outer(u, -1.0), outer(u, 1.0))).collect()
}

fn shell_rows() -> Rows {
    steps(0.0, 1.0, 120).map(|u| (y(u), shell(u, -1.0), shell(u, 1.0))).filter(|&(_, l, r)| r > l).collect()
}

fn strip_rows() -> Rows {
    steps(STRIP_TOP, STRIP_END, 120).map(|yy| (yy, CX - strip_half(yy), CX + strip_half(yy))).collect()
}

/// Inner edge of a main button: along the strip, then along the seam out to the shell edge.
fn button_inner(u: f32, side: f32) -> f32 {
    let strip = CX + side * (strip_half(y(u)) + 0.4);
    strip + (shell(u, side) - strip) * smooth((u - SEAM.0) / (SEAM.1 - SEAM.0))
}

/// Main button; `side` -1 = left, +1 = right.
fn button_rows(side: f32) -> Rows {
    steps(0.0, SEAM.1, 60)
        .map(|u| {
            let (outer_x, inner_x) = (shell(u, side), button_inner(u, side));
            if side < 0.0 { (y(u), outer_x, inner_x) } else { (y(u), inner_x, outer_x) }
        })
        .filter(|&(_, l, r)| r - l > 0.05)
        .collect()
}

fn seam_line(side: f32) -> Vec<Pos2> {
    steps(SEAM.0, SEAM.1, 12).map(|u| pos2(button_inner(u, side), y(u))).collect()
}

/// Closed outline of a shape given as rows.
fn outline(rows: &[(f32, f32, f32)]) -> Vec<Pos2> {
    rows.iter().map(|&(yy, l, _)| pos2(l, yy)).chain(rows.iter().rev().map(|&(yy, _, r)| pos2(r, yy))).collect()
}

fn main_button(side: f32) -> Vec<Pos2> {
    outline(&button_rows(side))
}

/// Centre line of a side button, along the outline in the thumb recess.
fn side_x(u: f32) -> f32 {
    outer(u, -1.0) + 3.2
}

fn side_line((from, to): (f32, f32)) -> Vec<Pos2> {
    steps(from, to, 12).map(|u| pos2(side_x(u), y(u))).collect()
}

/// Side button as a slim pill around its centre line, `grow` larger on every side.
fn side_rows((from, to): (f32, f32), grow: f32) -> Rows {
    let (from, to) = (from - grow / LEN, to + grow / LEN);
    steps(from, to, 24)
        .map(|u| {
            let s = (u - from) / (to - from);
            let half = (1.6 + grow) * (1.0 - (2.0 * s - 1.0).abs().powi(8)).max(0.0).sqrt();
            (y(u), side_x(u) - half, side_x(u) + half)
        })
        .collect()
}

/// Tilt arrow pointing outwards.
fn triangle(c: Pos2, dir: f32) -> Vec<Pos2> {
    vec![pos2(c.x + 2.0 * dir, c.y), pos2(c.x - 1.2 * dir, c.y - 2.6), pos2(c.x - 1.2 * dir, c.y + 2.6)]
}

fn wheel_part(r: Region) -> Rect {
    let h = WHEEL.height();
    match r {
        Region::WheelUp => Rect::from_min_max(WHEEL.min, pos2(WHEEL.max.x, WHEEL.min.y + h * 0.3)),
        Region::WheelDown => Rect::from_min_max(pos2(WHEEL.min.x, WHEEL.max.y - h * 0.3), WHEEL.max),
        _ => Rect::from_min_max(pos2(WHEEL.min.x, WHEEL.min.y + h * 0.3), pos2(WHEEL.max.x, WHEEL.max.y - h * 0.3)),
    }
}

fn inside(poly: &[Pos2], p: Pos2) -> bool {
    let mut odd = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
        j = i;
    }
    odd
}

fn near_line(line: &[Pos2], p: Pos2, dist: f32) -> bool {
    line.windows(2).any(|s| {
        let (a, b) = (s[0], s[1]);
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
        (a + ab * t).distance(p) <= dist
    })
}

/// Region under a point of the design space; small regions first.
fn hit(p: Pos2) -> Option<Region> {
    let tilt = |c: Pos2| Rect::from_center_size(c, vec2(9.0, 11.0)).contains(p);
    if tilt(TILT_L) {
        return Some(Region::TiltLeft);
    }
    if tilt(TILT_R) {
        return Some(Region::TiltRight);
    }
    for r in [Region::WheelUp, Region::Middle, Region::WheelDown] {
        if wheel_part(r).contains(p) {
            return Some(r);
        }
    }
    if DPI.contains(p) {
        return Some(Region::Dpi);
    }
    if PROFILE.contains(p) {
        return Some(Region::Profile);
    }
    if near_line(&side_line(FORWARD), p, 3.0) {
        return Some(Region::Forward);
    }
    if near_line(&side_line(BACK), p, 3.0) {
        return Some(Region::Back);
    }
    if inside(&main_button(-1.0), p) {
        return Some(Region::Left);
    }
    if inside(&main_button(1.0), p) {
        return Some(Region::Right);
    }
    None
}

/// LED bar as a bent line: the upper two arch over the strip, the bottom one wraps around the profile button.
fn led_bar(i: usize) -> Vec<Pos2> {
    match i {
        0 => steps(-1.0, 1.0, 12).map(|t| pos2(CX + 6.8 * t, 52.0 - 1.4 * (1.0 - t * t))).collect(),
        1 => steps(-1.0, 1.0, 12).map(|t| pos2(CX + 6.6 * t, 70.5 - 1.4 * (1.0 - t * t))).collect(),
        _ => steps(-1.0, 1.0, 24)
            .map(|t| {
                let a = t * std::f32::consts::FRAC_PI_2 * 1.12;
                pos2(CX + 6.6 * a.sin(), 80.5 + 7.0 * a.cos())
            })
            .collect(),
    }
}

/// Mesh between the left and right edge of `rows`, split into `cols` columns so that `shade` can give every
/// vertex its own colour from its position across the row (0 = left edge, 1 = right edge) and its design point.
fn band(
    rows: &[(f32, f32, f32)],
    cols: usize,
    to: &dyn Fn(Pos2) -> Pos2,
    shade: &dyn Fn(f32, Pos2) -> Color32,
) -> Shape {
    let mut mesh = Mesh::default();
    for &(yy, l, r) in rows {
        for c in 0..=cols {
            let t = c as f32 / cols as f32;
            let p = pos2(l + (r - l) * t, yy);
            mesh.colored_vertex(to(p), shade(t, p));
        }
    }
    let n = cols as u32 + 1;
    for row in 0..rows.len().saturating_sub(1) as u32 {
        for c in 0..cols as u32 {
            let a = row * n + c;
            mesh.add_triangle(a, a + 1, a + n);
            mesh.add_triangle(a + 1, a + n + 1, a + n);
        }
    }
    Shape::mesh(mesh)
}

/// Draws the mouse into a rect of the given height; returns the region clicked this frame.
/// `hover` gives the tooltip for a region (None = no tooltip, not clickable).
pub fn show(ui: &mut Ui, height: f32, art: &Art, hover: &dyn Fn(Region) -> Option<String>) -> Option<Region> {
    let size = vec2(height * W / H, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let s = rect.width() / W;
    let to = |p: Pos2| rect.min + vec2(p.x, p.y) * s;
    let pts = |v: Vec<Pos2>| v.into_iter().map(to).collect::<Vec<_>>();
    let from = |p: Pos2| pos2((p.x - rect.min.x) / s, (p.y - rect.min.y) / s);

    let hovered = response.hover_pos().map(from).and_then(hit).filter(|r| hover(*r).is_some());
    let painter = ui.painter_at(rect.expand(4.0));
    let flat = |c: Color32| move |_: f32, _: Pos2| c;

    // cable stub, soft shadow
    painter.line_segment([to(pos2(CX, 0.0)), to(pos2(CX, y(0.0) + 2.0))], Stroke::new(3.0 * s, RIM_FILL));
    painter.line_segment(
        [to(pos2(CX - 0.6, 0.0)), to(pos2(CX - 0.6, y(0.0)))],
        Stroke::new(0.6 * s, Color32::from_white_alpha(30)),
    );
    let body = body_rows();
    for (grow, alpha) in [(3.0, 14), (2.0, 22), (1.0, 34)] {
        let shadow: Rows = body
            .iter()
            .map(|&(yy, l, r)| (80.0 + (yy - 80.0) * (1.0 + grow / 76.0) + 2.0, l - grow, r + grow))
            .collect();
        painter.add(band(&shadow, 1, &to, &flat(Color32::from_black_alpha(alpha))));
    }

    // glossy rim and recesses
    painter.add(band(&body, 1, &to, &flat(RIM_FILL)));
    painter.add(Shape::closed_line(pts(outline(&body)), Stroke::new(1.0, Color32::from_rgb(0x08, 0x08, 0x0A))));

    // wavy grip in the lower part of the thumb recess
    for k in 0..6 {
        let u0 = 0.62 + 0.048 * k as f32;
        let (xo, xi) = (outer(u0, -1.0) + 1.0, shell(u0 + 0.03, -1.0) - 0.6);
        if xi - xo < 2.0 {
            continue;
        }
        let wave = |dy: f32| {
            steps(0.0, 1.0, 12)
                .map(|t| to(pos2(xo + (xi - xo) * t, y(u0) + 4.5 * t + 1.0 * (2.0 * PI * t).sin() + dy)))
                .collect::<Vec<_>>()
        };
        painter.add(Shape::line(wave(0.0), Stroke::new(1.1 * s, Color32::from_rgb(0x22, 0x24, 0x29))));
        painter.add(Shape::line(wave(0.8), Stroke::new(0.5 * s, Color32::from_white_alpha(14))));
    }

    // matte shell: domed, lit from the front left
    let shell_body = shell_rows();
    let dome = |_: f32, p: Pos2| {
        let (dx, dy) = ((p.x - CX) / 46.0, (p.y - 72.0) / 92.0);
        let d = (1.0 - dx * dx - dy * dy).max(0.0);
        let light = (1.0 - p.y / H) * 0.25 + (1.0 - p.x / W) * 0.1;
        MATTE_DARK.lerp_to_gamma(MATTE_LIGHT, (d * 0.75 + light).min(1.0))
    };
    painter.add(band(&shell_body, 10, &to, &dome));
    painter.add(Shape::closed_line(pts(outline(&shell_body)), Stroke::new(1.0, Color32::from_white_alpha(26))));

    // reflections on the glossy rim
    let gloss = |side: f32, from: f32, to_u: f32, inset: f32| {
        steps(from, to_u, 20).map(|u| to(pos2(outer(u, side) - side * inset, y(u)))).collect::<Vec<_>>()
    };
    painter.add(Shape::line(gloss(-1.0, 0.05, 0.3, 0.9), Stroke::new(0.7 * s, Color32::from_white_alpha(60))));
    painter.add(Shape::line(gloss(1.0, 0.08, 0.42, 0.9), Stroke::new(0.6 * s, Color32::from_white_alpha(38))));
    painter.add(Shape::line(gloss(1.0, 0.62, 0.88, 0.8), Stroke::new(0.5 * s, Color32::from_white_alpha(22))));
    let recess_edge = |side: f32, (_, from, to_u): (f32, f32, f32)| {
        steps(from + 0.03, to_u - 0.03, 24).map(|u| to(pos2(shell(u, side) - side * 0.5, y(u)))).collect::<Vec<_>>()
    };
    painter.add(Shape::line(recess_edge(-1.0, THUMB), Stroke::new(0.5 * s, Color32::from_white_alpha(40))));
    painter.add(Shape::line(recess_edge(1.0, FINGER), Stroke::new(0.5 * s, Color32::from_white_alpha(40))));

    // highlight of hovered / selected regions
    let highlight = |r: Region, fill: Color32| {
        let stroke = Stroke::new(1.5, ACCENT);
        match r {
            Region::Left | Region::Right => {
                let rows = button_rows(if r == Region::Left { -1.0 } else { 1.0 });
                painter.add(band(&rows, 1, &to, &flat(fill)));
                painter.add(Shape::closed_line(pts(outline(&rows)), stroke));
            }
            Region::Forward | Region::Back => {
                let rows = side_rows(if r == Region::Forward { FORWARD } else { BACK }, 1.4);
                painter.add(band(&rows, 1, &to, &flat(fill)));
                painter.add(Shape::closed_line(pts(outline(&rows)), Stroke::new(1.0, ACCENT.gamma_multiply(0.7))));
            }
            Region::TiltLeft | Region::TiltRight => {
                let c = if r == Region::TiltLeft { TILT_L } else { TILT_R };
                painter.circle(to(c), 5.0 * s, fill, stroke);
            }
            Region::Middle | Region::WheelUp | Region::WheelDown => {
                let part = wheel_part(r);
                painter.rect(
                    Rect::from_min_max(to(part.min), to(part.max)).expand(1.5 * s),
                    2.0 * s,
                    fill,
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }
            Region::Dpi | Region::Profile => {
                let b = if r == Region::Dpi { DPI } else { PROFILE };
                let b = Rect::from_min_max(to(b.min), to(b.max));
                painter.rect(b.expand(1.0 * s), 2.0 * s, fill, stroke, egui::StrokeKind::Outside);
            }
        }
    };
    if let Some(r @ (Region::Left | Region::Right)) = art.selected {
        highlight(r, ACCENT.gamma_multiply(0.35));
    }
    if let Some(r @ (Region::Left | Region::Right)) = hovered
        && Some(r) != art.selected
    {
        highlight(r, ACCENT.gamma_multiply(0.15));
    }

    // seams between the main buttons and the palm
    for side in [-1.0, 1.0] {
        painter.add(Shape::line(pts(seam_line(side)), Stroke::new(0.9 * s, Color32::from_rgb(0x0C, 0x0D, 0x0F))));
        let lit: Vec<Pos2> = seam_line(side).into_iter().map(|p| to(p + vec2(0.0, 0.7))).collect();
        painter.add(Shape::line(lit, Stroke::new(0.5 * s, Color32::from_white_alpha(18))));
    }

    // glossy centre strip with a reflection along its left side and a chrome edge
    let strip = strip_rows();
    let strip_shade = |t: f32, _: Pos2| {
        let streak = (-((t - 0.22) / 0.09).powi(2)).exp();
        Color32::from_rgb(0x0A, 0x0B, 0x0D).lerp_to_gamma(Color32::from_rgb(0x34, 0x37, 0x3D), streak * 0.8)
    };
    painter.add(band(&strip, 12, &to, &strip_shade));
    painter.add(Shape::closed_line(pts(outline(&strip)), Stroke::new(1.0, Color32::from_rgb(0x5A, 0x5E, 0x66))));

    // wheel in its slot: a ribbed cylinder, bright in the middle, ridges closer together towards the ends
    let wheel = Rect::from_min_max(to(WHEEL.min), to(WHEEL.max));
    painter.rect_filled(wheel.expand(1.2 * s), 2.5 * s, Color32::from_rgb(0x04, 0x04, 0x05));
    let wheel_rows: Rows = steps(WHEEL.min.y, WHEEL.max.y, 32).map(|yy| (yy, WHEEL.min.x, WHEEL.max.x)).collect();
    let cylinder = |t: f32, p: Pos2| {
        let v = (p.y - WHEEL.min.y) / WHEEL.height();
        let edge = 1.0 - (2.0 * t - 1.0).powi(4) * 0.4;
        Color32::from_rgb(0x0E, 0x0F, 0x12)
            .lerp_to_gamma(Color32::from_rgb(0x62, 0x66, 0x6E), (PI * v).sin() * edge * 0.85)
    };
    painter.add(band(&wheel_rows, 6, &to, &cylinder));
    for k in 1..18 {
        let a = PI * k as f32 / 18.0;
        let ry = WHEEL.center().y - WHEEL.height() / 2.0 * a.cos();
        painter.line_segment(
            [to(pos2(WHEEL.min.x + 0.5, ry)), to(pos2(WHEEL.max.x - 0.5, ry))],
            Stroke::new((0.7 * s * a.sin()).max(0.6), Color32::from_black_alpha(190)),
        );
    }
    for r in [Region::WheelUp, Region::Middle, Region::WheelDown, Region::Dpi, Region::Profile] {
        if art.selected == Some(r) {
            highlight(r, ACCENT.gamma_multiply(0.45));
        } else if hovered == Some(r) {
            highlight(r, ACCENT.gamma_multiply(0.2));
        }
    }

    // tilt arrows
    for (c, dir, r) in [(TILT_L, -1.0, Region::TiltLeft), (TILT_R, 1.0, Region::TiltRight)] {
        if art.selected == Some(r) {
            highlight(r, ACCENT.gamma_multiply(0.45));
        } else if hovered == Some(r) {
            highlight(r, ACCENT.gamma_multiply(0.2));
        }
        painter.add(Shape::convex_polygon(pts(triangle(c, dir)), SILVER, Stroke::new(1.0, SILVER_DARK)));
    }

    // LED bars: lit from the bottom up, with a glow
    for i in 0..3 {
        let bar = pts(led_bar(i));
        if 2 - i < art.lit {
            for (width, k) in [(6.0, 0.1), (3.8, 0.22)] {
                painter.add(Shape::line(bar.clone(), Stroke::new(width * s, art.led.gamma_multiply(k))));
            }
            painter.add(Shape::line(bar.clone(), Stroke::new(2.0 * s, art.led)));
            painter.add(Shape::line(bar, Stroke::new(0.7 * s, art.led.lerp_to_gamma(Color32::WHITE, 0.55))));
        } else {
            painter.add(Shape::line(bar, Stroke::new(2.0 * s, Color32::from_rgb(0x24, 0x28, 0x30))));
        }
    }

    // DPI and profile button: glossy keys
    let key_edge = Stroke::new(1.0, Color32::from_rgb(0x3C, 0x3F, 0x47));
    let dpi = Rect::from_min_max(to(DPI.min), to(DPI.max));
    painter.rect(dpi, 1.2 * s, Color32::from_rgb(0x15, 0x16, 0x19), key_edge, egui::StrokeKind::Inside);
    let format =
        TextFormat { font_id: FontId::proportional(4.8 * s), color: SILVER, italics: true, ..Default::default() };
    let label = painter.layout_job(LayoutJob::single_section("DPI".to_owned(), format));
    painter.galley(dpi.center() - label.size() / 2.0, label, SILVER);
    let profile = Rect::from_min_max(to(PROFILE.min), to(PROFILE.max));
    let (top, bottom) = ((1.2 * s) as u8, (4.5 * s) as u8);
    let radius = CornerRadius { nw: top, ne: top, sw: bottom, se: bottom };
    painter.rect(profile, radius, Color32::from_rgb(0x15, 0x16, 0x19), key_edge, egui::StrokeKind::Inside);
    centered_text(ui, profile.center(), egui_phosphor::regular::ARROWS_CLOCKWISE, 6.0 * s, SILVER);
    for key in [dpi, profile] {
        painter.line_segment(
            [key.left_top() + vec2(1.5 * s, 0.9 * s), key.right_top() + vec2(-1.5 * s, 0.9 * s)],
            Stroke::new(0.5 * s, Color32::from_white_alpha(40)),
        );
    }

    // side buttons: silver pills, hovered with a glow, selected in the accent colour
    for (range, r) in [(FORWARD, Region::Forward), (BACK, Region::Back)] {
        if hovered == Some(r) && art.selected != Some(r) {
            highlight(r, ACCENT.gamma_multiply(0.3));
        }
        let (dark, light) = if art.selected == Some(r) {
            (ACCENT.lerp_to_gamma(Color32::BLACK, 0.4), ACCENT.lerp_to_gamma(Color32::WHITE, 0.5))
        } else {
            (SILVER_DARK, SILVER_LIGHT)
        };
        let pill = |t: f32, _: Pos2| dark.lerp_to_gamma(light, (1.0 - ((t - 0.25) / 0.6).powi(2)).max(0.0));
        let rows = side_rows(range, 0.0);
        painter.add(band(&rows, 6, &to, &pill));
        painter.add(Shape::closed_line(pts(outline(&rows)), Stroke::new(1.0, Color32::from_rgb(0x0A, 0x0A, 0x0C))));
    }

    if let Some(r) = hovered {
        if let Some(text) = hover(r) {
            response.clone().on_hover_text_at_pointer(text);
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() { hovered } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_assignable_button_has_a_region() {
        for (b, _) in m6980x_core::names::BUTTON_IDS {
            let r = Region::from_button(b).expect("region");
            assert_eq!(r.button(), Some(b));
        }
    }

    #[test]
    fn hit_test_finds_regions() {
        assert_eq!(hit(pos2(25.0, 30.0)), Some(Region::Left));
        assert_eq!(hit(pos2(75.0, 30.0)), Some(Region::Right));
        assert_eq!(hit(wheel_part(Region::WheelUp).center()), Some(Region::WheelUp));
        assert_eq!(hit(WHEEL.center()), Some(Region::Middle));
        assert_eq!(hit(wheel_part(Region::WheelDown).center()), Some(Region::WheelDown));
        assert_eq!(hit(TILT_L), Some(Region::TiltLeft));
        assert_eq!(hit(TILT_R), Some(Region::TiltRight));
        assert_eq!(hit(DPI.center()), Some(Region::Dpi));
        assert_eq!(hit(PROFILE.center()), Some(Region::Profile));
        assert_eq!(hit(side_line(BACK)[6]), Some(Region::Back));
        assert_eq!(hit(side_line(FORWARD)[6]), Some(Region::Forward));
        // palm, lower thumb recess, outside the body
        assert_eq!(hit(pos2(50.0, 140.0)), None);
        assert_eq!(hit(pos2(outer(0.8, -1.0) + 3.0, y(0.8))), None);
        assert_eq!(hit(pos2(50.0, 1.0)), None);
    }

    #[test]
    fn parts_lie_on_the_body() {
        for u in steps(FORWARD.0, BACK.1, 40) {
            assert!(outer(u, -1.0) < side_x(u) - 1.7 && side_x(u) + 1.7 < shell(u, -1.0), "side buttons at {u}");
        }
        for yy in steps(STRIP_TOP, STRIP_END, 40) {
            let u = (yy - TOP) / LEN;
            assert!(outer(u, -1.0) <= CX - strip_half(yy) && CX + strip_half(yy) <= outer(u, 1.0), "strip at {yy}");
        }
        for i in 0..3 {
            assert!(led_bar(i).iter().all(|p| (p.x - CX).abs() + 1.0 < strip_half(p.y)), "LED bar {i}");
        }
    }
}
