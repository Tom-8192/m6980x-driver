//! Every `tr!("key", name = value, ...)` in the sources must pass exactly the placeholders `%{name}` that the text
//! has in each language file. Otherwise rust-i18n leaves `%{name}` unreplaced in the output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn placeholders(text: &str) -> BTreeSet<String> {
    text.split("%{").skip(1).filter_map(|s| s.split_once('}')).map(|(name, _)| name.to_string()).collect()
}

/// Flat language file: `key: "text"` per line.
fn load_locale(path: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .filter_map(|line| line.split_once(": \""))
        .filter(|(key, _)| !key.starts_with(' '))
        .map(|(key, value)| (key.to_string(), placeholders(value)))
        .collect()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Text from just after `tr!(` up to the matching closing parenthesis (string literals skipped).
fn call_body(src: &str) -> &str {
    let (mut depth, mut in_string, mut escaped) = (1, false, false);
    for (i, c) in src.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[..i];
                }
            }
            _ => {}
        }
    }
    src
}

/// Names before `=` of the top-level arguments after the key.
fn named_args(body: &str) -> BTreeSet<String> {
    let (mut depth, mut in_string, mut escaped, mut start) = (0, false, false, 0);
    let mut parts = Vec::new();
    for (i, c) in body.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
        .iter()
        .skip(1)
        .filter_map(|p| {
            let (name, rest) = p.split_once('=')?;
            let name = name.trim();
            (!rest.starts_with('=') && !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
                .then(|| name.to_string())
        })
        .collect()
}

#[test]
fn tr_arguments_match_placeholders() {
    let locales: Vec<(String, BTreeMap<String, BTreeSet<String>>)> = ["en", "de"]
        .iter()
        .map(|code| (code.to_string(), load_locale(&root().join(format!("locales/{code}.yml")))))
        .collect();
    let mut files = Vec::new();
    rust_files(&root().join("crates"), &mut files);
    let mut problems = Vec::new();
    let mut checked = 0;
    for file in files {
        let src = std::fs::read_to_string(&file).unwrap();
        for (pos, _) in src.match_indices("tr!(") {
            let body = call_body(&src[pos + 4..]);
            let Some(key) = body.trim_start().strip_prefix('"').and_then(|s| s.split_once('"')).map(|(k, _)| k) else {
                continue; // key built at runtime
            };
            let given = named_args(body);
            for (code, texts) in &locales {
                if let Some(wanted) = texts.get(key)
                    && *wanted != given
                {
                    problems.push(format!("{}: {key} ({code}) passes {given:?}, text has {wanted:?}", file.display()));
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} tr! calls found - parser broken?");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
