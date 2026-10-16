//! State of the window and every action that talks to the mouse. The pages (`pages/`) only draw this state
//! and call the actions; all mouse access runs as jobs in the worker thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, ThemePreference};
use m6980x_core::functions::{self as fnc, Function};
use m6980x_core::macro_steps::{self as steps, Step};
use m6980x_core::names::{button_name, rate_hz};
use m6980x_core::pc::{self, OriginalSoftware, WINDOWS_DEFAULT};
use m6980x_core::protocol::{
    BUTTONS, DPI_BUTTON_STAGES, DpiStage, FACTORY_INTERVAL_MS, LED_LEVELS, PROFILES, ROM_SIZE, TiltMode,
};
use m6980x_core::rom::{
    self as romlayout, ICONS_EMBEDDED, MACRO_INFO_BASE, MACRO_INFO_SIZE, MACRO_NAME_LEN, MACRO_SLOT_SIZE, MACRO_SLOTS,
};
use m6980x_core::{Result, backup, i18n, tr};

use crate::icons::{IconPicker, MacroIcons};
use crate::recorder::Recorder;
use crate::theme;
use crate::worker::{Apply, Device, G};

/// mode byte and id; long label in locales/ under `gui.mode.<id>`, short name under `mode.<id>`
pub const MODES: [(u8, &str); 3] = [(fnc::MODE_ONCE, "once"), (fnc::MODE_LOOP, "loop"), (fnc::MODE_HOLD, "fire")];
pub const LANG_KEY: &str = "lang";
pub const DPI_LINKED_KEY: &str = "dpi_linked";
pub const THEME_KEY: &str = "theme";
pub const MIN_DPI: i64 = 90;
const LIVE_INTERVAL: Duration = Duration::from_millis(1500);
const ORIGINAL_CHECK_INTERVAL: Duration = Duration::from_secs(3);
/// the 20 macro info entries (name + icon byte)
const INFO_TABLE: usize = MACRO_SLOTS * MACRO_INFO_SIZE;
/// how long a success message stays in the macro editor
pub const FLASH_TIME: Duration = Duration::from_secs(5);
const ICON_TEXTS: &str = include_str!("../../../assets/icon_descriptions.json");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Buttons,
    Lighting,
    Dpi,
    Performance,
    Macros,
    Backup,
}

#[derive(Clone)]
pub enum Target {
    Standard,
    Function(&'static Function),
    Macro(usize),
}

/// A longer job whose progress is shown (macro read/write).
pub struct Task {
    pub label: String,
    fraction: Arc<Mutex<f32>>,
}

impl Task {
    /// 0.0-1.0, written by the worker thread
    pub fn fraction(&self) -> f32 {
        self.fraction.lock().map_or(0.0, |f| *f)
    }
}

/// Yes/no question; on yes, `action` runs.
pub struct Confirm {
    pub title: String,
    pub text: String,
    pub danger: bool,
    pub action: Box<dyn FnOnce(&mut App)>,
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
    pub dev: Device,
    pub fake: bool,
    pub page: Page,
    pub log: Vec<String>,
    pub status: String,
    pub show_log: bool,
    pub show_settings: bool,
    pub theme: ThemePreference,
    pub persist: bool,
    pub live: bool,
    last_live: Instant,
    pub confirm: Option<Confirm>,
    pub error: Option<(String, String)>,
    icon_texts: HashMap<String, HashMap<u8, String>>,
    /// None until the first job has finished
    pub connected: Option<bool>,
    /// settings have been read at least once
    pub loaded: bool,

    pub name: String,
    /// active profile 1-3; also the profile edited on the buttons page
    pub profile: u8,
    /// active DPI stage 1-3
    pub stage: u8,
    pub rate_ms: u8,
    pub leds: [[u8; 3]; PROFILES],

    pub dpi: [[i64; 2]; DPI_BUTTON_STAGES],
    /// values last read from / written to the mouse (a change is written when the slider is released)
    pub dpi_device: [[i64; 2]; DPI_BUTTON_STAGES],
    /// X and Y DPI are edited together (one value per stage)
    pub dpi_linked: bool,

