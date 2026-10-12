//! Port of prototype/python/tests/test_protocol.py, test_backup.py and test_cli.py (device logic).

use std::time::Duration;

use crate::backup;
use crate::fake::FakeMouse;
use crate::functions::by_key;
use crate::protocol::*;
use crate::transport::{REPORT_LEN, Transport};
use crate::{Error, Result, hex};

fn h(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
}

fn pkt(text: &str) -> [u8; REPORT_LEN] {
    h(text).try_into().unwrap()
}

fn fast<T: Transport>(t: T) -> M6980X<T> {
    let mut g = M6980X::new(t);
    g.poll_interval = Duration::ZERO;
    g
}

fn mouse() -> M6980X<FakeMouse> {
    fast(FakeMouse::new())
}

fn is_invalid<T: std::fmt::Debug>(r: Result<T>) -> bool {
    matches!(r, Err(Error::Invalid(_)))
}

fn is_device<T: std::fmt::Debug>(r: Result<T>) -> bool {
    matches!(r, Err(Error::Device(_)))
}

/// Write commands: registers with bit 0x80 or ROM commands other than read.
fn writes(sent: &[[u8; 8]]) -> Vec<String> {
    sent.iter().filter(|p| p[1] & 0x80 != 0 || p[0] == 0x02 && p[1] != 0).map(|p| hex(p)).collect()
}

#[test]
fn identify_and_name() {
    let mut g = mouse();
    let id = g.identify().unwrap();
    assert!(id.signature_ok);
    assert_eq!(id.model, Some("6980X"));
    assert_eq!(g.name().unwrap(), "G3");
}

#[test]
fn model_like_original() {
    assert_eq!(model_of(&h("0c 12 09 2d 00 03 01")), Some("6980X")); // Tom's mouse
    assert_eq!(model_of(&h("0c 12 07 2d 00 05 01")), Some("M8000X"));
    assert_eq!(model_of(&h("0c 12 09 2d 00 03 00")), Some("6980"));
    assert_eq!(model_of(&h("0c 12 09 2d 00 05 04")), Some("M8600"));
    assert_eq!(model_of(&h("0c 12 09 2d 00 03 02")), None);
    assert_eq!(model_of(&h("00 00 00 00 00 00 01")), None);
}

#[test]
fn check_model_accepts_only_6980x() {
    assert_eq!(mouse().check_model().unwrap().model, Some("6980X"));
    for reply in ["40 0c 12 07 2d 00 05 01", "40 00 00 00 00 00 00 00"] {
        let mut m = FakeMouse::new();
        m.replies.insert(vec![0, 0], pkt(reply));
        assert!(is_device(fast(m).check_model()));
    }
}

#[test]
fn dpi_stage_uses_unit_and_ratio() {
    let s = mouse().dpi_stage(2).unwrap();
    assert_eq!((s.x_step, s.unit, s.x_num, s.x_den), (0x38, 90, 0x41, 0x38));
    assert_eq!(s.x_dpi(), 5850);
}

#[test]
fn supported_steps() {
    assert_eq!(mouse().dpi_supported_steps().unwrap(), (1..57).collect::<Vec<u8>>());
}

#[test]
fn led_and_profile() {
    let mut g = mouse();
    assert_eq!(g.led_color(1).unwrap(), [0, 2, 2]);
    assert_eq!(g.active_profile().unwrap(), 0);
}

#[test]
fn button_uses_device_numbers_and_default() {
    let mut g = mouse();
    assert_eq!(g.button(0, 4).unwrap(), Some(0x1D98));
    assert_eq!(g.t.sent.last().unwrap()[..4], [0x01, 0x05, 0x00, 0x04]);
    assert_eq!(g.button(0, 0).unwrap(), None);
}

#[test]
fn left_click_on_wheel_writes_click_record() {
    let mut g = mouse();
    g.assign_function(2, 8, by_key("linksklick").unwrap()).unwrap();
    assert_eq!(g.button(2, 8).unwrap(), Some(0x1EDC));
    assert_eq!(g.t.rom[0x1EDC..0x1EE0], h("04 21 41 00"));
}

#[test]
fn report_interval() {
    let mut g = mouse();
    g.set_report_interval(3).unwrap();
    assert_eq!(g.t.sent[0], pkt("01 8b 03 00 00 00 00 00"));
    assert_eq!(g.report_interval().unwrap(), 3);
    assert!(is_invalid(g.set_report_interval(9)));
}

