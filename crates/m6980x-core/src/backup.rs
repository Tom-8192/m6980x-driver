//! Komplette Sicherung/Wiederherstellung der Maus: das 8-KB-ROM plus die Einstellungen ausserhalb davon
//! (aktives Profil, DPI-Stufen, aktive DPI-Stufe, LED-Farben, Abtastrate, Tastenbelegung).
//!
//! Format: JSON, siehe docs/PROTOCOL.md. ROM-Bytes im Firmware-Bereich 0x1F00-0x1FFF werden nicht direkt
//! geschrieben, ausser der Tastenadress-Tabelle, die die Firmware nach "Standard" stehen laesst.
//! Dazu die Dateiformate des Originals: `.meb` (ROM-Sicherung) und Makro-`.ini` (Editor-Export).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::functions as fnc;
use crate::protocol::{BUTTONS, DPI_STAGES, M6980X, PROFILES, ROM_SIZE};
use crate::rom as romlayout;
use crate::transport::Transport;
use crate::{Error, Result, invalid, tr};

pub const FORMAT: &str = "m6980x-configtool-backup";
/// vor der Umbenennung (2026-10-11) geschrieben
pub const OLD_FORMATS: [&str; 1] = ["ghost-backup"];
pub const VERSION: u32 = 1;
pub const FIRMWARE_AREA: usize = 0x1F00;
pub const TABLE_START: usize = romlayout::BUTTON_TABLE;
pub const TABLE_END: usize = romlayout::BUTTON_TABLE + PROFILES * romlayout::BUTTON_TABLE_STRIDE;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiValues {
    pub x_dpi: u32,
    pub y_dpi: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub created: String,
    pub device: String,
    pub active_profile: u8,
    pub active_dpi_stage: u8,
    pub report_interval_ms: u8,
    pub dpi_stages: Vec<DpiValues>,
    pub led_colors: Vec<[u8; 3]>,
    pub buttons: Vec<Vec<Option<u16>>>,
    /// ROM als Hex-Text ohne Leerzeichen
    pub rom: String,
}

impl Backup {
    pub fn rom_bytes(&self) -> Result<Vec<u8>> {
        from_hex(&self.rom)
    }
}

pub fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(text: &str) -> Result<Vec<u8>> {
    let clean: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !clean.len().is_multiple_of(2) {
        return invalid(tr!("backup.bad_hex"));
    }
    clean
        .chunks(2)
        .map(|p| {
            std::str::from_utf8(p)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| Error::Invalid(tr!("backup.bad_hex")))
        })
        .collect()
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn create<T: Transport>(g: &mut M6980X<T>, progress: &mut dyn FnMut(usize, usize)) -> Result<Backup> {
    let stages = (0..DPI_STAGES as u8).map(|i| g.dpi_stage(i)).collect::<Result<Vec<_>>>()?;
    let mut buttons = Vec::new();
    for p in 0..PROFILES as u8 {
        buttons.push((0..BUTTONS as u8).map(|b| g.button(p, b)).collect::<Result<Vec<_>>>()?);
    }
    Ok(Backup {
        format: FORMAT.into(),
        version: VERSION,
        created: now(),
        device: g.name()?,
        active_profile: g.active_profile()?,
        active_dpi_stage: g.active_dpi_stage()?,
        report_interval_ms: g.report_interval()?,
        dpi_stages: stages.iter().map(|s| DpiValues { x_dpi: s.x_dpi(), y_dpi: s.y_dpi() }).collect(),
        led_colors: (0..PROFILES as u8).map(|p| g.led_color(p)).collect::<Result<Vec<_>>>()?,
        buttons,
        rom: to_hex(&g.read_rom_progress(0, ROM_SIZE, progress)?),
    })
}

/// JSON mit Einrueckung 1 wie der Prototyp (`json.dump(..., indent=1)`).
pub fn to_json(data: &Backup) -> String {
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    data.serialize(&mut ser).expect("Serialisierung in den Speicher");
    String::from_utf8(out).expect("JSON ist UTF-8")
}

pub fn save(data: &Backup, path: impl AsRef<Path>) -> Result<()> {
    fs::write(path, to_json(data))?;
    Ok(())
}

