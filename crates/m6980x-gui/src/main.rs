//! M6980X Configuration Tool - GUI (egui) with all features of the tool (Windows + Linux).
//!
//!     m6980x_configtool_gui [--device PATH] [--fake] [--lang en|de]
//!
//! `--fake` runs against a simulated mouse (dry run without a device). The language can be switched in the
//! window and is remembered; without a stored choice, `--lang`, `M6980X_LANG` or the system language applies.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod hid_keys;
mod mouse_art;
mod pages;
mod recorder;
mod theme;
mod widgets;
mod worker;

use std::time::Duration;

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, ThemePreference, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::names::led_name;
use m6980x_core::protocol::PROFILES;
use m6980x_core::{i18n, tr};

use app::{App, DPI_LINKED_KEY, LANG_KEY, Page, THEME_KEY};
use theme::{ACCENT, DANGER, SUCCESS, WARNING};

const PAGES: [(Page, &str, &str); 7] = [
    (Page::Overview, icon::HOUSE, "overview"),
    (Page::Buttons, icon::MOUSE, "buttons"),
    (Page::Lighting, icon::LIGHTBULB, "lighting"),
    (Page::Dpi, icon::CROSSHAIR, "dpi"),
    (Page::Performance, icon::GAUGE, "performance"),
    (Page::Macros, icon::KEYBOARD, "macros"),
    (Page::Backup, icon::ARCHIVE, "backup"),
];
/// below this window width the side bar shows icons only
const COMPACT_WIDTH: f32 = 900.0;