#[test]
fn write_rom_block_matches_original_capture() {
    let mut g = mouse();
    g.write_rom(0x1E64, &h("06 83 e0 ff ff 00"), false).unwrap();
    let w: Vec<String> = g.t.sent.iter().filter(|p| p[0] == 0x02 && p[1] != 0x00).map(|p| hex(p)).collect();
    assert_eq!(w, ["02 03 06 83 e0 ff ff 00", "02 04 64 1e 00 06 00 12"]);
    assert_eq!(g.t.rom[0x1E64..0x1E6A], h("06 83 e0 ff ff 00"));
}

#[test]
fn write_rom_splits_at_32_byte_boundaries() {
    let mut g = mouse();
    let data: Vec<u8> = (0..40).collect();
    g.write_rom(0x1D90, &data, false).unwrap(); // 0x1D90-0x1D9F, then 0x1DA0-0x1DB7
    let commits: Vec<(u16, u8)> =
        g.t.sent
            .iter()
            .filter(|p| p[..2] == [0x02, 0x04])
            .map(|c| (u16::from(c[2]) | u16::from(c[3]) << 8, c[5]))
            .collect();
    assert_eq!(commits, [(0x1D90, 16), (0x1DA0, 24)]);
    assert_eq!(g.t.rom[0x1D90..0x1D90 + 40], data);
}

#[test]
fn assign_function_writes_record_then_button() {
    let mut g = mouse();
    g.assign_function(0, 3, by_key("kopieren").unwrap()).unwrap();
    assert_eq!(g.t.rom[0x1DD8..0x1DE2], h("04 83 e0 83 06 84 06 84 e0 00"));
    assert!(g.t.sent.contains(&pkt("01 85 00 03 80 d8 1d 12")));
    assert_eq!(g.button(0, 3).unwrap(), Some(0x1DD8));
}

#[test]
fn speed_function_uses_original_2_byte_write() {
    let mut g = mouse();
    g.assign_function(2, 3, by_key("speed-2x").unwrap()).unwrap();
    // identical to the capture of the original software (profile 3, rear side button)
    assert_eq!(writes(&g.t.sent)[..2], ["02 02 c4 1e 00 c8 00 12", "01 85 02 03 80 c4 1e 12"]);
}

#[test]
fn save_settings_packet() {
    let mut g = mouse();
    g.save_settings().unwrap();
    assert_eq!(g.t.sent[0], pkt("01 02 00 00 00 00 00 12"));
    assert!(g.t.saved);
}

#[test]
fn reset_button() {
    let mut g = mouse();
    g.reset_button(0, 4).unwrap();
    assert!(g.t.sent.contains(&pkt("01 85 00 04 00 ff ff 12")));
    assert_eq!(g.button(0, 4).unwrap(), None);
}

#[test]
fn assign_unknown_function_is_refused() {
    let mut g = mouse();
    assert!(is_invalid(g.assign_function(0, 3, by_key("scroll-links").unwrap())));
    assert!(writes(&g.t.sent).is_empty());
}

#[test]
fn write_macro_like_original() {
    let mut g = mouse();
    let record = h("04 83 17 00"); // "Hallo-Wel" from the capture
    g.write_macro(2, &record, "Hallo-Wel", None).unwrap();
    assert_eq!(g.t.rom[0x02E0..0x02E4], record);
    assert_eq!(&g.t.rom[0x1CD5..0x1CDF], b"Hallo-Wel\x03");
    let bytes: Vec<String> = g.t.sent.iter().filter(|p| p[..2] == [0x02, 0x01]).map(|p| hex(p)).collect();
    assert_eq!(
        bytes[..5],
        [
            "02 01 e0 02 00 04 00 12",
            "02 01 e1 02 00 83 00 12",
            "02 01 e2 02 00 17 00 12",
            "02 01 e3 02 00 00 00 12",
            "02 01 de 1c 00 03 00 12"
        ]
    );
}

#[test]
fn write_macro_keeps_chosen_picture() {
    let mut g = mouse();
    let info9 = 0x1CC1 + 9; // icon byte slot 0
    g.t.rom[info9] = 0x68; // "Klicker" has icon 4 on the real mouse
    g.write_macro(0, &h("06 21 41 00"), "Klicker", None).unwrap();
    assert_eq!(g.t.rom[info9], 0x68);
    assert_eq!(g.macro_icon(0).unwrap(), Some(4));
}

