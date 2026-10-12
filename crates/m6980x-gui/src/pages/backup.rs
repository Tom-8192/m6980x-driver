use eframe::egui::Ui;
use egui_phosphor::regular as icon;
use m6980x_core::tr;

use crate::app::App;
use crate::theme::DANGER;
use crate::widgets::{self, card, card_title, card_title_in, page_header};

impl App {
    pub fn page_backup(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.backup"), &tr!("gui.backup_sub"));
        card(ui, |ui| {
            card_title(ui, icon::ARCHIVE, &tr!("gui.backup_card"), Some(&tr!("gui.backup_card_text")));
            ui.horizontal_wrapped(|ui| {
                if widgets::icon_button(ui, icon::DOWNLOAD_SIMPLE, &tr!("gui.backup_button")).clicked() {
                    self.do_backup();
                }
                if widgets::icon_button(ui, icon::UPLOAD_SIMPLE, &tr!("gui.restore_button")).clicked() {
                    self.do_restore();
                }
            });
        });
        ui.add_space(4.0);
        card(ui, |ui| {
            card_title(ui, icon::FLOPPY_DISK, &tr!("gui.memory_card"), Some(&tr!("gui.memory_card_text")));
            ui.horizontal_wrapped(|ui| {
                if widgets::icon_button(ui, icon::FILE_ARROW_DOWN, &tr!("gui.export_memory")).clicked() {
                    self.do_rom_export();
                }
                if widgets::icon_button(ui, icon::FILE_ARROW_UP, &tr!("gui.import_button")).clicked() {
                    self.do_rom_import();
                }
            });
        });
        ui.add_space(4.0);
        card(ui, |ui| {
            let text = tr!("gui.danger_card_text");
            card_title_in(ui, icon::SHIELD_WARNING, DANGER, &tr!("gui.danger_card"), Some(&text));
            if widgets::danger_button(ui, format!("{}  {}", icon::TRASH, tr!("gui.release_button"))).clicked() {
                self.release_all();
            }
        });
    }
}
