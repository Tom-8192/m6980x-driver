//! Tastenfunktionen, Tastencodes und das Makro-Ereignisformat der GHOST M6980X.
//!
//! Funktionsdatensaetze liegen an festen ROM-Adressen: 0x1D8C + Summe der Groessen aller frueheren
//! Eintraege aus Mappingdefault.ini [List]/[ListMemSize] (fuer 5 Funktionen belegt, siehe docs/PROTOCOL.md).

use std::sync::LazyLock;

use crate::{Result, invalid};

pub const FUNCTION_BASE: u16 = 0x1D8C;

pub const MODE_ONCE: u8 = 0x04;
pub const MODE_LOOP: u8 = 0x05;
pub const MODE_HOLD: u8 = 0x06;
pub const MODE_NAMES: [(&str, u8); 3] = [("einmal", MODE_ONCE), ("schleife", MODE_LOOP), ("feuer", MODE_HOLD)];

pub const EV_END: u8 = 0x00;
pub const EV_KEY_DOWN: u8 = 0x83;
pub const EV_KEY_UP: u8 = 0x84;
pub const EV_CONSUMER: u8 = 0x85;
pub const EV_WAIT8: u8 = 0x88;
pub const EV_WAIT16: u8 = 0x89;
/// | Tastenmaske
pub const EV_MOUSE_DOWN: u8 = 0x20;
pub const EV_MOUSE_UP: u8 = 0x40;
/// | vorzeichenbehaftetes 5-Bit-Delta: 61 = hoch 1, 7F = runter 1 (getestet)
pub const EV_WHEEL: u8 = 0x60;
/// Der Editor des Originals (0x410ae8) schreibt nur diese beiden.
pub const EV_WHEEL_UP: u8 = 0x61;
pub const EV_WHEEL_DOWN: u8 = 0x7F;
pub const MOUSE_BUTTONS: [(&str, u8); 5] =
    [("links", 0x01), ("rechts", 0x02), ("mitte", 0x04), ("zurueck", 0x08), ("vor", 0x10)];

pub fn mode_value(name: &str) -> Option<u8> {
    MODE_NAMES.iter().find(|(n, _)| *n == name).map(|&(_, v)| v)
}

/// HID-Tastatur-Usages (USB HID Usage Tables, Seite 0x07). Usages sind Tasten*positionen* einer US-Tastatur;
/// die Namen hier folgen dem deutschen QWERTZ-Layout, bei dem Y und Z vertauscht sind.
/// Reihenfolge zaehlt: bei doppelten Codes gewinnt beim Anzeigen der erste Name.
pub static KEYS: LazyLock<Vec<(String, u8)>> = LazyLock::new(|| {
    let mut keys: Vec<(String, u8)> = (0..26u8).map(|i| (((b'a' + i) as char).to_string(), 0x04 + i)).collect();
    keys[(b'y' - b'a') as usize].1 = 0x1D; // deutsches y = US-z-Position
    keys[(b'z' - b'a') as usize].1 = 0x1C;
    keys.extend((0..10u8).map(|d| (d.to_string(), 0x1E + (d + 9) % 10)));
    keys.extend((1..=12u8).map(|n| (format!("f{n}"), 0x3A + n - 1)));
    let named: &[(&str, u8)] = &[
        ("enter", 0x28),
        ("esc", 0x29),
        ("backspace", 0x2A),
        ("tab", 0x2B),
        ("space", 0x2C),
        ("leer", 0x2C),
        ("minus", 0x2D),
        ("plus", 0x2E),
        ("druck", 0x46),
        ("rollen", 0x47),
        ("pause", 0x48),
        ("einfg", 0x49),
        ("pos1", 0x4A),
        ("bildauf", 0x4B),
        ("entf", 0x4C),
        ("ende", 0x4D),
        ("bildab", 0x4E),
        ("rechts", 0x4F),
        ("links", 0x50),
        ("runter", 0x51),
        ("hoch", 0x52),
        ("strg", 0xE0),
        ("shift", 0xE1),
        ("alt", 0xE2),
        ("win", 0xE3),
        ("strg-r", 0xE4),
        ("shift-r", 0xE5),
        ("altgr", 0xE6),
        ("win-r", 0xE7),
    ];
    keys.extend(named.iter().map(|&(n, v)| (n.to_string(), v)));
    keys
});

