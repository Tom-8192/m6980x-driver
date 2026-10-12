//! Names and display texts shared by command line and GUI (in the prototype in cli.py).

use crate::functions as fnc;
use crate::i18n::{self, fold};
use crate::protocol::{BUTTONS, LED_LEVELS};
use crate::rom as romlayout;
use crate::{Result, invalid, tr};

/// Channel levels: 0 = off, 1 = half, 2 = full. (id, RGB); the display name is in locales/ under `led.<id>`.
/// Order matters: for equal values the first name wins.
pub const LED_COLORS: [(&str, [u8; 3]); 15] = [
    // the 8 colours of the original's colour picker (Macro.exe 0x451000-0x45115e)
    ("off", [0, 0, 0]),
    ("blue", [0, 0, 2]),
    ("green", [0, 2, 0]),
    ("light-blue", [0, 2, 2]),
    ("light-green", [1, 1, 0]),
    ("purple", [2, 0, 1]),
    ("yellow", [2, 1, 0]),
    ("red", [2, 0, 0]),
    // further combinations the mouse accepts
    ("white", [2, 2, 2]),
    ("magenta", [2, 0, 2]),
    ("light-yellow", [2, 2, 0]),
    ("violet", [1, 0, 2]),
    ("sky-blue", [0, 1, 2]),
    ("cyan", [0, 2, 2]),
    ("orange", [2, 1, 0]),
];

/// (name in the current language, RGB) of all colour presets.
pub fn led_names() -> Vec<(String, [u8; 3])> {
    LED_COLORS.iter().map(|(id, rgb)| (i18n::name(&format!("led.{id}")), *rgb)).collect()
}

pub fn led_name(rgb: [u8; 3]) -> Option<String> {
    LED_COLORS.iter().find(|(_, v)| *v == rgb).map(|(id, _)| i18n::name(&format!("led.{id}")))
}

pub fn format_led(rgb: [u8; 3]) -> String {
    let [r, g, b] = rgb;
    if rgb.iter().any(|c| !LED_LEVELS.contains(c)) {
        return tr!("names.led_invalid", r = format!("{r:3}"), g = format!("{g:3}"), b = format!("{b:3}"));
    }
    format!("R {r}  G {g}  B {b}   {}", led_name(rgb).unwrap_or_default())
}

pub fn parse_led<S: AsRef<str>>(values: &[S]) -> Result<[u8; 3]> {
    if let [one] = values
        && let Some((_, rgb)) = LED_COLORS
            .iter()
            .find(|(id, _)| *id == fold(one.as_ref()) || i18n::matches(&format!("led.{id}"), one.as_ref()))
    {
        return Ok(*rgb);
    }
    if values.len() == 3 && values.iter().all(|v| matches!(v.as_ref(), "0" | "1" | "2")) {
        let p = |i: usize| values[i].as_ref().parse::<u8>().expect("checked");
        return Ok([p(0), p(1), p(2)]);
    }
    let names: Vec<String> = led_names().into_iter().map(|(n, _)| n).collect();
    invalid(tr!("names.colour_syntax", names = names.join(", ")))
}

/// Device button numbers, found on the mouse (each button typed its number); 5 has no button.
/// (number, id); display name in locales/ under `button.<id>`.
pub const BUTTON_IDS: [(u8, &str); 9] = [
    (0, "left"),
    (1, "right"),
    (2, "middle"),
    (3, "back"),
    (4, "forward"),
    (6, "tilt-left"),
    (7, "tilt-right"),
    (8, "wheel-up"),
    (9, "wheel-down"),
];

/// (number, name in the current language) of all assignable buttons.
pub fn button_names() -> Vec<(u8, String)> {
    BUTTON_IDS.iter().map(|(b, id)| (*b, i18n::name(&format!("button.{id}")))).collect()
}

