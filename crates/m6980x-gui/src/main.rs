//! M6980X Configuration Tool - GUI (egui) with all features of the tool (Windows + Linux).
//!
//!     m6980x_configtool_gui [--device PATH] [--fake] [--lang en|de]
//!
//! `--fake` runs against a simulated mouse (dry run without a device). The language can be switched in the
//! window and is remembered; without a stored choice, `--lang`, `M6980X_LANG` or the system language applies.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod worker;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use m6980x_core::functions::{self as fnc, FUNCTIONS, Function};
use m6980x_core::i18n;
use m6980x_core::names::{button_names, describe_target, format_icon, led_names, rate_hz, rates, standard_label};
use m6980x_core::protocol::{
    BUTTONS, DPI_BUTTON_STAGES, DpiStage, FACTORY_INTERVAL_MS, LED_LEVELS, MAX_DPI, PROFILES, ROM_SIZE,
};
use m6980x_core::rom::{self as romlayout, ICON_COUNT, ICONS_EMBEDDED, MACRO_NAME_LEN, MACRO_SLOT_SIZE, MACRO_SLOTS};
use m6980x_core::{Result, backup, tr};
use worker::{Apply, Device, G};

/// mode byte and id; label in locales/ under `gui.mode.<id>`
const MODES: [(u8, &str); 3] = [(fnc::MODE_ONCE, "once"), (fnc::MODE_LOOP, "loop"), (fnc::MODE_HOLD, "fire")];
const LANG_KEY: &str = "lang";
const DPI_LINKED_KEY: &str = "dpi_linked";
const LIVE_INTERVAL: Duration = Duration::from_millis(1500);
const ICON_TEXTS: &str = include_str!("../../../assets/icon_descriptions.json");

fn level_to_8bit(level: u8) -> u8 {
    match level {
        0 => 0x00,
        1 => 0x80,
        _ => 0xFF,
    }
}