pub fn key(name: &str) -> Option<u8> {
    KEYS.iter().find(|(n, _)| n == name).map(|&(_, v)| v)
}

pub fn key_name(usage: u8) -> Option<&'static str> {
    KEYS.iter().find(|&&(_, v)| v == usage).map(|(n, _)| n.as_str())
}

fn k(name: &str) -> u8 {
    key(name).expect("Taste in KEYS")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub index: usize,
    /// Name auf der Kommandozeile
    pub key: &'static str,
    pub label: &'static str,
    pub size: usize,
    /// leer = Inhalt unbekannt, nicht zuweisbar
    pub record: Vec<u8>,
    /// Datensatz im Mitschnitt des Originals gesehen oder am Geraet getestet (2026-10-11)
    pub confirmed: bool,
    pub address: u16,
}

/// 04 + alle druecken, in umgekehrter Reihenfolge loslassen + 00 (Muster von Strg+C / Strg+V / Win+D).
pub fn combo(usages: &[u8]) -> Vec<u8> {
    let mut out = vec![MODE_ONCE];
    for &u in usages {
        out.extend([EV_KEY_DOWN, u]);
    }
    for &u in usages.iter().rev() {
        out.extend([EV_KEY_UP, u]);
    }
    out.push(EV_END);
    out
}

pub fn consumer(usage: u16) -> Vec<u8> {
    vec![MODE_ONCE, EV_CONSUMER, (usage & 0xFF) as u8, (usage >> 8) as u8, EV_CONSUMER, 0, 0, EV_END]
}

pub fn hold_key(usage: u8) -> Vec<u8> {
    vec![MODE_HOLD, EV_KEY_DOWN, usage, 0xFF, 0xFF, EV_END]
}

/// Alt+Leertaste oeffnet das Fenstermenue, dann der Menuebuchstabe (deutsches Windows: N minimieren, X maximieren).
fn window_menu(usage: u8) -> Vec<u8> {
    let (alt, space) = (k("alt"), k("space"));
    vec![
        MODE_ONCE,
        EV_KEY_DOWN,
        alt,
        EV_KEY_DOWN,
        space,
        EV_KEY_UP,
        space,
        EV_KEY_UP,
        alt,
        EV_KEY_DOWN,
        usage,
        EV_KEY_UP,
        usage,
        EV_END,
    ]
}

pub fn hold_mouse(mask: u8) -> Vec<u8> {
    vec![MODE_HOLD, EV_MOUSE_DOWN | mask, EV_WAIT16, 0xFF, 0xFF, EV_END]
}

fn h(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|b| u8::from_str_radix(b, 16).expect("Hex")).collect()
}

