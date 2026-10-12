//! Sprache der Ausgaben. Alle Texte stehen in `locales/<sprache>.yml` im Projektstamm (eine Datei pro Sprache,
//! beim Kompilieren eingebettet von rust-i18n); eine neue Sprache braucht nur eine neue Datei. Fehlt ein Text,
//! gilt der englische.
//!
//! Die Sprache gilt prozessweit; Kommandozeile und Oberflaeche setzen sie beim Start (`--lang`, Umgebungsvariable
//! `M6980X_LANG`, sonst Systemsprache). Namen, die man eingibt (Tasten, Farben, Funktionen, Makrotext), werden in
//! allen Sprachen erkannt, mit und ohne Umlaute; ein Eintrag "space|leer" nennt Anzeigename und weitere Eingaben.
//! Die Unit-Tests laufen auf Deutsch, weil sie die Texte des (deutschen) Python-Prototyps vergleichen.

use std::fmt::Display;
use std::sync::{LazyLock, RwLock};

// Pfad relativ zu crates/m6980x-core; build.rs baut neu, wenn sich dort etwas aendert.
rust_i18n::i18n!("../../locales", fallback = "en");

/// Ersatzsprache und Standard ausserhalb der Tests
pub const DEFAULT: &str = "en";

static CURRENT: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new(if cfg!(test) { "de" } else { DEFAULT }.to_string()));

/// Code der aktuellen Sprache, z. B. "en".
pub fn lang() -> String {
    CURRENT.read().map(|l| l.clone()).unwrap_or_else(|_| DEFAULT.into())
}

/// Sprache umschalten; `false`, wenn es dafuer keine Datei gibt.
pub fn set_lang(code: &str) -> bool {
    let Some(code) = resolve(code) else {
        return false;
    };
    if let Ok(mut current) = CURRENT.write() {
        *current = code;
    }
    true
}

/// Alle Sprachen mit Datei, Englisch zuerst.
pub fn available() -> Vec<String> {
    let mut codes: Vec<String> = _rust_i18n_available_locales().iter().map(|c| c.to_string()).collect();
    codes.sort_by_key(|c| (c != DEFAULT, c.clone()));
    codes
}

/// Passende vorhandene Sprache zu einem Code oder Locale-Namen ("de_DE.UTF-8", "de-AT", "DE"), sonst None.
pub fn resolve(text: &str) -> Option<String> {
    let t = text.trim().split(['.', '@']).next().unwrap_or("").replace('_', "-").to_lowercase();
    let primary = t.split('-').next().unwrap_or("");
    let codes = available();
    let find = |want: &str| codes.iter().find(|c| c.to_lowercase() == want).cloned();
    find(&t).or_else(|| find(primary)).or_else(|| matches!(primary, "c" | "posix").then(|| DEFAULT.to_string()))
}

/// Name einer Sprache in ihr selbst ("Deutsch").
pub fn native_name(code: &str) -> String {
    text_in(code, "language.name")
}

/// Sprache aus `M6980X_LANG`, sonst aus der Systemsprache, sonst Englisch.
pub fn detect() -> String {
    if let Some(code) = std::env::var("M6980X_LANG").ok().as_deref().and_then(resolve) {
        return code;
    }
    system_languages().iter().find_map(|l| resolve(l)).unwrap_or_else(|| DEFAULT.into())
}

/// Bevorzugte Oberflaechensprachen des Benutzers, z. B. ["de-DE", "en-US"].
#[cfg(windows)]
fn system_languages() -> Vec<String> {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    let (mut count, mut len) = (0u32, 0u32);
    // SAFETY: erster Aufruf nur mit Laengenabfrage (Puffer null), zweiter mit Puffer der gemeldeten Laenge.
    unsafe {
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, std::ptr::null_mut(), &mut len) == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len) == 0 {
            return Vec::new();
        }
        String::from_utf16_lossy(&buf).split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect()
    }
}

#[cfg(not(windows))]
fn system_languages() -> Vec<String> {
    // Reihenfolge wie gettext: LANGUAGE (Liste), LC_ALL, LC_MESSAGES, LANG
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
        {
            return value.split(':').filter(|v| !v.is_empty()).map(str::to_string).collect();
        }
    }
    Vec::new()
}

