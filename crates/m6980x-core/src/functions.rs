//! Button functions, key codes and the macro event format of the GHOST M6980X.
//!
//! Function records live at fixed ROM addresses: 0x1D8C + sum of the sizes of all earlier
//! entries from Mappingdefault.ini [List]/[ListMemSize] (proven for 5 functions, see docs/PROTOCOL.md).

use std::sync::LazyLock;

use crate::i18n::{self, fold};
use crate::{Result, invalid, tr};

pub const FUNCTION_BASE: u16 = 0x1D8C;

pub const MODE_ONCE: u8 = 0x04;
pub const MODE_LOOP: u8 = 0x05;
pub const MODE_HOLD: u8 = 0x06;
/// (id, mode byte); names in locales/ under `mode.<id>`
pub const MODES: [(&str, u8); 3] = [("once", MODE_ONCE), ("loop", MODE_LOOP), ("fire", MODE_HOLD)];

pub const EV_END: u8 = 0x00;
pub const EV_KEY_DOWN: u8 = 0x83;
pub const EV_KEY_UP: u8 = 0x84;
pub const EV_CONSUMER: u8 = 0x85;
pub const EV_WAIT8: u8 = 0x88;
pub const EV_WAIT16: u8 = 0x89;
/// | button mask
pub const EV_MOUSE_DOWN: u8 = 0x20;
pub const EV_MOUSE_UP: u8 = 0x40;
/// | signed 5-bit delta: 61 = up 1, 7F = down 1 (tested)
pub const EV_WHEEL: u8 = 0x60;
/// The original's editor (0x410ae8) only writes these two.
pub const EV_WHEEL_UP: u8 = 0x61;
pub const EV_WHEEL_DOWN: u8 = 0x7F;
/// (id, mask); names in locales/ under `mouse.<id>`
pub const MOUSE_BUTTONS: [(&str, u8); 5] =
    [("left", 0x01), ("right", 0x02), ("middle", 0x04), ("back", 0x08), ("forward", 0x10)];

/// Is `input` the id or a name (in any language) of the entry `<group>.<id>`?
fn is_named(group: &str, id: &str, input: &str) -> bool {
    id == fold(input) || i18n::matches(&format!("{group}.{id}"), input)
}

/// Mode name (any language) -> mode byte.
pub fn mode_value(name: &str) -> Option<u8> {
    MODES.iter().find(|(id, _)| is_named("mode", id, name)).map(|&(_, v)| v)
}

/// Names of the three modes in the current language.
pub fn mode_names() -> Vec<String> {
    MODES.iter().map(|(id, _)| i18n::name(&format!("mode.{id}"))).collect()
}

/// HID keyboard usages (USB HID Usage Tables, page 0x07). Usages are key *positions* on a US keyboard;
/// the letters follow the German QWERTZ layout, where Y and Z are swapped.
pub static KEYS: LazyLock<Vec<(String, u8)>> = LazyLock::new(|| {
    let mut keys: Vec<(String, u8)> = (0..26u8).map(|i| (((b'a' + i) as char).to_string(), 0x04 + i)).collect();
    keys[(b'y' - b'a') as usize].1 = 0x1D; // German y = US z position
    keys[(b'z' - b'a') as usize].1 = 0x1C;
    keys.extend((0..10u8).map(|d| (d.to_string(), 0x1E + (d + 9) % 10)));
    keys.extend((1..=12u8).map(|n| (format!("f{n}"), 0x3A + n - 1)));
    keys
});

/// Named keys (id, usage); names per language in locales/ under `key.<id>`.
pub const NAMED_KEYS: [(&str, u8); 28] = [
    ("enter", 0x28),
    ("esc", 0x29),
    ("backspace", 0x2A),
    ("tab", 0x2B),
    ("space", 0x2C),
    ("minus", 0x2D),
    ("plus", 0x2E),
    ("print", 0x46),
    ("scrolllock", 0x47),
    ("pause", 0x48),
    ("insert", 0x49),
    ("home", 0x4A),
    ("pageup", 0x4B),
    ("delete", 0x4C),
    ("end", 0x4D),
    ("pagedown", 0x4E),
    ("right", 0x4F),
    ("left", 0x50),
    ("down", 0x51),
    ("up", 0x52),
    ("ctrl", 0xE0),
    ("shift", 0xE1),
    ("alt", 0xE2),
    ("win", 0xE3),
    ("ctrl-r", 0xE4),
    ("shift-r", 0xE5),
    ("altgr", 0xE6),
    ("win-r", 0xE7),
];

