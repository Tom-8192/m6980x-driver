//! Small building blocks shared by the pages: cards, headers, segmented buttons, colour swatches.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, Ui};

use crate::theme::{self, ACCENT, CARD_RADIUS, DANGER};

/// Framed card that fills the available width.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Card title with icon and an optional line of explanation.
pub fn card_title(ui: &mut Ui, icon: &str, title: &str, subtitle: Option<&str>) {
    card_title_in(ui, icon, ACCENT, title, subtitle);
}

/// `card_title` with another icon colour (e.g. red for actions that delete data).
pub fn card_title_in(ui: &mut Ui, icon: &str, color: Color32, title: &str, subtitle: Option<&str>) {
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(18.0).color(color));
        ui.label(RichText::new(title).size(16.0).strong().color(p.strong));
    });
    if let Some(s) = subtitle {
        ui.label(RichText::new(s).color(p.muted));
    }
    ui.add_space(4.0);
}

pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(title).heading().strong().color(p.strong));
    ui.label(RichText::new(subtitle).color(p.muted));
    ui.add_space(10.0);
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(text.into()).color(p.muted))
}

/// Row of connected toggle buttons; returns true when the user picked another option.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, String)]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let last = options.len().saturating_sub(1);
        for (i, (value, label)) in options.iter().enumerate() {
            let selected = *current == *value;
            let r = theme::RADIUS;
            let radius = CornerRadius {
                nw: if i == 0 { r } else { 0 },
                sw: if i == 0 { r } else { 0 },
                ne: if i == last { r } else { 0 },
                se: if i == last { r } else { 0 },
            };
            let text = if selected { RichText::new(label).color(Color32::WHITE) } else { RichText::new(label) };
            let mut button = egui::Button::new(text).corner_radius(radius).min_size(egui::vec2(0.0, 28.0));
            if selected {
                button = button.fill(ACCENT).stroke(Stroke::new(1.0, ACCENT));
            }
            if ui.add(button).clicked() && !selected {
                *current = *value;
                changed = true;
            }
        }
    });
    changed
}

/// Round colour field; a ring marks the selected one.
pub fn swatch(ui: &mut Ui, color: Color32, selected: bool, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());
    let painter = ui.painter();
    let c = rect.center();
    let radius = size / 2.0 - 3.0;
    let ring = if selected {
        Stroke::new(2.0, ACCENT)
    } else if response.hovered() {
        Stroke::new(1.5, ui.visuals().widgets.hovered.bg_stroke.color)
    } else {
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
    };
    painter.circle(c, radius, color, Stroke::NONE);
    painter.circle_stroke(c, radius + 2.0, ring);
    if color == Color32::BLACK {
        // "off": draw a slash so it does not look like an empty hole
        let d = radius * 0.6;
        painter.line_segment([c + egui::vec2(-d, d), c + egui::vec2(d, -d)], Stroke::new(1.5, DANGER));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Value tile for the overview.
pub fn stat_tile(ui: &mut Ui, icon: &str, label: &str, value: &str, accent: Option<Color32>) {
    let p = theme::palette(ui.ctx());
    egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(CornerRadius::same(8)).inner_margin(12).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(22.0).color(accent.unwrap_or(ACCENT)));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(label).small().color(p.muted));
                    ui.label(RichText::new(value).size(17.0).strong().color(p.strong));
                });
            });
        },
    );
}

pub fn accent_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(egui::Button::new(RichText::new(text.into()).color(Color32::WHITE)).fill(ACCENT))
}

pub fn danger_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(egui::Button::new(RichText::new(text.into()).color(Color32::WHITE)).fill(DANGER))
}

/// Button with an icon in front of the text.
pub fn icon_button(ui: &mut Ui, icon: &str, text: &str) -> Response {
    ui.button(format!("{icon}  {text}"))
}

/// Small frameless icon button (list row actions).
pub fn icon_only(ui: &mut Ui, icon: &str, hint: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(icon).size(15.0)).frame(false)).on_hover_text(hint)
}

/// Small rounded label, e.g. "active".
pub fn badge(ui: &mut Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.0), color);
    let size = galley.size() + egui::vec2(12.0, 4.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), color.gamma_multiply(0.18));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Row of LED dots (lit = `lit` from the bottom up), as on the mouse.
pub fn led_dots(ui: &mut Ui, lit: usize, total: usize, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total as f32 * 12.0, 12.0), Sense::hover());
    let off = ui.visuals().widgets.inactive.bg_fill;
    for i in 0..total {
        let c = egui::pos2(rect.left() + 6.0 + i as f32 * 12.0, rect.center().y);
        ui.painter().circle_filled(c, 4.0, if i < lit { color } else { off });
    }
}

/// Text drawn with a centre anchor (used by the mouse drawing).
pub fn centered_text(ui: &Ui, pos: egui::Pos2, text: &str, size: f32, color: Color32) {
    ui.painter().text(pos, Align2::CENTER_CENTER, text, FontId::proportional(size), color);
}

/// "light-blue" -> "Light blue" for names that come from command-line words.
pub fn nice(name: &str) -> String {
    let name = name.replace('-', " ");
    let mut chars = name.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Left column of fixed width next to a right column that takes the rest; stacked when the window is narrow.
pub fn two_columns<T>(
    ui: &mut Ui,
    state: &mut T,
    left_width: f32,
    left: impl FnOnce(&mut Ui, &mut T),
    right: impl FnOnce(&mut Ui, &mut T),
) {
    if ui.available_width() < left_width + 320.0 {
        left(ui, state);
        ui.add_space(4.0);
        right(ui, state);
        return;
    }
    // separate child areas instead of a horizontal layout, which would limit the height of both columns
    let gap = ui.spacing().item_spacing.x;
    let area = ui.available_rect_before_wrap();
    let height = area.height().max(400.0);
    let layout = egui::Layout::top_down(egui::Align::Min);
    let left_rect = egui::Rect::from_min_size(area.min, egui::vec2(left_width, height));
    let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect).layout(layout));
    left(&mut left_ui, state);
    let right_min = egui::pos2(area.min.x + left_width + gap, area.min.y);
    let right_rect = egui::Rect::from_min_size(right_min, egui::vec2(area.max.x - right_min.x, height));
    let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect).layout(layout));
    right(&mut right_ui, state);
    ui.advance_cursor_after_rect(left_ui.min_rect().union(right_ui.min_rect()));
}

/// Thin bar showing how much of something is used, with the text after it.
pub fn usage_bar(ui: &mut Ui, fraction: f32, text: &str, color: Color32) {
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 180.0).max(80.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(4), ui.visuals().widgets.inactive.bg_fill);
        let mut fill = rect;
        fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
        if fill.width() > 0.0 {
            ui.painter().rect_filled(fill, CornerRadius::same(4), color);
        }
        let text_color = if color == DANGER { DANGER } else { theme::palette(ui.ctx()).muted };
        ui.label(RichText::new(text).small().color(text_color));
    });
}

/// Full-width progress bar (0.0-1.0).
pub fn progress_bar(ui: &mut Ui, fraction: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), ui.visuals().widgets.inactive.bg_fill);
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if fill.width() > 0.0 {
        ui.painter().rect_filled(fill, CornerRadius::same(4), color);
    }
}
