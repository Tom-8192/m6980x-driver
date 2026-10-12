use eframe::egui::Ui;
use egui_phosphor::regular as icon;
use m6980x_core::names::rates;
use m6980x_core::tr;

use crate::app::App;
use crate::widgets::{self, card, card_title, page_header, segmented};

impl App {
    pub fn page_performance(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.performance"), &tr!("gui.performance_sub"));
        card(ui, |ui| {
            card_title(ui, icon::PULSE, &tr!("gui.report_rate"), Some(&tr!("gui.rate_note")));
            let mut options: Vec<(u8, String)> = rates().into_iter().map(|(hz, ms)| (ms, format!("{hz} Hz"))).collect();
            options.reverse();
            let mut ms = self.rate_ms;
            if segmented(ui, &mut ms, &options) {
                self.set_rate(ms);
            }
            widgets::muted(ui, tr!("gui.rate_interval", ms = self.rate_ms));
        });
        ui.add_space(4.0);
        card(ui, |ui| {
            card_title(ui, icon::FLOPPY_DISK, &tr!("gui.save_permanently"), Some(&tr!("gui.save_note")));
            ui.checkbox(&mut self.persist, tr!("gui.auto_save"));
            if widgets::icon_button(ui, icon::FLOPPY_DISK, &tr!("gui.save_now")).clicked() {
                self.save_now();
            }
        });
    }
}
