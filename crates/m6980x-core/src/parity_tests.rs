//! Parity with the Python prototype: the same scenarios as prototype/python/tools/export_fixtures.py
//! must send byte-identical packets and leave the same ROM.

use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::backup;
use crate::fake::FakeMouse;
use crate::functions::{self as fnc, FUNCTIONS};
use crate::protocol::M6980X;
use crate::{Result, hex};

const FIXTURE: &str = include_str!("../../../tests/fixtures/parity.json");

const LONG_MACRO_PARTS: [&str; 10] = [
    "tippe strg+c",
    "warte 100",
    "klick links",
    "rad runter 3",
    "consumer 0x00E9",
    "consumer los",
    "warte 1000",
    "drueck shift",
    "tippe a+b",
    "los shift",
];

fn long_macro() -> String {
    std::iter::repeat_n(LONG_MACRO_PARTS.join(", "), 6).collect::<Vec<_>>().join(", ")
}

/// The prototype writes umlauts as ae/oe/ue/ss, Rust shows them.
fn ascii(text: &str) -> String {
    text.replace('ä', "ae").replace('ö', "oe").replace('ü', "ue").replace('ß', "ss")
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("parity.json")
}

fn sha(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

fn h(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
}

type G = M6980X<FakeMouse>;
type Scenario = fn(&mut G) -> Result<Value>;

fn runs_json(runs: &[(usize, usize)]) -> Value {
    Value::Array(runs.iter().map(|&(s, e)| serde_json::json!([s, e])).collect())
}

fn settings(g: &mut G) -> Result<Value> {
    let combos: Vec<[u8; 3]> =
        (0..3).flat_map(|r| (0..3).flat_map(move |gr| (0..3).map(move |b| [r, gr, b]))).collect();
    for p in 0..3u8 {
        for rgb in combos.iter().skip(p.into()).step_by(3) {
            g.set_led_color(p, *rgb)?;
        }
    }
    for ms in 1..=8 {
        g.set_report_interval(ms)?;
    }
    for stage in [0, 2, 1, 3] {
        g.set_active_dpi_stage(stage)?;
    }
    for profile in [2, 0, 1] {
        g.set_active_profile(profile)?;
    }
    g.save_settings()?;
    g.apply_factory_defaults()?;
    Ok(Value::Null)
}

fn dpi(g: &mut G) -> Result<Value> {
    for (stage, x, y) in [
        (0, 90, None),
        (1, 1600, None),
        (2, 5850, None),
        (3, 6030, None),
        (0, 4510, Some(3000)),
        (1, 4545, Some(4635)),
        (2, 5040, Some(5130)),
        (0, 1234, Some(999)),
    ] {
        g.set_dpi(stage, x, y)?;
    }
    Ok(Value::Null)
}

fn functions(g: &mut G) -> Result<Value> {
    for f in FUNCTIONS.iter().filter(|f| f.size > 0 && !f.record.is_empty()) {
        for button in [3, 8] {
            g.assign_function((f.index % 3) as u8, button, f)?;
        }
    }
    Ok(Value::Null)
}

fn macros(g: &mut G) -> Result<Value> {
    let mut long = vec![fnc::MODE_LOOP];
    long.extend(fnc::encode_events(&long_macro())?);
    long.push(0);
    g.write_macro(0, &long, "Lang", Some(12))?;
    g.write_macro(19, &h("06 21 41 00"), "Klicker", None)?;
    g.write_macro(7, &h("04 83 04 88 64 84 04 00"), "XY", Some(100))?;
    g.set_macro_icon(3, Some(40))?;
    g.set_macro_icon(7, None)?;
    g.clear_macro(5)?;
    g.assign_macro(1, 4, 7)?;
    g.reset_button(0, 4)?;
    Ok(Value::Null)
}

fn release(g: &mut G) -> Result<Value> {
    g.release_all()?;
    Ok(Value::Null)
}

fn restore(g: &mut G) -> Result<Value> {
    let snap = backup::create(g, &mut |_, _| {})?;
    g.write_macro(3, &h("04 83 04 84 04 00"), "Test", Some(7))?;
    g.assign_function(1, 3, fnc::by_key("kopieren").unwrap())?;
    g.assign_function(2, 9, fnc::by_key("browser-vor").unwrap())?;
    g.reset_button(0, 4)?;
    g.set_led_color(2, [0, 0, 0])?;
    g.set_dpi(0, 900, None)?;
    g.set_dpi(3, 4500, Some(2970))?;
    g.set_report_interval(8)?;
    g.set_active_dpi_stage(2)?;
    g.set_active_profile(2)?;
    g.t.sent.clear();
    let left = backup::restore(g, &snap, &mut |_| {})?;
    Ok(serde_json::json!({ "left": runs_json(&left) }))
}

fn import(g: &mut G) -> Result<Value> {
    let mut wanted = g.t.rom.clone();
    wanted[0x0100..0x0104].copy_from_slice(&[0x04, 0x83, 0x05, 0x00]);
    for (i, b) in wanted[0x0400..0x0450].iter_mut().enumerate() {
        *b = i as u8;
    }
    wanted[0x1F01..0x1F25].fill(0);
    wanted[0x1F01 + 2 * 12 + 3] = 0x80;
    let a = 0x1F3D + 2 * 0x18 + 3 * 2;
    wanted[a..a + 2].copy_from_slice(&0x1D98u16.to_le_bytes());
    wanted[0x1F01 + 12 + 9] = 0x80;
    let a = 0x1F3D + 0x18 + 9 * 2;
    wanted[a..a + 2].copy_from_slice(&0x0170u16.to_le_bytes());
    wanted[0x1F80] = 0x55;
    let left = backup::import_rom(g, &wanted, &mut |_| {})?;
    Ok(serde_json::json!({ "left": runs_json(&left) }))
}

fn reads(g: &mut G) -> Result<Value> {
    let ident = g.identify()?;
    let name = g.name()?;
    let steps = g.dpi_supported_steps()?;
    let dpi: Vec<[u32; 2]> = (0..4).map(|i| g.dpi_stage(i).map(|s| [s.x_dpi(), s.y_dpi()])).collect::<Result<_>>()?;
    let mut buttons = Vec::new();
    for p in 0..3 {
        buttons.push((0..10).map(|b| g.button(p, b)).collect::<Result<Vec<_>>>()?);
    }
    Ok(serde_json::json!({
        "identify": ident,
        "name": name,
        "steps": steps,
        "dpi": dpi,
        "buttons": buttons,
        "rom": backup::to_hex(&g.read_rom(0x1CC1, 200)?),
    }))
}

#[test]
fn scenarios_send_identical_packets() {
    let fx = fixture();
    let scenarios: [(&str, Scenario); 8] = [
        ("settings", settings),
        ("dpi", dpi),
        ("functions", functions),
        ("macros", macros),
        ("release", release),
        ("restore", restore),
        ("import", import),
        ("reads", reads),
    ];
    for (name, run) in scenarios {
        let expected = &fx["scenarios"][name];
        let mut g = M6980X::new(FakeMouse::new());
        g.poll_interval = Duration::ZERO;
        let extra = run(&mut g).unwrap_or_else(|e| panic!("{name}: {e}"));
        let sent: Vec<String> = g.t.sent.iter().map(|p| hex(p)).collect();
        let want: Vec<&str> = expected["sent"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        if let Some(i) =
            (0..sent.len().max(want.len())).find(|&i| sent.get(i).map(String::as_str) != want.get(i).copied())
        {
            panic!("{name}: packet {i} differs: Rust {:?}, Python {:?}", sent.get(i), want.get(i));
        }
        assert_eq!(sha(&g.t.rom), expected["rom_sha256"].as_str().unwrap(), "{name}: ROM");
        if let Value::Object(map) = extra {
            for (k, v) in map {
                assert_eq!(v, expected[&k], "{name}: {k}");
            }
        }
    }
}

#[test]
fn macro_text_encoding_matches() {
    let fx = fixture();
    for case in fx["encode"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        // The Python scenarios build the long macro text themselves - use the same one here.
        match (fnc::encode_events(text), case.get("hex")) {
            (Ok(bytes), Some(want)) => assert_eq!(hex(&bytes), want.as_str().unwrap(), "{text}"),
            (Err(e), None) => {
                let want = case["error"].as_str().unwrap();
                // Python int() error texts have no German counterpart
                if !want.starts_with("invalid literal") {
                    assert_eq!(ascii(&e.to_string()), want, "{text}");
                }
            }
            (got, _) => panic!("{text}: Rust {got:?}, Python {case}"),
        }
    }
    assert!(fx["encode"].as_array().unwrap().iter().any(|c| c["text"] == long_macro()));
}

#[test]
fn macro_decoding_matches() {
    for case in fixture()["decode"].as_array().unwrap() {
        let data = h(case["hex"].as_str().unwrap());
        let (mode, events) = fnc::decode_record(&data);
        // The prototype writes umlauts as ae/oe/ue ("drueck"), Rust shows them ("drück").
        let events: Vec<String> = events.iter().map(|e| ascii(e)).collect();
        assert_eq!(serde_json::json!(mode.as_deref().map(ascii)), case["mode"], "{case}");
        assert_eq!(serde_json::json!(events), case["events"], "{case}");
        assert_eq!(fnc::record_length(&data), case["length"].as_u64().unwrap() as usize, "{case}");
    }
}

#[test]
fn function_table_matches() {
    let fx = fixture();
    let table = fx["functions"].as_array().unwrap();
    assert_eq!(table.len(), FUNCTIONS.len());
    for (f, want) in FUNCTIONS.iter().zip(table) {
        assert_eq!(f.key, want["key"].as_str().unwrap());
        assert_eq!(u64::from(f.address), want["address"].as_u64().unwrap(), "{}", f.key);
        assert_eq!(hex(&f.record), want["record"].as_str().unwrap(), "{}", f.key);
    }
}

#[test]
fn file_formats_are_byte_identical() {
    let fx = fixture();
    let dir = std::env::temp_dir().join(format!("m6980x-parity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rom: Vec<u8> = (0..8192usize).map(|i| ((i * 13 + 5) & 0xFF) as u8).collect();
    backup::write_meb(&rom, dir.join("x.meb")).unwrap();
    backup::write_macro_ini(&h("05 85 e9 00 85 00 00 89 00 01 21 41 00"), dir.join("m.ini")).unwrap();
    for name in ["x.meb", "m.ini"] {
        assert_eq!(sha(&std::fs::read(dir.join(name)).unwrap()), fx["files"][name].as_str().unwrap(), "{name}");
    }
}
