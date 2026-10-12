//! Namen und Anzeigetexte, die Kommandozeile und Oberflaeche teilen (im Prototyp in cli.py).

use crate::functions as fnc;
use crate::protocol::{BUTTONS, LED_LEVELS};
use crate::rom as romlayout;
use crate::{Result, invalid};

/// Kanalstufen: 0 = aus, 1 = halb, 2 = voll. Reihenfolge zaehlt: bei gleichen Werten gewinnt der erste Name.
pub const LED_NAMES: [(&str, [u8; 3]); 15] = [
    // die 8 Farben der Farbauswahl im Original (Macro.exe 0x451000-0x45115e)
    ("aus", [0, 0, 0]),
    ("blau", [0, 0, 2]),
    ("gruen", [0, 2, 0]),
    ("hellblau", [0, 2, 2]),
    ("hellgruen", [1, 1, 0]),
    ("lila", [2, 0, 1]),
    ("gelb", [2, 1, 0]),
    ("rot", [2, 0, 0]),
    // weitere Kombinationen, die die Maus annimmt
    ("weiss", [2, 2, 2]),
    ("magenta", [2, 0, 2]),
    ("hellgelb", [2, 2, 0]),
    ("violett", [1, 0, 2]),
    ("himmelblau", [0, 1, 2]),
    ("cyan", [0, 2, 2]),
    ("orange", [2, 1, 0]),
];

pub fn led_name(rgb: [u8; 3]) -> Option<&'static str> {
    LED_NAMES.iter().find(|(_, v)| *v == rgb).map(|(n, _)| *n)
}

pub fn format_led(rgb: [u8; 3]) -> String {
    let [r, g, b] = rgb;
    if rgb.iter().any(|c| !LED_LEVELS.contains(c)) {
        return format!("R {r:3}  G {g:3}  B {b:3}   (ungueltig, erlaubt sind 0-2)");
    }
    format!("R {r}  G {g}  B {b}   {}", led_name(rgb).unwrap_or(""))
}

pub fn parse_led<S: AsRef<str>>(values: &[S]) -> Result<[u8; 3]> {
    if let [one] = values
        && let Some((_, rgb)) = LED_NAMES.iter().find(|(n, _)| *n == one.as_ref().to_lowercase())
    {
        return Ok(*rgb);
    }
    if values.len() == 3 && values.iter().all(|v| matches!(v.as_ref(), "0" | "1" | "2")) {
        let p = |i: usize| values[i].as_ref().parse::<u8>().expect("geprueft");
        return Ok([p(0), p(1), p(2)]);
    }
    let names: Vec<&str> = LED_NAMES.iter().map(|(n, _)| *n).collect();
    invalid(format!("Farbe als Name ({}) oder als drei Stufen R G B je 0-2 angeben", names.join(", ")))
}

/// Geraetetastennummern, an der Maus ermittelt (jede Taste hat ihre Nummer getippt); 5 hat keine Taste.
pub const BUTTON_NAMES: [(u8, &str); 9] = [
    (0, "links"),
    (1, "rechts"),
    (2, "mitte"),
    (3, "hinten"),
    (4, "vorne"),
    (6, "kipp-links"),
    (7, "kipp-rechts"),
    (8, "rad-hoch"),
    (9, "rad-runter"),
];

/// Eingebaute Funktion ("Standard") jeder Taste, aus den Input-Reports der Maus gemessen (2026-10-11).
pub const STANDARD_ACTIONS: [&str; 10] = [
    "Linksklick",
    "Rechtsklick",
    "Mittelklick",
    "Zurueck, Maustaste 4",
    "Vor, Maustaste 5",
    "keine Taste",
    "Bildlauf links",
    "Bildlauf rechts",
    "Bildlauf hoch",
    "Bildlauf runter",
];

pub fn button_name(button: u8) -> Option<&'static str> {
    BUTTON_NAMES.iter().find(|(b, _)| *b == button).map(|(_, n)| *n)
}

pub fn standard_label(button: Option<u8>) -> String {
    match button.and_then(|b| STANDARD_ACTIONS.get(usize::from(b))) {
        Some(action) => format!("Standard ({action})"),
        None => "Standard".into(),
    }
}

pub fn parse_button(text: &str) -> Result<u8> {
    if let Some((b, _)) = BUTTON_NAMES.iter().find(|(_, n)| *n == text) {
        return Ok(*b);
    }
    if !text.is_empty()
        && text.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = text.parse::<u8>()
        && usize::from(n) < BUTTONS
    {
        return Ok(n);
    }
    let names: Vec<&str> = BUTTON_NAMES.iter().map(|(_, n)| *n).collect();
    invalid(format!("Taste als Nummer 0-{} oder Name ({})", BUTTONS - 1, names.join(", ")))
}

