use eframe::egui::{self, RichText, Sense, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::functions::{self as fnc, FUNCTIONS};
use m6980x_core::i18n::fold;
use m6980x_core::names::{BUTTON_IDS, button_names, describe_target, standard_label};
use m6980x_core::rom as romlayout;
use m6980x_core::tr;

use crate::app::{App, Target};
use crate::mouse_art::{self, Art};
use crate::theme::{self, ACCENT};
use crate::widgets::{self, card, card_title, page_header, two_columns};

/// Group of every assignable function (id -> group); texts in locales/ under `gui.category.<group>`.
/// Functions missing here land in "other".
const GROUPS: [(&str, &[&str]); 7] = [
    ("mouse", &["linksklick", "rechtsklick", "mittelklick", "browser-vor", "browser-zurueck", "doppelklick"]),
    (
        "edit",
        &[
            "kopieren",
            "einfuegen",
            "ausschneiden",
            "rueckgaengig",
            "entf",
            "neu",
            "oeffnen",
            "schliessen",
            "enter",
            "shift",
            "strg",
            "alt",
        ],
    ),
    (
        "window",
        &[
            "desktop",
            "vorheriges-fenster",
            "minimieren",
            "maximieren",
            "programm-schliessen",
            "zoom-rein",
            "zoom-raus",
            "lupe",
        ],
    ),
    ("media", &["play-pause", "zurueck", "weiter", "lauter", "leiser", "stumm"]),
    ("internet", &["internet", "favoriten", "email"]),
    ("wheel", &["scroll-vor", "scroll-zurueck", "nur-senkrecht", "nur-waagerecht"]),
    ("speed", &["speed-2x", "speed-4x", "speed-8x"]),
];

impl App {
    /// Display name of a device button ("Tilt left").
    pub fn button_label(&self, b: u8) -> String {
        BUTTON_IDS.iter().find(|(n, _)| *n == b).map_or_else(String::new, |(_, id)| tr!(&format!("gui.button.{id}")))
    }

    pub fn assignment_label(&self, p: u8, b: u8) -> String {
        describe_target(self.button_state[usize::from(p)][usize::from(b)], &self.macro_names, Some(b))
    }

    /// Every target in picker order: (group id, label, target).
    fn targets(&self, b: u8) -> Vec<(&'static str, String, Target)> {
        let mut list = vec![("default", standard_label(Some(b)), Target::Standard)];
        let assignable = |key: &str| FUNCTIONS.iter().find(|f| f.key == key && f.size > 0 && !f.record.is_empty());
        for (group, keys) in GROUPS {
            list.extend(keys.iter().filter_map(|k| assignable(k)).map(|f| (group, f.label(), Target::Function(f))));
        }
        let grouped = |key: &str| GROUPS.iter().any(|(_, keys)| keys.contains(&key));
        list.extend(
            FUNCTIONS
                .iter()
                .filter(|f| f.size > 0 && !f.record.is_empty() && !grouped(f.key))
                .map(|f| ("other", f.label(), Target::Function(f))),
        );
        list.extend(self.macro_names.iter().enumerate().map(|(s, n)| {
            let label = if n.is_empty() {
                tr!("cli.macro_column", s = s + 1)
            } else {
                tr!("gui.macro_named", s = s + 1, n = n)
            };
            ("macros", label, Target::Macro(s))
        }));
        list
    }

    fn is_current(&self, p: u8, b: u8, target: &Target) -> bool {
        let state = self.button_state[usize::from(p)][usize::from(b)];
        match target {
            Target::Standard => state.is_none(),
            Target::Function(f) => state == Some(fnc::placement(f, usize::from(b)).0),
            Target::Macro(s) => state == Some(romlayout::macro_slot_addr(*s)),
        }
    }

    pub fn page_buttons(&mut self, ui: &mut Ui) {
        page_header(ui, &tr!("gui.nav.buttons"), &tr!("gui.buttons_sub", p = self.profile));
        two_columns(
            ui,
            self,
            280.0,
            |ui, app| {
                card(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let p = usize::from(app.profile - 1);
                        let art = Art {
                            led: theme::led_color(app.leds[p]),
                            lit: usize::from(app.stage),
                            selected: mouse_art::Region::from_button(app.selected_button),
                        };
                        let not_assignable = tr!("gui.not_assignable");
                        let hover = |r: mouse_art::Region| {
                            Some(match r.button() {
                                Some(b) => app.button_label(b) + "\n" + &app.assignment_label(app.profile - 1, b),
                                None => not_assignable.clone(),
                            })
                        };
                        if let Some(b) = mouse_art::show(ui, 380.0, &art, &hover).and_then(|r| r.button()) {
                            app.selected_button = b;
                        }
                    });
                });
            },
            |ui, app| {
                card(ui, |ui| {
                    card_title(ui, icon::MOUSE, &tr!("gui.buttons_of_profile", p = app.profile), None);
                    let p = app.profile - 1;
                    let pal = theme::palette(ui.ctx());
                    for (b, _) in button_names() {
                        let selected = app.selected_button == b;
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), Sense::click());
                        let fill = if selected {
                            pal.accent_soft
                        } else if resp.hovered() {
                            ui.visuals().faint_bg_color
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, 6.0, fill);
                        let name_color = if selected { ACCENT } else { pal.strong };
                        ui.painter().text(
                            rect.left_center() + egui::vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            app.button_label(b),
                            egui::FontId::proportional(14.0),
                            name_color,
                        );
                        let assignment = app.assignment_label(p, b);
                        let galley = ui.painter().layout(
                            assignment,
                            egui::FontId::proportional(13.0),
                            if app.button_state[usize::from(p)][usize::from(b)].is_some() {
                                pal.strong
                            } else {
                                pal.muted
                            },
                            (rect.width() * 0.6).max(60.0),
                        );
                        let pos =
                            egui::pos2(rect.right() - 10.0 - galley.size().x, rect.center().y - galley.size().y / 2.0);
                        ui.painter().galley(pos, galley, pal.strong);
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            app.selected_button = b;
                        }
                    }
                    ui.add_space(6.0);
                    if widgets::icon_button(ui, icon::BROOM, &tr!("gui.reset_profile_button")).clicked() {
                        app.reset_profile_buttons();
                    }
                });
            },
        );
        ui.add_space(4.0);
        self.ui_picker(ui);
    }

    /// Searchable list of all targets for the selected button; a click assigns at once.
    fn ui_picker(&mut self, ui: &mut Ui) {
        let (p, b) = (self.profile - 1, self.selected_button);
        let title = tr!("gui.assign_title", button = self.button_label(b));
        card(ui, |ui| {
            card_title(ui, icon::HAND_TAP, &title, Some(&tr!("gui.current", label = self.assignment_label(p, b))));
            ui.add(
                egui::TextEdit::singleline(&mut self.picker_search)
                    .hint_text(format!("{}  {}", icon::MAGNIFYING_GLASS, tr!("gui.search")))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);
            let needle = fold(self.picker_search.trim());
            let targets = self.targets(b);
            let mut chosen = None;
            egui::ScrollArea::vertical().id_salt("picker").max_height(320.0).auto_shrink([false, true]).show(
                ui,
                |ui| {
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        let mut last_group = "";
                        for (group, label, target) in &targets {
                            if !needle.is_empty() && !fold(label).contains(&needle) {
                                continue;
                            }
                            if *group != last_group {
                                last_group = group;
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(tr!(&format!("gui.category.{group}")).to_uppercase())
                                        .small()
                                        .strong()
                                        .color(theme::palette(ui.ctx()).muted),
                                );
                            }
                            let current = self.is_current(p, b, target);
                            let text =
                                if current { format!("{}  {label}", icon::CHECK) } else { format!("      {label}") };
                            if ui.selectable_label(current, text).clicked() && !current {
                                chosen = Some((target.clone(), label.clone()));
                            }
                        }
                    });
                },
            );
            if let Some((target, label)) = chosen {
                self.assign(p, b, target, label);
            }
        });
    }
}