/// (key, label, ListMemSize, record, confirmed) in der Reihenfolge von Mappingdefault.ini
fn table() -> Vec<(&'static str, &'static str, usize, Vec<u8>, bool)> {
    let (s, c, w, alt) = (k("strg"), k("shift"), k("win"), k("alt"));
    vec![
        ("linksklick", "Linksklick", 6, hold_mouse(0x01), true),
        ("rechtsklick", "Rechtsklick", 6, hold_mouse(0x02), true),
        ("mittelklick", "Mittelklick", 6, h("06 24 89 ff ff 00"), true),
        ("browser-vor", "Browser vor", 6, hold_mouse(0x10), true),
        ("browser-zurueck", "Browser zurueck", 6, hold_mouse(0x08), true),
        // INI-Name "Perfect"; auf der 6980 sendet das Original AL Calculator (andere Maeuse: Sensorbefehl C1 00)
        ("rechner", "Rechner (Consumer 0x0192)", 8, h("04 85 92 01 85 00 00 00"), true),
        ("scroll-links", "Bildlauf links (= Standard von Kipprad links)", 0, vec![], true),
        ("scroll-rechts", "Bildlauf rechts (= Standard von Kipprad rechts)", 0, vec![], true),
        ("scroll-vor", "Bildlauf hoch", 3, vec![MODE_ONCE, EV_WHEEL_UP, EV_END], true),
        ("scroll-zurueck", "Bildlauf runter", 3, vec![MODE_ONCE, EV_WHEEL_DOWN, EV_END], true),
        ("doppelklick", "Doppelklick", 6, h("04 21 41 21 41 00"), true),
        ("entf", "Entfernen", 6, h("04 83 4c 84 4c 00"), true),
        ("rueckgaengig", "Rueckgaengig (Strg+Z)", 10, combo(&[s, k("z")]), true),
        ("ausschneiden", "Ausschneiden (Strg+X)", 10, combo(&[s, k("x")]), true),
        ("kopieren", "Kopieren (Strg+C)", 10, h("04 83 e0 83 06 84 06 84 e0 00"), true),
        ("einfuegen", "Einfuegen (Strg+V)", 10, h("04 83 e0 83 19 84 19 84 e0 00"), true),
        ("neu", "Neu (Strg+N)", 10, combo(&[s, k("n")]), true),
        ("schliessen", "Schliessen (Strg+W)", 10, combo(&[s, k("w")]), true),
        ("oeffnen", "Oeffnen (Strg+O)", 10, combo(&[s, k("o")]), true),
        ("minimieren", "Minimieren", 14, window_menu(k("n")), true),
        ("maximieren", "Maximieren", 14, window_menu(k("x")), true),
        ("zoom-rein", "Zoom (Original: rein, sendet Strg+Rad runter)", 11, h("04 83 e0 88 0a 7f 88 0a 84 e0 00"), true),
        ("zoom-raus", "Zoom (Original: raus, sendet Strg+Rad hoch)", 11, h("04 83 e0 88 0a 61 88 0a 84 e0 00"), true),
        ("programm-schliessen", "Programm schliessen (Alt+F4)", 10, combo(&[alt, k("f4")]), true),
        ("lauter", "Lauter", 8, consumer(0x00E9), true),
        ("leiser", "Leiser", 8, consumer(0x00EA), true),
        ("stumm", "Stumm", 8, consumer(0x00E2), true),
        ("shift", "Shift", 6, hold_key(c), true),
        ("strg", "Strg", 6, h("06 83 e0 ff ff 00"), true),
        ("alt", "Alt", 6, hold_key(alt), true),
        ("enter", "Enter", 6, hold_key(k("enter")), true),
        ("lupe", "Lupe (Win+U)", 10, h("04 83 e3 83 18 84 18 84 e3 00"), true),
        ("desktop", "Desktop (Win+D)", 10, combo(&[w, k("d")]), true),
        ("internet", "Internet / Browser", 8, consumer(0x0196), true),
        ("favoriten", "Browser-Favoriten", 8, consumer(0x022A), true),
        ("vorheriges-fenster", "Vorheriges Fenster (Alt+Tab)", 10, combo(&[alt, k("tab")]), true),
        ("email", "E-Mail", 8, consumer(0x018A), true),
        ("play-pause", "Play/Pause", 8, consumer(0x00CD), true),
        ("zurueck", "Vorheriger Titel", 8, consumer(0x00B6), true),
        ("weiter", "Naechster Titel", 8, consumer(0x00B5), true),
        ("speed-2x", "2x Tempo (solange gehalten)", 2, h("c8 00"), true),
        ("speed-4x", "4x Tempo (solange gehalten)", 2, h("ca 00"), true),
        ("speed-8x", "8x Tempo (solange gehalten)", 2, h("ce 00"), true),
        // Datensaetze aus dem Code des Originals (0x423630), am Geraet getestet 2026-10-11 (Runde 6)
        ("nur-senkrecht", "Nur senkrecht bewegen (solange gehalten)", 2, h("c6 00"), true),
        ("nur-waagerecht", "Nur waagerecht bewegen (solange gehalten)", 2, h("c7 00"), true),
        // Strg+Alt+K = Hotkey von GHOSTOPEN.exe (Autostart des Originals), der Macro.exe oeffnet
        ("gigabyte-ghost", "Gigabyte-GHOST-Software oeffnen (Strg+Alt+K)", 14, combo(&[s, alt, k("k")]), true),
    ]
}