impl App {
    /// One entry of the side bar; returns true when clicked.
    fn nav_item(ui: &mut Ui, glyph: &str, label: &str, selected: bool, compact: bool) -> bool {
        let pal = theme::palette(ui.ctx());
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), Sense::click());
        let fill = if selected {
            pal.accent_soft
        } else if resp.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 8.0, fill);
        let color = if selected { ACCENT } else { pal.strong };
        let icon_x = if compact { rect.center().x } else { rect.left() + 20.0 };
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(19.0),
            color,
        );
        if !compact {
            ui.painter().text(
                rect.left_center() + egui::vec2(42.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                FontId::proportional(14.5),
                color,
            );
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let resp = if compact { resp.on_hover_text(label) } else { resp };
        resp.clicked()
    }

    fn ui_sidebar(&mut self, ui: &mut Ui, compact: bool) {
        let pal = theme::palette(ui.ctx());
        ui.add_space(6.0);
        if compact {
            ui.vertical_centered(|ui| ui.label(RichText::new(icon::MOUSE_SIMPLE).size(24.0).color(ACCENT)));
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::MOUSE_SIMPLE).size(24.0).color(ACCENT));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new("M6980X").size(16.0).strong().color(pal.strong));
                    ui.label(RichText::new(tr!("gui.app_subtitle")).small().color(pal.muted));
                });
            });
        }
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        for (page, glyph, id) in PAGES {
            if Self::nav_item(ui, glyph, &tr!(&format!("gui.nav.{id}")), self.page == page, compact) {
                if self.recorder.active && page != Page::Macros {
                    self.stop_recording();
                }
                self.page = page;
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(6.0);
            if !compact {
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().color(pal.muted));
            }
            if Self::nav_item(ui, icon::GEAR, &tr!("gui.settings"), self.show_settings, compact) {
                self.show_settings = true;
            }
        });
    }

    /// Profile button of the header in the LED colour of the profile.
    fn profile_button(&mut self, ui: &mut Ui, p: u8) {
        let pal = theme::palette(ui.ctx());
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(58.0, 34.0), Sense::click());
        let selected = self.profile == p;
        let led = theme::led_color(self.leds[usize::from(p - 1)]);
        let (fill, stroke) = if selected {
            (pal.accent_soft, Stroke::new(1.5, ACCENT))
        } else if resp.hovered() {
            (ui.visuals().widgets.hovered.weak_bg_fill, Stroke::new(1.0, pal.card_stroke))
        } else {
            (ui.visuals().widgets.inactive.weak_bg_fill, Stroke::new(1.0, pal.card_stroke))
        };
        ui.painter().rect(rect, CornerRadius::same(8), fill, stroke, egui::StrokeKind::Inside);
        let dot = rect.left_center() + egui::vec2(17.0, 0.0);
        if led == Color32::BLACK {
            ui.painter().circle_stroke(dot, 6.0, Stroke::new(1.5, pal.muted));
        } else {
            ui.painter().circle_filled(dot, 9.0, led.gamma_multiply(0.25));
            ui.painter().circle_filled(dot, 6.0, led);
        }
        ui.painter().text(
            rect.center() + egui::vec2(9.0, 0.0),
            egui::Align2::CENTER_CENTER,
            p.to_string(),
            FontId::proportional(16.0),
            if selected { ACCENT } else { pal.strong },
        );
        let colour = led_name(self.leds[usize::from(p - 1)]).map(|n| widgets::nice(&n)).unwrap_or_default();
        let resp = resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("{}  ·  {colour}", tr!("cli.profile", p = p)));
        if resp.clicked() && !selected {
            self.set_profile(p);
        }
    }

    fn ui_header(&mut self, ui: &mut Ui) {
        let pal = theme::palette(ui.ctx());
        ui.horizontal(|ui| {
            let (color, text) = match self.connected {
                _ if self.fake => (ACCENT, tr!("gui.status.simulation")),
                None => (WARNING, tr!("gui.connecting")),
                Some(true) => (SUCCESS, tr!("gui.status.connected")),
                Some(false) => (DANGER, tr!("gui.status.disconnected")),
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 5.0, color);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let name = if self.name.is_empty() { "M6980X".to_string() } else { self.name.clone() };
                ui.label(RichText::new(name).strong().color(pal.strong));
                ui.label(RichText::new(text).small().color(pal.muted));
            });
            if self.connected == Some(false) && !self.fake {
                ui.add_space(8.0);
                if widgets::icon_button(ui, icon::PLUGS_CONNECTED, &tr!("gui.reconnect")).clicked() {
                    self.refresh_all();
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for p in (1..=PROFILES as u8).rev() {
                    self.profile_button(ui, p);
                }
                ui.label(RichText::new(tr!("gui.profile")).color(pal.muted));
            });
        });
    }

    fn ui_status(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if self.busy() {
                ui.spinner();
            }
            if let Some(task) = &self.task {
                let text = format!("{}  {:.0} %", task.label, task.fraction() * 100.0);
                ui.label(RichText::new(text).small().strong().color(ACCENT));
                return;
            }
            ui.add(egui::Label::new(RichText::new(&self.status).small()).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let arrow = if self.show_log { icon::CARET_DOWN } else { icon::CARET_RIGHT };
                let text = RichText::new(format!("{}  {}  {arrow}", icon::LIST, tr!("gui.log"))).small();
                if ui.add(egui::Button::new(text).frame(false)).clicked() {
                    self.show_log = !self.show_log;
                }
            });
        });
    }

    fn ui_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::GEAR).size(20.0).color(ACCENT));
                ui.heading(tr!("gui.settings"));
            });
            ui.add_space(6.0);
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
                ui.label(tr!("gui.language"));
                let mut lang = i18n::lang();
                egui::ComboBox::from_id_salt("lang").selected_text(i18n::native_name(&lang)).show_ui(ui, |ui| {
                    for l in i18n::available() {
                        let name = i18n::native_name(&l);
                        ui.selectable_value(&mut lang, l, name);
                    }
                });
                if lang != i18n::lang() {
                    i18n::set_lang(&lang);
                }
                ui.end_row();
                ui.label(tr!("gui.theme"));
                let options = [
                    (ThemePreference::System, tr!("gui.theme.system")),
                    (ThemePreference::Light, tr!("gui.theme.light")),
                    (ThemePreference::Dark, tr!("gui.theme.dark")),
                ];
                if widgets::segmented(ui, &mut self.theme, &options) {
                    ctx.set_theme(self.theme);
                }
                ui.end_row();
                ui.label(tr!("gui.live_label"));
                ui.checkbox(&mut self.live, tr!("gui.live"));
                ui.end_row();
            });
            ui.add_space(8.0);
            close = ui.button(tr!("gui.close")).clicked();
        });
        if close || modal.should_close() {
            self.show_settings = false;
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(LANG_KEY, i18n::lang());
        storage.set_string(DPI_LINKED_KEY, if self.dpi_linked { "1" } else { "0" }.into());
        storage.set_string(THEME_KEY, theme::theme_id(self.theme).into());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // before any widget: the recorder takes the keyboard input of this frame
        if self.recorder.active {
            let new = self.recorder.capture_keys(&ctx);
            self.add_recorded(new);
        }
        self.finish_jobs();
        self.poll_live();
        // a running job with progress is redrawn often so the bar moves smoothly
        ctx.request_repaint_after(Duration::from_millis(if self.task.is_some() { 50 } else { 500 }));

        let pal = theme::palette(&ctx);
        let compact = ctx.content_rect().width() < COMPACT_WIDTH;
        let bar = egui::Frame::new().fill(pal.sidebar).inner_margin(egui::Margin::symmetric(16, 4));
        egui::Panel::bottom("status").frame(bar).exact_size(28.0).show(ui, |ui| self.ui_status(ui));
        if self.show_log {
            let frame = egui::Frame::new().fill(ui.visuals().extreme_bg_color).inner_margin(10);
            egui::Panel::bottom("log").frame(frame).resizable(true).default_size(150.0).show(ui, |ui| {
                egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    for line in &self.log {
                        ui.label(RichText::new(line).monospace().small());
                    }
                });
            });
        }
        let side = egui::Frame::new().fill(pal.sidebar).inner_margin(egui::Margin::symmetric(10, 12));
        egui::Panel::left("nav")
            .frame(side)
            .resizable(false)
            .show_separator_line(false)
            .exact_size(if compact { 64.0 } else { 210.0 })
            .show(ui, |ui| self.ui_sidebar(ui, compact));
        let head = egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(egui::Margin::symmetric(24, 10))
            .stroke(Stroke::new(1.0, pal.card_stroke));
        egui::Panel::top("header").frame(head).show_separator_line(false).show(ui, |ui| self.ui_header(ui));

        let central = egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::symmetric(24, 18));
        egui::CentralPanel::no_frame().frame(central).show(ui, |ui| {
            // while recording, the wheel goes to the recording pad instead of scrolling the page
            let source = if self.recorder.active {
                egui::scroll_area::ScrollSource { mouse_wheel: false, ..egui::scroll_area::ScrollSource::ALL }
            } else {
                egui::scroll_area::ScrollSource::ALL
            };
            egui::ScrollArea::vertical().id_salt("page").scroll_source(source).auto_shrink([false, false]).show(
                ui,
                |ui| {
                    ui.set_max_width(ui.available_width().min(1000.0));
                    match self.page {
                        Page::Overview => self.page_overview(ui),
                        Page::Buttons => self.page_buttons(ui),
                        Page::Lighting => self.page_lighting(ui),
                        Page::Dpi => self.page_dpi(ui),
                        Page::Performance => self.page_performance(ui),
                        Page::Macros => self.page_macros(ui),
                        Page::Backup => self.page_backup(ui),
                    }
                },
            );
        });
        self.ui_settings(&ctx);
        self.ui_dialogs(&ctx);
    }
}

fn main() -> eframe::Result {
    let mut device = None;
    let mut fake = false;
    let mut lang = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fake" => fake = true,
            "--device" => device = args.next(),
            "--lang" => lang = args.next().as_deref().and_then(i18n::resolve),
            _ => {}
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("M6980X Configuration Tool")
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([760.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "M6980X Configuration Tool",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, device, fake, lang)))),
    )
}