/// Key name (any language) -> HID usage.
pub fn key(name: &str) -> Option<u8> {
    KEYS.iter()
        .find(|(n, _)| n == name)
        .map(|&(_, v)| v)
        .or_else(|| NAMED_KEYS.iter().find(|(id, _)| is_named("key", id, name)).map(|&(_, v)| v))
}

/// Display name of a HID usage in the current language.
pub fn key_name(usage: u8) -> Option<String> {
    KEYS.iter()
        .find(|&&(_, v)| v == usage)
        .map(|(n, _)| n.clone())
        .or_else(|| NAMED_KEYS.iter().find(|&&(_, v)| v == usage).map(|(id, _)| i18n::name(&format!("key.{id}"))))
}

fn k(name: &str) -> u8 {
    key(name).expect("key in KEYS")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub index: usize,
    /// fixed id = German name in ASCII as in the prototype (e.g. "rueckgaengig")
    pub key: &'static str,
    pub size: usize,
    /// empty = content unknown, cannot be assigned
    pub record: Vec<u8>,
    /// record seen in a capture of the original or tested on the device (2026-10-11)
    pub confirmed: bool,
    pub address: u16,
}

impl Function {
    /// Command-line name in the current language.
    pub fn name(&self) -> String {
        i18n::name(&format!("function.{}.name", self.key))
    }

    /// Description in the current language.
    pub fn label(&self) -> String {
        i18n::text(&format!("function.{}.label", self.key))
    }
}

/// 04 + press all, release in reverse order + 00 (pattern of Ctrl+C / Ctrl+V / Win+D).
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

/// Alt+Space opens the window menu, then the menu letter (Windows: N minimize, X maximize).
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

/// (id, ListMemSize, record, confirmed) in the order of Mappingdefault.ini. id = German name of the
/// prototype in ASCII; name and text per language in locales/ under `function.<id>.name`/`.label`.
fn table() -> Vec<(&'static str, usize, Vec<u8>, bool)> {
    let (s, c, w, alt) = (k("ctrl"), k("shift"), k("win"), k("alt"));
    vec![
        ("linksklick", 6, hold_mouse(0x01), true),
        ("rechtsklick", 6, hold_mouse(0x02), true),
        ("mittelklick", 6, h("06 24 89 ff ff 00"), true),
        ("browser-vor", 6, hold_mouse(0x10), true),
        ("browser-zurueck", 6, hold_mouse(0x08), true),
        // INI name "Perfect"; on the 6980 the original sends AL Calculator (other mice: sensor command C1 00)
        ("rechner", 8, h("04 85 92 01 85 00 00 00"), true),
        ("scroll-links", 0, vec![], true),
        ("scroll-rechts", 0, vec![], true),
        ("scroll-vor", 3, vec![MODE_ONCE, EV_WHEEL_UP, EV_END], true),
        ("scroll-zurueck", 3, vec![MODE_ONCE, EV_WHEEL_DOWN, EV_END], true),
        ("doppelklick", 6, h("04 21 41 21 41 00"), true),
        ("entf", 6, h("04 83 4c 84 4c 00"), true),
        ("rueckgaengig", 10, combo(&[s, k("z")]), true),
        ("ausschneiden", 10, combo(&[s, k("x")]), true),
        ("kopieren", 10, h("04 83 e0 83 06 84 06 84 e0 00"), true),
        ("einfuegen", 10, h("04 83 e0 83 19 84 19 84 e0 00"), true),
        ("neu", 10, combo(&[s, k("n")]), true),
        ("schliessen", 10, combo(&[s, k("w")]), true),
        ("oeffnen", 10, combo(&[s, k("o")]), true),
        ("minimieren", 14, window_menu(k("n")), true),
        ("maximieren", 14, window_menu(k("x")), true),
        ("zoom-rein", 11, h("04 83 e0 88 0a 7f 88 0a 84 e0 00"), true),
        ("zoom-raus", 11, h("04 83 e0 88 0a 61 88 0a 84 e0 00"), true),
        ("programm-schliessen", 10, combo(&[alt, k("f4")]), true),
        ("lauter", 8, consumer(0x00E9), true),
        ("leiser", 8, consumer(0x00EA), true),
        ("stumm", 8, consumer(0x00E2), true),
        ("shift", 6, hold_key(c), true),
        ("strg", 6, h("06 83 e0 ff ff 00"), true),
        ("alt", 6, hold_key(alt), true),
        ("enter", 6, hold_key(k("enter")), true),
        ("lupe", 10, h("04 83 e3 83 18 84 18 84 e3 00"), true),
        ("desktop", 10, combo(&[w, k("d")]), true),
        ("internet", 8, consumer(0x0196), true),
        ("favoriten", 8, consumer(0x022A), true),
        ("vorheriges-fenster", 10, combo(&[alt, k("tab")]), true),
        ("email", 8, consumer(0x018A), true),
        ("play-pause", 8, consumer(0x00CD), true),
        ("zurueck", 8, consumer(0x00B6), true),
        ("weiter", 8, consumer(0x00B5), true),
        ("speed-2x", 2, h("c8 00"), true),
        ("speed-4x", 2, h("ca 00"), true),
        ("speed-8x", 2, h("ce 00"), true),
        // records from the original's code (0x423630), tested on the device 2026-10-11 (round 6)
        ("nur-senkrecht", 2, h("c6 00"), true),
        ("nur-waagerecht", 2, h("c7 00"), true),
        // Ctrl+Alt+K = hotkey of GHOSTOPEN.exe (autostart of the original), which opens Macro.exe
        ("gigabyte-ghost", 14, combo(&[s, alt, k("k")]), true),
    ]
}