pub static FUNCTIONS: LazyLock<Vec<Function>> = LazyLock::new(|| {
    let mut addr = FUNCTION_BASE;
    table()
        .into_iter()
        .enumerate()
        .map(|(index, (key, label, size, record, confirmed))| {
            assert!(record.is_empty() || record.len() == size, "{key}");
            assert!(size > 0 || record.is_empty(), "{key}");
            let f = Function { index, key, label, size, record, confirmed, address: addr };
            addr += size as u16;
            f
        })
        .collect()
});

/// Rad hoch/runter (Geraetetasten 8/9) haben kein Loslassen, deshalb gibt das Original (0x423630) den fuenf
/// Maustasten-Funktionen dort einen einfachen Klick statt des Halte-Datensatzes: [ListMemSize] 46-50, nach der Liste.
pub const WHEEL_BUTTONS: [usize; 2] = [8, 9];

/// (Funktion, Adresse, Datensatz)
pub static WHEEL_CLICKS: LazyLock<Vec<(&'static str, u16, Vec<u8>)>> = LazyLock::new(|| {
    let last = FUNCTIONS.last().expect("Tabelle");
    let mut addr = last.address + last.size as u16;
    [
        ("linksklick", 0x01),
        ("rechtsklick", 0x02),
        ("mittelklick", 0x04),
        ("browser-vor", 0x10),
        ("browser-zurueck", 0x08),
    ]
    .into_iter()
    .map(|(key, mask)| {
        let entry = (key, addr, vec![MODE_ONCE, EV_MOUSE_DOWN | mask, EV_MOUSE_UP | mask, EV_END]);
        addr += 4;
        entry
    })
    .collect()
});

pub fn by_key(key: &str) -> Option<&'static Function> {
    let key = if key == "perfect" { "rechner" } else { key }; // alter CLI-Name
    FUNCTIONS.iter().find(|f| f.key == key)
}

pub fn by_address(addr: u16) -> Option<&'static Function> {
    if let Some(f) = FUNCTIONS.iter().find(|f| f.size > 0 && f.address == addr) {
        return Some(f);
    }
    WHEEL_CLICKS.iter().find(|(_, a, _)| *a == addr).and_then(|(key, _, _)| by_key(key))
}

/// (Adresse, Datensatz), die das Original schreibt, wenn diese Funktion auf diese Geraetetaste kommt.
pub fn placement(function: &Function, button: usize) -> (u16, Vec<u8>) {
    if WHEEL_BUTTONS.contains(&button)
        && let Some((_, addr, record)) = WHEEL_CLICKS.iter().find(|(key, _, _)| *key == function.key)
    {
        return (*addr, record.clone());
    }
    (function.address, function.record.clone())
}

// -- Makro-Ereignisse <-> Text --------------------------------------------------------------