#[test]
fn write_macro_with_picture_and_reset() {
    let mut g = mouse();
    g.write_macro(5, &h("04 83 04 84 04 00"), "Test", Some(21)).unwrap();
    assert_eq!(g.t.rom[0x1CC1 + 5 * 10 + 9], 100 + 21);
    g.set_macro_icon(5, None).unwrap(); // like a new macro: slot + 1
    assert_eq!(g.t.rom[0x1CC1 + 5 * 10 + 9], 6);
    assert_eq!(g.macro_icon(5).unwrap(), None);
    assert!(is_invalid(g.set_macro_icon(5, Some(101))));
}

#[test]
fn clear_macro_like_original_release() {
    let mut g = mouse();
    g.write_macro(2, &h("04 83 17 00"), "Hallo-Wel", None).unwrap();
    g.t.sent.clear();
    g.clear_macro(2).unwrap();
    // capture 4 ("Release"): 02 01 d5 1c 00 00 00 12
    assert!(g.t.sent.contains(&pkt("02 01 d5 1c 00 00 00 12")));
    assert_eq!(g.t.rom[0x1CD5], 0);
    assert_eq!(g.t.rom[0x02E0..0x02E4], h("04 83 17 00")); // data stays
}

#[test]
fn write_macro_rejects_bad_input() {
    let mut g = mouse();
    assert!(is_invalid(g.write_macro(0, &h("04 83 17"), "x", None))); // no end mark
    assert!(is_invalid(g.write_macro(0, &h("04 00"), "zehnzeichn", None))); // name too long
    assert!(is_invalid(g.write_macro(20, &h("04 00"), "x", None)));
    assert!(is_invalid(g.write_macro(0, &h("04 00"), "€", None))); // not Latin-1
    assert!(g.t.sent.is_empty());
}

#[test]
fn rom_read_chunks() {
    let mut g = mouse();
    let data = g.read_rom(0x0100, 10).unwrap();
    assert_eq!(data, (0..10).collect::<Vec<u8>>());
    assert_eq!(g.t.sent[0], pkt("02 00 00 01 00 00 00 12"));
}

#[test]
fn never_sends_write_commands() {
    let mut g = mouse();
    g.identify().unwrap();
    g.name().unwrap();
    g.dpi_stage(2).unwrap();
    g.led_color(1).unwrap();
    g.button(0, 4).unwrap();
    g.read_rom(0, 14).unwrap();
    g.report_interval().unwrap();
    for p in &g.t.sent {
        let (group, cmd) = (p[0], p[1]);
        assert!(!(group == 0x01 && cmd & 0x80 != 0), "{}", hex(p));
        assert!(!(group == 0x02 && cmd != 0x00), "{}", hex(p));
        assert!(!(group == 0x00 && cmd > 0x01), "{}", hex(p));
    }
}

#[test]
fn set_active_profile_sends_original_packet_and_verifies() {
    let mut g = mouse();
    g.set_active_profile(2).unwrap();
    assert_eq!(g.t.sent[0], pkt("01 88 02 00 00 00 00 12"));
    assert_eq!(g.t.sent[1][..2], [0x01, 0x08]);
    assert_eq!(g.active_profile().unwrap(), 2);
}

#[test]
fn set_active_profile_rejects_invalid() {
    let mut g = mouse();
    assert!(is_invalid(g.set_active_profile(3)));
    assert!(g.t.sent.is_empty());
}

#[test]
fn set_active_profile_detects_ignored_switch() {
    let mut m = FakeMouse::new();
    m.accept_profile_switch = false;
    assert!(is_device(fast(m).set_active_profile(1)));
}

#[test]
fn set_led_color_sends_original_packet_and_verifies() {
    let mut g = mouse();
    g.set_led_color(1, [2, 0, 1]).unwrap();
    assert_eq!(g.t.sent[0], pkt("01 86 01 02 00 01 00 00"));
    assert_eq!(g.led_color(1).unwrap(), [2, 0, 1]);
    assert!(!g.t.sent.iter().any(|p| p[..2] == [0x01, 0x88])); // no profile switch
}

