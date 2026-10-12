//! m6980x_configtool: Kommandozeile des M6980X Configuration Tool (Maus Gigabyte GHOST M6980X).

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand};
use m6980x_core::functions::{self as fnc, FUNCTIONS};
use m6980x_core::names::{
    self, describe_target, format_icon, format_led, parse_button, parse_icon, parse_led, py_repr, rate_hz,
};
use m6980x_core::protocol::{
    BUTTONS, DPI_BUTTON_STAGES, FACTORY_INTERVAL_MS, M6980X, PROFILES, ROM_SIZE, SUPPORTED_MODEL,
};
use m6980x_core::rom::{self as romlayout, MACRO_NAME_LEN, MACRO_SLOT_SIZE};
use m6980x_core::{Error, Result, Transport, backup, open_transport};
use serde_json::json;
use sha2::{Digest, Sha256};

type G = M6980X<Box<dyn Transport>>;

const RATE_CHOICES: [&str; 8] = ["1000", "500", "333", "250", "200", "166", "142", "125"];

#[derive(Parser)]
#[command(name = "m6980x_configtool", version, about = "M6980X Configuration Tool (Gigabyte GHOST M6980X)")]
struct Cli {
    /// Geraetepfad (hidapi-Pfad oder /dev/hidrawN)
    #[arg(long, global = true)]
    device: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// grafische Oberflaeche starten
    Gui,
    /// Geraet, Profil, DPI und LED anzeigen
    Info {
        #[arg(long)]
        json: bool,
    },
    /// aktives Profil anzeigen oder wechseln
    Profile {
        /// Profil 1-3
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        number: Option<u8>,
        /// nur bis zum Abstecken, nicht dauerhaft speichern
        #[arg(long)]
        temporaer: bool,
    },
    /// LED-Farben anzeigen oder setzen, z. B. 'led 1 rot' oder 'led 1 2 0 1'
    Led {
        /// Profil 1-3
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        profile: Option<u8>,
        /// Farbname oder R G B mit Stufen 0 (aus), 1 (mittel), 2 (voll)
        color: Vec<String>,
    },
    /// DPI-Stufen anzeigen, auswaehlen oder setzen
    Dpi {
        #[command(subcommand)]
        action: Option<DpiCmd>,
    },
    /// Profil und DPI-Stufe live beobachten (nur lesend)
    Watch {
        #[arg(long, default_value_t = 30.0)]
        seconds: f64,
    },
    /// Abtastrate anzeigen oder setzen
    Rate {
        /// Hz
        #[arg(value_parser = PossibleValuesParser::new(RATE_CHOICES))]
        hz: Option<String>,
        /// nur bis zum Abstecken, nicht dauerhaft speichern
        #[arg(long)]
        temporaer: bool,
    },
    /// Tastenbelegung aller Profile anzeigen
    Buttons,
    /// Taste belegen, z. B. 'button 1 hinten kopieren' oder 'button 1 vorne makro:3'
    Button {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        profile: u8,
        /// links, rechts, mitte, hinten, vorne, kipp-links, kipp-rechts, rad-hoch, rad-runter oder 0-9
        button: String,
        /// Funktion (siehe 'functions'), makro:N oder standard
        target: String,
    },
    /// belegbare Funktionen auflisten
    Functions,
    /// Makro schreiben, z. B. 'macro 5 Test tippe strg+c, warte 100, klick links'
    Macro {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        /// max. 9 Zeichen
        name: String,
        /// drueck/los/tippe TASTE[+TASTE], warte MS, maus-drueck/maus-los/klick links|rechts|mitte
        #[arg(required = true, num_args = 1..)]
        events: Vec<String>,
        #[arg(long, default_value = "einmal", value_parser = PossibleValuesParser::new(["einmal", "schleife", "feuer"]))]
        mode: String,
        /// Vorschaubild: 1-40 eingebaut, 41-100 eigene bmp-Dateien; ohne Angabe bleibt das vorhandene
        #[arg(long, value_name = "1-100", value_parser = clap::value_parser!(u8).range(1..=100))]
        bild: Option<u8>,
    },
    /// Makro als Datei im Format der Original-Software (.ini) speichern
    MacroExport {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        file: PathBuf,
    },
    /// Makrodatei der Original-Software (.ini) in einen Slot schreiben
    MacroImport {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        file: PathBuf,
        /// max. 9 Zeichen; ohne Angabe der Dateiname (die Datei enthaelt keinen Namen)
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_name = "1-100", value_parser = clap::value_parser!(u8).range(1..=100))]
        bild: Option<u8>,
    },
    /// Vorschaubild eines Makros anzeigen oder setzen ('keins' entfernt es)
    MacroBild {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=20))]
        slot: u8,
        /// 1-100 oder keins
        bild: Option<String>,
    },
    /// Abtastrate/DPI dauerhaft speichern (sonst beim Abstecken verloren)
    Save,
    /// komplette Sicherung (ROM + DPI, LED, Abtastrate, Tasten) als JSON
    Backup { file: PathBuf },
    /// Sicherung zurueckschreiben (nur Abweichungen)
    Restore { file: PathBuf },
    /// 8-KB-ROM in Datei sichern (.meb = Format der Original-Software, sonst roh)
    RomDump { file: PathBuf },
    /// 8-KB-ROM aus .meb (Original-Software) oder .bin zurueckschreiben
    RomImport { file: PathBuf },
    /// Makro-Slots und Info-Tabelle anzeigen
    Macros {
        /// ROM-Datei statt Maus lesen
        #[arg(long)]
        rom: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DpiCmd {
    /// aktive DPI-Stufe waehlen
    Select {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        stage: u8,
        /// nur bis zum Abstecken, nicht dauerhaft speichern
        #[arg(long)]
        temporaer: bool,
    },
    /// DPI einer Stufe setzen (Vielfache von 90, max. 6030)
    Set {
        #[arg(value_parser = clap::value_parser!(u8).range(1..=3))]
        stage: u8,
        /// DPI (X und Y, wenn kein Y-Wert folgt)
        x_dpi: i64,
        /// eigener Y-Wert
        y_dpi: Option<i64>,
        /// nur bis zum Abstecken, nicht dauerhaft speichern
        #[arg(long)]
        temporaer: bool,
    },
    /// Werkswerte 810/1620/3240 und 500 Hz wie der Default-Knopf des Originals
    Default {
        /// nur bis zum Abstecken, nicht dauerhaft speichern
        #[arg(long)]
        temporaer: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Cmd::Led { color, .. } = &cli.command
        && !color.is_empty()
        && let Err(e) = parse_led(color)
    {
        clap::Error::raw(clap::error::ErrorKind::InvalidValue, format!("{e}\n")).exit();
    }
    let result = match &cli.command {
        Cmd::Gui => launch_gui(cli.device.as_deref()),
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
            eprintln!("Fehler: {e}");
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
        Cmd::Profile { number, temporaer } => {
            if let Some(n) = number {
                g.set_active_profile(n - 1)?;
                persist(g, *temporaer)?;
            }
            println!("Aktives Profil: {}", g.active_profile()? + 1);
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
                println!("LED Profil {}: {}", p + 1, format_led(g.led_color(p)?));
            }
            Ok(())
        }
        Cmd::Dpi { action } => cmd_dpi(g, action.as_ref()),
        Cmd::Watch { seconds } => cmd_watch(g, *seconds),
        Cmd::Rate { hz, temporaer } => {
            if let Some(hz) = hz {
                let ms = names::interval_for_hz(hz.parse().expect("Auswahl")).expect("Auswahl");
                g.set_report_interval(ms)?;
                persist(g, *temporaer)?;
            }
            let ms = g.report_interval()?;
            println!("Abtastrate: {} Hz (Intervall {ms} ms)", rate_hz(ms));
            Ok(())
        }
        Cmd::Buttons => cmd_buttons(g),
        Cmd::Button { profile, button, target } => cmd_button(g, *profile, button, target),
        Cmd::Macro { slot, name, events, mode, bild } => cmd_macro(g, *slot, name, events, mode, *bild),
        Cmd::MacroExport { slot, file } => {
            let data = g.read_rom(romlayout::macro_slot_addr(usize::from(slot - 1)), MACRO_SLOT_SIZE)?;
            let record = &data[..fnc::record_length(&data)];
            backup::write_macro_ini(record, file)?;
            let (mode, events) = fnc::decode_record(record);
            println!(
                "Makro {slot} -> {} ({} Bytes, Modus {}): {}",
                file.display(),
                record.len(),
                mode.unwrap_or("None".into()),
                events.join(", ")
            );
            Ok(())
        }
        Cmd::MacroImport { slot, file, name, bild } => {
            let record = backup::read_macro_ini(file, MACRO_SLOT_SIZE)?;
            let name = name.clone().unwrap_or_else(|| file_stem(file));
            let name: String = name.chars().take(MACRO_NAME_LEN).collect();
            g.write_macro(usize::from(slot - 1), &record, &name, *bild)?;
            let (mode, events) = fnc::decode_record(&record);
            println!(
                "{} -> Makro {slot} {} ({} Bytes, Modus {}): {}",
                file.display(),
                py_repr(&name),
                record.len(),
                mode.unwrap_or("None".into()),
                events.join(", ")
            );
            Ok(())
        }
        Cmd::MacroBild { slot, bild } => {
            let slot = usize::from(slot - 1);
            if let Some(b) = bild {
                g.set_macro_icon(slot, parse_icon(b)?)?;
            }
            println!("Makro {}: {}", slot + 1, format_icon(g.macro_icon(slot)?));
            Ok(())
        }
        Cmd::Save => {
            g.save_settings()?;
            println!("Einstellungen dauerhaft gespeichert (01 02).");
            Ok(())
        }
        Cmd::Backup { file } => {
            let data = backup::create(g, &mut |_, _| {})?;
            backup::save(&data, file)?;
            println!(
                "Sicherung -> {}  (Profil {}, {} Hz, ROM {} Bytes)",
                file.display(),
                data.active_profile + 1,
                rate_hz(data.report_interval_ms),
                data.rom.len() / 2
            );
            Ok(())
        }
        Cmd::Restore { file } => {
            let data = backup::load(file)?;
            println!("Stelle Sicherung vom {} wieder her ...", data.created);
            let left = backup::restore(g, &data, &mut |m| println!("{m}"))?;
            g.save_settings()?;
            if left.is_empty() {
                println!("Fertig - ROM bytegleich mit der Sicherung, Einstellungen wiederhergestellt.");
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
            println!("{} Bytes -> {}", data.len(), file.display());
            println!("SHA-256 {}", sha256(&data));
            Ok(())
        }
        Cmd::RomImport { file } => {
            let data = backup::load_rom_file(file)?;
            let left = backup::import_rom(g, &data, &mut |m| println!("{m}"))?;
            println!("Importiert: {} (SHA-256 {})", file.display(), &sha256(&data)[..8]);
            if !left.is_empty() {
                print_left(&left);
            }
            println!(
                "Hinweis: .meb/.bin enthalten nur den Speicher (inkl. Tastenbelegung), nicht DPI, LED und Abtastrate."
            );
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
    println!("Abweichungen im Firmware-Bereich (nicht direkt geschrieben):");
    for (s, e) in left {
        println!("  0x{s:04X}-0x{e:04X}");
    }
}

/// Abtastrate, DPI, aktives Profil/Stufe gehen beim Abstecken verloren, wenn sie nicht mit 01 02 gespeichert werden (getestet).
fn persist(g: &mut G, temporaer: bool) -> Result<()> {
    if temporaer {
        println!("(nicht dauerhaft gespeichert - geht beim Abstecken verloren)");
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
    let ok = if ident.signature_ok { "OK" } else { "FALSCH" };
    println!("Geraet:            {name}  (Signatur {ok}: {})", ident.raw);
    let model = ident.model.unwrap_or("unbekannt");
    let note = if model == SUPPORTED_MODEL {
        String::new()
    } else {
        format!("   (nicht unterstuetzt, nur {SUPPORTED_MODEL})")
    };
    println!("Modell:            {model}{note}");
    println!("Aktives Profil:    {}", profile + 1);
    println!("Aktive DPI-Stufe:  {}   (Register 07; LEDs zeigen die Stufe)", stage + 1);
    println!("Register 0A:       0x{reg0a:04X}   (ROM-Adresse des Konfigbereichs)");
    println!("Abtastrate:        {} Hz   (Intervall {interval} ms)", rate_hz(interval));
    match (steps.first(), steps.last()) {
        (Some(a), Some(b)) => {
            println!("DPI-Schritte:      {} unterstuetzt ({a}..{b})", steps.len())
        }
        _ => println!("DPI-Schritte:      keine"),
    }
    for (i, s) in stages.iter().enumerate() {
        println!(
            "DPI-Stufe {}:       X {:5}  Y {:5}   (Schritt {}/{} x {}, Bruch {}/{} {}/{})",
            i + 1,
            s.x_dpi(),
            s.y_dpi(),
            s.x_step,
            s.y_step,
            s.unit,
            s.x_num,
            s.x_den,
            s.y_num,
            s.y_den
        );
    }
    for (p, rgb) in leds.iter().enumerate() {
        println!("LED Profil {}:      {}", p + 1, format_led(*rgb));
    }
    Ok(())
}

fn cmd_dpi(g: &mut G, action: Option<&DpiCmd>) -> Result<()> {
    match action {
        Some(DpiCmd::Select { stage, temporaer }) => {
            g.set_active_dpi_stage(stage - 1)?;
            persist(g, *temporaer)?;
        }
        Some(DpiCmd::Set { stage, x_dpi, y_dpi, temporaer }) => {
            g.set_dpi(stage - 1, *x_dpi, *y_dpi)?;
            persist(g, *temporaer)?;
        }
        Some(DpiCmd::Default { temporaer }) => {
            g.apply_factory_defaults()?;
            persist(g, *temporaer)?;
            println!("Abtastrate: {} Hz", rate_hz(FACTORY_INTERVAL_MS));
        }
        None => {}
    }
    let active = g.active_dpi_stage()?;
    for i in 0..DPI_BUTTON_STAGES as u8 {
        let s = g.dpi_stage(i)?;
        let marks = if i == active { "  <- aktiv" } else { "" };
        let axes =
            if s.x_dpi() == s.y_dpi() { s.x_dpi().to_string() } else { format!("X {} / Y {}", s.x_dpi(), s.y_dpi()) };
        println!("Stufe {}: {axes:>18} DPI{marks}", i + 1);
    }
    Ok(())
}

/// Nur lesend: jede Aenderung von aktivem Profil / Register 07 ausgeben (z. B. beim Druecken der Tasten).
fn cmd_watch(g: &mut G, seconds: f64) -> Result<()> {
    let end = Instant::now() + Duration::from_secs_f64(seconds.max(0.0));
    let mut last = None;
    println!("Beobachte {seconds} s - Tasten an der Maus druecken ...");
    while Instant::now() < end {
        let now = (g.active_profile()?, g.active_dpi_stage()?);
        if last != Some(now) {
            let time = chrono::Local::now().format("%H:%M:%S");
            println!("{time}  Profil {}   DPI-Stufe {} (Register 07 = {})", now.0 + 1, now.1 + 1, now.1);
            last = Some(now);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn cmd_buttons(g: &mut G) -> Result<()> {
    let names = g.macro_names()?;
    let header: String = (0..PROFILES).map(|p| format!("  Profil {:<25}", p + 1)).collect();
    println!("Taste          {header}");
    for b in 0..BUTTONS as u8 {
        let cells = (0..PROFILES as u8)
            .map(|p| g.button(p, b).map(|a| describe_target(a, &names, Some(b))))
            .collect::<Result<Vec<_>>>()?;
        let label = format!("{b} {}", names::button_name(b).unwrap_or(""));
        let row: String = cells.iter().map(|c| format!("  {c:<32}")).collect();
        println!("{label:14} {row}");
    }
    Ok(())
}

fn cmd_button(g: &mut G, profile: u8, button: &str, target: &str) -> Result<()> {
    let (p, button) = (profile - 1, parse_button(button)?);
    if target == "standard" {
        g.reset_button(p, button)?;
    } else if target.starts_with("makro") {
        let slot = match target.split_once(':') {
            Some((_, n)) => {
                n.trim().parse::<i64>().map_err(|_| Error::Invalid(format!("ungueltige Makronummer '{n}'")))? - 1
            }
            None => -1,
        };
        if !(0..romlayout::MACRO_SLOTS as i64).contains(&slot) {
            return Err(Error::Invalid(format!("Makro als makro:1 bis makro:{}", romlayout::MACRO_SLOTS)));
        }
        g.assign_macro(p, button, slot as usize)?;
    } else {
        let Some(f) = fnc::by_key(target) else {
            return Err(Error::Invalid("unbekannte Funktion - Liste mit: m6980x_configtool functions".into()));
        };
        if !f.confirmed {
            println!("Hinweis: Inhalt von '{}' ist abgeleitet, noch nicht am Geraet bestaetigt.", f.key);
        }
        g.assign_function(p, button, f)?;
    }
    let names = g.macro_names()?;
    println!("Profil {profile}, Taste {button}: {}", describe_target(g.button(p, button)?, &names, Some(button)));
    Ok(())
}

fn cmd_functions() {
    for f in FUNCTIONS.iter() {
        let state = if f.size == 0 {
            "nur als standard"
        } else if f.confirmed {
            "bestaetigt"
        } else if !f.record.is_empty() {
            "abgeleitet"
        } else {
            "unbekannt"
        };
        println!("{:20} {:28} 0x{:04X}  {state}", f.key, f.label, f.address);
    }
}

fn read_full_rom(g: &mut G) -> Result<Vec<u8>> {
    if !std::io::stderr().is_terminal() {
        return g.read_rom(0, ROM_SIZE);
    }
    let data = g.read_rom_progress(0, ROM_SIZE, &mut |done, total| {
        eprint!("\r  ROM lesen: {:3} %", done * 100 / total);
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
        let events = if events.is_empty() { "(keine Ereignisse)".to_string() } else { events.join(", ") };
        println!(
            "Makro {:2} {:12} {:30} {:3}/{MACRO_SLOT_SIZE} B  Modus {:9} {events}",
            s + 1,
            py_repr(&name),
            format_icon(romlayout::macro_icon(info)),
            fnc::record_length(slot),
            mode.as_deref().unwrap_or("-"),
        );
    }
}

fn cmd_macro(g: &mut G, slot: u8, name: &str, events: &[String], mode: &str, bild: Option<u8>) -> Result<()> {
    let mut record = vec![fnc::mode_value(mode).expect("Auswahl")];
    record.extend(fnc::encode_events(&events.join(" "))?);
    record.push(fnc::EV_END);
    let slot = usize::from(slot - 1);
    g.write_macro(slot, &record, name, bild)?;
    let (mode, events) = fnc::decode_record(&g.read_rom(romlayout::macro_slot_addr(slot), record.len())?);
    println!(
        "Makro {} {} geschrieben ({}/{MACRO_SLOT_SIZE} Bytes, {}): Modus {}, {}",
        slot + 1,
        py_repr(name),
        record.len(),
        format_icon(g.macro_icon(slot)?),
        mode.unwrap_or("None".into()),
        events.join(", ")
    );
    Ok(())
}

/// Startet die Oberflaeche (m6980x_configtool_gui), die neben dieser Datei liegt.
fn launch_gui(device: Option<&str>) -> Result<()> {
    let exe = std::env::current_exe()?;
    let gui = exe.with_file_name(format!("m6980x_configtool_gui{}", std::env::consts::EXE_SUFFIX));
    if !gui.exists() {
        return Err(Error::Invalid(format!("Oberflaeche nicht gefunden: {}", gui.display())));
    }
    let mut cmd = std::process::Command::new(gui);
    if let Some(d) = device {
        cmd.args(["--device", d]);
    }
    cmd.spawn()?;
    Ok(())
}