pub static FUNCTIONS: LazyLock<Vec<Function>> = LazyLock::new(|| {
    let mut addr = FUNCTION_BASE;
    table()
        .into_iter()
        .enumerate()
        .map(|(index, (key, size, record, confirmed))| {
            assert!(record.is_empty() || record.len() == size, "{key}");
            assert!(size > 0 || record.is_empty(), "{key}");
            let f = Function { index, key, size, record, confirmed, address: addr };
            addr += size as u16;
            f
        })
        .collect()
});

/// Wheel up/down (device buttons 8/9) have no release, so the original (0x423630) gives the five
/// mouse button functions a simple click there instead of the hold record: [ListMemSize] 46-50, after the list.
pub const WHEEL_BUTTONS: [usize; 2] = [8, 9];

/// (function, address, record)
pub static WHEEL_CLICKS: LazyLock<Vec<(&'static str, u16, Vec<u8>)>> = LazyLock::new(|| {
    let last = FUNCTIONS.last().expect("table");
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

/// Function by id or command-line name in any language (with or without umlauts).
pub fn by_key(key: &str) -> Option<&'static Function> {
    let folded = fold(key);
    let folded = if folded == "perfect" { "rechner" } else { folded.as_str() }; // old CLI name
    FUNCTIONS
        .iter()
        .find(|f| f.key == folded)
        .or_else(|| FUNCTIONS.iter().find(|f| i18n::matches(&format!("function.{}.name", f.key), key)))
}

pub fn by_address(addr: u16) -> Option<&'static Function> {
    if let Some(f) = FUNCTIONS.iter().find(|f| f.size > 0 && f.address == addr) {
        return Some(f);
    }
    WHEEL_CLICKS.iter().find(|(_, a, _)| *a == addr).and_then(|(key, _, _)| by_key(key))
}

/// (address, record) the original writes when this function is put on this device button.
pub fn placement(function: &Function, button: usize) -> (u16, Vec<u8>) {
    if WHEEL_BUTTONS.contains(&button)
        && let Some((_, addr, record)) = WHEEL_CLICKS.iter().find(|(key, _, _)| *key == function.key)
    {
        return (*addr, record.clone());
    }
    (function.address, function.record.clone())
}

// -- Macro events <-> text ------------------------------------------------------------------