    pub button_state: Vec<Vec<Option<u16>>>,
    pub selected_button: u8,
    pub picker_search: String,

    pub macro_names: Vec<String>,
    pub macro_rom: Option<Vec<u8>>,
    macros_requested: bool,
    pub selected_macro: Option<usize>,
    pub m_slot: usize,
    pub m_name: String,
    pub m_mode: usize,
    pub m_icon: Option<u8>,
    pub icons: MacroIcons,
    /// open icon dialog
    pub icon_picker: Option<IconPicker>,
    pub m_steps: Vec<Step>,
    pub m_text: String,
    pub m_text_mode: bool,
    pub recorder: Recorder,
    /// running job with progress
    pub task: Option<Task>,
    /// success message of the last macro job and when it came
    pub flash: Option<(String, Instant)>,

    /// Windows lines per wheel notch on the slider
    pub scroll: u32,
    /// value last read from / written to Windows
    pub scroll_pc: u32,
    /// programs of the original software, checked every few seconds
    pub original: OriginalSoftware,
    last_original_check: Instant,
}

pub fn done(f: impl FnOnce(&mut App) + Send + 'static) -> Result<Apply> {
    Ok(Box::new(f))
}

pub fn nothing() -> Result<Apply> {
    done(|_| {})
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

fn stage_dpi(st: &DpiStage) -> [i64; 2] {
    [i64::from(st.x_dpi()), i64::from(st.y_dpi())]
}

/// Windows lines per wheel notch; the Windows default where it cannot be read (other systems).
fn read_scroll() -> u32 {
    pc::scroll_lines().unwrap_or(WINDOWS_DEFAULT)
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, device: Option<String>, fake: bool, lang: Option<String>) -> Self {
        let stored = |key: &str| cc.storage.and_then(|s| s.get_string(key));
        // --lang before the stored choice before environment/system language
        let stored_lang = stored(LANG_KEY).as_deref().and_then(i18n::resolve);
        i18n::set_lang(&lang.or(stored_lang).unwrap_or_else(i18n::detect));
        let theme = stored(THEME_KEY).as_deref().map_or(ThemePreference::Dark, theme::theme_from_id);
        theme::install(&cc.egui_ctx, theme);
        let mut app = Self {
            dev: Device::spawn(cc.egui_ctx.clone(), device, fake),
            fake,
            page: Page::Overview,
            log: Vec::new(),
            status: tr!("gui.connecting"),
            show_log: false,
            show_settings: false,
            theme,
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
            connected: None,
            loaded: false,
            name: String::new(),
            profile: 1,
            stage: 1,
            rate_ms: 2,
            leds: [[0; 3]; PROFILES],
            dpi: [[0; 2]; DPI_BUTTON_STAGES],
            dpi_device: [[0; 2]; DPI_BUTTON_STAGES],
            dpi_linked: stored(DPI_LINKED_KEY).is_none_or(|v| v != "0"),
            button_state: vec![vec![None; BUTTONS]; PROFILES],
            selected_button: 0,
            picker_search: String::new(),
            macro_names: vec![String::new(); MACRO_SLOTS],
            macro_rom: None,
            macros_requested: false,
            selected_macro: None,
            m_slot: 1,
            m_name: String::new(),
            m_mode: 0,
            m_icon: None,
            icons: MacroIcons::default(),
            icon_picker: None,
            m_steps: Vec::new(),
            m_text: String::new(),
            m_text_mode: false,
            recorder: Recorder::default(),
            task: None,
            flash: None,
            scroll: read_scroll(),
            scroll_pc: read_scroll(),
            original: pc::original_software(),
            last_original_check: Instant::now(),
        };
        if fake {
            app.log(tr!("gui.dry_run"));
        }
        app.refresh_all();
        app
    }

    // -- helpers ----------------------------------------------------------------------
    pub fn log(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.log.last() != Some(&text) {
            self.log.push(text.clone());
        }
        self.status = text;
    }

    /// Queue a mouse job; optionally save permanently afterwards (01 02).
    pub fn run(
        &mut self,
        label: impl Into<String>,
        persist: bool,
        job: impl FnOnce(&mut G) -> Result<Apply> + Send + 'static,
    ) {
        let label = label.into();
        self.status = format!("{label} ...");
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

    /// Like `run`, with a progress value (0.0-1.0) the job reports and the window shows.
    pub fn run_tracked(
        &mut self,
        label: impl Into<String>,
        job: impl FnOnce(&mut G, &dyn Fn(f32)) -> Result<Apply> + Send + 'static,
    ) {
        let label = label.into();
        let fraction = Arc::new(Mutex::new(0.0));
        self.task = Some(Task { label: label.clone(), fraction: fraction.clone() });
        self.flash = None;
        self.run(label, false, move |g| {
            let report = |f: f32| {
                if let Ok(mut v) = fraction.lock() {
                    *v = f.clamp(0.0, 1.0);
                }
            };
            job(g, &report)
        });
    }

    pub fn flash(&mut self, text: String) {
        self.log(text.clone());
        self.flash = Some((text, Instant::now()));
    }

    pub fn finish_jobs(&mut self) {
        while let Ok(d) = self.dev.rx.try_recv() {
            self.dev.pending -= 1;
            self.connected = Some(d.connected);
            if self.task.as_ref().is_some_and(|t| t.label == d.label) {
                self.task = None;
            }
            match d.result {
                Err(e) => {
                    self.log(tr!("gui.job_failed", label = d.label, e = e));
                    if !d.quiet {
                        self.error = Some((tr!("gui.error"), format!("{}:\n{e}", d.label)));
                    }
                }
                Ok(apply) => {
                    if !d.quiet {
                        self.log(tr!("gui.job_ok", label = d.label));
                    }
                    apply(self);
                }
            }
        }
    }

    pub fn ask(&mut self, title: impl Into<String>, text: impl Into<String>, action: impl FnOnce(&mut App) + 'static) {
        self.confirm =
            Some(Confirm { title: title.into(), text: text.into(), danger: false, action: Box::new(action) });
    }

    /// Like `ask`, with a red confirm button for actions that delete data.
    pub fn ask_danger(
        &mut self,
        title: impl Into<String>,
        text: impl Into<String>,
        action: impl FnOnce(&mut App) + 'static,
    ) {
        self.confirm = Some(Confirm { title: title.into(), text: text.into(), danger: true, action: Box::new(action) });
    }

    pub fn busy(&self) -> bool {
        !self.dev.idle()
    }

    // -- settings ---------------------------------------------------------------------
    pub fn refresh_all(&mut self) {
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
        self.loaded = true;
        self.name = s.name;
        self.profile = s.profile + 1;
        self.stage = s.stage + 1;
        self.rate_ms = s.interval;
        for (p, rgb) in s.leds.iter().enumerate() {
            self.leds[p] = rgb.map(|l| if LED_LEVELS.contains(&l) { l } else { 0 });
        }
        for (i, st) in s.dpi.iter().enumerate() {
            self.dpi[i] = stage_dpi(st);
        }
        self.dpi_device = self.dpi;
        // the mouse has different X/Y values: show them instead of hiding Y behind the link
        if self.dpi.iter().any(|[x, y]| x != y) {
            self.dpi_linked = false;
        }
        self.macro_names = s.names;
        self.button_state = s.buttons;
    }

    pub fn poll_live(&mut self) {
        if !self.live || !self.dev.idle() || self.last_live.elapsed() < LIVE_INTERVAL {
            return;
        }
        self.last_live = Instant::now();
        if self.connected == Some(false) {
            // try again quietly; reading everything brings the window up to date once the mouse is back
            self.dev.submit(
                tr!("gui.read_settings"),
                true,
                Box::new(|g| {
                    g.name()?;
                    done(|app| app.refresh_all())
                }),
            );
            return;
        }
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

    pub fn set_profile(&mut self, p: u8) {
        self.profile = p;
        self.run(tr!("gui.activate_profile_job", p = p), true, move |g| {
            g.set_active_profile(p - 1)?;
            nothing()
        });
    }

    pub fn set_stage(&mut self, s: u8) {
        self.stage = s;
        self.run(tr!("gui.activate_stage_job", s = s), true, move |g| {
            g.set_active_dpi_stage(s - 1)?;
            nothing()
        });
    }

    pub fn set_rate(&mut self, ms: u8) {
        self.rate_ms = ms;
        self.run(tr!("gui.rate_job", hz = rate_hz(ms)), true, move |g| {
            g.set_report_interval(ms)?;
            nothing()
        });
    }

    /// LED colours need no permanent save: the mouse always keeps them.
    pub fn set_led(&mut self, p: usize, rgb: [u8; 3]) {
        self.leds[p] = rgb;
        let label = tr!("gui.led_job", p = p + 1, rgb = format!("{} {} {}", rgb[0], rgb[1], rgb[2]));
        self.run(label, false, move |g| {
            g.set_led_color(p as u8, rgb)?;
            nothing()
        });
    }

    /// Writes stage `i` if it differs from the mouse; shows the value the mouse rounded to.
    pub fn commit_dpi(&mut self, i: usize) {
        if self.dpi_linked {
            self.dpi[i][1] = self.dpi[i][0];
        }
        if self.dpi[i] == self.dpi_device[i] {
            return;
        }
        let [x, y] = self.dpi[i];
        self.dpi_device[i] = self.dpi[i];
        self.run(tr!("gui.dpi_stage_job", i = i + 1, x = x, y = y), true, move |g| {
            let st = g.set_dpi(i as u8, x, Some(y))?;
            done(move |app| {
                app.dpi[i] = stage_dpi(&st);
                app.dpi_device[i] = app.dpi[i];
            })
        });
    }

    pub fn factory_dpi(&mut self) {
        self.run(tr!("gui.factory_job"), true, |g| {
            let stages = g.apply_factory_defaults()?;
            done(move |app| {
                for (i, st) in stages.iter().take(DPI_BUTTON_STAGES).enumerate() {
                    app.dpi[i] = stage_dpi(st);
                }
                app.dpi_device = app.dpi;
                app.rate_ms = FACTORY_INTERVAL_MS;
            })
        });
    }

    pub fn save_now(&mut self) {
        self.run(tr!("gui.save_permanently"), false, |g| {
            g.save_settings()?;
            nothing()
        });
    }

    // -- PC settings ------------------------------------------------------------------
    /// Which programs of the original software run (Tilt.exe changes the tilt mode, Macro.exe uses the mouse).
    pub fn poll_original(&mut self) {
        if self.last_original_check.elapsed() >= ORIGINAL_CHECK_INTERVAL {
            self.last_original_check = Instant::now();
            self.original = pc::original_software();
        }
    }

    /// Writes the lines per wheel notch to Windows if the slider changed. The dry run (`--fake`) leaves Windows alone.
    pub fn commit_scroll(&mut self) {
        let value = self.scroll;
        if value == self.scroll_pc {
            return;
        }
        let result = if self.fake { Ok(()) } else { pc::set_scroll_lines(value) };
        match result {
            Ok(()) => {
                self.scroll_pc = value;
                self.log(tr!("gui.scroll_lines_set", n = value));
            }
            Err(e) => {
                self.error = Some((tr!("gui.error"), e.to_string()));
                self.scroll = if self.fake { self.scroll_pc } else { read_scroll() };
                self.scroll_pc = self.scroll;
            }
        }
    }

    pub fn scroll_windows_default(&mut self) {
        self.scroll = WINDOWS_DEFAULT;
        self.commit_scroll();
    }

    /// Tilt wheel back to native mode (Tilt.exe of the original switches it to its own mode when it starts).
    pub fn tilt_native(&mut self) {
        self.run(tr!("gui.tilt_native_job"), false, |g| {
            g.set_tilt_mode(TiltMode::Native)?;
            nothing()
        });
    }

    // -- buttons ----------------------------------------------------------------------
    pub fn assign(&mut self, p: u8, b: u8, target: Target, label: String) {
        let bname = button_name(b).unwrap_or_default();
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

    pub fn reset_profile_buttons(&mut self) {
        let p = self.profile - 1;
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

    // -- macros -----------------------------------------------------------------------
    pub fn icon_text(&self, n: u8) -> String {
        if n <= ICONS_EMBEDDED {
            let texts = self.icon_texts.get(&i18n::lang());
            texts.and_then(|m| m.get(&n)).cloned().unwrap_or_else(|| tr!("icon.built_in"))
        } else {
            tr!("gui.custom_icon", n = n - ICONS_EMBEDDED)
        }
    }

    pub fn icon_label(&self, icon: Option<u8>) -> String {
        icon.map_or_else(|| tr!("names.no_icon"), |n| format!("{n}: {}", self.icon_text(n)))
    }

    /// Reads the macros the first time the macro page is shown.
    pub fn ensure_macros(&mut self) {
        if !self.macros_requested {
            self.macros_requested = true;
            self.read_macros();
        }
    }

    pub fn read_macros(&mut self) {
        self.run_tracked(tr!("gui.read_macros"), |g, report| {
            let rom = g.read_rom_progress(0, ROM_SIZE, &mut |done, total| report(done as f32 / total as f32))?;
            done(move |app| app.show_macros(rom))
        });
    }

    /// Puts what was read back after a macro job into the cached memory: the slot (if written) and the info table.
    fn patch_macros(&mut self, slot: Option<(usize, Vec<u8>)>, info: Vec<u8>) {
        let Some(mut rom) = self.macro_rom.take() else {
            self.read_macros();
            return;
        };
        if let Some((s, data)) = slot {
            let a = usize::from(romlayout::macro_slot_addr(s));
            rom[a..a + data.len()].copy_from_slice(&data);
        }
        let a = usize::from(MACRO_INFO_BASE);
        rom[a..a + info.len()].copy_from_slice(&info);
        self.show_macros(rom);
    }

    /// Runs a small macro job (icon, delete), then reads the info table back.
    fn macro_info_job(
        &mut self,
        label: String,
        message: String,
        job: impl FnOnce(&mut G) -> Result<()> + Send + 'static,
    ) {
        self.run_tracked(label, move |g, report| {
            job(g)?;
            report(0.5);
            let info = g.read_rom(MACRO_INFO_BASE, INFO_TABLE)?;
            done(move |app| {
                app.patch_macros(None, info);
                app.flash(message);
            })
        });
    }

    /// Event bytes of the editor (steps, or the text while the text view is open).
    pub fn editor_events(&self) -> Result<Vec<u8>> {
        if self.m_text_mode { fnc::encode_events(&self.m_text) } else { Ok(steps::events_from_steps(&self.m_steps)) }
    }

    pub fn editor_record(&self) -> Result<Vec<u8>> {
        let mut record = vec![MODES[self.m_mode].0];
        record.extend(self.editor_events()?);
        record.push(fnc::EV_END);
        Ok(record)
    }

    pub fn steps_text(steps: &[Step]) -> String {
        let mut record = vec![fnc::MODE_ONCE];
        record.extend(steps::events_from_steps(steps));
        record.push(fnc::EV_END);
        fnc::decode_record(&record).1.join("\n")
    }

    /// Switch between step list and text; stays in the text view if the text has an error.
    pub fn set_text_mode(&mut self, text: bool) -> Result<()> {
        if text == self.m_text_mode {
            return Ok(());
        }
        if text {
            self.m_text = Self::steps_text(&self.m_steps);
        } else {
            self.m_steps = steps::steps_from_events(&fnc::encode_events(&self.m_text)?);
        }
        self.m_text_mode = text;
        Ok(())
    }

    pub fn set_editor(&mut self, record: &[u8]) {
        let (mode, _) = fnc::decode_record(record);
        let value = mode.as_deref().and_then(fnc::mode_value);
        self.m_mode = MODES.iter().position(|(v, _)| Some(*v) == value).unwrap_or(0);
        self.m_steps = if record.is_empty() { Vec::new() } else { steps::steps_from_events(&record[1..]) };
        self.m_text = Self::steps_text(&self.m_steps);
    }

    pub fn new_macro(&mut self, slot: usize) {
        self.selected_macro = Some(slot);
        self.m_slot = slot + 1;
        self.m_name.clear();
        self.m_icon = None;
        self.m_mode = 0;
        self.m_steps.clear();
        self.m_text.clear();
    }

    fn show_macros(&mut self, rom: Vec<u8>) {
        self.macro_names = (0..MACRO_SLOTS).map(|s| romlayout::macro_name(romlayout::macro_info(&rom, s))).collect();
        self.macro_rom = Some(rom);
        if self.selected_macro.is_none() {
            self.load_macro(0);
        }
    }

    pub fn load_macro(&mut self, s: usize) {
        let Some(rom) = self.macro_rom.clone() else {
            return;
        };
        let info = romlayout::macro_info(&rom, s);
        self.selected_macro = Some(s);
        self.m_slot = s + 1;
        self.m_name = romlayout::macro_name(info);
        let record = romlayout::used_macro_slot(&rom, s);
        self.m_icon = if record.is_empty() { None } else { romlayout::macro_icon(info) };
        self.m_text_mode = false;
        self.set_editor(record);
    }

    pub fn write_macro(&mut self) {
        let record = match self.editor_record() {
            Ok(r) => r,
            Err(e) => {
                self.error = Some((tr!("gui.macro"), e.to_string()));
                return;
            }
        };
        if record.len() > MACRO_SLOT_SIZE {
            self.error = Some((tr!("gui.macro"), tr!("gui.too_long_error", max = MACRO_SLOT_SIZE)));
            return;
        }
        // a macro needs a name on the mouse; without one it is numbered after its slot
        if self.m_name.trim().is_empty() {
            self.m_name = self.default_macro_name();
        }
        let (slot, name, icon) = (self.m_slot - 1, self.m_name.clone(), self.m_icon);
        let label = tr!("gui.write_macro_job", slot = slot + 1, name = name, count = record.len());
        let message = tr!("gui.macro_written", slot = slot + 1, name = name);
        self.run_tracked(label, move |g, report| {
            // writing + verifying the record, then reading back the slot and the info table
            let written = 2 * record.len();
            let total = (written + MACRO_SLOT_SIZE + INFO_TABLE) as f32;
            g.write_macro_progress(slot, &record, &name, icon, &mut |d, _| report(d as f32 / total))?;
            let addr = romlayout::macro_slot_addr(slot);
            let data = g.read_rom_progress(addr, MACRO_SLOT_SIZE, &mut |d, _| report((written + d) as f32 / total))?;
            let base = written + MACRO_SLOT_SIZE;
            let info =
                g.read_rom_progress(MACRO_INFO_BASE, INFO_TABLE, &mut |d, _| report((base + d) as f32 / total))?;
            done(move |app| {
                app.patch_macros(Some((slot, data)), info);
                app.selected_macro = Some(slot);
                app.flash(message);
            })
        });
    }

    /// Name used when the name field is empty: "Macro 3" for slot 3.
    pub fn default_macro_name(&self) -> String {
        tr!("gui.default_macro_name", n = self.m_slot).chars().take(MACRO_NAME_LEN).collect()
    }

    pub fn set_macro_icon_only(&mut self) {
        let (slot, icon) = (self.m_slot - 1, self.m_icon);
        let message = tr!("gui.icon_saved", slot = slot + 1);
        self.macro_info_job(tr!("gui.icon_job", slot = slot + 1), message, move |g| g.set_macro_icon(slot, icon));
    }

    pub fn delete_macro(&mut self) {
        let slot = self.m_slot - 1;
        self.ask_danger(tr!("gui.delete_macro"), tr!("gui.delete_macro_question", slot = slot + 1), move |app| {
            let message = tr!("gui.macro_deleted", slot = slot + 1);
            app.macro_info_job(tr!("gui.delete_macro_n", slot = slot + 1), message, move |g| g.clear_macro(slot));
        });
    }

    /// Like the import in the original's editor: fills mode and events; the file has no name.
    pub fn import_macro_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.macro_file_filter"), &["ini"]).pick_file() else {
            return;
        };
        match backup::read_macro_ini(&path, MACRO_SLOT_SIZE) {
            Ok(record) => {
                self.m_text_mode = false;
                self.set_editor(&record);
                if self.m_name.is_empty() {
                    self.m_name = file_stem(&path).chars().take(MACRO_NAME_LEN).collect();
                }
                self.log(tr!("gui.macro_file_loaded", path = path.display(), count = record.len()));
            }
            Err(e) => self.error = Some((tr!("gui.macro_file"), e.to_string())),
        }
    }

    pub fn export_macro_file(&mut self) {
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

    /// Adds recorded steps; stops when the slot is full.
    pub fn add_recorded(&mut self, new: Vec<Step>) {
        if new.is_empty() {
            return;
        }
        self.m_steps.extend(new);
        if steps::record_size(&self.m_steps) > MACRO_SLOT_SIZE {
            self.recorder.stop();
            steps::fit(&mut self.m_steps, MACRO_SLOT_SIZE);
            self.log(tr!("gui.record_full"));
        }
    }

    pub fn start_recording(&mut self) {
        if self.m_text_mode
            && let Err(e) = self.set_text_mode(false)
        {
            self.error = Some((tr!("gui.macro"), e.to_string()));
            return;
        }
        self.recorder.start();
        self.log(tr!("gui.record_started"));
    }

    pub fn stop_recording(&mut self) {
        let rest = self.recorder.stop();
        self.m_steps.extend(rest);
        self.log(tr!("gui.record_stopped", count = self.m_steps.len()));
    }

    // -- backup -----------------------------------------------------------------------
    pub fn do_backup(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.m6980x_backup"), &["json"]).save_file() else {
            return;
        };
        let path = with_extension(path, "json");
        self.run(tr!("gui.backup_to", path = path.display()), false, move |g| {
            backup::save(&backup::create(g, &mut |_, _| {})?, &path)?;
            nothing()
        });
    }

    pub fn do_restore(&mut self) {
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
                    app.macros_requested = false;
                })
            });
        });
    }

    pub fn do_rom_export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("gui.meb_filter"), &["meb"])
            .add_filter(tr!("gui.raw_data"), &["bin"])
            .save_file()
        else {
            return;
        };
        let path = with_extension(path, "meb");
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

    pub fn do_rom_import(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr!("gui.memory"), &["meb", "bin"]).pick_file() else {
            return;
        };
        let data = match backup::load_rom_file(&path) {
            Ok(d) => d,
            Err(e) => {
                self.error = Some((tr!("gui.import"), e.to_string()));
                return;
            }
        };
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let question = tr!("gui.import_question", file = file);
        self.ask(tr!("gui.import"), question, move |app| {
            app.run(tr!("gui.import_memory"), false, move |g| {
                backup::import_rom(g, &data, &mut |_| {})?;
                done(|app| {
                    app.refresh_all();
                    app.macros_requested = false;
                })
            });
        });
    }

    pub fn release_all(&mut self) {
        self.ask_danger(tr!("gui.release"), tr!("gui.release_question"), |app| {
            app.run(tr!("gui.release_all"), false, |g| {
                g.release_all()?;
                done(|app| {
                    app.refresh_all();
                    app.macros_requested = false;
                })
            });
        });
    }

    // -- dialogs ----------------------------------------------------------------------
    pub fn ui_dialogs(&mut self, ctx: &egui::Context) {
        if let Some((title, text)) = &self.error {
            let mut close = false;
            let modal = egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(egui_phosphor::regular::WARNING).size(22.0).color(theme::DANGER));
                    ui.heading(title);
                });
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
                ui.set_max_width(420.0);
                ui.heading(&c.title);
                ui.label(&c.text);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let yes = if c.danger {
                        crate::widgets::danger_button(ui, tr!("gui.yes"))
                    } else {
                        crate::widgets::accent_button(ui, tr!("gui.yes"))
                    };
                    if yes.clicked() {
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
