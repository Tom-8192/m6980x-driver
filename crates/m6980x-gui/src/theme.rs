//! Colours, fonts and spacing of the window (dark and light theme).

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Theme, ThemePreference, Visuals,
};

/// Accent colour (close to the blue LEDs of the mouse).
pub const ACCENT: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
pub const DANGER: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
pub const SUCCESS: Color32 = Color32::from_rgb(0x30, 0xA4, 0x6C);
pub const WARNING: Color32 = Color32::from_rgb(0xE8, 0xA3, 0x17);
pub const RADIUS: u8 = 6;
pub const CARD_RADIUS: u8 = 10;

/// Theme-dependent colours that egui's `Visuals` have no slot for.
pub struct Palette {
    pub card: Color32,
    pub card_stroke: Color32,
    pub sidebar: Color32,
    pub muted: Color32,
    pub accent_soft: Color32,
    pub strong: Color32,
}

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.global_style().visuals.dark_mode {
        Palette {
            card: Color32::from_rgb(0x1F, 0x22, 0x29),
            card_stroke: Color32::from_rgb(0x2C, 0x30, 0x39),
            sidebar: Color32::from_rgb(0x13, 0x15, 0x1A),
            muted: Color32::from_rgb(0x8B, 0x92, 0xA1),
            accent_soft: ACCENT.gamma_multiply(0.22),
            strong: Color32::from_rgb(0xEE, 0xF0, 0xF4),
        }
    } else {
        Palette {
            card: Color32::WHITE,
            card_stroke: Color32::from_rgb(0xDF, 0xE2, 0xE8),
            sidebar: Color32::from_rgb(0xEB, 0xED, 0xF1),
            muted: Color32::from_rgb(0x67, 0x6E, 0x7B),
            accent_soft: ACCENT.gamma_multiply(0.15),
            strong: Color32::from_rgb(0x16, 0x18, 0x1D),
        }
    }
}

pub fn install(ctx: &egui::Context, preference: ThemePreference) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    ctx.set_visuals_of(Theme::Dark, dark());
    ctx.set_visuals_of(Theme::Light, light());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(21.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 8.0);
        s.button_padding = egui::vec2(10.0, 5.0);
        s.interact_size.y = 26.0;
        s.slider_width = 260.0;
        s.menu_margin = Margin::same(8);
    });
    ctx.set_theme(preference);
}

fn widgets(v: &mut Visuals, inactive: Color32, hovered: Color32, active: Color32, stroke: Color32, text: Color32) {
    let r = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    for (wv, fill) in [(&mut w.inactive, inactive), (&mut w.hovered, hovered), (&mut w.active, active)] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.corner_radius = r;
        wv.expansion = 0.0;
    }
    w.open.corner_radius = r;
    w.noninteractive.corner_radius = r;
    w.inactive.bg_stroke = Stroke::new(1.0, stroke);
    w.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.7));
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.inactive.fg_stroke = Stroke::new(1.0, text);
    w.hovered.fg_stroke = Stroke::new(1.5, text);
    w.active.fg_stroke = Stroke::new(1.5, text);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(8);
    v.slider_trailing_fill = true;
    v.error_fg_color = DANGER;
    v.warn_fg_color = WARNING;
}

fn dark() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x17, 0x19, 0x1E);
    v.window_fill = Color32::from_rgb(0x1F, 0x22, 0x29);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(0x2C, 0x30, 0x39));
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x14, 0x18);
    v.faint_bg_color = Color32::from_rgb(0x24, 0x27, 0x2F);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2C, 0x30, 0x39));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(0xC9, 0xCE, 0xD8));
    let text = Color32::from_rgb(0xEE, 0xF0, 0xF4);
    let (i, h, a) =
        (Color32::from_rgb(0x2A, 0x2E, 0x37), Color32::from_rgb(0x33, 0x38, 0x43), Color32::from_rgb(0x3B, 0x41, 0x4E));
    widgets(&mut v, i, h, a, Color32::from_rgb(0x36, 0x3B, 0x46), text);
    v
}

fn light() -> Visuals {
    let mut v = Visuals::light();
    v.panel_fill = Color32::from_rgb(0xF5, 0xF6, 0xF8);
    v.window_fill = Color32::WHITE;
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(0xDF, 0xE2, 0xE8));
    v.extreme_bg_color = Color32::from_rgb(0xFA, 0xFB, 0xFC);
    v.faint_bg_color = Color32::from_rgb(0xF0, 0xF2, 0xF5);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xDF, 0xE2, 0xE8));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3F, 0x4A));
    let text = Color32::from_rgb(0x16, 0x18, 0x1D);
    let (i, h, a) =
        (Color32::from_rgb(0xEC, 0xEE, 0xF2), Color32::from_rgb(0xE2, 0xE6, 0xEC), Color32::from_rgb(0xD6, 0xDB, 0xE3));
    widgets(&mut v, i, h, a, Color32::from_rgb(0xD3, 0xD7, 0xDF), text);
    v
}

/// LED level 0/1/2 -> 8-bit channel for drawing.
pub fn led_channel(level: u8) -> u8 {
    match level {
        0 => 0x00,
        1 => 0x90,
        _ => 0xFF,
    }
}

pub fn led_color(rgb: [u8; 3]) -> Color32 {
    let [r, g, b] = rgb.map(led_channel);
    Color32::from_rgb(r, g, b)
}

pub fn theme_id(p: ThemePreference) -> &'static str {
    match p {
        ThemePreference::Dark => "dark",
        ThemePreference::Light => "light",
        ThemePreference::System => "system",
    }
}

pub fn theme_from_id(id: &str) -> ThemePreference {
    match id {
        "light" => ThemePreference::Light,
        "system" => ThemePreference::System,
        _ => ThemePreference::Dark,
    }
}