/// Python `int()` for decimal numbers (with sign, no surrounding whitespace).
fn parse_int(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

/// 'press a, wait 100, release a, click left, tap ctrl+c' -> event bytes (without mode/end).
/// Commands, keys and mouse buttons are recognised in every language from locales/ ('drück a, warte 100', also 'drueck').
pub fn encode_events(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for raw in text.replace('\n', ",").split(',') {
        let lower = fold(raw.trim());
        let words: Vec<&str> = lower.split_whitespace().collect();
        let Some((&cmd, rest)) = words.split_first() else {
            continue;
        };
        let arg = rest.join(" ");
        let verb = |id: &str| is_named("macro", id, cmd);
        let (press, release, tap) = (verb("press"), verb("release"), verb("tap"));
        let (mouse_down, mouse_up, click) = (verb("mouse_down"), verb("mouse_up"), verb("click"));
        if press || release || tap {
            let keys = arg.split('+').map(key_or_err).collect::<Result<Vec<u8>>>()?;
            if !release {
                for &key in &keys {
                    out.extend([EV_KEY_DOWN, key]);
                }
            }
            if !press {
                for &key in keys.iter().rev() {
                    out.extend([EV_KEY_UP, key]);
                }
            }
        } else if verb("wait") {
            let ms = parse_int(&arg).filter(|ms| (1..=0xFFFF).contains(ms));
            let Some(ms) = ms else {
                return invalid(tr!("macro.wait_range"));
            };
            if ms <= 0xFF {
                out.extend([EV_WAIT8, ms as u8]);
            } else {
                out.extend([EV_WAIT16, (ms & 0xFF) as u8, (ms >> 8) as u8]);
            }
        } else if verb("wheel") {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            let up = parts.first().is_some_and(|p| is_named("macro", "up", p));
            if parts.is_empty() || !(up || is_named("macro", "down", parts[0])) {
                return invalid(tr!("macro.wheel_syntax"));
            }
            let steps_err = || crate::Error::Invalid(tr!("macro.wheel_steps"));
            let n = match parts.get(1) {
                Some(text) => parse_int(text).ok_or_else(steps_err)?,
                None => 1,
            };
            if !(1..=15).contains(&n) {
                return Err(steps_err());
            }
            let ev = if up { EV_WHEEL_UP } else { EV_WHEEL_DOWN };
            out.extend(std::iter::repeat_n(ev, n as usize)); // one event per notch
        } else if verb("consumer") {
            let usage = if is_named("macro", "release", &arg) { Some(0) } else { parse_hex(&arg) };
            let Some(usage) = usage.filter(|u| (0..=0xFFFF).contains(u)) else {
                return invalid(tr!("macro.consumer_range"));
            };
            out.extend([EV_CONSUMER, (usage & 0xFF) as u8, (usage >> 8) as u8]);
        } else if mouse_down || mouse_up || click {
            let Some(&(_, m)) = MOUSE_BUTTONS.iter().find(|(id, _)| is_named("mouse", id, &arg)) else {
                let names: Vec<String> =
                    MOUSE_BUTTONS.iter().map(|(id, _)| i18n::name(&format!("mouse.{id}"))).collect();
                return invalid(tr!("macro.mouse_button", names = names.join(", ")));
            };
            if !mouse_up {
                out.push(EV_MOUSE_DOWN | m);
            }
            if !mouse_down {
                out.push(EV_MOUSE_UP | m);
            }
        } else {
            return invalid(tr!("macro.unknown_command", cmd = cmd));
        }
    }
    Ok(out)
}

/// Python `int(text, 16)`: optional 0x prefix.
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
    key(name).map_or_else(|| invalid(tr!("macro.unknown_key", name = name)), Ok)
}

fn event_size(b: u8) -> usize {
    match b {
        EV_KEY_DOWN | EV_KEY_UP | EV_WAIT8 => 2,
        EV_CONSUMER | EV_WAIT16 => 3,
        _ => 1,
    }
}

/// Used bytes of a record/macro including the 00 end mark; 0 for an empty slot.
/// Walks the events because 00 also occurs in arguments (e.g. 85 E9 00).
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

/// Mode name in the current language.
pub fn mode_name(mode: u8) -> String {
    MODES
        .iter()
        .find(|&&(_, v)| v == mode)
        .map_or_else(|| format!("0x{mode:02X}"), |(id, _)| i18n::name(&format!("mode.{id}")))
}