#[test]
fn set_led_color_rejects_levels_above_2() {
    let mut g = mouse();
    for bad in [[3, 0, 0], [0, 255, 0]] {
        assert!(is_invalid(g.set_led_color(0, bad)));
    }
    assert!(is_invalid(g.set_led_color(3, [0, 0, 0])));
    assert!(g.t.sent.is_empty());
}

#[test]
fn set_led_color_detects_ignored_write() {
    let mut g = mouse();
    g.t.accept_led = false;
    assert!(is_device(g.set_led_color(0, [0, 0, 2])));
}

#[test]
fn dpi_plan_matches_original_capture() {
    let steps: Vec<u8> = (1..57).collect();
    assert_eq!(dpi_write_plan(65, 65, &steps).unwrap(), [(0, 56, 65, 56), (1, 56, 65, 56)]);
    assert_eq!(dpi_write_plan(67, 67, &steps).unwrap(), [(0, 56, 67, 56), (1, 56, 67, 56)]);
    assert_eq!(dpi_write_plan(18, 42, &steps).unwrap(), [(0, 18, 1, 1), (1, 42, 1, 1)]);
    // unsupported step: next higher one, fraction reduced (30/32 -> 15/16)
    assert_eq!(dpi_write_plan(30, 30, &[16, 32]).unwrap()[0], (0, 32, 15, 16));
}

#[test]
fn dpi_rounding_like_python() {
    assert_eq!(dpi_to_steps(4510, 90).unwrap(), 50);
    assert_eq!(dpi_to_steps(3000, 90).unwrap(), 33);
    assert_eq!(dpi_to_steps(4545, 90).unwrap(), 50); // 50.5 -> even
    assert_eq!(dpi_to_steps(4635, 90).unwrap(), 52); // 51.5 -> even
    assert!(dpi_to_steps(45, 90).is_err()); // 0.5 -> 0
}

#[test]
fn set_dpi_sends_original_packets_and_verifies() {
    let mut g = mouse();
    let stage = g.set_dpi(2, 5850, None).unwrap();
    let sets: Vec<String> = g.t.sent.iter().filter(|p| p[1] & 0x80 != 0).map(|p| hex(p)).collect();
    assert_eq!(
        sets,
        ["01 83 00 02 38 00 00 12", "01 83 01 02 38 00 00 12", "01 84 00 02 41 38 00 12", "01 84 01 02 41 38 00 12"]
    );
    assert_eq!((stage.x_dpi(), stage.y_dpi()), (5850, 5850));
}

#[test]
fn set_dpi_separate_axes_and_rounding() {
    let mut g = mouse();
    let stage = g.set_dpi(3, 4510, Some(3000)).unwrap(); // rounded to 50 / 33 steps of 90
    assert_eq!((stage.x_dpi(), stage.y_dpi()), (4500, 2970));
    assert_eq!(g.dpi_stage(0).unwrap().x_dpi(), 1620); // other stages untouched
}

#[test]
fn set_dpi_rejects_out_of_range() {
    let mut g = mouse();
    for bad in [0, 30, 6030 + 90, 10000, -90] {
        assert!(is_invalid(g.set_dpi(0, bad, None)), "{bad}");
    }
    assert!(is_invalid(g.set_dpi(4, 1600, None)));
    assert!(!g.t.sent.iter().any(|p| p[1] & 0x80 != 0));
}

#[test]
fn set_active_dpi_stage() {
    let mut g = mouse();
    g.set_active_dpi_stage(2).unwrap();
    assert_eq!(g.t.sent[0], pkt("01 87 02 00 00 00 00 12"));
    assert_eq!(g.active_dpi_stage().unwrap(), 2);
    assert!(is_invalid(g.set_active_dpi_stage(4)));
}

#[test]
fn get_register_refuses_set_bit() {
    assert!(is_invalid(mouse().get_register(0x86, &[0])));
}

struct FlakyMouse {
    inner: FakeMouse,
    reads: u32,
}

impl Transport for FlakyMouse {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        self.inner.set_feature(data)
    }
    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        self.reads += 1;
        if self.reads % 2 == 1 {
            // every second read fails like the real short report
            return Err(Error::Transport("unexpected feature report length 1: 00".into()));
        }
        self.inner.get_feature()
    }
}

#[test]
fn failed_reads_are_retried_like_busy() {
    let mut g = fast(FlakyMouse { inner: FakeMouse::new(), reads: 0 });
    assert_eq!(g.name().unwrap(), "G3");
    g.set_active_profile(2).unwrap();
    assert_eq!(g.active_profile().unwrap(), 2);
}

