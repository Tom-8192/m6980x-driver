use eframe::egui::{self, RichText, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::protocol::{DPI_BUTTON_STAGES, MAX_DPI};
use m6980x_core::tr;

use crate::app::{App, MIN_DPI};
use crate::theme::{self, ACCENT};
use crate::widgets::{self, badge, card, led_dots, page_header};

impl App {
    pub fn page_dpi(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.dpi"), &tr!("gui.dpi_note", max_dpi = MAX_DPI));
        card(ui, |ui| {
            if ui.checkbox(&mut self.dpi_linked, tr!("gui.dpi_linked")).changed() && self.dpi_linked {
                for i in 0..DPI_BUTTON_STAGES {
                    self.commit_dpi(i);
                }
            }
        });
        ui.add_space(4.0);
        let led = theme::led_color(self.leds[usize::from(self.profile - 1)]);
        let led = if led == egui::Color32::BLACK { ACCENT } else { led };
        for i in 0..DPI_BUTTON_STAGES {
            let active = usize::from(self.stage) == i + 1;
            let mut commit = false;
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr!("gui.stage_row", i = i + 1)).size(16.0).strong());
                    led_dots(ui, i + 1, DPI_BUTTON_STAGES, led);
                    if active {
                        badge(ui, &tr!("gui.active"), ACCENT);
                    } else if ui.small_button(tr!("gui.activate")).clicked() {
                        self.set_stage(i as u8 + 1);
                    }
                });
                let axes: &[(usize, &str)] = if self.dpi_linked { &[(0, "DPI")] } else { &[(0, "X"), (1, "Y")] };
                for &(axis, label) in axes {
                    ui.horizontal(|ui| {
                        ui.add_sized(egui::vec2(28.0, 20.0), egui::Label::new(RichText::new(label).strong()));
                        ui.spacing_mut().slider_width = (ui.available_width() - 120.0).clamp(140.0, 520.0);
                        let r = ui.add(
                            egui::Slider::new(&mut self.dpi[i][axis], MIN_DPI..=i64::from(MAX_DPI))
                                .step_by(90.0)
                                .suffix(" DPI"),
                        );
                        // write when the slider is released or a typed/stepped value is done, not while dragging
                        if r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus()) {
                            commit = true;
                        }
                    });
                }
            });
            if self.dpi_linked {
                self.dpi[i][1] = self.dpi[i][0];
            }
            if commit {
                self.commit_dpi(i);
            }
            ui.add_space(4.0);
        }
        ui.horizontal_wrapped(|ui| {
            if widgets::icon_button(ui, icon::ARROW_CLOCKWISE, &tr!("gui.factory_button")).clicked() {
                self.ask(tr!("gui.factory_title"), tr!("gui.factory_question"), |app| app.factory_dpi());
            }
        });
    }
}