/// Pythons `int()` fuer Dezimalzahlen (mit Vorzeichen, ohne Leerraum am Rand).
fn parse_int(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

/// 'drueck a, warte 100, los a, klick links, tippe strg+c' -> Ereignisbytes (ohne Modus/Ende).
pub fn encode_events(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for raw in text.replace('\n', ",").split(',') {
        let lower = raw.trim().to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let Some((&cmd, rest)) = words.split_first() else {
            continue;
        };
        let arg = rest.join(" ");
        match cmd {
            "drueck" | "los" | "tippe" => {
                let keys = arg.split('+').map(key_or_err).collect::<Result<Vec<u8>>>()?;
                if cmd != "los" {
                    for &key in &keys {
                        out.extend([EV_KEY_DOWN, key]);
                    }
                }
                if cmd != "drueck" {
                    for &key in keys.iter().rev() {
                        out.extend([EV_KEY_UP, key]);
                    }
                }
            }
            "warte" => {
                let ms = parse_int(&arg).filter(|ms| (1..=0xFFFF).contains(ms));
                let Some(ms) = ms else {
                    return invalid("Wartezeit 1-65535 ms");
                };
                if ms <= 0xFF {
                    out.extend([EV_WAIT8, ms as u8]);
                } else {
                    out.extend([EV_WAIT16, (ms & 0xFF) as u8, (ms >> 8) as u8]);
                }
            }
            "rad" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() || !matches!(parts[0], "hoch" | "runter") {
                    return invalid("rad hoch|runter [1-15]");
                }
                let n = match parts.get(1) {
                    Some(t) => parse_int(t).ok_or_else(|| crate::Error::Invalid("Rad-Schritte 1-15".into()))?,
                    None => 1,
                };
                if !(1..=15).contains(&n) {
                    return invalid("Rad-Schritte 1-15");
                }
                let ev = if parts[0] == "hoch" { EV_WHEEL_UP } else { EV_WHEEL_DOWN };
                out.extend(std::iter::repeat_n(ev, n as usize)); // ein Ereignis pro Raste
            }
            "consumer" => {
                let usage = if arg == "los" { Some(0) } else { parse_hex(&arg) };
                let Some(usage) = usage.filter(|u| (0..=0xFFFF).contains(u)) else {
                    return invalid("consumer 0x0000-0xFFFF oder 'consumer los'");
                };
                out.extend([EV_CONSUMER, (usage & 0xFF) as u8, (usage >> 8) as u8]);
            }
            "maus-drueck" | "maus-los" | "klick" => {
                let Some(&(_, m)) = MOUSE_BUTTONS.iter().find(|(n, _)| *n == arg) else {
                    let names: Vec<&str> = MOUSE_BUTTONS.iter().map(|(n, _)| *n).collect();
                    return invalid(format!("Maustaste: {}", names.join(", ")));
                };
                if cmd != "maus-los" {
                    out.push(EV_MOUSE_DOWN | m);
                }
                if cmd != "maus-drueck" {
                    out.push(EV_MOUSE_UP | m);
                }
            }
            _ => {
                return invalid(format!(
                    "unbekannter Befehl '{cmd}' (drueck, los, tippe, warte, maus-drueck, maus-los, klick, rad, consumer)"
                ));
            }
        }
    }
    Ok(out)
}

/// Pythons `int(text, 16)`: optional 0x-Praefix.
fn parse_hex(text: &str) -> Option<i64> {
    let t = text.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    let v = i64::from_str_radix(t, 16).ok()?;
    Some(if neg { -v } else { v })
}

fn key_or_err(name: &str) -> Result<u8> {
    key(name).map_or_else(|| invalid(format!("unbekannte Taste '{name}'")), Ok)
}

fn event_size(b: u8) -> usize {
    match b {
        EV_KEY_DOWN | EV_KEY_UP | EV_WAIT8 => 2,
        EV_CONSUMER | EV_WAIT16 => 3,
        _ => 1,
    }
}

/// Belegte Bytes eines Datensatzes/Makros inklusive 00-Endmarke; 0 fuer einen leeren Slot.
/// Laeuft die Ereignisse ab, weil 00 auch in Argumenten vorkommt (z. B. 85 E9 00).
pub fn record_length(data: &[u8]) -> usize {
    if data.is_empty() || matches!(data[0], 0x00 | 0xFF) {
        return 0;
    }
    let mut i = 1;
    while i < data.len() && data[i] != EV_END {
        i += event_size(data[i]);
    }
    (i + 1).min(data.len())
}

pub fn mode_name(mode: u8) -> String {
    MODE_NAMES.iter().find(|&&(_, v)| v == mode).map_or_else(|| format!("0x{mode:02X}"), |(n, _)| n.to_string())
}

