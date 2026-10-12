//! m6980x_configtool: Kommandozeile des M6980X Configuration Tool (Maus Gigabyte GHOST M6980X).
//!
//! Alle Texte (auch die Hilfe) kommen aus locales/*.yml in der Sprache von `--lang`, `M6980X_LANG` oder des Systems.

mod help;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use m6980x_core::functions::{self as fnc, FUNCTIONS};
use m6980x_core::i18n::{self, fold};
use m6980x_core::names::{
    self, describe_target, format_icon, format_led, is_standard_word, parse_button, parse_icon, parse_led, py_repr,
    rate_hz,
};
use m6980x_core::protocol::{
    BUTTONS, DPI_BUTTON_STAGES, FACTORY_INTERVAL_MS, M6980X, PROFILES, ROM_SIZE, SUPPORTED_MODEL,
};
use m6980x_core::rom::{self as romlayout, MACRO_NAME_LEN, MACRO_SLOT_SIZE};
use m6980x_core::{Error, Result, Transport, backup, open_transport, tr};
use serde_json::json;
use sha2::{Digest, Sha256};

type G = M6980X<Box<dyn Transport>>;

const RATE_CHOICES: [&str; 8] = ["1000", "500", "333", "250", "200", "166", "142", "125"];

/// Hilfetexte: locales/*.yml unter cli.about, cli.cmd.*, cli.arg.*, cli.opt.* (siehe help.rs).
#[derive(Parser)]
#[command(name = "m6980x_configtool", version)]
struct Cli {
    #[arg(long, global = true)]
    device: Option<String>,
    #[arg(long, global = true, value_parser = parse_lang)]
    lang: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Gui,
    Info {
        #[arg(long)]
        json: bool,
    },
    Profile {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        number: Option<u8>,
        #[arg(long, alias = "temporaer")]
        temporary: bool,
    },
    Led {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        profile: Option<u8>,
        color: Vec<String>,
    },
    Dpi {
        #[command(subcommand)]
        action: Option<DpiCmd>,
    },
    Watch {
        #[arg(long, default_value_t = 30.0)]
        seconds: f64,
    },
    Rate {
        #[arg(value_parser = PossibleValuesParser::new(RATE_CHOICES))]
        hz: Option<String>,
        #[arg(long, alias = "temporaer")]
        temporary: bool,
    },
    Buttons,
    Button {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        profile: u8,
        button: String,
        target: String,
    },
    Functions,
    Macro {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        name: String,
        #[arg(required = true, num_args = 1..)]
        events: Vec<String>,
        #[arg(long, default_value = "once", value_parser = parse_mode)]
        mode: u8,
        #[arg(long, alias = "bild", value_name = "1-100", value_parser = clap::value_parser!(u8).range(1..=100))]
        icon: Option<u8>,
    },
    MacroExport {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        file: PathBuf,
    },
    MacroImport {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        file: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, alias = "bild", value_name = "1-100", value_parser = clap::value_parser!(u8).range(1..=100))]
        icon: Option<u8>,
    },
    #[command(alias = "macro-bild")]
    MacroIcon {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        icon: Option<String>,
    },
    Save,
    Backup {
        file: PathBuf,
    },
    Restore {
        file: PathBuf,
    },
    RomDump {
        file: PathBuf,
    },
    RomImport {
        file: PathBuf,
    },
    Macros {
        #[arg(long)]
        rom: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DpiCmd {
    Select {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        stage: u8,
        #[arg(long, alias = "temporaer")]
        temporary: bool,
    },
    Set {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        stage: u8,
        x_dpi: i64,
        y_dpi: Option<i64>,
        #[arg(long, alias = "temporaer")]
        temporary: bool,
    },
    Default {
        #[arg(long, alias = "temporaer")]
        temporary: bool,
    },
}

fn parse_mode(text: &str) -> std::result::Result<u8, String> {
    fnc::mode_value(text).ok_or_else(|| tr!("cli.expected_one_of", names = fnc::mode_names().join(", ")))
}

fn parse_lang(text: &str) -> std::result::Result<String, String> {
    i18n::resolve(text).ok_or_else(|| tr!("cli.expected_one_of", names = i18n::available().join(", ")))
}

/// `--lang` muss vor dem Parsen bekannt sein, damit Hilfe und Fehlermeldungen schon in der richtigen Sprache kommen.
fn lang_from_args() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--" {
            break;
        }
        if a == "--lang" {
            return args.next().as_deref().and_then(i18n::resolve);
        }
        if let Some(v) = a.strip_prefix("--lang=") {
            return i18n::resolve(v);
        }
    }
    None
}