#[test]
fn timeout_when_never_done() {
    let mut g = mouse();
    g.t.busy_polls = 10_000;
    g.busy_tries = 3;
    g.done_tries = 3;
    assert!(is_device(g.name()));
}

/// Counts lock/unlock so that every command is locked and released exactly once.
struct LockCounting {
    inner: FakeMouse,
    locks: u32,
    unlocks: u32,
}

impl Transport for LockCounting {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        self.inner.set_feature(data)
    }
    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        self.inner.get_feature()
    }
    fn lock(&mut self) -> Result<()> {
        self.locks += 1;
        Ok(())
    }
    fn unlock(&mut self) {
        self.unlocks += 1;
    }
}

#[test]
fn every_command_runs_under_the_lock() {
    let mut g = fast(LockCounting { inner: FakeMouse::new(), locks: 0, unlocks: 0 });
    g.set_active_profile(1).unwrap();
    g.t.inner.busy_polls = 10_000;
    g.busy_tries = 2;
    assert!(g.name().is_err()); // released even on error
    assert_eq!(g.t.locks, 3);
    assert_eq!(g.t.unlocks, 3);
}

// -- factory values (test_cli.py) ---------------------------------------------------------

#[test]
fn dpi_and_500_hz_like_original_default_button() {
    let mut g = mouse();
    g.t.interval = 8;
    let stages = g.apply_factory_defaults().unwrap();
    assert_eq!(stages.iter().map(|s| s.x_dpi()).collect::<Vec<_>>(), FACTORY_DPI);
    assert_eq!(g.t.interval, 2);
    assert!(g.t.sent.iter().any(|p| p[..3] == [0x01, 0x8B, 0x02]));
}

// -- backup (test_backup.py) --------------------------------------------------------------

fn quiet() -> impl FnMut(&str) {
    |_| {}
}