pub fn parse(text: &str) -> Result<Backup> {
    let foreign = || Error::Invalid(tr!("backup.foreign"));
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| Error::Invalid(e.to_string()))?;
    let format = value.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if !(format == FORMAT || OLD_FORMATS.contains(&format)) || value.get("version").and_then(|v| v.as_u64()) != Some(1)
    {
        return Err(foreign());
    }
    let data: Backup = serde_json::from_value(value).map_err(|e| Error::Invalid(tr!("backup.damaged", e = e)))?;
    if data.rom_bytes()?.len() != ROM_SIZE {
        return invalid(tr!("backup.rom_image_not_8192"));
    }
    if data.buttons.len() != PROFILES
        || data.buttons.iter().any(|p| p.len() != BUTTONS)
        || data.led_colors.len() != PROFILES
    {
        return invalid(tr!("backup.damaged_incomplete"));
    }
    Ok(data)
}

pub fn load(path: impl AsRef<Path>) -> Result<Backup> {
    parse(&fs::read_to_string(path)?)
}

fn latin1_lines(path: &Path) -> Result<Vec<String>> {
    let text: String = fs::read(path)?.iter().map(|&c| c as char).collect();
    // wie Pythons Textmodus: \r\n, \r und \n beenden eine Zeile
    Ok(text.replace("\r\n", "\n").replace('\r', "\n").lines().map(str::to_string).collect())
}

/// Wie Pythons `str.strip()` fuer die hier vorkommenden Zeichen.
fn py_strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// Sicherungsdatei des Originals ("Backup all data to file"): INI-Abschnitt [Mem], count=8192,
/// dann eine Zeile '<Adresse>=<Dezimalwert>' pro Byte. Enthaelt nur das 8-KB-ROM.
pub fn read_meb(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let bad = || Error::Invalid(tr!("backup.bad_meb_file"));
    let mut values: HashMap<usize, i64> = HashMap::new();
    let mut count = None;
    for line in latin1_lines(path.as_ref())? {
        let Some((key, value)) = py_strip(&line).split_once('=') else {
            continue;
        };
        if key.to_lowercase() == "count" {
            count = Some(value.trim().parse::<i64>().map_err(|_| bad())?);
        } else if !key.is_empty() && key.chars().all(|c| c.is_ascii_digit()) {
            let addr = key.parse::<usize>().map_err(|_| bad())?;
            values.insert(addr, value.trim().parse::<i64>().map_err(|_| bad())?);
        }
    }
    if count != Some(ROM_SIZE as i64) || values.len() != ROM_SIZE {
        return Err(bad());
    }
    (0..ROM_SIZE)
        .map(|i| values.get(&i).copied().filter(|v| (0..=255).contains(v)).map(|v| v as u8).ok_or_else(bad))
        .collect()
}

fn ini_bytes(section: &str, data: &[u8]) -> Vec<u8> {
    let mut text = format!("[{section}]\r\ncount={}\r\n", data.len());
    for (i, b) in data.iter().enumerate() {
        text.push_str(&format!("{i}={b}\r\n"));
    }
    text.into_bytes()
}

/// Bytegleich mit den Dateien, die das Original schreibt (CRLF-Zeilenenden).
pub fn write_meb(rom: &[u8], path: impl AsRef<Path>) -> Result<()> {
    fs::write(path, ini_bytes("Mem", rom))?;
    Ok(())
}

/// Makro-Exportdatei des Editors im Original (0x4148a0): TIniFile, Abschnitt [Macro], count=<Bytes>,
/// dann '<Index>=<Byte dezimal>' fuer den rohen Datensatz (Modusbyte, Ereignisse, 00). Der Name wird nicht
/// gespeichert. Wie im Original wird ein leeres Makro (nur Modus + 00) nicht exportiert.
pub fn write_macro_ini(record: &[u8], path: impl AsRef<Path>) -> Result<()> {
    if record.len() <= 2 {
        return invalid(tr!("backup.empty_macro"));
    }
    fs::write(path, ini_bytes("Macro", record))?;
    Ok(())
}

