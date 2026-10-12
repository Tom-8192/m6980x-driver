//! Schematic drawing of the M6980X (drawn with shapes, no product images): two main buttons, a centre strip
//! with tilt wheel, three LED bars, DPI and profile button, two side buttons on the left.
//! Works in a 100 x 160 design space that is scaled into the given rect.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};

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
const STRIP: f32 = 7.0;
/// where the main buttons end (fraction of the length)
const SPLIT: f32 = 0.46;

const WHEEL: Rect = Rect { min: pos2(45.5, 20.0), max: pos2(54.5, 44.0) };
const DPI: Rect = Rect { min: pos2(44.5, 53.0), max: pos2(55.5, 61.0) };
const PROFILE: Rect = Rect { min: pos2(44.5, 67.0), max: pos2(55.5, 77.0) };
const TILT_L: Pos2 = pos2(38.5, 32.0);
const TILT_R: Pos2 = pos2(61.5, 32.0);
const FORWARD: (f32, f32) = (0.35, 0.465);
const BACK: (f32, f32) = (0.485, 0.60);

fn y(u: f32) -> f32 {
    TOP + LEN * u
}

/// Half width of the body at fraction `u` of the length: egg shape, wider at the back.
fn half_width(u: f32) -> f32 {
    let p = 2.4_f32;
    let e = (1.0 - (2.0 * u - 1.0).abs().powf(p)).max(0.0).powf(1.0 / p);
    (34.0 + 10.0 * u) * e
}

fn steps(from: f32, to: f32, n: usize) -> impl Iterator<Item = f32> {
    (0..=n).map(move |k| from + (to - from) * k as f32 / n as f32)
}

fn body() -> Vec<Pos2> {
    let right = steps(0.0, 1.0, 60).map(|u| pos2(CX + half_width(u), y(u)));
    let left: Vec<Pos2> = steps(1.0, 0.0, 60).map(|u| pos2(CX - half_width(u), y(u))).collect();
    right.chain(left).collect()
}

/// Main button polygon; `side` -1 = left, +1 = right.
fn main_button(side: f32) -> Vec<Pos2> {
    let mut pts = vec![pos2(CX + side * STRIP, y(SPLIT))];
    pts.extend(steps(SPLIT, 0.0, 30).map(|u| pos2(CX + side * half_width(u).max(STRIP), y(u))));
    pts
}

/// Side button: line along the left edge.
fn side_line((from, to): (f32, f32)) -> Vec<Pos2> {
    steps(from, to, 12).map(|u| pos2(CX - half_width(u) + 2.8, y(u))).collect()
}