#[test]
fn round_trip_restores_everything() {
    let mut g = mouse();
    let snap = backup::create(&mut g, &mut |_, _| {}).unwrap();
    let rom_before = g.t.rom.clone();

    // change a bit of everything, like a factory reset or experiments do
    g.write_macro(3, &h("04 83 04 84 04 00"), "Test", Some(7)).unwrap();
    g.assign_function(1, 3, by_key("kopieren").unwrap()).unwrap();
    g.reset_button(0, 4).unwrap();
    g.set_led_color(2, [0, 0, 0]).unwrap();
    g.set_dpi(0, 900, None).unwrap();
    g.set_report_interval(8).unwrap();
    g.set_active_dpi_stage(2).unwrap();
    g.set_active_profile(2).unwrap();

    let left = backup::restore(&mut g, &snap, &mut quiet()).unwrap();
    assert!(left.is_empty());
    assert_eq!(g.t.rom, rom_before);
    let mut again = backup::create(&mut g, &mut |_, _| {}).unwrap();
    again.created = snap.created.clone();
    assert_eq!(again, snap);
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("m6980x-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn meb_round_trip_and_import() {
    let rom: Vec<u8> = (0..8192usize).map(|i| ((i * 7) & 0xFF) as u8).collect();
    let d = temp_dir("meb");
    let path = d.join("x.meb");
    backup::write_meb(&rom, &path).unwrap();
    let raw = std::fs::read(&path).unwrap();
    assert!(raw.starts_with(b"[Mem]\r\ncount=8192\r\n0=0\r\n1=7\r\n"));
    assert_eq!(backup::read_meb(&path).unwrap(), rom);
    std::fs::write(&path, "[Mem]\ncount=8192\n0=1\n").unwrap();
    assert!(is_invalid(backup::read_meb(&path)));

    let mut g = mouse();
    let mut wanted = g.t.rom.clone();
    wanted[0x0100..0x0104].copy_from_slice(&[0x04, 0x83, 0x05, 0x00]);
    wanted[backup::TABLE_START] = 0x12;
    let left = backup::import_rom(&mut g, &wanted, &mut quiet()).unwrap();
    assert!(left.is_empty());
    assert_eq!(g.t.rom, wanted);
}

#[test]
fn meb_import_applies_button_assignments() {
    // like restoring in the original: flags 0x1F01 + p*12 + b decide, the table entry gives the address
    let mut g = mouse();
    let mut wanted = g.t.rom.clone();
    wanted[0x1F01..0x1F25].fill(0); // all default ...
    wanted[0x1F01 + 2 * 12 + 3] = 0x80; // ... except profile 3, rear button
    let a = 0x1F3D + 2 * 0x18 + 3 * 2;
    wanted[a..a + 2].copy_from_slice(&0x1D98u16.to_le_bytes()); // -> middle click
    let stale = 0x1F3D + 4 * 2;
    wanted[stale..stale + 2].copy_from_slice(&0u16.to_le_bytes()); // old entry, flag 00 -> stays default
    assert_eq!(g.button(0, 4).unwrap(), Some(0x1D98)); // FakeMouse starts with this assigned
    backup::import_rom(&mut g, &wanted, &mut quiet()).unwrap();
    assert_eq!(g.button(2, 3).unwrap(), Some(0x1D98));
    assert_eq!(g.button(0, 4).unwrap(), None);
}

#[test]
fn save_load_and_reject_foreign_files() {
    let mut g = mouse();
    let snap = backup::create(&mut g, &mut |_, _| {}).unwrap();
    let d = temp_dir("json");
    let path = d.join("b.json");
    backup::save(&snap, &path).unwrap();
    assert_eq!(backup::load(&path).unwrap(), snap);
    std::fs::write(&path, r#"{"format": "etwas anderes"}"#).unwrap();
    assert!(is_invalid(backup::load(&path)));
}

#[test]
fn macro_ini_writes_original_layout() {
    let path = temp_dir("ini1").join("XY.ini");
    backup::write_macro_ini(&h("04 83 04 88 64 84 04 00"), &path).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"[Macro]\r\ncount=8\r\n0=4\r\n1=131\r\n2=4\r\n3=136\r\n4=100\r\n5=132\r\n6=4\r\n7=0\r\n"
    );
}

#[test]
fn macro_ini_round_trip_keeps_zero_inside_events() {
    let path = temp_dir("ini2").join("XY.ini");
    let record = h("05 85 e9 00 85 00 00 89 00 01 21 41 00"); // 00 in consumer/wait arguments
    backup::write_macro_ini(&record, &path).unwrap();
    assert_eq!(backup::read_macro_ini(&path, 368).unwrap(), record);
}

#[test]
fn macro_ini_reads_like_original() {
    let path = temp_dir("ini3").join("XY.ini");
    // keys case-insensitive, bytes after 00 ignored, end mark not doubled
    std::fs::write(&path, "[MACRO]\nCount=5\n0=6\n1=33\n2=65\n3=0\n4=255\n").unwrap();
    assert_eq!(backup::read_macro_ini(&path, 368).unwrap(), h("06 21 41 00"));
    std::fs::write(&path, "[Macro]\ncount=3\n0=4\n1=97\n2=127\n[SpecialMacro]\nindex=2\n").unwrap();
    assert_eq!(backup::read_macro_ini(&path, 368).unwrap(), h("04 00")); // the original keeps only the mode
}

#[test]
fn macro_ini_rejects_empty_and_foreign_files() {
    let path = temp_dir("ini4").join("XY.ini");
    assert!(is_invalid(backup::write_macro_ini(&h("04 00"), &path)));
    for content in ["[Mem]\ncount=8192\n", "[Macro]\ncount=3\n0=9\n1=0\n2=0\n", "[Macro]\ncount=999\n"] {
        std::fs::write(&path, content).unwrap();
        assert!(is_invalid(backup::read_macro_ini(&path, 368)), "{content}");
    }
}

/// Read existing backups of the prototype: `M6980X_BACKUP_DIR=<folder> cargo test -- --ignored`
#[test]
#[ignore]
fn loads_existing_backups() {
    let dir = std::env::var("M6980X_BACKUP_DIR").expect("set M6980X_BACKUP_DIR");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            // only backups of this tool (not e.g. the info output einstellungen_vor_originalsoftware.json)
            if !std::fs::read_to_string(&path).unwrap().contains("\"format\"") {
                continue;
            }
            let data = backup::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(data.rom_bytes().unwrap().len(), ROM_SIZE);
            n += 1;
        } else if path.extension().is_some_and(|e| e == "bin") {
            assert_eq!(backup::load_rom_file(&path).unwrap().len(), ROM_SIZE);
        }
    }
    assert!(n > 0);
}