/// (Modusname, [Ereignistexte]) fuer einen Funktionsdatensatz oder ein Makro; stoppt bei 00.
/// Geloeschte oder genullte Slots ergeben (None, []).
pub fn decode_record(data: &[u8]) -> (Option<String>, Vec<String>) {
    if data.is_empty() || matches!(data[0], 0x00 | 0xFF) {
        return (None, vec![]);
    }
    let mut events: Vec<String> = Vec::new();
    let mut i = 1;
    while i < data.len() && data[i] != EV_END {
        let b = data[i];
        if matches!(b, EV_KEY_DOWN | EV_KEY_UP) && i + 1 < data.len() {
            let usage = data[i + 1];
            let name = key_name(usage).map_or_else(|| format!("0x{usage:02X}"), str::to_string);
            events.push(format!("{} {name}", if b == EV_KEY_DOWN { "drueck" } else { "los" }));
            i += 2;
        } else if b == EV_CONSUMER && i + 2 < data.len() {
            let u = u16::from(data[i + 1]) | u16::from(data[i + 2]) << 8;
            events.push(if u != 0 { format!("consumer 0x{u:04X}") } else { "consumer los".into() });
            i += 3;
        } else if b == EV_WAIT8 && i + 1 < data.len() {
            events.push(format!("warte {}", data[i + 1]));
            i += 2;
        } else if b == EV_WAIT16 && i + 2 < data.len() {
            events.push(format!("warte {}", u16::from(data[i + 1]) | u16::from(data[i + 2]) << 8));
            i += 3;
        } else if b & 0xE0 == EV_WHEEL && b & 0x1F != 0 {
            let raw = i32::from(b & 0x1F);
            let mut d = if raw & 0x10 != 0 { raw - 32 } else { raw };
            let direction = if d > 0 { "hoch" } else { "runter" };
            let prefix = format!("rad {direction} ");
            // mehrere Rasten hintereinander -> eine Zeile
            if let Some(last) = events.pop_if(|e| e.starts_with(&prefix)) {
                let prev: i32 = last.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                d += prev * if d > 0 { 1 } else { -1 };
            }
            events.push(format!("{prefix}{}", d.abs()));
            i += 1;
        } else if matches!(b & 0xE0, EV_MOUSE_DOWN | EV_MOUSE_UP) && b & 0x1F != 0 {
            let mut names: Vec<String> =
                MOUSE_BUTTONS.iter().filter(|&&(_, m)| b & m != 0).map(|(n, _)| n.to_string()).collect();
            if names.is_empty() {
                names.push(format!("0x{:02X}", b & 0x1F));
            }
            let verb = if b & 0xE0 == EV_MOUSE_DOWN { "maus-drueck" } else { "maus-los" };
            events.push(format!("{verb} {}", names.join("+")));
            i += 1;
        } else {
            events.push(format!("0x{b:02X}?"));
            i += 1;
        }
    }
    (Some(mode_name(data[0])), events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn addresses_match_original_captures() {
        for (key, addr) in
            [("mittelklick", 0x1D98), ("kopieren", 0x1DD8), ("einfuegen", 0x1DE2), ("lauter", 0x1E46), ("strg", 0x1E64)]
        {
            assert_eq!(by_key(key).unwrap().address, addr, "{key}");
        }
    }

    #[test]
    fn records_fit_their_size() {
        for f in FUNCTIONS.iter().filter(|f| !f.record.is_empty()) {
            assert_eq!(f.record.len(), f.size, "{}", f.key);
            assert_eq!(*f.record.last().unwrap(), EV_END, "{}", f.key);
        }
    }

    #[test]
    fn derived_patterns_reproduce_known_records() {
        assert_eq!(combo(&[k("win"), k("d")]), h("04 83 e3 83 07 84 07 84 e3 00"));
        assert_eq!(combo(&[k("strg"), k("v")]), by_key("einfuegen").unwrap().record);
        assert_eq!(consumer(0x00EA), h("04 85 ea 00 85 00 00 00"));
        assert_eq!(hold_key(k("strg")), by_key("strg").unwrap().record);
        assert_eq!(hold_mouse(0x04), by_key("mittelklick").unwrap().record);
    }

    #[test]
    fn german_layout_y_z() {
        assert_eq!(key("z"), Some(0x1C));
        assert_eq!(key("y"), Some(0x1D));
        assert_eq!(key("0"), Some(0x27));
        assert_eq!(key("1"), Some(0x1E));
        assert_eq!(key_name(0x2C), Some("space"));
        assert_eq!(by_key("rueckgaengig").unwrap().record, h("04 83 e0 83 1c 84 1c 84 e0 00"));
    }

    #[test]
    fn encode_xy_macro_from_capture() {
        let ev = encode_events("drueck a, warte 100, los a, warte 500, maus-drueck links, maus-drueck mitte").unwrap();
        let mut rec = vec![MODE_ONCE];
        rec.extend(ev);
        rec.push(0);
        assert_eq!(rec, h("04 83 04 88 64 84 04 89 f4 01 21 24 00"));
    }

    #[test]
    fn tippe_and_klick() {
        assert_eq!(encode_events("tippe strg+c").unwrap(), h("83 e0 83 06 84 06 84 e0"));
        assert_eq!(encode_events("klick links").unwrap(), h("21 41"));
        assert_eq!(encode_events("consumer 0x00E9, consumer los").unwrap(), h("85 e9 00 85 00 00"));
    }

    #[test]
    fn decode_round_trip() {
        let (mode, events) = decode_record(&h("05 83 04 88 64 84 04 89 f4 01 21 41 00 ff"));
        assert_eq!(mode.as_deref(), Some("schleife"));
        assert_eq!(events, s(&["drueck a", "warte 100", "los a", "warte 500", "maus-drueck links", "maus-los links"]));
    }

    #[test]
    fn decode_klicker_macro_on_mouse() {
        assert_eq!(
            decode_record(&h("06 21 41 00")),
            (Some("feuer".into()), s(&["maus-drueck links", "maus-los links"]))
        );
    }

    #[test]
    fn wheel_events_tested_on_device() {
        assert_eq!(encode_events("rad hoch, rad runter").unwrap(), h("61 7f"));
        assert_eq!(decode_record(&h("04 61 7f 00")).1, s(&["rad hoch 1", "rad runter 1"]));
        assert_eq!(by_key("scroll-vor").unwrap().record, h("04 61 00"));
    }

    #[test]
    fn several_notches_like_original_editor() {
        assert_eq!(encode_events("rad runter 3, rad hoch 2").unwrap(), h("7f 7f 7f 61 61"));
        assert_eq!(decode_record(&h("04 7e 00")).1, s(&["rad runter 2"]));
        assert_eq!(decode_record(&h("04 7f 7f 7f 61 00")).1, s(&["rad runter 3", "rad hoch 1"]));
    }

    #[test]
    fn records_from_original_code() {
        assert_eq!(FUNCTIONS.len(), 46);
        assert_eq!(by_key("nur-senkrecht").unwrap().address, 0x1ECA);
        assert_eq!(by_key("gigabyte-ghost").unwrap().address, 0x1ECE);
        assert_eq!(by_key("gigabyte-ghost").unwrap().record, h("04 83 e0 83 e2 83 0e 84 0e 84 e2 84 e0 00"));
        assert_eq!(by_key("perfect").unwrap().key, "rechner");
    }

    #[test]
    fn wheel_buttons_get_click_records() {
        let left = by_key("linksklick").unwrap();
        assert_eq!(placement(left, 0), (left.address, left.record.clone()));
        assert_eq!(placement(left, 8), (0x1EDC, h("04 21 41 00")));
        assert_eq!(placement(by_key("browser-zurueck").unwrap(), 9), (0x1EEC, h("04 28 48 00")));
        let copy = by_key("kopieren").unwrap();
        assert_eq!(placement(copy, 9), (copy.address, copy.record.clone()));
        assert_eq!(by_address(0x1EE4).unwrap().key, "mittelklick");
    }

    #[test]
    fn record_length_skips_zero_arguments() {
        assert_eq!(record_length(&h("04 85 e9 00 85 00 00 00 ff ff")), 8);
        assert_eq!(record_length(&h("04 89 00 01 83 04 00 ff")), 7);
        assert_eq!(record_length(&h("ff ff ff")), 0);
        assert_eq!(record_length(&h("00 00 00")), 0);
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["drueck xyz", "warte 0", "warte 70000", "klick oben", "springe", "rad seitwaerts", "rad hoch 16"] {
            assert!(encode_events(bad).is_err(), "{bad}");
        }
    }
}