fn parse_cli() -> Cli {
    let matches = help::localize(Cli::command()).get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

fn main() -> ExitCode {
    i18n::set_lang(&lang_from_args().unwrap_or_else(i18n::detect));
    let cli = parse_cli();
    if let Cmd::Led { color, .. } = &cli.command
        && !color.is_empty()
        && let Err(e) = parse_led(color)
    {
        clap::Error::raw(clap::error::ErrorKind::InvalidValue, format!("{e}\n")).exit();
    }
    let result = match &cli.command {
        Cmd::Gui => launch_gui(cli.device.as_deref(), cli.lang.as_deref()),
        Cmd::Functions => {
            cmd_functions();
            Ok(())
        }
        Cmd::Macros { rom: Some(path) } => backup::load_rom_file(path).map(|data| print_macros(&data)),
        _ => with_device(&cli),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{} {e}", tr!("cli.error"));
            ExitCode::FAILURE
        }
    }
}

fn with_device(cli: &Cli) -> Result<()> {
    let mut g = M6980X::new(open_transport(cli.device.as_deref())?);
    if !matches!(cli.command, Cmd::Info { .. }) {
        g.check_model()?; // info bleibt nutzbar, um zu sehen, welches Modell steckt
    }
    run(&mut g, &cli.command)
}