fn triangle(c: Pos2, dir: f32) -> Vec<Pos2> {
    vec![pos2(c.x + 3.0 * dir, c.y), pos2(c.x - 2.0 * dir, c.y - 3.5), pos2(c.x - 2.0 * dir, c.y + 3.5)]
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
    if near_line(&side_line(FORWARD), p, 4.0) {
        return Some(Region::Forward);
    }
    if near_line(&side_line(BACK), p, 4.0) {
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

/// LED bar as a slightly bent line (the bottom one wraps around the profile button).
fn led_bar(i: usize) -> Vec<Pos2> {
    match i {
        0 => steps(-1.0, 1.0, 10).map(|t| pos2(CX + 6.0 * t, 50.0 - 1.2 * (1.0 - t * t))).collect(),
        1 => steps(-1.0, 1.0, 10).map(|t| pos2(CX + 6.0 * t, 64.0 - 1.2 * (1.0 - t * t))).collect(),
        _ => steps(-1.0, 1.0, 16)
            .map(|t| {
                let a = t * std::f32::consts::FRAC_PI_2 * 1.1;
                pos2(CX + 6.5 * a.sin(), 75.0 + 6.0 * a.cos())
            })
            .collect(),
    }
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

    let body_fill = Color32::from_rgb(0x2A, 0x2C, 0x31);
    let edge = Color32::from_rgb(0x4A, 0x4E, 0x57);
    let gloss = Color32::from_rgb(0x12, 0x13, 0x16);
    let silver = Color32::from_rgb(0x9C, 0xA2, 0xAB);
    let unlit = Color32::from_rgb(0x30, 0x33, 0x3A);

    // shadow + body
    let shadow: Vec<Pos2> = body().into_iter().map(|p| to(p) + vec2(0.0, 3.0 * s)).collect();
    painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha(60), Stroke::NONE));
    painter.add(Shape::convex_polygon(pts(body()), body_fill, Stroke::new(1.5, edge)));

    // grips on both sides (cosmetic)
    let grip = |side: f32, from: f32, to_u: f32| {
        steps(from, to_u, 16).map(|u| to(pos2(CX + side * (half_width(u) - 3.5), y(u)))).collect::<Vec<_>>()
    };
    let grip_stroke = Stroke::new(4.0 * s, Color32::from_rgb(0x1E, 0x20, 0x24));
    painter.add(Shape::line(grip(1.0, 0.42, 0.82), grip_stroke));
    painter.add(Shape::line(grip(-1.0, 0.63, 0.84), grip_stroke));

    // highlight of hovered / selected regions
    let highlight = |r: Region, fill: Color32| {
        let stroke = Stroke::new(1.5, ACCENT);
        match r {
            Region::Left | Region::Right => {
                let side = if r == Region::Left { -1.0 } else { 1.0 };
                painter.add(Shape::convex_polygon(pts(main_button(side)), fill, stroke));
            }
            Region::Forward | Region::Back => {
                let line = side_line(if r == Region::Forward { FORWARD } else { BACK });
                painter.add(Shape::line(pts(line), Stroke::new(8.0 * s, fill)));
            }
            Region::TiltLeft | Region::TiltRight => {
                let c = if r == Region::TiltLeft { TILT_L } else { TILT_R };
                painter.circle(to(c), 6.0 * s, fill, stroke);
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
    if let Some(r) = art.selected
        && !matches!(r, Region::Middle | Region::WheelUp | Region::WheelDown | Region::Dpi | Region::Profile)
    {
        highlight(r, ACCENT.gamma_multiply(0.4));
    }
    if let Some(r) = hovered
        && Some(r) != art.selected
        && matches!(r, Region::Left | Region::Right)
    {
        highlight(r, ACCENT.gamma_multiply(0.18));
    }

    // seams between the main buttons and the palm
    let seam = |side: f32| {
        steps(0.0, 1.0, 8)
            .map(|t| {
                let x0 = CX + side * STRIP;
                let x1 = CX + side * (half_width(SPLIT) - 0.5);
                to(pos2(x0 + (x1 - x0) * t, y(SPLIT) - 3.0 * (t * std::f32::consts::PI).sin()))
            })
            .collect::<Vec<_>>()
    };
    painter.add(Shape::line(seam(-1.0), Stroke::new(1.2, edge)));
    painter.add(Shape::line(seam(1.0), Stroke::new(1.2, edge)));

    // centre strip
    let strip = Rect::from_min_max(to(pos2(CX - STRIP, 12.0)), to(pos2(CX + STRIP, 88.0)));
    painter.rect(strip, 6.0 * s, gloss, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    painter.line_segment(
        [strip.left_top() + vec2(2.5 * s, 8.0 * s), strip.left_bottom() + vec2(2.5 * s, -8.0 * s)],
        Stroke::new(1.0, Color32::from_white_alpha(18)),
    );

    // wheel with ridges
    let wheel = Rect::from_min_max(to(WHEEL.min), to(WHEEL.max));
    painter.rect_filled(wheel, 3.0 * s, Color32::from_rgb(0x3A, 0x3D, 0x44));
    let mut ry = WHEEL.min.y + 2.0;
    while ry < WHEEL.max.y - 1.0 {
        painter.line_segment(
            [to(pos2(WHEEL.min.x + 1.0, ry)), to(pos2(WHEEL.max.x - 1.0, ry))],
            Stroke::new(1.0, Color32::from_rgb(0x24, 0x26, 0x2B)),
        );
        ry += 2.4;
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
        painter.add(Shape::convex_polygon(pts(triangle(c, dir)), silver, Stroke::NONE));
    }

    // LED bars: lit from the bottom up
    for i in 0..3 {
        let on = 2 - i < art.lit;
        let color = if on { art.led } else { unlit };
        if on {
            painter.add(Shape::line(pts(led_bar(i)), Stroke::new(5.0 * s, color.gamma_multiply(0.25))));
        }
        painter.add(Shape::line(pts(led_bar(i)), Stroke::new(1.8 * s, color)));
    }

    // DPI and profile button
    let dpi = Rect::from_min_max(to(DPI.min), to(DPI.max));
    painter.rect_filled(dpi, 1.5 * s, Color32::from_rgb(0x22, 0x24, 0x29));
    centered_text(ui, dpi.center(), "DPI", 4.6 * s, silver);
    let profile = Rect::from_min_max(to(PROFILE.min), to(PROFILE.max));
    painter.rect_filled(profile, 1.5 * s, Color32::from_rgb(0x22, 0x24, 0x29));
    centered_text(ui, profile.center(), egui_phosphor::regular::ARROWS_CLOCKWISE, 6.0 * s, silver);

    // side buttons
    for (range, r) in [(FORWARD, Region::Forward), (BACK, Region::Back)] {
        painter.add(Shape::line(pts(side_line(range)), Stroke::new(3.6 * s, silver)));
        if hovered == Some(r) && art.selected != Some(r) {
            painter.add(Shape::line(pts(side_line(range)), Stroke::new(7.0 * s, ACCENT.gamma_multiply(0.3))));
        }
    }
    if let Some(r @ (Region::Forward | Region::Back)) = art.selected {
        let line = side_line(if r == Region::Forward { FORWARD } else { BACK });
        painter.add(Shape::line(pts(line), Stroke::new(3.6 * s, ACCENT)));
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
        assert_eq!(hit(WHEEL.center()), Some(Region::Middle));
        assert_eq!(hit(TILT_L), Some(Region::TiltLeft));
        assert_eq!(hit(DPI.center()), Some(Region::Dpi));
        assert_eq!(hit(side_line(BACK)[6]), Some(Region::Back));
        assert_eq!(hit(side_line(FORWARD)[6]), Some(Region::Forward));
        assert_eq!(hit(pos2(50.0, 140.0)), None);
    }
}