/// Icon descriptions per language ("en" is the image AI prompt but works as a short description);
/// if a language is missing in the file, English is used.
fn load_icon_texts(lang: &str) -> HashMap<u8, String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(ICON_TEXTS) else {
        return HashMap::new();
    };
    json["macro_pictures"]["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| {
                    let text = i[lang].as_str().or_else(|| i[i18n::DEFAULT].as_str())?;
                    Some((u8::try_from(i["index"].as_u64()?).ok()?, text.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Dpi,
    Buttons,
    Macros,
    Backup,
}

#[derive(Clone)]
enum Target {
    Standard,
    Function(&'static Function),
    Macro(usize),
}

/// Yes/no question; on yes, `action` runs.
struct Confirm {
    title: String,
    text: String,
    action: Box<dyn FnOnce(&mut App)>,
}

struct Snapshot {
    name: String,
    profile: u8,
    stage: u8,
    interval: u8,
    dpi: Vec<DpiStage>,
    leds: Vec<[u8; 3]>,
    buttons: Vec<Vec<Option<u16>>>,
    names: Vec<String>,
}

pub struct App {
    dev: Device,
    tab: Tab,
    log: Vec<String>,
    status: String,
    persist: bool,
    live: bool,
    last_live: Instant,
    confirm: Option<Confirm>,
    error: Option<(String, String)>,
    icon_texts: HashMap<String, HashMap<u8, String>>,

    info: String,
    profile: u8,
    stage: u8,
    rate_ms: u8,
    leds: [[u8; 3]; PROFILES],

    dpi: [[i64; 2]; DPI_BUTTON_STAGES],
    /// X and Y DPI are edited together (one value per stage)
    dpi_linked: bool,

    btn_profile: u8,
    button_state: Vec<Vec<Option<u16>>>,
    btn_choice: [usize; BUTTONS],

    macro_names: Vec<String>,
    macro_rom: Option<Vec<u8>>,
    selected_macro: Option<usize>,
    m_slot: usize,
    m_name: String,
    m_mode: usize,
    m_icon: Option<u8>,
    m_events: String,
}

fn done(f: impl FnOnce(&mut App) + Send + 'static) -> Result<Apply> {
    Ok(Box::new(f))
}

fn nothing() -> Result<Apply> {
    done(|_| {})
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, device: Option<String>, fake: bool, lang: Option<String>) -> Self {
        // --lang before the stored choice before environment/system language
        let stored = cc.storage.and_then(|s| s.get_string(LANG_KEY)).as_deref().and_then(i18n::resolve);
        i18n::set_lang(&lang.or(stored).unwrap_or_else(i18n::detect));
        let mut app = Self {
            dev: Device::spawn(cc.egui_ctx.clone(), device, fake),
            tab: Tab::Overview,
            log: Vec::new(),
            status: tr!("gui.connecting"),
            persist: true,
            live: true,
            last_live: Instant::now(),
            confirm: None,
            error: None,
            icon_texts: i18n::available()
                .into_iter()
                .map(|l| {
                    let texts = load_icon_texts(&l);
                    (l, texts)
                })
                .collect(),
            info: "-".into(),
            profile: 1,
            stage: 1,
            rate_ms: 2,
            leds: [[0; 3]; PROFILES],
            dpi: [[0; 2]; DPI_BUTTON_STAGES],
            dpi_linked: cc.storage.and_then(|s| s.get_string(DPI_LINKED_KEY)).is_none_or(|v| v != "0"),
            btn_profile: 1,
            button_state: vec![vec![None; BUTTONS]; PROFILES],
            btn_choice: [0; BUTTONS],
            macro_names: vec![String::new(); MACRO_SLOTS],
            macro_rom: None,
            selected_macro: None,
            m_slot: 1,
            m_name: String::new(),
            m_mode: 0,
            m_icon: None,
            m_events: String::new(),
        };
        if fake {
            app.log(tr!("gui.dry_run"));
        }
        app.refresh_all();
        app
    }

    // -- helpers ----------------------------------------------------------------------
    fn log(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.log.last() != Some(&text) {
            self.log.push(text.clone());
        }
        self.status = text;
    }

    /// Queue a mouse job; optionally save permanently afterwards (01 02).
    fn run(
        &mut self,
        label: impl Into<String>,
        persist: bool,
        job: impl FnOnce(&mut G) -> Result<Apply> + Send + 'static,
    ) {
        let label = label.into();
        self.status = format!("... {label}");
        let save = persist && self.persist;
        self.dev.submit(
            label,
            false,
            Box::new(move |g| {
                let apply = job(g)?;
                if save {
                    g.save_settings()?;
                }
                Ok(apply)
            }),
        );
    }

    fn finish_jobs(&mut self) {
        while let Ok(d) = self.dev.rx.try_recv() {
            self.dev.pending -= 1;
            match d.result {
                Err(e) => {
                    self.log(tr!("gui.job_failed", label = d.label, e = e));
                    if !d.quiet {
                        self.error = Some((tr!("gui.error"), format!("{}:\n{e}", d.label)));
                    }
                }
                Ok(apply) => {
                    if !d.quiet {
                        self.log(format!("OK: {}", d.label));
                    }
                    apply(self);
                }
            }
        }
    }

    fn ask(&mut self, title: impl Into<String>, text: impl Into<String>, action: impl FnOnce(&mut App) + 'static) {
        self.confirm = Some(Confirm { title: title.into(), text: text.into(), action: Box::new(action) });
    }

    // -- overview ---------------------------------------------------------------------
    fn refresh_all(&mut self) {
        self.run(tr!("gui.read_settings"), false, |g| {
            let mut buttons = Vec::new();
            for p in 0..PROFILES as u8 {
                buttons.push((0..BUTTONS as u8).map(|b| g.button(p, b)).collect::<Result<Vec<_>>>()?);
            }
            let s = Snapshot {
                name: g.name()?,
                profile: g.active_profile()?,
                stage: g.active_dpi_stage()?,
                interval: g.report_interval()?,
                dpi: (0..DPI_BUTTON_STAGES as u8).map(|i| g.dpi_stage(i)).collect::<Result<_>>()?,
                leds: (0..PROFILES as u8).map(|p| g.led_color(p)).collect::<Result<_>>()?,
                buttons,
                names: g.macro_names()?,
            };
            done(move |app| app.show_all(s))
        });
    }

    fn show_all(&mut self, s: Snapshot) {
        self.info = tr!(
            "gui.info_line",
            name = s.name,
            profile = s.profile + 1,
            stage = s.stage + 1,
            interval = rate_hz(s.interval)
        );
        self.profile = s.profile + 1;
        self.stage = s.stage + 1;
        self.rate_ms = s.interval;
        for (p, rgb) in s.leds.iter().enumerate() {
            self.leds[p] = rgb.map(|l| if LED_LEVELS.contains(&l) { l } else { 0 });
        }
        for (i, st) in s.dpi.iter().enumerate() {
            self.dpi[i] = [i64::from(st.x_dpi()), i64::from(st.y_dpi())];
        }
        // the mouse has different X/Y values: show them instead of hiding Y behind the link
        if self.dpi.iter().any(|[x, y]| x != y) {
            self.dpi_linked = false;
        }
        self.macro_names = s.names;
        self.button_state = s.buttons;
    }

    fn poll_live(&mut self) {
        if !self.live || !self.dev.idle() || self.last_live.elapsed() < LIVE_INTERVAL {
            return;
        }
        self.last_live = Instant::now();
        self.dev.submit(
            "Live".into(),
            true,
            Box::new(|g| {
                let (p, st) = (g.active_profile()?, g.active_dpi_stage()?);
                done(move |app| {
                    if (p + 1, st + 1) != (app.profile, app.stage) {
                        app.profile = p + 1;
                        app.stage = st + 1;
                        app.log(tr!("gui.mouse_changed", p = p + 1, st = st + 1));
                    }
                })
            }),
        );
    }

    fn ui_overview(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(&self.info).strong());
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(tr!("gui.active_profile"));
                    for p in 1..=PROFILES as u8 {
                        if ui.radio_value(&mut self.profile, p, tr!("cli.profile", p = p)).clicked() {
                            self.run(tr!("gui.activate_profile_job", p = p), true, move |g| {
                                g.set_active_profile(p - 1)?;
                                nothing()
                            });
                        }
                    }
                });
            });
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(tr!("gui.active_dpi_stage"));
                    for s in 1..=DPI_BUTTON_STAGES as u8 {
                        if ui.radio_value(&mut self.stage, s, tr!("gui.stage_radio", s = s)).clicked() {
                            self.run(tr!("gui.activate_stage_job", s = s), true, move |g| {
                                g.set_active_dpi_stage(s - 1)?;
                                nothing()
                            });
                        }
                    }
                });
            });
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(tr!("gui.report_rate"));
                    egui::ComboBox::from_id_salt("rate")
                        .selected_text(format!("{} Hz", rate_hz(self.rate_ms)))
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for (hz, ms) in rates() {
                                ui.selectable_value(&mut self.rate_ms, ms, format!("{hz} Hz"));
                            }
                        });
                    if ui.button(tr!("gui.set")).clicked() {
                        let ms = self.rate_ms;
                        self.run(tr!("gui.rate_job", hz = rate_hz(ms)), true, move |g| {
                            g.set_report_interval(ms)?;
                            nothing()
                        });
                    }
                });
            });
        });

        ui.add_space(6.0);
        ui.group(|ui| {
            ui.label(tr!("gui.led_heading"));
            egui::Grid::new("leds").num_columns(6).spacing([8.0, 6.0]).show(ui, |ui| {
                for p in 0..PROFILES {
                    ui.label(tr!("cli.profile", p = p + 1));
                    let [r, gr, b] = self.leds[p].map(level_to_8bit);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(r, gr, b));
                    ui.horizontal(|ui| {
                        for (c, name) in ["R", "G", "B"].iter().enumerate() {
                            ui.label(*name);
                            egui::ComboBox::from_id_salt(("led", p, c))
                                .selected_text(self.leds[p][c].to_string())
                                .width(40.0)
                                .show_ui(ui, |ui| {
                                    for level in LED_LEVELS {
                                        ui.selectable_value(&mut self.leds[p][c], level, level.to_string());
                                    }
                                });
                        }
                    });
                    let current =
                        led_names().into_iter().find(|(_, v)| *v == self.leds[p]).map_or(tr!("gui.preset"), |(n, _)| n);
                    egui::ComboBox::from_id_salt(("preset", p)).selected_text(current).width(110.0).show_ui(ui, |ui| {
                        for (name, rgb) in led_names() {
                            if ui.selectable_label(self.leds[p] == rgb, name).clicked() {
                                self.leds[p] = rgb;
                            }
                        }
                    });
                    if ui.button(tr!("gui.set")).clicked() {
                        let rgb = self.leds[p];
                        self.run(tr!("gui.led_job", p = p + 1, rgb = format!("{rgb:?}")), false, move |g| {
                            g.set_led_color(p as u8, rgb)?;
                            nothing()
                        });
                    }
                    ui.end_row();
                }
            });
        });

        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr!("gui.read_everything_again")).clicked() {
                self.refresh_all();
            }
            if ui.button(tr!("gui.save_now")).clicked() {
                self.run(tr!("gui.save_permanently"), false, |g| {
                    g.save_settings()?;
                    nothing()
                });
            }
            ui.checkbox(&mut self.persist, tr!("gui.auto_save"));
            ui.checkbox(&mut self.live, tr!("gui.live"));
        });
    }

    // -- DPI ---------------------------------------------------------------------------
    fn ui_dpi(&mut self, ui: &mut egui::Ui) {
        ui.label(tr!("gui.dpi_note", max_dpi = MAX_DPI));
        ui.add_space(6.0);
        if ui.checkbox(&mut self.dpi_linked, tr!("gui.dpi_linked")).changed() && self.dpi_linked {
            for [x, y] in &mut self.dpi {
                *y = *x;
            }
        }
        ui.add_space(4.0);
        let linked = self.dpi_linked;
        egui::Grid::new("dpi").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
            for i in 0..DPI_BUTTON_STAGES {
                ui.label(tr!("gui.stage_row", i = i + 1));
                for (axis, label) in ["X", "Y"].iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(*label);
                        let field =
                            egui::DragValue::new(&mut self.dpi[i][axis]).range(90..=i64::from(MAX_DPI)).speed(10);
                        // while linked, Y follows X and cannot be edited on its own
                        ui.add_enabled(!(linked && axis == 1), field);
                    });
                }
                if linked {
                    self.dpi[i][1] = self.dpi[i][0];
                }
                if ui.button(tr!("gui.set")).clicked() {
                    let [x, y] = self.dpi[i];
                    self.run(tr!("gui.dpi_stage_job", i = i + 1, x = x, y = y), true, move |g| {
                        let st = g.set_dpi(i as u8, x, Some(y))?;
                        done(move |app| app.dpi[i] = [i64::from(st.x_dpi()), i64::from(st.y_dpi())])
                    });
                }
                ui.end_row();
            }
        });
        ui.add_space(10.0);
        if ui.button(tr!("gui.factory_button")).clicked() {
            self.run(tr!("gui.factory_job"), true, |g| {
                let stages = g.apply_factory_defaults()?;
                done(move |app| {
                    for (i, st) in stages.iter().enumerate() {
                        app.dpi[i] = [i64::from(st.x_dpi()), i64::from(st.y_dpi())];
                    }
                    app.rate_ms = FACTORY_INTERVAL_MS;
                })
            });
        }
    }

    // -- buttons ----------------------------------------------------------------------
    fn targets(&self) -> Vec<(String, Target)> {
        let mut list = vec![(tr!("names.default"), Target::Standard)];
        list.extend(
            FUNCTIONS
                .iter()
                .filter(|f| f.size > 0 && !f.record.is_empty())
                .map(|f| (f.label().to_string(), Target::Function(f))),
        );
        list.extend(self.macro_names.iter().enumerate().map(|(s, n)| {
            let label = if n.is_empty() {
                tr!("cli.macro_column", s = s + 1)
            } else {
                tr!("gui.macro_named", s = s + 1, n = n)
            };
            (label, Target::Macro(s))
        }));
        list
    }

    fn ui_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr!("gui.profile"));
            for p in 1..=PROFILES as u8 {
                ui.radio_value(&mut self.btn_profile, p, p.to_string());
            }
            ui.add_space(12.0);
            if ui.button(tr!("gui.reset_profile_button")).clicked() {
                let p = self.btn_profile - 1;
                let question = tr!("gui.reset_profile_question", p = p + 1);
                self.ask(tr!("names.default"), question, move |app| {
                    app.run(tr!("gui.reset_profile_job", p = p + 1), false, move |g| {
                        for b in 0..BUTTONS as u8 {
                            g.reset_button(p, b)?;
                        }
                        done(move |app| app.button_state[usize::from(p)] = vec![None; BUTTONS])
                    });
                });
            }
        });
        ui.add_space(6.0);
        let targets = self.targets();
        let p = self.btn_profile - 1;
        egui::Grid::new("buttons").num_columns(4).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for (b, bname) in button_names() {
                ui.label(format!("{b}  {bname}"));
                let current =
                    describe_target(self.button_state[usize::from(p)][usize::from(b)], &self.macro_names, Some(b));
                ui.allocate_ui_with_layout(
                    egui::vec2(260.0, 18.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| ui.add(egui::Label::new(current).truncate()),
                );
                let label_of = |i: usize| {
                    if i == 0 { standard_label(Some(b)) } else { targets[i].0.clone() }
                };
                let choice = &mut self.btn_choice[usize::from(b)];
                egui::ComboBox::from_id_salt(("target", b)).selected_text(label_of(*choice)).width(320.0).show_ui(
                    ui,
                    |ui| {
                        for i in 0..targets.len() {
                            ui.selectable_value(choice, i, label_of(i));
                        }
                    },
                );
                if ui.button(tr!("gui.assign")).clicked() {
                    let target = targets[self.btn_choice[usize::from(b)]].1.clone();
                    self.assign(p, b, target, label_of(self.btn_choice[usize::from(b)]));
                }
                ui.end_row();
            }
        });
    }

    fn assign(&mut self, p: u8, b: u8, target: Target, label: String) {
        let bname = button_names().into_iter().find(|(n, _)| *n == b).map_or_else(String::new, |(_, n)| n);
        self.run(tr!("gui.assign_job", p = p + 1, bname = bname, label = label), false, move |g| {
            let state = match target {
                Target::Standard => {
                    g.reset_button(p, b)?;
                    None
                }
                Target::Function(f) => {
                    g.assign_function(p, b, f)?;
                    Some(fnc::placement(f, usize::from(b)).0)
                }
                Target::Macro(s) => {
                    g.assign_macro(p, b, s)?;
                    Some(romlayout::macro_slot_addr(s))
                }
            };
            done(move |app| app.button_state[usize::from(p)][usize::from(b)] = state)
        });
    }

    // -- macros -----------------------------------------------------------------------
    fn icon_text(&self, n: u8) -> String {
        if n <= ICONS_EMBEDDED {
            let texts = self.icon_texts.get(&i18n::lang());
            texts.and_then(|m| m.get(&n)).cloned().unwrap_or_else(|| tr!("icon.built_in"))
        } else {
            tr!("gui.custom_icon", n = n - ICONS_EMBEDDED)
        }
    }

    fn icon_label(&self, icon: Option<u8>) -> String {
        icon.map_or_else(|| tr!("names.no_icon"), |n| format!("{n}: {}", self.icon_text(n)))
    }

    fn editor_record(&self) -> Result<Vec<u8>> {
        let mut record = vec![MODES[self.m_mode].0];
        record.extend(fnc::encode_events(&self.m_events)?);
        record.push(fnc::EV_END);
        Ok(record)
    }

    fn set_editor(&mut self, record: &[u8]) {
        let (mode, events) = fnc::decode_record(record);
        let value = mode.as_deref().and_then(fnc::mode_value);
        self.m_mode = MODES.iter().position(|(v, _)| Some(*v) == value).unwrap_or(0);
        self.m_events = events.join("\n");
    }

    fn read_full_rom_then_show(&mut self, label: String, job: impl FnOnce(&mut G) -> Result<()> + Send + 'static) {
        self.run(label, false, move |g| {
            job(g)?;
            let rom = g.read_rom(0, ROM_SIZE)?;
            done(move |app| app.show_macros(rom))
        });
    }

    fn show_macros(&mut self, rom: Vec<u8>) {
        self.macro_names = (0..MACRO_SLOTS).map(|s| romlayout::macro_name(romlayout::macro_info(&rom, s))).collect();
        self.macro_rom = Some(rom);
    }

    fn load_macro(&mut self, s: usize) {
        let Some(rom) = self.macro_rom.clone() else {
            return;
        };
        let info = romlayout::macro_info(&rom, s);
        self.selected_macro = Some(s);
        self.m_slot = s + 1;
        self.m_name = romlayout::macro_name(info);
        let record = romlayout::used_macro_slot(&rom, s);
        self.m_icon = if record.is_empty() { None } else { romlayout::macro_icon(info) };
        self.set_editor(record);
    }

    fn ui_macros(&mut self, ui: &mut egui::Ui) {
        let mut clicked = None;
        egui::ScrollArea::vertical().id_salt("macro_table").max_height(220.0).show(ui, |ui| {
            egui::Grid::new("macros").num_columns(6).striped(true).spacing([12.0, 3.0]).show(ui, |ui| {
                let headers = ["no", "name", "icon", "mode", "bytes", "events"].map(|c| tr!(&format!("gui.col.{c}")));
                for h in headers {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                if let Some(rom) = &self.macro_rom {
                    for s in 0..MACRO_SLOTS {
                        let slot = romlayout::used_macro_slot(rom, s);
                        let info = romlayout::macro_info(rom, s);
                        let (mode, events) = fnc::decode_record(slot);
                        let selected = self.selected_macro == Some(s);
                        if ui.selectable_label(selected, (s + 1).to_string()).clicked() {
                            clicked = Some(s);
                        }
                        ui.label(romlayout::macro_name(info));
                        ui.label(if slot.is_empty() { "-".into() } else { format_icon(romlayout::macro_icon(info)) });
                        ui.label(mode.unwrap_or_else(|| "-".into()));
                        ui.label(fnc::record_length(slot).to_string());
                        ui.add(egui::Label::new(events.join(", ")).truncate());
                        ui.end_row();
                    }
                }
            });
        });
        if let Some(s) = clicked {
            self.load_macro(s);
        }
        if ui.button(tr!("gui.read_macros_button")).clicked() {
            self.read_full_rom_then_show(tr!("gui.read_macros"), |_| Ok(()));
        }

        ui.add_space(6.0);
        ui.group(|ui| {
            ui.label(RichText::new("Editor").strong());
            ui.horizontal(|ui| {
                ui.label("Slot");
                ui.add(egui::DragValue::new(&mut self.m_slot).range(1..=MACRO_SLOTS));
                ui.label(tr!("gui.name_max_9"));
                ui.add(egui::TextEdit::singleline(&mut self.m_name).char_limit(MACRO_NAME_LEN).desired_width(90.0));
                ui.label(tr!("gui.mode"));
                let mode_label = |id: &str| tr!(&format!("gui.mode.{id}"));
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(mode_label(MODES[self.m_mode].1))
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for (i, (_, id)) in MODES.iter().enumerate() {
                            ui.selectable_value(&mut self.m_mode, i, mode_label(id));
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label(tr!("gui.icon"));
                let selected = self.icon_label(self.m_icon);
                let options: Vec<(Option<u8>, String)> =
                    std::iter::once(None).chain((1..=ICON_COUNT).map(Some)).map(|i| (i, self.icon_label(i))).collect();
                egui::ComboBox::from_id_salt("icon").selected_text(selected).width(480.0).height(300.0).show_ui(
                    ui,
                    |ui| {
                        for (icon, label) in options {
                            ui.selectable_value(&mut self.m_icon, icon, label);
                        }
                    },
                );
            });
            ui.add(
                egui::TextEdit::multiline(&mut self.m_events)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(7)
                    .desired_width(f32::INFINITY),
            );
            match self.editor_record() {
                Ok(r) => {
                    let too_long = if r.len() > MACRO_SLOT_SIZE { tr!("gui.too_long") } else { String::new() };
                    ui.label(format!("{} / {MACRO_SLOT_SIZE} Bytes{too_long}", r.len()));
                }
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("{} {e}", tr!("cli.error")));
                }
            }
            ui.label(RichText::new(tr!("gui.event_help")).monospace().weak());
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr!("gui.write_to_mouse")).clicked() {
                    self.write_macro();
                }
                if ui.button(tr!("gui.set_icon_only")).clicked() {
                    let (slot, icon) = (self.m_slot - 1, self.m_icon);
                    self.read_full_rom_then_show(tr!("gui.icon_job", slot = slot + 1), move |g| {
                        g.set_macro_icon(slot, icon)
                    });
                }
                if ui.button(tr!("gui.delete_macro_button")).clicked() {
                    let slot = self.m_slot - 1;
                    self.ask(tr!("gui.delete_macro"), tr!("gui.delete_macro_question", slot = slot + 1), move |app| {
                        app.read_full_rom_then_show(tr!("gui.delete_macro_n", slot = slot + 1), move |g| {
                            g.clear_macro(slot)
                        });
                    });
                }
                ui.add_space(24.0);
                if ui.button(tr!("gui.load_file")).clicked() {
                    self.import_macro_file();
                }
                if ui.button(tr!("gui.save_file")).clicked() {
                    self.export_macro_file();
                }
            });
        });
    }

    fn write_macro(&mut self) {
        let record = match self.editor_record() {
            Ok(r) => r,
            Err(e) => {
                self.error = Some((tr!("gui.macro"), e.to_string()));
                return;
            }
        };
        let (slot, name, icon) = (self.m_slot - 1, self.m_name.clone(), self.m_icon);
        let label = tr!("gui.write_macro_job", slot = slot + 1, name = name, count = record.len());
        self.read_full_rom_then_show(label, move |g| g.write_macro(slot, &record, &name, icon));
    }

    /// Like the import in the original's editor: fills mode and events; the file has no name.
    fn import_macro_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.macro_file_filter"), &["ini"]).pick_file() else {
            return;
        };
        match backup::read_macro_ini(&path, MACRO_SLOT_SIZE) {
            Ok(record) => {
                self.set_editor(&record);
                if self.m_name.is_empty() {
                    self.m_name = file_stem(&path).chars().take(MACRO_NAME_LEN).collect();
                }
                self.log(tr!("gui.macro_file_loaded", path = path.display(), count = record.len()));
            }
            Err(e) => self.error = Some((tr!("gui.macro_file"), e.to_string())),
        }
    }

    fn export_macro_file(&mut self) {
        let record = match self.editor_record() {
            Ok(r) => r,
            Err(e) => {
                self.error = Some((tr!("gui.macro"), e.to_string()));
                return;
            }
        };
        let default = format!("{}.ini", if self.m_name.is_empty() { "default" } else { &self.m_name });
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("gui.macro_file_filter"), &["ini"])
            .set_file_name(default)
            .save_file()
        else {
            return;
        };
        match backup::write_macro_ini(&record, &path) {
            Ok(()) => self.log(tr!("gui.macro_saved", path = path.display(), count = record.len())),
            Err(e) => self.error = Some((tr!("gui.macro_file"), e.to_string())),
        }
    }

    // -- backup -----------------------------------------------------------------------
    fn ui_backup(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;
        let backup = tr!("gui.backup_button");
        if ui.button(backup).clicked() {
            self.do_backup();
        }
        if ui.button(tr!("gui.restore_button")).clicked() {
            self.do_restore();
        }
        let export = tr!("gui.export_memory");
        if ui.button(export).clicked() {
            self.do_rom_export();
        }
        if ui.button(tr!("gui.import_button")).clicked() {
            self.do_rom_import();
        }
        let release = tr!("gui.release_button");
        if ui.button(release).clicked() {
            self.ask(tr!("gui.release"), tr!("gui.release_question"), |app| {
                app.run(tr!("gui.release_all"), false, |g| {
                    g.release_all()?;
                    done(|app| app.refresh_all())
                });
            });
        }
    }

    fn do_backup(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.m6980x_backup"), &["json"]).save_file() else {
            return;
        };
        let path = with_extension(path, "json");
        self.run(tr!("gui.backup_to", path = path.display()), false, move |g| {
            backup::save(&backup::create(g, &mut |_, _| {})?, &path)?;
            nothing()
        });
    }

    fn do_restore(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.m6980x_backup"), &["json"]).pick_file() else {
            return;
        };
        let data = match backup::load(&path) {
            Ok(d) => d,
            Err(e) => {
                self.error = Some((tr!("gui.restore"), e.to_string()));
                return;
            }
        };
        let question = tr!("gui.restore_question", created = data.created);
        self.ask(tr!("gui.restore"), question, move |app| {
            app.run(tr!("gui.restore_backup"), false, move |g| {
                let left = backup::restore(g, &data, &mut |_| {})?;
                g.save_settings()?;
                done(move |app| {
                    if left.is_empty() {
                        app.log(tr!("gui.rom_byte_identical"));
                    } else {
                        app.log(tr!("gui.remaining_differences", left = format!("{left:?}")));
                    }
                    app.refresh_all();
                })
            });
        });
    }

    fn do_rom_export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("gui.meb_filter"), &["meb"])
            .add_filter(tr!("gui.raw_data"), &["bin"])
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_none() { path.with_extension("meb") } else { path };
        self.run(tr!("gui.memory_to", path = path.display()), false, move |g| {
            let data = g.read_rom(0, ROM_SIZE)?;
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("meb")) {
                backup::write_meb(&data, &path)?;
            } else {
                std::fs::write(&path, &data)?;
            }
            nothing()
        });
    }

    fn do_rom_import(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.memory"), &["meb", "bin"]).pick_file() else {
            return;
        };
        let data = match backup::load_rom_file(&path) {
            Ok(d) => d,
            Err(e) => {
                self.error = Some(("Import".into(), e.to_string()));
                return;
            }
        };
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let question = tr!("gui.import_question", file = file);
        self.ask("Import", question, move |app| {
            app.run(tr!("gui.import_memory"), false, move |g| {
                backup::import_rom(g, &data, &mut |_| {})?;
                done(|app| app.refresh_all())
            });
        });
    }

    // -- dialogs ----------------------------------------------------------------------
    fn ui_dialogs(&mut self, ctx: &egui::Context) {
        if let Some((title, text)) = &self.error {
            let mut close = false;
            let modal = egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
                ui.heading(title);
                ui.label(text);
                ui.add_space(8.0);
                close = ui.button("OK").clicked();
            });
            if close || modal.should_close() {
                self.error = None;
            }
        }
        if let Some(c) = &self.confirm {
            let mut answer = None;
            let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
                ui.heading(&c.title);
                ui.label(&c.text);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr!("gui.yes")).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(tr!("gui.no")).clicked() {
                        answer = Some(false);
                    }
                });
            });
            if modal.should_close() && answer.is_none() {
                answer = Some(false);
            }
            if let Some(yes) = answer {
                let c = self.confirm.take().expect("question open");
                if yes {
                    (c.action)(self);
                }
            }
        }
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn with_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().is_none() { path.with_extension(ext) } else { path }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(LANG_KEY, i18n::lang());
        storage.set_string(DPI_LINKED_KEY, if self.dpi_linked { "1" } else { "0" }.into());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.finish_jobs();
        self.poll_live();
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(500));

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        egui::Panel::bottom("log").resizable(true).default_size(140.0).show(ui, |ui| {
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for line in &self.log {
                    ui.monospace(line);
                }
            });
        });
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (Tab::Overview, tr!("gui.tab_overview")),
                    (Tab::Dpi, "DPI".to_string()),
                    (Tab::Buttons, tr!("gui.tab_buttons")),
                    (Tab::Macros, tr!("gui.tab_macros")),
                    (Tab::Backup, tr!("gui.tab_backup")),
                ] {
                    ui.selectable_value(&mut self.tab, tab, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut lang = i18n::lang();
                    egui::ComboBox::from_id_salt("lang").selected_text(i18n::native_name(&lang)).show_ui(ui, |ui| {
                        for l in i18n::available() {
                            let name = i18n::native_name(&l);
                            ui.selectable_value(&mut lang, l, name);
                        }
                    });
                    if lang != i18n::lang() {
                        i18n::set_lang(&lang);
                        self.refresh_all(); // rebuild the info line and the texts read from the mouse
                    }
                });
            });
        });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("page").show(ui, |ui| match self.tab {
                Tab::Overview => self.ui_overview(ui),
                Tab::Dpi => self.ui_dpi(ui),
                Tab::Buttons => self.ui_buttons(ui),
                Tab::Macros => self.ui_macros(ui),
                Tab::Backup => self.ui_backup(ui),
            });
        });
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
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "M6980X Configuration Tool",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, device, fake, lang)))),
    )
}
