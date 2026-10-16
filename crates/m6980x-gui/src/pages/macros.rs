use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui};
use egui_phosphor::regular as icon;
use m6980x_core::functions::{self as fnc, KEYS, MOUSE_BUTTONS, NAMED_KEYS, key_name};
use m6980x_core::i18n;
use m6980x_core::macro_steps::{self as steps, Step};
use m6980x_core::rom::{self as romlayout, ICON_COUNT, ICONS_EMBEDDED, MACRO_NAME_LEN, MACRO_SLOT_SIZE, MACRO_SLOTS};
use m6980x_core::tr;

use crate::app::{App, FLASH_TIME, MODES};
use crate::icons::{IconPicker, MacroIcons};
use crate::theme::{self, ACCENT, DANGER, RADIUS, SUCCESS};
use crate::widgets::{self, card, card_title, page_header, segmented, two_columns};

/// icon dialog: tiles per row, tile size and gap in points
const PICKER_COLUMNS: usize = 10;
const TILE: f32 = 58.0;
const TILE_GAP: f32 = 6.0;

/// One clickable tile of the icon dialog (highlighted when hovered, framed in the accent colour when chosen).
fn icon_tile(ui: &mut Ui, icons: &mut MacroIcons, icon: Option<u8>, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(TILE, TILE), Sense::click());
    let pal = theme::palette(ui.ctx());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, RADIUS, pal.accent_soft);
    } else if resp.hovered() {
        painter.rect_filled(rect, RADIUS, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    icons.paint(painter, rect.shrink(3.0), icon);
    if selected {
        painter.rect_stroke(rect, RADIUS, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, pal.muted), egui::StrokeKind::Inside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[derive(Clone, Copy, PartialEq)]
enum Edit {
    Up(usize),
    Down(usize),
    Delete(usize),
}

/// (usage, display name) of every key the macro text knows.
fn key_choices() -> Vec<(u8, String)> {
    let mut keys: Vec<u8> = KEYS.iter().map(|(_, u)| *u).collect();
    keys.extend(NAMED_KEYS.iter().map(|(_, u)| *u));
    keys.into_iter().filter_map(|u| key_name(u).map(|n| (u, n))).collect()
}

fn mouse_name(mask: u8) -> String {
    MOUSE_BUTTONS
        .iter()
        .find(|(_, m)| *m == mask)
        .map_or_else(|| format!("0x{mask:02X}"), |(id, _)| widgets::nice(&i18n::name(&format!("mouse.{id}"))))
}

fn step_icon(step: &Step) -> &'static str {
    match step {
        Step::KeyDown(_) | Step::KeyUp(_) => icon::KEYBOARD,
        Step::Wait(_) => icon::TIMER,
        Step::MouseDown(_) | Step::MouseUp(_) => icon::MOUSE_LEFT_CLICK,
        Step::Wheel(_) => icon::MOUSE_SCROLL,
        Step::Consumer(_) => icon::SPEAKER_HIGH,
        Step::Raw(_) => icon::CODE,
    }
}

/// Editing controls of one step.
fn step_editor(ui: &mut Ui, i: usize, step: &mut Step, keys: &[(u8, String)]) {
    let down_up = |down: bool| if down { tr!("gui.step.down") } else { tr!("gui.step.up") };
    match step {
        Step::KeyDown(_) | Step::KeyUp(_) => {
            let (mut down, mut usage) = match *step {
                Step::KeyDown(k) => (true, k),
                Step::KeyUp(k) => (false, k),
                _ => unreachable!(),
            };
            ui.label(tr!("gui.step.key"));
            let name = key_name(usage).unwrap_or_else(|| format!("0x{usage:02X}"));
            egui::ComboBox::from_id_salt(("key", i)).selected_text(name).width(110.0).height(320.0).show_ui(ui, |ui| {
                for (u, n) in keys {
                    ui.selectable_value(&mut usage, *u, n);
                }
            });
            egui::ComboBox::from_id_salt(("key_dir", i)).selected_text(down_up(down)).width(90.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut down, true, down_up(true));
                ui.selectable_value(&mut down, false, down_up(false));
            });
            *step = if down { Step::KeyDown(usage) } else { Step::KeyUp(usage) };
        }
        Step::Wait(ms) => {
            ui.label(tr!("gui.step.wait"));
            ui.add(egui::DragValue::new(ms).range(1..=u16::MAX).speed(5).suffix(" ms"));
        }
        Step::MouseDown(_) | Step::MouseUp(_) => {
            let (mut down, mut mask) = match *step {
                Step::MouseDown(m) => (true, m),
                Step::MouseUp(m) => (false, m),
                _ => unreachable!(),
            };
            ui.label(tr!("gui.step.mouse"));
            egui::ComboBox::from_id_salt(("mouse", i)).selected_text(mouse_name(mask)).width(110.0).show_ui(ui, |ui| {
                for (_, value) in MOUSE_BUTTONS {
                    ui.selectable_value(&mut mask, value, mouse_name(value));
                }
            });
            egui::ComboBox::from_id_salt(("mouse_dir", i)).selected_text(down_up(down)).width(90.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut down, true, down_up(true));
                ui.selectable_value(&mut down, false, down_up(false));
            });
            *step = if down { Step::MouseDown(mask) } else { Step::MouseUp(mask) };
        }
        Step::Wheel(n) => {
            let mut up = *n > 0;
            let mut count = n.unsigned_abs().clamp(1, 15);
            ui.label(tr!("gui.step.wheel"));
            let dir = |up: bool| if up { tr!("gui.step.wheel_up") } else { tr!("gui.step.wheel_down") };
            egui::ComboBox::from_id_salt(("wheel", i)).selected_text(dir(up)).width(90.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut up, true, dir(true));
                ui.selectable_value(&mut up, false, dir(false));
            });
            ui.add(egui::DragValue::new(&mut count).range(1..=15).suffix("×"));
            *n = if up { count as i8 } else { -(count as i8) };
        }
        Step::Consumer(u) => {
            ui.label(tr!("gui.step.consumer"));
            ui.add(egui::DragValue::new(u).hexadecimal(4, false, true).prefix("0x"))
                .on_hover_text(tr!("gui.step.consumer_hint"));
            if *u == 0 {
                widgets::muted(ui, tr!("gui.step.consumer_release"));
            }
        }
        Step::Raw(b) => {
            ui.label(format!("0x{b:02X}"));
            widgets::muted(ui, tr!("gui.step.raw"));
        }
    }
}