pub fn button_name(button: u8) -> Option<String> {
    BUTTON_IDS.iter().find(|(b, _)| *b == button).map(|(_, id)| i18n::name(&format!("button.{id}")))
}

/// Built-in function ("default") of every button 0-9, measured from the mouse input reports (2026-10-11);
/// texts in locales/ under `button_action.<number>`.
pub fn standard_label(button: Option<u8>) -> String {
    match button.filter(|&b| usize::from(b) < BUTTONS) {
        Some(b) => tr!("names.default_of", action = tr!(&format!("button_action.{b}"))),
        None => tr!("names.default"),
    }
}

/// Target word for "back to the built-in function" (command line), in any language.
pub fn is_standard_word(text: &str) -> bool {
    matches!(fold(text).as_str(), "standard" | "default") || i18n::matches("names.default", text)
}

pub fn parse_button(text: &str) -> Result<u8> {
    let folded = fold(text);
    if let Some((b, _)) =
        BUTTON_IDS.iter().find(|(_, id)| *id == folded || i18n::matches(&format!("button.{id}"), text))
    {
        return Ok(*b);
    }
    if !text.is_empty()
        && text.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = text.parse::<u8>()
        && usize::from(n) < BUTTONS
    {
        return Ok(n);
    }
    let names: Vec<String> = button_names().into_iter().map(|(_, n)| n).collect();
    invalid(tr!("names.button_syntax", buttons = BUTTONS - 1, names = names.join(", ")))
}

/// Python `repr()` for short names: 'Klicker'.
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
            tr!("names.macro", slot = slot + 1)
        } else {
            tr!("names.macro_named", slot = slot + 1, name = py_repr(name))
        };
    }
    match fnc::by_address(addr) {
        Some(f) => f.label().to_string(),
        None => format!("ROM 0x{addr:04X}"),
    }
}

/// Report rates: (Hz, interval ms) - 1000, 500, 333, 250, 200, 166, 142, 125 Hz.
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
        None => tr!("names.no_icon"),
        Some(n) => tr!("names.icon", n = n, source = romlayout::icon_source(n)),
    }
}

pub fn parse_icon(text: &str) -> Result<Option<u8>> {
    if fold(text) == "none" || i18n::matches("names.icon_none", text) {
        return Ok(None);
    }
    match text.parse::<u8>() {
        Ok(n) if text.chars().all(|c| c.is_ascii_digit()) && (1..=romlayout::ICON_COUNT).contains(&n) => Ok(Some(n)),
        _ => invalid(tr!("names.icon_syntax", icon_count = romlayout::ICON_COUNT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names_and_levels() {
        assert_eq!(parse_led(&["Rot"]).unwrap(), [2, 0, 0]);
        assert_eq!(parse_led(&["0", "1", "2"]).unwrap(), [0, 1, 2]);
        assert_eq!(parse_led(&["Light-Blue"]).unwrap(), [0, 2, 2]); // English names always work
        assert_eq!(parse_led(&["gruen"]).unwrap(), parse_led(&["Grün"]).unwrap()); // with and without umlaut
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
        assert!(LED_COLORS.iter().all(|(_, rgb)| rgb.iter().all(|c| LED_LEVELS.contains(c))));
    }

    #[test]
    fn format_flags_invalid_values() {
        assert!(format_led([255, 255, 0]).contains("ungültig"));
        assert!(format_led([2, 2, 0]).contains("gelb"));
        assert_eq!(format_led([2, 1, 0]), "R 2  G 1  B 0   gelb");
    }

    #[test]
    fn rates_and_buttons() {
        let hz: Vec<u32> = rates().iter().map(|r| r.0).collect();
        assert_eq!(hz, [1000, 500, 333, 250, 200, 166, 142, 125]);
        assert_eq!(parse_button("hinten").unwrap(), 3);
        assert_eq!(parse_button("tilt-left").unwrap(), 6);
        assert!(parse_icon("none").unwrap().is_none());
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