/// Pythons `repr()` fuer kurze Namen: 'Klicker'.
pub fn py_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (0x7F..0xA1).contains(&(c as u32)) || c == '\u{AD}' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

pub fn describe_target(addr: Option<u16>, names: &[String], button: Option<u8>) -> String {
    let Some(addr) = addr else {
        return standard_label(button);
    };
    if let Some(slot) = romlayout::macro_slot_of(addr) {
        let name = names.get(slot).map(String::as_str).unwrap_or("");
        return if name.is_empty() {
            format!("Makro {}", slot + 1)
        } else {
            format!("Makro {} {}", slot + 1, py_repr(name))
        };
    }
    match fnc::by_address(addr) {
        Some(f) => f.label.to_string(),
        None => format!("ROM 0x{addr:04X}"),
    }
}

/// Abtastraten: (Hz, Intervall ms) - 1000, 500, 333, 250, 200, 166, 142, 125 Hz.
pub fn rates() -> Vec<(u32, u8)> {
    (1..=8u8).map(|ms| (rate_hz(ms), ms)).collect()
}

pub fn rate_hz(ms: u8) -> u32 {
    1000 / u32::from(ms.max(1))
}

pub fn interval_for_hz(hz: u32) -> Option<u8> {
    rates().into_iter().find(|&(h, _)| h == hz).map(|(_, ms)| ms)
}

pub fn format_icon(icon: Option<u8>) -> String {
    match icon {
        None => "kein Bild".into(),
        Some(n) => format!("Bild {n} ({})", romlayout::icon_source(n)),
    }
}

pub fn parse_icon(text: &str) -> Result<Option<u8>> {
    if matches!(text, "keins" | "kein") {
        return Ok(None);
    }
    match text.parse::<u8>() {
        Ok(n) if text.chars().all(|c| c.is_ascii_digit()) && (1..=romlayout::ICON_COUNT).contains(&n) => Ok(Some(n)),
        _ => invalid(format!("Bild als Nummer 1-{} oder 'keins'", romlayout::ICON_COUNT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names_and_levels() {
        assert_eq!(parse_led(&["Rot"]).unwrap(), [2, 0, 0]);
        assert_eq!(parse_led(&["0", "1", "2"]).unwrap(), [0, 1, 2]);
    }

    #[test]
    fn parse_rejects_out_of_range() {
        let bad: [&[&str]; 5] = [&["255", "0", "0"], &["3", "0", "0"], &["rot", "1"], &["regenbogen"], &[]];
        for b in bad {
            assert!(parse_led(b).is_err(), "{b:?}");
        }
    }

    #[test]
    fn all_names_use_valid_levels() {
        assert!(LED_NAMES.iter().all(|(_, rgb)| rgb.iter().all(|c| LED_LEVELS.contains(c))));
    }

    #[test]
    fn format_flags_invalid_values() {
        assert!(format_led([255, 255, 0]).contains("ungueltig"));
        assert!(format_led([2, 2, 0]).contains("gelb"));
        assert_eq!(format_led([2, 1, 0]), "R 2  G 1  B 0   gelb");
    }

    #[test]
    fn rates_and_buttons() {
        let hz: Vec<u32> = rates().iter().map(|r| r.0).collect();
        assert_eq!(hz, [1000, 500, 333, 250, 200, 166, 142, 125]);
        assert_eq!(parse_button("hinten").unwrap(), 3);
        assert_eq!(parse_button("9").unwrap(), 9);
        assert!(parse_button("10").is_err());
        assert_eq!(standard_label(Some(6)), "Standard (Bildlauf links)");
    }

    #[test]
    fn describe_targets() {
        let mut names = vec![String::new(); 20];
        names[0] = "Klicker".into();
        assert_eq!(describe_target(Some(0), &names, Some(4)), "Makro 1 'Klicker'");
        assert_eq!(describe_target(Some(0x170), &names, Some(4)), "Makro 2");
        assert_eq!(describe_target(Some(0x1DD8), &names, None), "Kopieren (Strg+C)");
        assert_eq!(describe_target(Some(0x1234), &names, None), "ROM 0x1234");
        assert_eq!(describe_target(None, &names, Some(0)), "Standard (Linksklick)");
    }
}