/// Gibt es diesen Schluessel (in der englischen Datei)?
pub fn has(key: &str) -> bool {
    _rust_i18n_try_translate(DEFAULT, key).is_some()
}

/// Text in einer bestimmten Sprache (fehlt er, der englische; fehlt auch der, der Schluessel selbst).
pub fn text_in(lang: &str, key: &str) -> String {
    _rust_i18n_translate(lang, key).into_owned()
}

/// Text in der aktuellen Sprache.
pub fn text(key: &str) -> String {
    text_in(&lang(), key)
}

/// Text mit Platzhaltern `%{name}`; Werte schon fertig formatiert (z. B. `format!("{x:04X}")`).
pub fn text_with(key: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = text(key);
    for (name, value) in args {
        out = out.replace(&format!("%{{{name}}}"), &value.to_string());
    }
    out
}

/// Anzeigename in der aktuellen Sprache: bei "space|leer" der erste Teil.
pub fn name(key: &str) -> String {
    text(key).split('|').next().unwrap_or_default().trim().to_string()
}

/// Passt die Eingabe zu diesem Namen in irgendeiner Sprache (alle Teile von "a|b", Umlaute egal)?
pub fn matches(key: &str, input: &str) -> bool {
    let input = fold(input.trim());
    available().iter().any(|l| text_in(l, key).split('|').any(|n| fold(n.trim()) == input))
}

/// Fuer Vergleiche von Eingaben: klein, Umlaute als ae/oe/ue/ss ("Rückgängig" == "rueckgaengig").
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            c => out.push(c),
        }
    }
    out
}

/// `tr!("error.no_reply", packet = hex(pkt))` - Text der aktuellen Sprache aus `locales/`, `%{packet}` ersetzt.
#[macro_export]
macro_rules! tr {
    ($key:expr $(,)?) => {
        $crate::i18n::text($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::text_with($key, &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn resolve_locale_names() {
        assert_eq!(resolve("de_DE.UTF-8").as_deref(), Some("de"));
        assert_eq!(resolve("en-US").as_deref(), Some("en"));
        assert_eq!(resolve("DE").as_deref(), Some("de"));
        assert_eq!(resolve("C").as_deref(), Some("en"));
        assert_eq!(resolve("xx_XX"), None);
    }

    #[test]
    fn fold_umlauts() {
        assert_eq!(fold("Rückgängig"), "rueckgaengig");
        assert_eq!(fold("schließen"), "schliessen");
        assert_eq!(fold("Öffnen"), "oeffnen");
    }

    #[test]
    fn placeholders_and_names() {
        assert_eq!(text_in("en", "error.no_reply").replace("%{packet}", "01"), "no reply to 01");
        assert_eq!(text_with("error.no_reply", &[("packet", &"01")]), "keine Antwort auf 01");
        assert!(matches("key.space", "leer") && matches("key.space", "space") && !matches("key.space", "enter"));
        assert!(matches("led.green", "gruen") && matches("led.green", "Grün") && matches("led.green", "green"));
    }

    fn keys(lang: &str) -> BTreeSet<String> {
        _rust_i18n_backend()
            .messages_for_locale(lang)
            .unwrap_or_default()
            .into_iter()
            .map(|(k, _)| k.to_string())
            .collect()
    }

    /// Jede Sprachdatei hat genau die Schluessel der englischen und dieselben Platzhalter.
    #[test]
    fn every_language_is_complete() {
        let en = keys(DEFAULT);
        assert!(en.len() > 100, "en.yml nicht geladen");
        for lang in available() {
            let have = keys(&lang);
            let missing: Vec<_> = en.difference(&have).collect();
            let extra: Vec<_> = have.difference(&en).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{lang}: fehlt {missing:?}, zu viel {extra:?}");
            for key in &en {
                let holders = |l: &str| -> BTreeSet<String> {
                    let t = text_in(l, key);
                    t.split("%{").skip(1).filter_map(|p| p.split_once('}')).map(|(n, _)| n.to_string()).collect()
                };
                assert_eq!(holders(&lang), holders(DEFAULT), "{lang}: Platzhalter in {key}");
            }
        }
    }
}
