use eframe::egui::{self, RichText, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::names::{led_name, led_names};
use m6980x_core::protocol::PROFILES;
use m6980x_core::tr;

use crate::app::App;
use crate::theme::{self, ACCENT};
use crate::widgets::{self, badge, card, nice, page_header, segmented, swatch};

impl App {
    pub fn page_lighting(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.lighting"), &tr!("gui.lighting_sub"));
        // presets with the same levels (e.g. cyan and light blue) are shown once, the first name wins
        let mut presets: Vec<(String, [u8; 3])> = Vec::new();
        for (name, rgb) in led_names() {
            if !presets.iter().any(|(_, v)| *v == rgb) {
                presets.push((nice(&name), rgb));
            }
        }
        let levels = [(0u8, tr!("gui.level.off")), (1, tr!("gui.level.medium")), (2, tr!("gui.level.full"))];
        for p in 0..PROFILES {
            let mut picked = None;
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr!("cli.profile", p = p + 1)).size(16.0).strong());
                    if usize::from(self.profile) == p + 1 {
                        badge(ui, &tr!("gui.active"), ACCENT);
                    }
                });
                ui.add_space(2.0);
                ui.horizontal_top(|ui| {
                    // preview
                    ui.vertical(|ui| {
                        ui.set_width(96.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                        let c = theme::led_color(self.leds[p]);
                        ui.painter().circle_filled(rect.center(), 34.0, c.gamma_multiply(0.25));
                        ui.painter().circle(
                            rect.center(),
                            24.0,
                            c,
                            egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                        );
                        let name = led_name(self.leds[p]).map(|n| nice(&n)).unwrap_or_else(|| tr!("gui.custom_colour"));
                        ui.label(RichText::new(name).strong());
                    });
                    ui.vertical(|ui| {
                        widgets::muted(ui, tr!("gui.presets"));
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for (name, rgb) in &presets {
                                if swatch(ui, theme::led_color(*rgb), self.leds[p] == *rgb, 30.0)
                                    .on_hover_text(name)
                                    .clicked()
                                {
                                    picked = Some(*rgb);
                                }
                            }
                        });
                        ui.add_space(4.0);
                        widgets::muted(ui, tr!("gui.custom_colour"));
                        egui::Grid::new(("channels", p)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                            for (c, label) in [tr!("gui.red"), tr!("gui.green"), tr!("gui.blue")].iter().enumerate() {
                                ui.label(label);
                                let mut level = self.leds[p][c];
                                if segmented(ui, &mut level, &levels) {
                                    let mut rgb = self.leds[p];
                                    rgb[c] = level;
                                    picked = Some(rgb);
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            });
            if let Some(rgb) = picked {
                self.set_led(p, rgb);
            }
            ui.add_space(4.0);
        }
        widgets::muted(ui, format!("{}  {}", icon::LIGHTBULB, tr!("gui.lighting_note")));
    }
}
