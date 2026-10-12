use eframe::egui::{self, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::names::{led_name, rate_hz};
use m6980x_core::protocol::BUTTONS;
use m6980x_core::rom::MACRO_SLOTS;
use m6980x_core::tr;

use crate::app::{App, Page};
use crate::mouse_art::{self, Art};
use crate::theme;
use crate::widgets::{self, card, card_title, nice, page_header, stat_tile, two_columns};

impl App {
    pub fn page_overview(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.overview"), &tr!("gui.overview_sub"));
        two_columns(
            ui,
            self,
            300.0,
            |ui, app| {
                card(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let p = usize::from(app.profile - 1);
                        let art =
                            Art { led: theme::led_color(app.leds[p]), lit: usize::from(app.stage), selected: None };
                        let hover = |r: mouse_art::Region| {
                            r.button().map(|b| app.button_label(b) + "\n" + &app.assignment_label(app.profile - 1, b))
                        };
                        if let Some(b) = mouse_art::show(ui, 360.0, &art, &hover).and_then(|r| r.button()) {
                            app.selected_button = b;
                            app.page = Page::Buttons;
                        }
                        ui.add_space(6.0);
                        widgets::muted(ui, tr!("gui.overview_click_hint"));
                    });
                });
            },
            |ui, app| {
                let p = usize::from(app.profile - 1);
                let colour = led_name(app.leds[p]).map(|n| nice(&n)).unwrap_or_default();
                let [x, y] = app.dpi[usize::from(app.stage - 1)];
                let dpi = if x == y { format!("{x} DPI") } else { format!("{x} / {y} DPI") };
                let assigned = app.button_state[p].iter().filter(|s| s.is_some()).count();
                let macros = app.macro_names.iter().filter(|n| !n.is_empty()).count();
                // device button 5 does not exist
                let buttons = BUTTONS - 1;
                ui.columns(2, |cols| {
                    stat_tile(
                        &mut cols[0],
                        icon::STACK,
                        &tr!("gui.active_profile"),
                        &format!("{}  ·  {colour}", app.profile),
                        Some(theme::led_color(app.leds[p])).filter(|c| *c != egui::Color32::BLACK),
                    );
                    stat_tile(
                        &mut cols[1],
                        icon::CROSSHAIR,
                        &tr!("gui.active_dpi_stage"),
                        &format!("{}  ·  {dpi}", app.stage),
                        None,
                    );
                    stat_tile(
                        &mut cols[0],
                        icon::PULSE,
                        &tr!("gui.report_rate"),
                        &format!("{} Hz", rate_hz(app.rate_ms)),
                        None,
                    );
                    stat_tile(
                        &mut cols[1],
                        icon::MOUSE_LEFT_CLICK,
                        &tr!("gui.assigned_buttons"),
                        &format!("{assigned} / {buttons}"),
                        None,
                    );
                    stat_tile(
                        &mut cols[0],
                        icon::CODE,
                        &tr!("gui.used_macros"),
                        &format!("{macros} / {MACRO_SLOTS}"),
                        None,
                    );
                    stat_tile(
                        &mut cols[1],
                        icon::USB,
                        &tr!("gui.device"),
                        if app.name.is_empty() { "-" } else { &app.name },
                        None,
                    );
                });
                ui.add_space(4.0);
                card(ui, |ui| {
                    card_title(ui, icon::ARROWS_CLOCKWISE, &tr!("gui.quick_actions"), None);
                    ui.horizontal_wrapped(|ui| {
                        if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, &tr!("gui.read_everything_again")).clicked()
                        {
                            app.refresh_all();
                        }
                        if widgets::icon_button(ui, icon::FLOPPY_DISK, &tr!("gui.save_now")).clicked() {
                            app.save_now();
                        }
                    });
                });
            },
        );
    }
}