/// (mode name, [event texts]) in the current language for a function record or a macro; stops at 00.
/// Deleted or zeroed slots give (None, []).
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
            let name = key_name(usage).unwrap_or_else(|| format!("0x{usage:02X}"));
            let verb = if b == EV_KEY_DOWN { i18n::name("macro.press") } else { i18n::name("macro.release") };
            events.push(format!("{verb} {name}"));
            i += 2;
        } else if b == EV_CONSUMER && i + 2 < data.len() {
            let u = u16::from(data[i + 1]) | u16::from(data[i + 2]) << 8;
            events.push(if u != 0 {
                format!("{} 0x{u:04X}", i18n::name("macro.consumer"))
            } else {
                i18n::name("macro.consumer_release")
            });
            i += 3;
        } else if b == EV_WAIT8 && i + 1 < data.len() {
            events.push(format!("{} {}", i18n::name("macro.wait"), data[i + 1]));
            i += 2;
        } else if b == EV_WAIT16 && i + 2 < data.len() {
            events.push(format!(
                "{} {}",
                i18n::name("macro.wait"),
                u16::from(data[i + 1]) | u16::from(data[i + 2]) << 8
            ));
            i += 3;
        } else if b & 0xE0 == EV_WHEEL && b & 0x1F != 0 {
            let raw = i32::from(b & 0x1F);
            let mut d = if raw & 0x10 != 0 { raw - 32 } else { raw };
            let direction = if d > 0 { i18n::name("macro.up") } else { i18n::name("macro.down") };
            let prefix = format!("{} {direction} ", i18n::name("macro.wheel"));
            // several notches in a row -> one line
            if let Some(last) = events.pop_if(|e| e.starts_with(&prefix)) {
                let prev: i32 = last.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                d += prev * if d > 0 { 1 } else { -1 };
            }
            events.push(format!("{prefix}{}", d.abs()));
            i += 1;
        } else if matches!(b & 0xE0, EV_MOUSE_DOWN | EV_MOUSE_UP) && b & 0x1F != 0 {
            let mut names: Vec<String> = MOUSE_BUTTONS
                .iter()
                .filter(|&&(_, m)| b & m != 0)
                .map(|(id, _)| i18n::name(&format!("mouse.{id}")))
                .collect();
            if names.is_empty() {
                names.push(format!("0x{:02X}", b & 0x1F));
            }
            let verb =
                if b & 0xE0 == EV_MOUSE_DOWN { i18n::name("macro.mouse_down") } else { i18n::name("macro.mouse_up") };
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
        assert_eq!(combo(&[k("strg"), k("v")]), by_key("einfuegen").unwrap().record); // German names work as well
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
        assert_eq!(key_name(0x2C).as_deref(), Some("space"));
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
    fn english_and_ascii_commands_encode_like_german() {
        let de = "drück shift, tippe strg+c, warte 300, los shift, klick links, maus-drück zurück, rad runter 2, consumer los";
        let ascii = "drueck shift, tippe strg+c, warte 300, los shift, klick links, maus-drueck zurueck, rad runter 2, consumer los";
        let en = "press shift, tap ctrl+c, wait 300, release shift, click left, mouse-down back, wheel down 2, consumer release";
        assert_eq!(encode_events(en).unwrap(), encode_events(de).unwrap());
        assert_eq!(encode_events(ascii).unwrap(), encode_events(de).unwrap());
        assert_eq!(mode_value("loop"), Some(MODE_LOOP));
        assert_eq!(key("pageup"), key("bildauf"));
    }

    #[test]
    fn decode_round_trip() {
        let (mode, events) = decode_record(&h("05 83 04 88 64 84 04 89 f4 01 21 41 00 ff"));
        assert_eq!(mode.as_deref(), Some("schleife"));
        assert_eq!(events, s(&["drück a", "warte 100", "los a", "warte 500", "maus-drück links", "maus-los links"]));
    }

    #[test]
    fn decode_klicker_macro_on_mouse() {
        assert_eq!(
            decode_record(&h("06 21 41 00")),
            (Some("feuer".into()), s(&["maus-drück links", "maus-los links"]))
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
        assert_eq!(by_key("copy").unwrap().key, "kopieren"); // English names always work
        assert_eq!(by_key("Rückgängig").unwrap().key, "rueckgaengig");
        assert_eq!(by_key("schliessen").unwrap().name(), "schließen");
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
        for bad in ["drück xyz", "warte 0", "warte 70000", "klick oben", "springe", "rad seitwaerts", "rad hoch 16"] {
            assert!(encode_events(bad).is_err(), "{bad}");
        }
    }
}
