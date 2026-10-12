//! Language of the output. All texts live in `locales/<language>.yml` in the project root (one file per
//! language, embedded at compile time by rust-i18n); a new language only needs a new file. If a text is
//! missing, the English one is used.
//!
//! The language applies to the whole process; command line and GUI set it at startup (`--lang`, environment
//! variable `M6980X_LANG`, otherwise the system language). Names that are typed (keys, colours, functions, macro
//! text) are recognised in all languages, with and without umlauts; an entry "space|leer" gives the display name and further inputs.
//! The unit tests run in German because they compare the texts of the (German) Python prototype.

use std::fmt::Display;
use std::sync::{LazyLock, RwLock};

// path relative to crates/m6980x-core; build.rs rebuilds when something changes there.
rust_i18n::i18n!("../../locales", fallback = "en");

/// fallback language and default outside the tests
pub const DEFAULT: &str = "en";

static CURRENT: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new(if cfg!(test) { "de" } else { DEFAULT }.to_string()));

/// Code of the current language, e.g. "en".
pub fn lang() -> String {
    CURRENT.read().map(|l| l.clone()).unwrap_or_else(|_| DEFAULT.into())
}

/// Switch the language; `false` if there is no file for it.
pub fn set_lang(code: &str) -> bool {
    let Some(code) = resolve(code) else {
        return false;
    };
    if let Ok(mut current) = CURRENT.write() {
        *current = code;
    }
    true
}

/// All languages that have a file, English first.
pub fn available() -> Vec<String> {
    let mut codes: Vec<String> = _rust_i18n_available_locales().iter().map(|c| c.to_string()).collect();
    codes.sort_by_key(|c| (c != DEFAULT, c.clone()));
    codes
}

/// Matching available language for a code or locale name ("de_DE.UTF-8", "de-AT", "DE"), otherwise None.
pub fn resolve(text: &str) -> Option<String> {
    let t = text.trim().split(['.', '@']).next().unwrap_or("").replace('_', "-").to_lowercase();
    let primary = t.split('-').next().unwrap_or("");
    let codes = available();
    let find = |want: &str| codes.iter().find(|c| c.to_lowercase() == want).cloned();
    find(&t).or_else(|| find(primary)).or_else(|| matches!(primary, "c" | "posix").then(|| DEFAULT.to_string()))
}

/// Name of a language in itself ("Deutsch").
pub fn native_name(code: &str) -> String {
    text_in(code, "language.name")
}

/// Language from `M6980X_LANG`, otherwise from the system language, otherwise English.
pub fn detect() -> String {
    if let Some(code) = std::env::var("M6980X_LANG").ok().as_deref().and_then(resolve) {
        return code;
    }
    system_languages().iter().find_map(|l| resolve(l)).unwrap_or_else(|| DEFAULT.into())
}

/// Preferred UI languages of the user, e.g. ["de-DE", "en-US"].
#[cfg(windows)]
fn system_languages() -> Vec<String> {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    let (mut count, mut len) = (0u32, 0u32);
    // SAFETY: first call only queries the length (null buffer), second one with a buffer of the reported length.
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
    // order as in gettext: LANGUAGE (list), LC_ALL, LC_MESSAGES, LANG
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
        {
            return value.split(':').filter(|v| !v.is_empty()).map(str::to_string).collect();
        }
    }
    Vec::new()
}

/// Does this key exist (in the English file)?
pub fn has(key: &str) -> bool {
    _rust_i18n_try_translate(DEFAULT, key).is_some()
}

/// Text in a given language (if missing, the English one; if that is missing too, the key itself).
pub fn text_in(lang: &str, key: &str) -> String {
    _rust_i18n_translate(lang, key).into_owned()
}

/// Text in the current language.
pub fn text(key: &str) -> String {
    text_in(&lang(), key)
}

/// Text with placeholders `%{name}`; values already formatted (e.g. `format!("{x:04X}")`).
pub fn text_with(key: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = text(key);
    for (name, value) in args {
        out = out.replace(&format!("%{{{name}}}"), &value.to_string());
    }
    out
}

/// Display name in the current language: the first part of "space|leer".
pub fn name(key: &str) -> String {
    text(key).split('|').next().unwrap_or_default().trim().to_string()
}

/// Does the input match this name in any language (all parts of "a|b", umlauts ignored)?
pub fn matches(key: &str, input: &str) -> bool {
    let input = fold(input.trim());
    available().iter().any(|l| text_in(l, key).split('|').any(|n| fold(n.trim()) == input))
}

/// For comparing inputs: lower case, umlauts as ae/oe/ue/ss ("Rückgängig" == "rueckgaengig").
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

/// `tr!("error.no_reply", packet = hex(pkt))` - text of the current language from `locales/` with `%{packet}` replaced.
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

    /// Every language file has exactly the keys of the English one and the same placeholders.
    #[test]
    fn every_language_is_complete() {
        let en = keys(DEFAULT);
        assert!(en.len() > 100, "en.yml not loaded");
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