/// Makro-Exportdatei des Originals lesen (Import 0x418408): liefert die Datensatzbytes, abgeschnitten an der
/// 00-Endmarke. [SpecialMacro] index != 0 laesst das Original nur das Modusbyte behalten (0x415df8).
pub fn read_macro_ini(path: impl AsRef<Path>, max_size: usize) -> Result<Vec<u8>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in latin1_lines(path.as_ref())? {
        let line = py_strip(&line).trim_start_matches(['\u{feff}', '\u{ef}', '\u{bb}', '\u{bf}']);
        if line.starts_with('[') && line.ends_with(']') && line.len() >= 2 {
            let name = py_strip(&line[1..line.len() - 1]).to_lowercase();
            sections.entry(name.clone()).or_default();
            current = Some(name);
        } else if let (Some(sec), Some((key, value))) = (&current, line.split_once('=')) {
            sections.get_mut(sec).unwrap().insert(py_strip(key).to_lowercase(), py_strip(value).to_string());
        }
    }
    // wie TIniFile.ReadInteger mit Standardwert 0
    let number = |section: &str, key: &str| -> i64 {
        sections.get(section).and_then(|s| s.get(key)).and_then(|v| v.parse().ok()).unwrap_or(0)
    };
    let count = number("macro", "count");
    if !(2..=max_size as i64).contains(&count) {
        return invalid(tr!("backup.bad_macro_file", count = count, max_size = max_size));
    }
    let data: Vec<u8> = (0..count).map(|i| (number("macro", &i.to_string()) & 0xFF) as u8).collect();
    if !matches!(data[0], 0x04..=0x06) {
        return invalid(tr!("backup.unknown_mode", mode = format!("{:02X}", data[0])));
    }
    if number("specialmacro", "index") != 0 {
        return Ok(vec![data[0], 0x00]);
    }
    // laeuft die Ereignisse ab: 00 kommt auch in Argumenten vor
    let length = fnc::record_length(&data);
    if data[length - 1] == fnc::EV_END {
        Ok(data[..length].to_vec())
    } else {
        let mut out = data;
        out.push(fnc::EV_END);
        Ok(out)
    }
}

/// .meb (Format des Originals) oder rohe 8-KB-.bin-Datei laden.
pub fn load_rom_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("meb")) {
        return read_meb(path);
    }
    let data = fs::read(path)?;
    if data.len() != ROM_SIZE {
        return invalid(tr!("backup.rom_file_size", rom_size = ROM_SIZE));
    }
    Ok(data)
}

/// Zusammenhaengende Bereiche [start, ende] (inklusive), in denen sich current und wanted unterscheiden.
pub fn diff_runs(current: &[u8], wanted: &[u8], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for i in start..end {
        if current[i] != wanted[i] {
            match runs.last_mut() {
                Some(last) if i <= last.1 + 1 => last.1 = i,
                _ => runs.push((i, i)),
            }
        }
    }
    runs
}

fn set_or_reset<T: Transport>(g: &mut M6980X<T>, p: u8, b: u8, target: Option<u16>) -> Result<()> {
    match target {
        None => g.reset_button(p, b),
        Some(addr) => g.set_button(p, b, addr),
    }
}