fn run(g: &mut G, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Info { json } => cmd_info(g, *json),
        Cmd::Profile { number, temporary } => {
            if let Some(n) = number {
                g.set_active_profile(n - 1)?;
                persist(g, *temporary)?;
            }
            println!("{} {}", tr!("cli.active_profile"), g.active_profile()? + 1);
            Ok(())
        }
        Cmd::Led { profile, color } => {
            if !color.is_empty() {
                let Some(p) = profile else { unreachable!("Farbe ohne Profil parst clap nicht") };
                g.set_led_color(p - 1, parse_led(color)?)?;
            }
            let profiles: Vec<u8> = match profile {
                Some(p) => vec![p - 1],
                None => (0..PROFILES as u8).collect(),
            };
            for p in profiles {
                println!("{}: {}", tr!("cli.led_profile", p = p + 1), format_led(g.led_color(p)?));
            }
            Ok(())
        }
        Cmd::Dpi { action } => cmd_dpi(g, action.as_ref()),
        Cmd::Watch { seconds } => cmd_watch(g, *seconds),
        Cmd::Rate { hz, temporary } => {
            if let Some(hz) = hz {
                let ms = names::interval_for_hz(hz.parse().expect("Auswahl")).expect("Auswahl");
                g.set_report_interval(ms)?;
                persist(g, *temporary)?;
            }
            let ms = g.report_interval()?;
            println!("{}", tr!("cli.report_rate_line", hz = rate_hz(ms), ms = ms));
            Ok(())
        }
        Cmd::Buttons => cmd_buttons(g),
        Cmd::Button { profile, button, target } => cmd_button(g, *profile, button, target),
        Cmd::Macro { slot, name, events, mode, icon } => cmd_macro(g, *slot, name, events, *mode, *icon),
        Cmd::MacroExport { slot, file } => {
            let data = g.read_rom(romlayout::macro_slot_addr(usize::from(slot - 1)), MACRO_SLOT_SIZE)?;
            let record = &data[..fnc::record_length(&data)];
            backup::write_macro_ini(record, file)?;
            let (mode, events) = fnc::decode_record(record);
            println!(
                "{}",
                tr!(
                    "cli.macro_exported",
                    slot = slot,
                    file = file.display(),
                    count = record.len(),
                    into = mode.unwrap_or("None".into()),
                    events = events.join(", ")
                )
            );
            Ok(())
        }
        Cmd::MacroImport { slot, file, name, icon } => {
            let record = backup::read_macro_ini(file, MACRO_SLOT_SIZE)?;
            let name = name.clone().unwrap_or_else(|| file_stem(file));
            let name: String = name.chars().take(MACRO_NAME_LEN).collect();
            g.write_macro(usize::from(slot - 1), &record, &name, *icon)?;
            let (mode, events) = fnc::decode_record(&record);
            println!(
                "{}",
                tr!(
                    "cli.macro_imported",
                    file = file.display(),
                    slot = slot,
                    name = py_repr(&name),
                    count = record.len(),
                    into = mode.unwrap_or("None".into()),
                    events = events.join(", ")
                )
            );
            Ok(())
        }
        Cmd::MacroIcon { slot, icon } => {
            let slot = usize::from(slot - 1);
            if let Some(i) = icon {
                g.set_macro_icon(slot, parse_icon(i)?)?;
            }
            println!("{}: {}", tr!("names.macro", slot = slot + 1), format_icon(g.macro_icon(slot)?));
            Ok(())
        }
        Cmd::Save => {
            g.save_settings()?;
            println!("{}", tr!("cli.saved"));
            Ok(())
        }
        Cmd::Backup { file } => {
            let data = backup::create(g, &mut |_, _| {})?;
            backup::save(&data, file)?;
            println!(
                "{}",
                tr!(
                    "cli.backup_done",
                    file = file.display(),
                    active_profile = data.active_profile + 1,
                    report_interval_ms = rate_hz(data.report_interval_ms),
                    count = data.rom.len() / 2
                )
            );
            Ok(())
        }
        Cmd::Restore { file } => {
            let data = backup::load(file)?;
            println!("{}", tr!("cli.restoring", created = data.created));
            let left = backup::restore(g, &data, &mut |m| println!("{m}"))?;
            g.save_settings()?;
            if left.is_empty() {
                println!("{}", tr!("cli.restore_done"));
            } else {
                print_left(&left);
            }
            Ok(())
        }
        Cmd::RomDump { file } => {
            let data = read_full_rom(g)?;
            if is_meb(file) {
                backup::write_meb(&data, file)?; // Format der Original-Software
            } else {
                std::fs::write(file, &data)?;
            }
            println!("{} {} -> {}", data.len(), tr!("cli.bytes"), file.display());
            println!("SHA-256 {}", sha256(&data));
            Ok(())
        }
        Cmd::RomImport { file } => {
            let data = backup::load_rom_file(file)?;
            let left = backup::import_rom(g, &data, &mut |m| println!("{m}"))?;
            println!("{}", tr!("cli.imported", file = file.display(), data = &sha256(&data)[..8]));
            if !left.is_empty() {
                print_left(&left);
            }
            println!("{}", tr!("cli.rom_import_note"));
            Ok(())
        }
        Cmd::Macros { .. } => {
            let data = read_full_rom(g)?;
            print_macros(&data);
            Ok(())
        }
        Cmd::Gui | Cmd::Functions => unreachable!("ohne Geraet behandelt"),
    }
}

