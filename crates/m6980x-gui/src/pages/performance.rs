use eframe::egui::{self, RichText, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::names::rates;
use m6980x_core::pc::{self, PAGE_SCROLL, SCROLL_MAX, SCROLL_MIN, WINDOWS_DEFAULT};
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
        // only Windows has a system-wide scroll speed and needs the tilt switch (Linux: always native after plugging in)
        if pc::scroll_supported() {
            ui.add_space(4.0);
            card(ui, |ui| self.scroll_card(ui));
        }
    }

    /// Like the scroll area on the original's DPI page, but with Windows settings instead of Tilt.exe.
    fn scroll_card(&mut self, ui: &mut Ui) {
        card_title(ui, icon::MOUSE_SCROLL, &tr!("gui.scroll"), Some(&tr!("gui.scroll_note")));
        let rows = [
            (0, tr!("gui.scroll_vertical"), tr!("gui.scroll_lines_unit")),
            (1, tr!("gui.scroll_horizontal"), tr!("gui.scroll_chars_unit")),
        ];
        for (axis, label, unit) in rows {
            let mut commit = false;
            ui.horizontal(|ui| {
                ui.add_sized(egui::vec2(110.0, 20.0), egui::Label::new(RichText::new(label).strong()));
                ui.spacing_mut().slider_width = (ui.available_width() - 160.0).clamp(140.0, 480.0);
                let r = ui
                    .add(egui::Slider::new(&mut self.scroll[axis], SCROLL_MIN..=SCROLL_MAX).suffix(format!(" {unit}")));
                // write when the slider is released or a typed value is done, like the DPI sliders
                if r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus()) {
                    commit = true;
                }
            });
            if commit {
                self.commit_scroll(axis);
            }
        }
        if self.scroll_pc[0] == PAGE_SCROLL {
            widgets::muted(ui, tr!("gui.scroll_page_note"));
        }
        ui.horizontal_wrapped(|ui| {
            let text = tr!("gui.scroll_windows_default", n = WINDOWS_DEFAULT);
            if widgets::icon_button(ui, icon::ARROW_CLOCKWISE, &text).clicked() {
                self.scroll_windows_default();
            }
        });
        ui.add_space(6.0);
        if self.original.tilt {
            widgets::warning_banner(ui, &tr!("gui.tilt_running"));
            ui.add_space(4.0);
        } else {
            widgets::muted(ui, tr!("gui.tilt_note"));
        }
        if widgets::icon_button(ui, icon::ARROWS_HORIZONTAL, &tr!("gui.tilt_native")).clicked() {
            self.tilt_native();
        }
    }
}