/// ROM-Abbild schreiben (z. B. aus einer .meb-Datei): alles unterhalb des Firmware-Bereichs und die Tastentabelle,
/// dann die Tastenbelegung aus dem Abbild (Flags 0x1F01 + Tabelle) per 01 85, wie restore() es macht.
/// Das Original schreibt beim Wiederherstellen alle 256 Seiten neu; hier nur, was abweicht.
/// Liefert die verbleibenden Abweichungen.
pub fn import_rom<T: Transport>(
    g: &mut M6980X<T>,
    wanted: &[u8],
    log: &mut dyn FnMut(&str),
) -> Result<Vec<(usize, usize)>> {
    if wanted.len() != ROM_SIZE {
        return invalid(tr!("backup.rom_image_size", rom_size = ROM_SIZE));
    }
    let current = g.read_rom(0, ROM_SIZE)?;
    for (s, e) in diff_runs(&current, wanted, 0, FIRMWARE_AREA) {
        g.write_rom(s as u16, &wanted[s..=e], false)?;
        log(&tr!("backup.rom_written", s = format!("{s:04X}"), e = format!("{e:04X}")));
    }
    for (s, e) in diff_runs(&current, wanted, TABLE_START, TABLE_END) {
        g.write_rom(s as u16, &wanted[s..=e], false)?;
        log(&tr!("backup.table_written", s = format!("{s:04X}"), e = format!("{e:04X}")));
    }
    for p in 0..PROFILES {
        for b in 0..BUTTONS {
            let target = romlayout::button_assignment(wanted, p, b);
            if let Some(t) = target.filter(|&t| usize::from(t) >= ROM_SIZE) {
                log(&tr!("backup.button_bad_address", p = p + 1, b = b, address = format!("{t:04X}")));
                continue;
            }
            if g.button(p as u8, b as u8)? != target {
                set_or_reset(g, p as u8, b as u8, target)?;
                let what = target.map_or_else(|| tr!("backup.default"), |a| format!("0x{a:04X}"));
                log(&tr!("backup.button_set", p = p + 1, b = b, what = what));
            }
        }
    }
    let fin = g.read_rom(0, ROM_SIZE)?;
    Ok(diff_runs(&fin, wanted, 0, ROM_SIZE))
}

/// Alles zurueckschreiben, was abweicht; liefert die verbleibenden Abweichungen (Firmware-Bereich).
pub fn restore<T: Transport>(
    g: &mut M6980X<T>,
    data: &Backup,
    log: &mut dyn FnMut(&str),
) -> Result<Vec<(usize, usize)>> {
    let wanted = data.rom_bytes()?;
    let current = g.read_rom(0, ROM_SIZE)?;
    for (s, e) in diff_runs(&current, &wanted, 0, FIRMWARE_AREA) {
        g.write_rom(s as u16, &wanted[s..=e], false)?;
        log(&tr!("backup.rom_written_back", s = format!("{s:04X}"), e = format!("{e:04X}")));
    }
    for p in 0..PROFILES {
        for b in 0..BUTTONS {
            let target = data.buttons[p][b];
            if g.button(p as u8, b as u8)? != target {
                set_or_reset(g, p as u8, b as u8, target)?;
                log(&tr!("backup.button_restored", p = p + 1, b = b));
            }
        }
    }
    let current = g.read_rom(TABLE_START as u16, TABLE_END - TABLE_START)?;
    for (s, e) in diff_runs(&current, &wanted[TABLE_START..TABLE_END], 0, TABLE_END - TABLE_START) {
        let a = TABLE_START + s;
        g.write_rom(a as u16, &wanted[a..=TABLE_START + e], false)?;
        log(&tr!("backup.table_written_back", a = format!("{a:04X}")));
    }
    for (p, rgb) in data.led_colors.iter().enumerate() {
        if g.led_color(p as u8)? != *rgb {
            g.set_led_color(p as u8, *rgb)?;
            log(&tr!("backup.led_profile_restored", p = p + 1));
        }
    }
    for (i, st) in data.dpi_stages.iter().enumerate() {
        let s = g.dpi_stage(i as u8)?;
        if (s.x_dpi(), s.y_dpi()) != (st.x_dpi, st.y_dpi) {
            g.set_dpi(i as u8, i64::from(st.x_dpi), Some(i64::from(st.y_dpi)))?;
            log(&tr!("backup.dpi_stage_restored", i = i + 1));
        }
    }
    if g.report_interval()? != data.report_interval_ms {
        g.set_report_interval(data.report_interval_ms)?;
        log(&tr!("backup.report_rate_restored"));
    }
    if g.active_dpi_stage()? != data.active_dpi_stage {
        g.set_active_dpi_stage(data.active_dpi_stage)?;
    }
    if g.active_profile()? != data.active_profile {
        g.set_active_profile(data.active_profile)?;
    }
    let fin = g.read_rom(0, ROM_SIZE)?;
    Ok(diff_runs(&fin, &wanted, 0, ROM_SIZE))
}