fn file_stem(path: &std::path::Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn is_meb(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("meb"))
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

fn print_left(left: &[(usize, usize)]) {
    println!("{}", tr!("cli.firmware_differences"));
    for (s, e) in left {
        println!("  0x{s:04X}-0x{e:04X}");
    }
}

/// Abtastrate, DPI, aktives Profil/Stufe gehen beim Abstecken verloren, wenn sie nicht mit 01 02 gespeichert werden (getestet).
fn persist(g: &mut G, temporary: bool) -> Result<()> {
    if temporary {
        println!("{}", tr!("cli.not_saved"));
        Ok(())
    } else {
        g.save_settings()
    }
}

fn cmd_info(g: &mut G, as_json: bool) -> Result<()> {
    let ident = g.identify()?;
    let name = g.name()?;
    let profile = g.active_profile()?;
    let stage = g.active_dpi_stage()?;
    let reg0a = g.register_0a()?;
    let interval = g.report_interval()?;
    let steps = g.dpi_supported_steps()?;
    let stages = (0..DPI_BUTTON_STAGES as u8).map(|i| g.dpi_stage(i)).collect::<Result<Vec<_>>>()?;
    let leds = (0..PROFILES as u8).map(|p| g.led_color(p)).collect::<Result<Vec<_>>>()?;
    if as_json {
        let stages_json: Vec<_> = stages
            .iter()
            .map(|s| {
                json!({"x_step": s.x_step, "y_step": s.y_step, "unit": s.unit, "x_num": s.x_num, "x_den": s.x_den,
                       "y_num": s.y_num, "y_den": s.y_den, "x_dpi": s.x_dpi(), "y_dpi": s.y_dpi()})
            })
            .collect();
        let info = json!({
            "identify": ident, "name": name, "active_profile": profile, "active_dpi_stage": stage,
            "register_0a": reg0a, "report_interval_ms": interval, "dpi_supported_steps": steps,
            "dpi_stages": stages_json, "led_colors": leds,
        });
        println!("{}", serde_json::to_string_pretty(&info).expect("JSON"));
        return Ok(());
    }
    let ok = if ident.signature_ok { "OK".to_string() } else { tr!("cli.wrong") };
    println!("{:<19}{name}  ({} {ok}: {})", tr!("cli.device"), tr!("cli.signature"), ident.raw);
    let model = ident.model.map_or_else(|| tr!("common.unknown"), str::to_string);
    let note = if model == SUPPORTED_MODEL {
        String::new()
    } else {
        tr!("cli.model_not_supported", supported_model = SUPPORTED_MODEL)
    };
    println!("{:<19}{model}{note}", tr!("cli.model"));
    println!("{:<19}{}", tr!("cli.active_profile"), profile + 1);
    println!("{:<19}{}   {}", tr!("cli.active_dpi_stage"), stage + 1, tr!("cli.stage_note"));
    println!("{:<19}0x{reg0a:04X}   {}", tr!("cli.register_0a"), tr!("cli.register_0a_note"));
    println!("{:<19}{} Hz   ({} {interval} ms)", tr!("cli.report_rate"), rate_hz(interval), tr!("cli.interval"));
    let steps_label = tr!("cli.dpi_steps");
    match (steps.first(), steps.last()) {
        (Some(a), Some(b)) => {
            println!("{steps_label:<19}{} {} ({a}..{b})", steps.len(), tr!("cli.supported"))
        }
        _ => println!("{steps_label:<19}{}", tr!("cli.none")),
    }
    for (i, s) in stages.iter().enumerate() {
        println!(
            "{:<19}X {:5}  Y {:5}   ({} {}/{} x {}, {} {}/{} {}/{})",
            tr!("cli.dpi_stage_label", i = i + 1),
            s.x_dpi(),
            s.y_dpi(),
            tr!("cli.step"),
            s.x_step,
            s.y_step,
            s.unit,
            tr!("cli.fraction"),
            s.x_num,
            s.x_den,
            s.y_num,
            s.y_den
        );
    }
    for (p, rgb) in leds.iter().enumerate() {
        println!("{:<19}{}", tr!("cli.led_profile_label", p = p + 1), format_led(*rgb));
    }
    Ok(())
}

fn cmd_dpi(g: &mut G, action: Option<&DpiCmd>) -> Result<()> {
    match action {
        Some(DpiCmd::Select { stage, temporary }) => {
            g.set_active_dpi_stage(stage - 1)?;
            persist(g, *temporary)?;
        }
        Some(DpiCmd::Set { stage, x_dpi, y_dpi, temporary }) => {
            g.set_dpi(stage - 1, *x_dpi, *y_dpi)?;
            persist(g, *temporary)?;
        }
        Some(DpiCmd::Default { temporary }) => {
            g.apply_factory_defaults()?;
            persist(g, *temporary)?;
            println!("{} {} Hz", tr!("cli.report_rate"), rate_hz(FACTORY_INTERVAL_MS));
        }
        None => {}
    }
    let active = g.active_dpi_stage()?;
    for i in 0..DPI_BUTTON_STAGES as u8 {
        let s = g.dpi_stage(i)?;
        let marks = if i == active { tr!("cli.active_mark") } else { String::new() };
        let axes =
            if s.x_dpi() == s.y_dpi() { s.x_dpi().to_string() } else { format!("X {} / Y {}", s.x_dpi(), s.y_dpi()) };
        println!("{} {axes:>18} DPI{marks}", tr!("cli.stage_label", i = i + 1));
    }
    Ok(())
}

/// Nur lesend: jede Aenderung von aktivem Profil / Register 07 ausgeben (z. B. beim Druecken der Tasten).
fn cmd_watch(g: &mut G, seconds: f64) -> Result<()> {
    let end = Instant::now() + Duration::from_secs_f64(seconds.max(0.0));
    let mut last = None;
    println!("{}", tr!("cli.watching", seconds = seconds));
    while Instant::now() < end {
        let now = (g.active_profile()?, g.active_dpi_stage()?);
        if last != Some(now) {
            let time = chrono::Local::now().format("%H:%M:%S");
            println!("{time}  {}", tr!("cli.watch_line", profile = now.0 + 1, stage = now.1 + 1, register = now.1));
            last = Some(now);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn cmd_buttons(g: &mut G) -> Result<()> {
    let names = g.macro_names()?;
    let header: String = (0..PROFILES).map(|p| format!("  {:<32}", tr!("cli.profile", p = p + 1))).collect();
    println!("{:<14} {header}", tr!("cli.button"));
    for b in 0..BUTTONS as u8 {
        let cells = (0..PROFILES as u8)
            .map(|p| g.button(p, b).map(|a| describe_target(a, &names, Some(b))))
            .collect::<Result<Vec<_>>>()?;
        let label = format!("{b} {}", names::button_name(b).unwrap_or_default());
        let row: String = cells.iter().map(|c| format!("  {c:<32}")).collect();
        println!("{label:14} {row}");
    }
    Ok(())
}

fn cmd_button(g: &mut G, profile: u8, button: &str, target: &str) -> Result<()> {
    let (p, button) = (profile - 1, parse_button(button)?);
    let folded = fold(target);
    if is_standard_word(target) {
        g.reset_button(p, button)?;
    } else if folded.starts_with("makro") || folded.starts_with("macro") {
        let slot = match target.split_once(':') {
            Some((_, n)) => {
                n.trim().parse::<i64>().map_err(|_| Error::Invalid(tr!("cli.invalid_macro_number", n = n)))? - 1
            }
            None => -1,
        };
        if !(0..romlayout::MACRO_SLOTS as i64).contains(&slot) {
            return Err(Error::Invalid(tr!("cli.macro_target_range", macro_slots = romlayout::MACRO_SLOTS)));
        }
        g.assign_macro(p, button, slot as usize)?;
    } else {
        let Some(f) = fnc::by_key(target) else {
            return Err(Error::Invalid(tr!("cli.unknown_function")));
        };
        if !f.confirmed {
            println!("{}", tr!("cli.function_derived", name = f.name()));
        }
        g.assign_function(p, button, f)?;
    }
    let names = g.macro_names()?;
    println!(
        "{}: {}",
        tr!("cli.profile_button", profile = profile, button = button),
        describe_target(g.button(p, button)?, &names, Some(button))
    );
    Ok(())
}

fn cmd_functions() {
    for f in FUNCTIONS.iter() {
        let state = if f.size == 0 {
            tr!("cli.only_as_default")
        } else if f.confirmed {
            tr!("cli.confirmed")
        } else if !f.record.is_empty() {
            tr!("cli.derived")
        } else {
            tr!("common.unknown")
        };
        println!("{:20} {:46} 0x{:04X}  {state}", f.name(), f.label(), f.address);
    }
}

fn read_full_rom(g: &mut G) -> Result<Vec<u8>> {
    if !std::io::stderr().is_terminal() {
        return g.read_rom(0, ROM_SIZE);
    }
    let label = tr!("cli.reading_rom");
    let data = g.read_rom_progress(0, ROM_SIZE, &mut |done, total| {
        eprint!("\r  {label}: {:3} %", done * 100 / total);
    })?;
    eprintln!();
    Ok(data)
}

fn print_macros(data: &[u8]) {
    for s in 0..romlayout::MACRO_SLOTS {
        let slot = romlayout::macro_slot(data, s);
        let (mode, events) = fnc::decode_record(slot);
        let info = romlayout::macro_info(data, s);
        let name = romlayout::macro_name(info);
        if mode.is_none() && name.is_empty() {
            continue;
        }
        let events = if events.is_empty() { tr!("cli.no_events") } else { events.join(", ") };
        println!(
            "{:9} {:12} {:30} {:3}/{MACRO_SLOT_SIZE} B  {} {:9} {events}",
            tr!("cli.macro_column", s = format!("{:2}", s + 1)),
            py_repr(&name),
            format_icon(romlayout::macro_icon(info)),
            fnc::record_length(slot),
            tr!("cli.mode"),
            mode.as_deref().unwrap_or("-"),
        );
    }
}

fn cmd_macro(g: &mut G, slot: u8, name: &str, events: &[String], mode: u8, icon: Option<u8>) -> Result<()> {
    let mut record = vec![mode];
    record.extend(fnc::encode_events(&events.join(" "))?);
    record.push(fnc::EV_END);
    let slot = usize::from(slot - 1);
    g.write_macro(slot, &record, name, icon)?;
    let (mode, events) = fnc::decode_record(&g.read_rom(romlayout::macro_slot_addr(slot), record.len())?);
    println!(
        "{}",
        tr!(
            "cli.macro_written",
            slot = slot + 1,
            name = py_repr(name),
            count = record.len(),
            macro_slot_size = MACRO_SLOT_SIZE,
            slot2 = format_icon(g.macro_icon(slot)?),
            into = mode.unwrap_or("None".into()),
            events = events.join(", ")
        )
    );
    Ok(())
}

/// Startet die Oberflaeche (m6980x_configtool_gui), die neben dieser Datei liegt.
fn launch_gui(device: Option<&str>, lang: Option<&str>) -> Result<()> {
    let exe = std::env::current_exe()?;
    let gui = exe.with_file_name(format!("m6980x_configtool_gui{}", std::env::consts::EXE_SUFFIX));
    if !gui.exists() {
        return Err(Error::Invalid(tr!("cli.gui_not_found", gui = gui.display())));
    }
    let mut cmd = std::process::Command::new(gui);
    if let Some(d) = device {
        cmd.args(["--device", d]);
    }
    if let Some(l) = lang {
        cmd.args(["--lang", l]);
    }
    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn every_command_and_argument_has_help() {
        let missing = help::missing(Cli::command());
        assert!(missing.is_empty(), "ohne Hilfetext in locales/en.yml: {missing:#?}");
    }

    #[test]
    fn german_and_english_arguments_parse() {
        let cli =
            Cli::try_parse_from(["m6980x_configtool", "macro", "1", "X", "tap", "a", "--mode", "schleife"]).unwrap();
        assert!(matches!(cli.command, Cmd::Macro { mode: fnc::MODE_LOOP, .. }));
        let cli = Cli::try_parse_from(["m6980x_configtool", "macro-bild", "1", "keins"]).unwrap();
        assert!(matches!(cli.command, Cmd::MacroIcon { .. }));
        let cli = Cli::try_parse_from(["m6980x_configtool", "rate", "500", "--temporaer"]).unwrap();
        assert!(matches!(cli.command, Cmd::Rate { temporary: true, .. }));
    }
}