impl App {
    pub fn page_macros(&mut self, ui: &mut Ui) {
        self.ensure_macros();
        page_header(ui, &tr!("gui.nav.macros"), &tr!("gui.macros_sub"));
        two_columns(ui, self, 240.0, |ui, app| app.ui_macro_list(ui), |ui, app| app.ui_macro_editor(ui));
        self.ui_icon_picker(ui.ctx());
    }

    /// Dialog with a grid of all icons: "no icon", pictures 1-40 and number tiles 41-100.
    fn ui_icon_picker(&mut self, ctx: &egui::Context) {
        let Some(picker) = self.icon_picker.as_mut() else {
            return;
        };
        let (mut draft, scroll) = (picker.draft, std::mem::take(&mut picker.scroll));
        let pictures = 1..=ICONS_EMBEDDED;
        let numbers = ICONS_EMBEDDED + 1..=ICON_COUNT;
        // labels are built up front because the dialog borrows the icon textures mutably
        let labels: Vec<String> =
            std::iter::once(None).chain((1..=ICON_COUNT).map(Some)).map(|i| self.icon_label(i)).collect();
        let label = |icon: Option<u8>| &labels[usize::from(icon.unwrap_or(0))];
        let icons = &mut self.icons;
        let mut apply = false;
        let mut cancel = false;
        let max_height = (ctx.content_rect().height() - 260.0).clamp(180.0, 720.0);
        let modal = egui::Modal::new(egui::Id::new("icon_picker")).show(ctx, |ui| {
            ui.set_width(PICKER_COLUMNS as f32 * (TILE + TILE_GAP) + 12.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::IMAGES).size(20.0).color(ACCENT));
                ui.heading(tr!("gui.icon_picker_title"));
            });
            ui.add_space(6.0);
            let mut hovered = None;
            let mut tile = |ui: &mut Ui, icon: Option<u8>| {
                let resp = icon_tile(ui, icons, icon, draft == icon).on_hover_text(label(icon));
                if scroll && draft == icon {
                    resp.scroll_to_me(Some(egui::Align::Center));
                }
                if resp.hovered() {
                    hovered = Some(icon);
                }
                if resp.clicked() {
                    draft = icon;
                }
                if resp.double_clicked() {
                    apply = true;
                }
            };
            ui.horizontal(|ui| {
                tile(ui, None);
                ui.label(tr!("names.no_icon"));
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
                for (title, range) in
                    [(tr!("gui.icon_section_pictures"), pictures), (tr!("gui.icon_section_numbers"), numbers)]
                {
                    ui.add_space(4.0);
                    widgets::muted(ui, title);
                    let range: Vec<u8> = range.collect();
                    for row in range.chunks(PICKER_COLUMNS) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(TILE_GAP, TILE_GAP);
                            for &n in row {
                                tile(ui, Some(n));
                            }
                        });
                    }
                }
            });
            ui.separator();
            ui.add(egui::Label::new(RichText::new(label(hovered.unwrap_or(draft))).strong()).truncate());
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                apply |= widgets::accent_button(ui, tr!("gui.apply")).clicked();
                cancel = ui.button(tr!("gui.cancel")).clicked();
            });
        });
        if apply {
            self.m_icon = draft;
            self.icon_picker = None;
        } else if cancel || modal.should_close() {
            self.icon_picker = None;
        } else if let Some(picker) = self.icon_picker.as_mut() {
            picker.draft = draft;
        }
    }

    fn ui_macro_list(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr!("gui.slots")).size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_only(ui, icon::ARROWS_CLOCKWISE, &tr!("gui.read_macros_button")).clicked() {
                        self.read_macros();
                    }
                });
            });
            let Some(rom) = self.macro_rom.clone() else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    widgets::muted(ui, tr!("gui.reading_macros"));
                });
                let fraction = self.task.as_ref().map_or(0.0, |t| t.fraction());
                widgets::progress_bar(ui, fraction, ACCENT);
                return;
            };
            let pal = theme::palette(ui.ctx());
            let mut clicked = None;
            for s in 0..MACRO_SLOTS {
                let used = romlayout::used_macro_slot(&rom, s);
                let name = romlayout::macro_name(romlayout::macro_info(&rom, s));
                let selected = self.selected_macro == Some(s);
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::click());
                let fill = if selected {
                    pal.accent_soft
                } else if resp.hovered() {
                    ui.visuals().faint_bg_color
                } else {
                    Color32::TRANSPARENT
                };
                let painter = ui.painter();
                painter.rect_filled(rect, 6.0, fill);
                let font = egui::FontId::proportional(13.5);
                painter.text(
                    rect.left_center() + egui::vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{:>2}", s + 1),
                    egui::FontId::monospace(12.0),
                    pal.muted,
                );
                let (text, color) = if name.is_empty() {
                    (tr!("gui.free"), pal.muted)
                } else {
                    (name, if selected { ACCENT } else { pal.strong })
                };
                if let Some(n) = romlayout::macro_icon(romlayout::macro_info(&rom, s)) {
                    let icon_rect = egui::Rect::from_center_size(
                        rect.left_center() + egui::vec2(47.0, 0.0),
                        egui::vec2(22.0, 22.0),
                    );
                    self.icons.paint(painter, icon_rect, Some(n));
                }
                painter.text(rect.left_center() + egui::vec2(66.0, 0.0), egui::Align2::LEFT_CENTER, text, font, color);
                let bytes = fnc::record_length(used);
                if bytes > 0 {
                    painter.text(
                        rect.right_center() - egui::vec2(10.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        format!("{bytes} B"),
                        egui::FontId::proportional(11.5),
                        pal.muted,
                    );
                }
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    clicked = Some((s, bytes > 0 || !self.macro_names[s].is_empty()));
                }
            }
            if let Some((s, used)) = clicked {
                if self.recorder.active {
                    self.stop_recording();
                }
                if used { self.load_macro(s) } else { self.new_macro(s) }
            }
        });
    }

    fn ui_macro_editor(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            card_title(ui, icon::PENCIL_SIMPLE, &tr!("gui.macro_editor"), None);
            egui::Grid::new("macro_head").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(tr!("gui.slot"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.m_slot).range(1..=MACRO_SLOTS));
                    widgets::muted(ui, tr!("gui.slot_hint"));
                });
                ui.end_row();
                ui.label(tr!("gui.name"));
                let hint = tr!("gui.name_hint", name = self.default_macro_name());
                ui.add(
                    egui::TextEdit::singleline(&mut self.m_name)
                        .char_limit(MACRO_NAME_LEN)
                        .hint_text(hint)
                        .desired_width(200.0),
                );
                ui.end_row();
                ui.label(tr!("gui.mode"));
                ui.horizontal(|ui| {
                    let options: Vec<(usize, String)> = MODES
                        .iter()
                        .enumerate()
                        .map(|(i, (_, id))| (i, widgets::nice(&i18n::name(&format!("mode.{id}")))))
                        .collect();
                    segmented(ui, &mut self.m_mode, &options);
                    widgets::muted(ui, tr!(&format!("gui.mode.{}", MODES[self.m_mode].1)));
                });
                ui.end_row();
                ui.label(tr!("gui.icon"));
                ui.horizontal(|ui| {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), Sense::click());
                    self.icons.paint(ui.painter(), rect, self.m_icon);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.icon_picker = Some(IconPicker::open(self.m_icon));
                    }
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        ui.add(egui::Label::new(self.icon_label(self.m_icon)).truncate());
                        if widgets::icon_button(ui, icon::IMAGES, &tr!("gui.choose_icon")).clicked() {
                            self.icon_picker = Some(IconPicker::open(self.m_icon));
                        }
                    });
                });
                ui.end_row();
            });

            // memory use of the slot
            ui.add_space(4.0);
            match self.editor_record() {
                Ok(r) => {
                    let over = r.len() > MACRO_SLOT_SIZE;
                    let text = tr!("gui.bytes_used", n = r.len(), max = MACRO_SLOT_SIZE);
                    let text = if over { format!("{text}  –  {}", tr!("gui.too_long")) } else { text };
                    let color = if over { DANGER } else { ACCENT };
                    widgets::usage_bar(ui, r.len() as f32 / MACRO_SLOT_SIZE as f32, &text, color);
                }
                Err(e) => {
                    ui.colored_label(DANGER, format!("{} {e}", tr!("cli.error")));
                }
            }

            ui.add_space(6.0);
            self.ui_macro_toolbar(ui);
            ui.add_space(4.0);
            if self.recorder.active {
                self.ui_record_pad(ui);
            }
            if self.m_text_mode {
                ui.add(
                    egui::TextEdit::multiline(&mut self.m_text)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );
                ui.label(RichText::new(tr!("gui.event_help")).monospace().small().weak());
            } else {
                self.ui_steps(ui);
            }

            ui.add_space(8.0);
            ui.separator();
            self.ui_task_banner(ui);
            let idle = self.task.is_none();
            ui.add_enabled_ui(idle, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if widgets::accent_button(ui, format!("{}  {}", icon::UPLOAD_SIMPLE, tr!("gui.write_to_mouse")))
                        .clicked()
                    {
                        if self.recorder.active {
                            self.stop_recording();
                        }
                        self.write_macro();
                    }
                    if widgets::icon_button(ui, icon::IMAGE, &tr!("gui.set_icon_only")).clicked() {
                        self.set_macro_icon_only();
                    }
                    if widgets::icon_button(ui, icon::TRASH, &tr!("gui.delete_macro_button")).clicked() {
                        self.delete_macro();
                    }
                    ui.menu_button(format!("{}  {}", icon::FILE, tr!("gui.file")), |ui| {
                        if ui.button(format!("{}  {}", icon::FILE_ARROW_UP, tr!("gui.load_file"))).clicked() {
                            ui.close();
                            self.import_macro_file();
                        }
                        if ui.button(format!("{}  {}", icon::FILE_ARROW_DOWN, tr!("gui.save_file"))).clicked() {
                            ui.close();
                            self.export_macro_file();
                        }
                    });
                })
            });
        });
    }

    /// Progress of the running macro job, or for a few seconds the message that it worked.
    fn ui_task_banner(&self, ui: &mut Ui) {
        let banner = |ui: &mut Ui, color: Color32, add: &mut dyn FnMut(&mut Ui)| {
            egui::Frame::new()
                .fill(color.gamma_multiply(0.12))
                .stroke(Stroke::new(1.0, color))
                .corner_radius(8)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add(ui);
                });
            ui.add_space(6.0);
        };
        if let Some(task) = &self.task {
            let fraction = task.fraction();
            banner(ui, ACCENT, &mut |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(&task.label).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.0} %", fraction * 100.0)).strong().color(ACCENT));
                    });
                });
                widgets::progress_bar(ui, fraction, ACCENT);
            });
        } else if let Some((text, at)) = &self.flash
            && at.elapsed() < FLASH_TIME
        {
            banner(ui, SUCCESS, &mut |ui| {
                ui.label(RichText::new(format!("{}  {text}", icon::CHECK_CIRCLE)).strong().color(SUCCESS));
            });
            ui.ctx().request_repaint_after(FLASH_TIME.saturating_sub(at.elapsed()));
        }
    }

    fn ui_macro_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut text = self.m_text_mode;
            let options = [
                (false, format!("{}  {}", icon::LIST, tr!("gui.view_steps"))),
                (true, format!("{}  {}", icon::TEXT_T, tr!("gui.view_text"))),
            ];
            if segmented(ui, &mut text, &options)
                && let Err(e) = self.set_text_mode(text)
            {
                self.error = Some((tr!("gui.macro"), e.to_string()));
            }
            ui.add_space(12.0);
            if self.recorder.active {
                if widgets::danger_button(ui, format!("{}  {}", icon::STOP, tr!("gui.record_stop"))).clicked() {
                    self.stop_recording();
                }
            } else {
                let record =
                    egui::Button::new(RichText::new(format!("{}  {}", icon::RECORD, tr!("gui.record"))).color(DANGER));
                if ui.add(record).on_hover_text(tr!("gui.record_hint")).clicked() {
                    self.start_recording();
                }
            }
            ui.checkbox(&mut self.recorder.with_delays, tr!("gui.record_delays"));
            if !self.recorder.with_delays {
                ui.add(egui::DragValue::new(&mut self.recorder.fixed_delay).range(0..=10_000).speed(5).suffix(" ms"))
                    .on_hover_text(tr!("gui.record_fixed_delay"));
            }
        });
    }

    /// Area that takes mouse clicks and wheel notches while recording.
    fn ui_record_pad(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 84.0), Sense::click());
        let t = ui.input(|i| i.time);
        let pulse = 0.55 + 0.45 * (t * 4.0).sin().abs() as f32;
        ui.painter().rect(
            rect,
            8.0,
            DANGER.gamma_multiply(0.08),
            Stroke::new(1.5, DANGER.gamma_multiply(pulse)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center() - egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            format!("{}  {}", icon::RECORD, tr!("gui.record_active")),
            egui::FontId::proportional(15.0),
            DANGER,
        );
        ui.painter().text(
            rect.center() + egui::vec2(0.0, 12.0),
            egui::Align2::CENTER_CENTER,
            tr!("gui.record_pad"),
            egui::FontId::proportional(12.5),
            theme::palette(ui.ctx()).muted,
        );
        let new = self.recorder.capture_pad(ui.ctx(), rect);
        self.add_recorded(new);
        ui.ctx().request_repaint();
    }

    fn ui_steps(&mut self, ui: &mut Ui) {
        let keys = key_choices();
        let mut edit = None;
        let count = self.m_steps.len();
        if count == 0 {
            widgets::muted(ui, tr!("gui.no_steps"));
        }
        egui::ScrollArea::vertical()
            .id_salt("steps")
            .max_height(360.0)
            .auto_shrink([false, true])
            .stick_to_bottom(self.recorder.active)
            .show(ui, |ui| {
                for (i, step) in self.m_steps.iter_mut().enumerate() {
                    let fill = if i % 2 == 0 { ui.visuals().faint_bg_color } else { Color32::TRANSPARENT };
                    egui::Frame::new().fill(fill).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 3)).show(
                        ui,
                        |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{:>3}", i + 1)).monospace().weak());
                                ui.label(RichText::new(step_icon(step)).size(16.0).color(ACCENT));
                                step_editor(ui, i, step, &keys);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::icon_only(ui, icon::X, &tr!("gui.step.delete")).clicked() {
                                        edit = Some(Edit::Delete(i));
                                    }
                                    if i + 1 < count
                                        && widgets::icon_only(ui, icon::ARROW_DOWN, &tr!("gui.step.move_down"))
                                            .clicked()
                                    {
                                        edit = Some(Edit::Down(i));
                                    }
                                    if i > 0
                                        && widgets::icon_only(ui, icon::ARROW_UP, &tr!("gui.step.move_up")).clicked()
                                    {
                                        edit = Some(Edit::Up(i));
                                    }
                                });
                            });
                        },
                    );
                }
            });
        match edit {
            Some(Edit::Up(i)) => self.m_steps.swap(i, i - 1),
            Some(Edit::Down(i)) => self.m_steps.swap(i, i + 1),
            Some(Edit::Delete(i)) => {
                self.m_steps.remove(i);
            }
            None => {}
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(format!("{}  {}", icon::PLUS, tr!("gui.step.add")), |ui| {
                let a = fnc::key("a").unwrap_or(0x04);
                let items: [(&str, &[Step]); 9] = [
                    ("tap", &[Step::KeyDown(a), Step::KeyUp(a)]),
                    ("key_down", &[Step::KeyDown(a)]),
                    ("key_up", &[Step::KeyUp(a)]),
                    ("wait", &[Step::Wait(50)]),
                    ("click", &[Step::MouseDown(1), Step::MouseUp(1)]),
                    ("mouse_down", &[Step::MouseDown(1)]),
                    ("mouse_up", &[Step::MouseUp(1)]),
                    ("wheel", &[Step::Wheel(1)]),
                    ("consumer", &[Step::Consumer(0x00E9), Step::Consumer(0)]),
                ];
                for (id, add) in items {
                    if ui.button(tr!(&format!("gui.step.add_{id}"))).clicked() {
                        self.m_steps.extend_from_slice(add);
                        ui.close();
                    }
                }
            });
            if !self.m_steps.is_empty() && widgets::icon_button(ui, icon::BROOM, &tr!("gui.step.clear")).clicked() {
                self.m_steps.clear();
            }
            widgets::muted(
                ui,
                tr!("gui.step.count", n = self.m_steps.len(), bytes = steps::record_size(&self.m_steps)),
            );
        });
    }
}
