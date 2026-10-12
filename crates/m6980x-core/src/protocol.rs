//! Command layer of the GHOST M6980X (read commands plus write commands confirmed one by one on the device).
//!
//! Packet on the wire (8 bytes, no report ID): `[group, command, arg0..arg5]`.
//! Handshake from the original Macro.exe (0x444aac):
//!   1. GET_FEATURE until status bit 0x80 (busy) is clear
//!   2. SET_FEATURE with the command
//!   3. GET_FEATURE until status bit 0x40 (done) is set; reply = `[status, data0..data6]`
//!
//! Reply bytes a command does not fill keep stale values - only evaluate documented bytes.
//! Evidence for every command: docs/PROTOCOL.md.

use std::thread::sleep;
use std::time::Duration;

use crate::functions::{self as fnc, Function};
use crate::rom as romlayout;
use crate::transport::{REPORT_LEN, Transport};
use crate::{Error, Result, hex, invalid, tr};

pub const STATUS_BUSY: u8 = 0x80;
pub const STATUS_DONE: u8 = 0x40;

pub const GROUP_SYSTEM: u8 = 0x00;
pub const GROUP_REGISTER: u8 = 0x01;
pub const GROUP_ROM: u8 = 0x02;

/// 8 KB, mirrored above (checked)
pub const ROM_SIZE: usize = 0x2000;
pub const ROM_READ_CHUNK: usize = 7;
/// buffer bytes 0-5, 6-11, ..., 30-31
pub const ROM_FILL_CMDS: [u8; 6] = [0x03, 0x05, 0x06, 0x07, 0x08, 0x09];
pub const PROFILES: usize = 3;
pub const BUTTONS: usize = 10;
/// entries in registers 03/04
pub const DPI_STAGES: usize = 4;
/// The DPI button cycles 0 -> 1 -> 2 (observed); the LEDs show stage + 1.
pub const DPI_BUTTON_STAGES: usize = 3;
/// Mappingdefault.ini [6980DPIdefault] max
pub const MAX_DPI: u32 = 6030;
/// per channel: off, half, full
pub const LED_LEVELS: [u8; 3] = [0, 1, 2];
/// constant last byte in the ROM and in the original's set commands
pub const ROM_TRAILER: u8 = 0x12;
/// Macro.exe waits 10 ms after a set command.
pub const WRITE_DELAY: Duration = Duration::from_millis(10);
pub const SUPPORTED_MODEL: &str = "6980X";

/// Factory values of the three stages ([6980DPIdefault]; the 4th INI value belongs to an unused stage)
pub const FACTORY_DPI: [u32; 3] = [810, 1620, 3240];
/// 500 Hz: the original's default button (0x44abbc) also writes 01 8B 02
pub const FACTORY_INTERVAL_MS: u8 = 2;

pub type Reply = [u8; REPORT_LEN];

fn device<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Device(msg.into()))
}

/// Model name as in the original (0x42ebd8) from the identify reply data (0C 12 ...):
/// byte 6 = 0 -> 6980, 1 and byte 2 = 9 -> 6980X, 1 otherwise -> M8000X, 4 -> M8600, otherwise unknown (None).
pub fn model_of(data: &[u8]) -> Option<&'static str> {
    if data.len() < 7 || data[0..2] != [0x0C, 0x12] {
        return None;
    }
    match data[6] {
        0 => Some("6980"),
        1 if data[2] == 9 => Some("6980X"),
        1 => Some("M8000X"),
        4 => Some("M8600"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Identify {
    pub signature_ok: bool,
    pub raw: String,
    pub model: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiStage {
    pub x_step: u8,
    pub y_step: u8,
    pub unit: u8,
    pub x_num: u8,
    pub x_den: u8,
    pub y_num: u8,
    pub y_den: u8,
}

impl DpiStage {
    pub fn x_dpi(&self) -> u32 {
        u32::from(self.x_step) * u32::from(self.unit) * u32::from(self.x_num) / u32::from(self.x_den)
    }
    pub fn y_dpi(&self) -> u32 {
        u32::from(self.y_step) * u32::from(self.unit) * u32::from(self.y_num) / u32::from(self.y_den)
    }
}

/// Like Python `round(dpi / unit)` (exactly .5 goes to the even number), with range check.
pub fn dpi_to_steps(dpi: i64, unit: u8) -> Result<u8> {
    let unit = i64::from(unit);
    let steps = if dpi < 0 {
        -1
    } else {
        let (q, r) = (dpi / unit, dpi % unit);
        if 2 * r > unit || (2 * r == unit && q % 2 == 1) { q + 1 } else { q }
    };
    if steps < 1 || steps * unit > i64::from(MAX_DPI) || steps > 0xFF {
        return invalid(tr!("error.dpi_range", unit = unit, max_dpi = MAX_DPI));
    }
    Ok(steps as u8)
}

/// 0x445900: smallest supported sensor step >= value, otherwise the largest supported one.
pub fn snap_step(value: u8, supported: &[u8]) -> Option<u8> {
    supported.iter().copied().filter(|&s| s >= value).min().or_else(|| supported.iter().copied().max())
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Port of 0x445900: every axis gets a supported sensor step plus the fraction
/// wanted/sensor (reduced like 0x445640) that the firmware scales with.
/// Returns [(axis, sensor step, numerator, denominator)] for X (0) and Y (1).
pub fn dpi_write_plan(x_steps: u8, y_steps: u8, supported: &[u8]) -> Result<Vec<(u8, u8, u8, u8)>> {
    let mut plan = Vec::new();
    for (axis, wanted) in [(0u8, x_steps), (1, y_steps)] {
        let Some(step) = snap_step(wanted, supported) else {
            return device(tr!("error.no_dpi_steps"));
        };
        let g = gcd(u32::from(wanted), u32::from(step));
        plan.push((axis, step, (u32::from(wanted) / g) as u8, (u32::from(step) / g) as u8));
    }
    Ok(plan)
}

pub struct M6980X<T: Transport = Box<dyn Transport>> {
    pub t: T,
    pub poll_interval: Duration,
    pub busy_tries: u32,
    pub done_tries: u32,
}

impl<T: Transport> M6980X<T> {
    pub fn new(t: T) -> Self {
        Self { t, poll_interval: Duration::from_millis(4), busy_tries: 20, done_tries: 200 }
    }

    // -- lowest level ------------------------------------------------------------------------
    /// One command with handshake. `trailer` goes into byte 7, `delay` is the wait after sending.
    pub fn xfer(&mut self, group: u8, cmd: u8, args: &[u8], trailer: Option<u8>, delay: Duration) -> Result<Reply> {
        if args.len() > 6 {
            return invalid(tr!("error.too_many_args"));
        }
        let mut pkt = [0u8; REPORT_LEN];
        pkt[0] = group;
        pkt[1] = cmd;
        pkt[2..2 + args.len()].copy_from_slice(args);
        if let Some(t) = trailer {
            pkt[7] = t;
        }
        self.t.lock()?;
        let result = self.handshake(&pkt, delay);
        self.t.unlock();
        result
    }

    fn handshake(&mut self, pkt: &[u8; REPORT_LEN], delay: Duration) -> Result<Reply> {
        let mut ready = false;
        for _ in 0..self.busy_tries {
            if self.poll().is_some_and(|s| s[0] & STATUS_BUSY == 0) {
                ready = true;
                break;
            }
            self.pause(self.poll_interval);
        }
        if !ready {
            return device(tr!("error.always_busy"));
        }

        self.t.set_feature(pkt)?;
        self.pause(delay);

        for _ in 0..self.done_tries {
            if let Some(reply) = self.poll().filter(|r| r[0] & STATUS_DONE != 0) {
                return Ok(reply);
            }
            self.pause(self.poll_interval);
        }
        device(tr!("error.no_reply", packet = hex(pkt)))
    }

    fn pause(&self, d: Duration) {
        if !d.is_zero() {
            sleep(d);
        }
    }

    /// The original treats a failed GET_FEATURE as "not ready" and retries; the mouse returns short
    /// reports e.g. while the DPI/profile button is pressed.
    fn poll(&mut self) -> Option<Reply> {
        self.t.get_feature().ok()
    }

    pub fn get_register(&mut self, reg: u8, args: &[u8]) -> Result<Reply> {
        if reg & 0x80 != 0 {
            return invalid(tr!("error.read_with_write_bit"));
        }
        self.xfer(GROUP_REGISTER, reg, args, None, Duration::ZERO)
    }

    /// The original's set commands wait 10 ms; most of them end with 0x12.
    fn set_register(&mut self, reg: u8, args: &[u8], trailer: Option<u8>) -> Result<Reply> {
        self.xfer(GROUP_REGISTER, reg | 0x80, args, trailer, WRITE_DELAY)
    }

    // -- system ------------------------------------------------------------------------------
    /// The original only accepts the device if reply bytes 1-2 = 0C 12 (0x44407c).
    pub fn identify(&mut self) -> Result<Identify> {
        let r = self.xfer(GROUP_SYSTEM, 0x00, &[], None, Duration::ZERO)?;
        Ok(Identify { signature_ok: r[1..3] == [0x0C, 0x12], raw: hex(&r[1..]), model: model_of(&r[1..]) })
    }

    /// Only allow the 6980X: the other GHOST models have a different memory layout
    /// (M8000X/M8600: 32 KB, 5 profiles, 70 macros; 6980: different DPI range), our addresses would be wrong.
    pub fn check_model(&mut self) -> Result<Identify> {
        let ident = self.identify()?;
        if !ident.signature_ok {
            return device(tr!("error.not_a_ghost_mouse", raw = ident.raw));
        }
        if ident.model != Some(SUPPORTED_MODEL) {
            return device(tr!(
                "error.model_not_supported",
                model = ident.model.map_or_else(|| tr!("common.unknown"), str::to_string),
                supported_model = SUPPORTED_MODEL,
                raw = ident.raw
            ));
        }
        Ok(ident)
    }

    pub fn name(&mut self) -> Result<String> {
        let r = self.xfer(GROUP_SYSTEM, 0x01, &[], None, Duration::ZERO)?;
        let raw = r[1..7].split(|&c| c == 0).next().unwrap_or_default();
        Ok(raw.iter().map(|&c| if c.is_ascii() { c as char } else { '\u{FFFD}' }).collect())
    }

    // -- registers ---------------------------------------------------------------------------
    pub fn active_profile(&mut self) -> Result<u8> {
        Ok(self.get_register(0x08, &[])?[1])
    }

    /// Switch the active hardware profile (0-2) and verify by reading back.
    pub fn set_active_profile(&mut self, profile: u8) -> Result<()> {
        if usize::from(profile) >= PROFILES {
            return invalid(tr!("error.profile_range", profiles = PROFILES - 1));
        }
        self.set_register(0x08, &[profile], Some(ROM_TRAILER))?;
        let actual = self.active_profile()?;
        if actual != profile {
            return device(tr!("error.profile_not_applied", actual = actual));
        }
        Ok(())
    }

    /// Register 07: active DPI stage, 0-based (1 = middle, 2 of 3 LEDs lit - seen on the device).
    pub fn active_dpi_stage(&mut self) -> Result<u8> {
        Ok(self.get_register(0x07, &[])?[1])
    }

    pub fn register_0a(&mut self) -> Result<u16> {
        let r = self.get_register(0x0A, &[])?;
        Ok(u16::from(r[1]) | u16::from(r[2]) << 8)
    }

    /// Register 02 (01 02 00 .. 12, 4 ms), sent by the original when leaving the settings page.
    /// Without it, report rate and DPI are lost when unplugged (tested); LED colours stay anyway.
    pub fn save_settings(&mut self) -> Result<()> {
        self.xfer(GROUP_REGISTER, 0x02, &[0], Some(ROM_TRAILER), Duration::from_millis(4)).map(|_| ())
    }

    /// Register 0B: USB polling interval in ms (1-8); report rate = 1000 / value Hz.
    pub fn report_interval(&mut self) -> Result<u8> {
        Ok(self.get_register(0x0B, &[])?[1])
    }

    /// Like the original (capture: 500 -> 333 Hz = 01 8B 02 -> 01 8B 03).
    pub fn set_report_interval(&mut self, ms: u8) -> Result<()> {
        if !(1..=8).contains(&ms) {
            return invalid(tr!("error.interval_range"));
        }
        self.set_register(0x0B, &[ms], None)?;
        let actual = self.report_interval()?;
        if actual != ms {
            return device(tr!("error.rate_not_applied", actual = actual));
        }
        Ok(())
    }

    pub fn led_color(&mut self, profile: u8) -> Result<[u8; 3]> {
        let r = self.get_register(0x06, &[profile])?;
        Ok([r[1], r[2], r[3]])
    }

    /// Set the LED colour of a profile. Every channel has three levels: 0 off, 1 half, 2 full.
    /// Like the original (0x445ba8), without switching the active profile.
    pub fn set_led_color(&mut self, profile: u8, rgb: [u8; 3]) -> Result<()> {
        if usize::from(profile) >= PROFILES {
            return invalid(tr!("error.profile_range", profiles = PROFILES - 1));
        }
        if rgb.iter().any(|c| !LED_LEVELS.contains(c)) {
            return invalid(tr!("error.led_levels"));
        }
        self.set_register(0x06, &[profile, rgb[0], rgb[1], rgb[2]], None)?;
        let actual = self.led_color(profile)?;
        if actual != rgb {
            return device(tr!("error.led_not_applied", actual = format!("{actual:?}")));
        }
        Ok(())
    }

    pub fn dpi_supported_steps(&mut self) -> Result<Vec<u8>> {
        let r = self.get_register(0x03, &[1])?;
        Ok((0..7u8)
            .flat_map(|byte| {
                (0..8u8).filter(move |bit| r[1 + byte as usize] >> bit & 1 != 0).map(move |bit| byte * 8 + bit + 1)
            })
            .collect())
    }

    pub fn dpi_stage(&mut self, stage: u8) -> Result<DpiStage> {
        let a = self.get_register(0x03, &[0, stage])?;
        let b = self.get_register(0x04, &[0, stage])?;
        let or = |v: u8, d: u8| if v == 0 { d } else { v };
        // like the original (0x4457fc): only denominators 0 are replaced by 1
        Ok(DpiStage {
            x_step: a[1],
            y_step: or(a[2], a[1]),
            unit: or(a[3], 100),
            x_num: b[1],
            x_den: or(b[2], 1),
            y_num: b[3],
            y_den: or(b[4], 1),
        })
    }

    /// Select the active DPI stage like the original (0x445280). Software can also select stage 3,
    /// which the DPI button never reaches.
    pub fn set_active_dpi_stage(&mut self, stage: u8) -> Result<()> {
        if usize::from(stage) >= DPI_STAGES {
            return invalid(tr!("error.dpi_stage_range", dpi_stages = DPI_STAGES - 1));
        }
        self.set_register(0x07, &[stage], Some(ROM_TRAILER))?;
        let actual = self.active_dpi_stage()?;
        if actual != stage {
            return device(tr!("error.dpi_stage_not_applied", actual = actual));
        }
        Ok(())
    }

    /// Set the DPI of a stage, rounded to the device unit (90). Returns the stage as read back.
    pub fn set_dpi(&mut self, stage: u8, x_dpi: i64, y_dpi: Option<i64>) -> Result<DpiStage> {
        if usize::from(stage) >= DPI_STAGES {
            return invalid(tr!("error.dpi_stage_range", dpi_stages = DPI_STAGES - 1));
        }
        let unit = self.dpi_stage(stage)?.unit;
        let x = dpi_to_steps(x_dpi, unit)?;
        let y = match y_dpi {
            Some(y) => dpi_to_steps(y, unit)?,
            None => x,
        };
        let supported = self.dpi_supported_steps()?;
        let plan = dpi_write_plan(x, y, &supported)?;
        // same order as the original: sensor steps X, Y, then fractions X, Y
        for &(axis, step, _, _) in &plan {
            self.set_register(0x03, &[axis, stage, step], Some(ROM_TRAILER))?;
        }
        for &(axis, _, num, den) in &plan {
            self.set_register(0x04, &[axis, stage, num, den], Some(ROM_TRAILER))?;
        }
        let actual = self.dpi_stage(stage)?;
        let unit = u32::from(unit);
        if (actual.x_dpi(), actual.y_dpi()) != (u32::from(x) * unit, u32::from(y) * unit) {
            return device(tr!("error.dpi_not_applied", x_dpi = actual.x_dpi(), y_dpi = actual.y_dpi()));
        }
        Ok(actual)
    }

    /// Like the original's default button: DPI from [6980DPIdefault], then report rate 500 Hz.
    /// Returns the new DPI stages.
    pub fn apply_factory_defaults(&mut self) -> Result<Vec<DpiStage>> {
        let stages = FACTORY_DPI
            .iter()
            .enumerate()
            .map(|(stage, &dpi)| self.set_dpi(stage as u8, i64::from(dpi), None))
            .collect::<Result<Vec<_>>>()?;
        self.set_report_interval(FACTORY_INTERVAL_MS)?;
        Ok(stages)
    }

    /// ROM address of the record of a device button, or None for its built-in function.
    /// Device numbers (found on the mouse): 0 left, 1 right, 2 middle, 3 back, 4 forward,
    /// 5 no button, 6 tilt left, 7 tilt right, 8 wheel up, 9 wheel down.
    pub fn button(&mut self, profile: u8, button: u8) -> Result<Option<u16>> {
        let r = self.get_register(0x05, &[profile, button])?;
        if r[1] & 0x80 == 0 {
            return Ok(None);
        }
        Ok(Some(u16::from(r[2]) | u16::from(r[3]) << 8))
    }

    fn check_button(profile: u8, button: u8) -> Result<()> {
        if usize::from(profile) >= PROFILES || usize::from(button) >= BUTTONS {
            return invalid(tr!("error.button_range"));
        }
        Ok(())
    }

    /// Put a button on a function record or macro slot like the original
    /// (01 85 <profile> <button> 80 <lo> <hi> 12, then re-activate the active profile).
    pub fn set_button(&mut self, profile: u8, button: u8, address: u16) -> Result<()> {
        Self::check_button(profile, button)?;
        if usize::from(address) >= ROM_SIZE {
            return invalid(tr!("error.address_outside_rom"));
        }
        let active = self.active_profile()?;
        let [lo, hi] = address.to_le_bytes();
        self.set_register(0x05, &[profile, button, 0x80, lo, hi], Some(ROM_TRAILER))?;
        self.set_active_profile(active)?;
        let actual = self.button(profile, button)?;
        if actual != Some(address) {
            return device(tr!("error.button_not_applied", actual = format!("{actual:?}")));
        }
        Ok(())
    }

    // -- ROM ---------------------------------------------------------------------------------
    pub fn read_rom(&mut self, addr: u16, length: usize) -> Result<Vec<u8>> {
        self.read_rom_progress(addr, length, &mut |_, _| {})
    }

    pub fn read_rom_progress(
        &mut self,
        addr: u16,
        length: usize,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(length + ROM_READ_CHUNK);
        while out.len() < length {
            let a = usize::from(addr) + out.len();
            let (lo, hi) = (a as u8, (a >> 8) as u8);
            let r = self.xfer(GROUP_ROM, 0x00, &[lo, hi, 0, 0, 0], Some(ROM_TRAILER), Duration::ZERO)?;
            out.extend_from_slice(&r[1..1 + ROM_READ_CHUNK]);
            progress(out.len().min(length), length);
        }
        out.truncate(length);
        Ok(out)
    }

    /// 02 01 <lo> <hi> 00 <value> 00 12 - used by the original's macro editor (1 ms).
    pub fn write_rom_byte(&mut self, addr: u16, value: u8) -> Result<()> {
        let [lo, hi] = addr.to_le_bytes();
        self.xfer(GROUP_ROM, 0x01, &[lo, hi, 0, value, 0], Some(ROM_TRAILER), Duration::from_millis(1)).map(|_| ())
    }

    /// 02 02 <lo> <hi> 00 <b0> <b1> 12 - 2-byte write of the original (0x444ba8, 1 ms).
    pub fn write_rom_word(&mut self, addr: u16, data: [u8; 2]) -> Result<()> {
        let [lo, hi] = addr.to_le_bytes();
        self.xfer(GROUP_ROM, 0x02, &[lo, hi, 0, data[0], data[1]], Some(ROM_TRAILER), Duration::from_millis(1))
            .map(|_| ())
    }

    /// Up to 32 bytes via the transfer buffer like the original (0x444c30): fill commands
    /// 03/05/06/07/08/09 with 6 bytes each, then 02 04 <lo> <hi> 00 <length> 00 12.
    pub fn write_rom_block(&mut self, addr: u16, data: &[u8]) -> Result<()> {
        if !(1..=32).contains(&data.len()) {
            return invalid(tr!("error.block_size"));
        }
        for (cmd, chunk) in ROM_FILL_CMDS.iter().zip(data.chunks(6)) {
            self.xfer(GROUP_ROM, *cmd, chunk, None, Duration::from_millis(1))?;
        }
        // the wait grows with the length exactly like [ebp-4] in the original
        let delay_ms = match data.len() {
            0..=12 => 1,
            13..=18 => 3,
            19..=24 => 6,
            25..=30 => 9,
            _ => 10,
        };
        let [lo, hi] = addr.to_le_bytes();
        self.xfer(
            GROUP_ROM,
            0x04,
            &[lo, hi, 0, data.len() as u8, 0],
            Some(ROM_TRAILER),
            Duration::from_millis(delay_ms),
        )
        .map(|_| ())
    }

    /// Write and verify. Blocks never cross a 32-byte boundary.
    pub fn write_rom(&mut self, addr: u16, data: &[u8], bytewise: bool) -> Result<()> {
        self.write_rom_progress(addr, data, bytewise, &mut |_, _| {})
    }

    /// `write_rom` with progress (done, total): writing counts the bytes, the verify read follows,
    /// total = 2 × length.
    pub fn write_rom_progress(
        &mut self,
        addr: u16,
        data: &[u8],
        bytewise: bool,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<()> {
        let total = 2 * data.len();
        if usize::from(addr) + data.len() > ROM_SIZE {
            return invalid(tr!("error.write_outside_rom"));
        }
        if bytewise {
            for (i, &b) in data.iter().enumerate() {
                self.write_rom_byte(addr + i as u16, b)?;
                progress(i + 1, total);
            }
        } else {
            let mut pos = 0;
            while pos < data.len() {
                let a = addr + pos as u16;
                let n = (32 - usize::from(a) % 32).min(data.len() - pos);
                match n {
                    1 => self.write_rom_byte(a, data[pos])?,
                    2 => self.write_rom_word(a, [data[pos], data[pos + 1]])?,
                    _ => self.write_rom_block(a, &data[pos..pos + n])?,
                }
                pos += n;
                progress(pos, total);
            }
        }
        let n = data.len();
        let actual = self.read_rom_progress(addr, n, &mut |done, _| progress(n + done, total))?;
        if actual != data {
            return device(tr!("error.rom_not_written", addr = format!("{addr:04X}"), actual = hex(&actual)));
        }
        Ok(())
    }

    // -- upper level: functions and macros (formats in docs/PROTOCOL.md) ----------------------
    /// Back to the built-in function of the button. The original has no option for this; it writes
    /// the state the mouse reports for untouched buttons (flags 00, address FFFF).
    pub fn reset_button(&mut self, profile: u8, button: u8) -> Result<()> {
        Self::check_button(profile, button)?;
        let active = self.active_profile()?;
        self.set_register(0x05, &[profile, button, 0x00, 0xFF, 0xFF], Some(ROM_TRAILER))?;
        self.set_active_profile(active)?;
        if let Some(actual) = self.button(profile, button)? {
            return device(tr!("error.button_not_reset", actual = format!("{actual:04X}")));
        }
        Ok(())
    }

    /// Like the original: (re)write the function record at its fixed address, then put the button on it.
    /// Mouse button functions on the wheel buttons get the original's click record (functions::placement).
    pub fn assign_function(&mut self, profile: u8, button: u8, function: &Function) -> Result<()> {
        if function.size == 0 {
            return invalid(tr!("error.function_without_record", name = function.name()));
        }
        if function.record.is_empty() {
            return invalid(tr!("error.function_unknown_content", name = function.name()));
        }
        let (address, record) = fnc::placement(function, usize::from(button));
        self.write_rom(address, &record, false)?;
        self.set_button(profile, button, address)
    }

    pub fn assign_macro(&mut self, profile: u8, button: u8, slot: usize) -> Result<()> {
        Self::check_slot(slot)?;
        self.set_button(profile, button, romlayout::macro_slot_addr(slot))
    }

    /// Write a macro (mode byte + events + 00) into a slot, then its info entry
    /// (icon byte, name max. 9 characters) - same order as the original.
    /// icon: icon 1-100; None keeps an icon already chosen for this slot.
    pub fn write_macro(&mut self, slot: usize, record: &[u8], name: &str, icon: Option<u8>) -> Result<()> {
        self.write_macro_progress(slot, record, name, icon, &mut |_, _| {})
    }

    /// `write_macro` with progress (done, total) of writing and verifying the record, which is
    /// nearly all of the time.
    pub fn write_macro_progress(
        &mut self,
        slot: usize,
        record: &[u8],
        name: &str,
        icon: Option<u8>,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<()> {
        Self::check_slot(slot)?;
        if !(2..=romlayout::MACRO_SLOT_SIZE).contains(&record.len()) || record.last() != Some(&0x00) {
            return invalid(tr!("error.macro_format", macro_slot_size = romlayout::MACRO_SLOT_SIZE));
        }
        let Some(raw_name) = name.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect::<Option<Vec<u8>>>() else {
            return invalid(tr!("error.name_latin1"));
        };
        if !(1..=romlayout::MACRO_NAME_LEN).contains(&raw_name.len()) {
            return invalid(tr!("error.name_length", macro_name_len = romlayout::MACRO_NAME_LEN));
        }
        if let Some(icon) = icon {
            Self::check_icon(icon)?;
        }
        let info = romlayout::macro_info_addr(slot);
        let icon = match icon {
            Some(i) => Some(i),
            None => romlayout::macro_icon(&self.read_rom(info, romlayout::MACRO_INFO_SIZE)?),
        };
        self.write_rom_progress(romlayout::macro_slot_addr(slot), record, true, progress)?;
        self.set_macro_icon(slot, icon)?;
        let mut padded = raw_name;
        padded.resize(romlayout::MACRO_NAME_LEN, 0);
        self.write_rom(info, &padded, false)
    }

    /// Like "Release" in the original: byte 0 of the macro name = 00, the macro data stays.
    pub fn clear_macro(&mut self, slot: usize) -> Result<()> {
        Self::check_slot(slot)?;
        self.write_rom(romlayout::macro_info_addr(slot), &[0x00], true)
    }

    /// "Release" of the original (capture 4): clear all macro names, all buttons of all profiles to default.
    pub fn release_all(&mut self) -> Result<()> {
        for slot in 0..romlayout::MACRO_SLOTS {
            self.clear_macro(slot)?;
        }
        for profile in 0..PROFILES as u8 {
            for button in 0..BUTTONS as u8 {
                self.reset_button(profile, button)?;
            }
        }
        Ok(())
    }

    /// Only the icon byte, like "Change Icon" in the original (100 + n); None = no icon (slot + 1).
    pub fn set_macro_icon(&mut self, slot: usize, icon: Option<u8>) -> Result<()> {
        Self::check_slot(slot)?;
        if let Some(icon) = icon {
            Self::check_icon(icon)?;
        }
        let value = icon.map_or(slot as u8 + 1, |i| romlayout::ICON_OFFSET + i);
        self.write_rom(romlayout::macro_info_addr(slot) + romlayout::MACRO_NAME_LEN as u16, &[value], true)
    }

    pub fn macro_icon(&mut self, slot: usize) -> Result<Option<u8>> {
        Self::check_slot(slot)?;
        Ok(romlayout::macro_icon(&self.read_rom(romlayout::macro_info_addr(slot), romlayout::MACRO_INFO_SIZE)?))
    }

    /// The 20 macro names from the info table.
    pub fn macro_names(&mut self) -> Result<Vec<String>> {
        let table = self.read_rom(romlayout::MACRO_INFO_BASE, romlayout::MACRO_SLOTS * romlayout::MACRO_INFO_SIZE)?;
        Ok(romlayout::macro_names(&table))
    }

    fn check_slot(slot: usize) -> Result<()> {
        if slot >= romlayout::MACRO_SLOTS {
            return invalid(tr!("error.macro_slot_range", macro_slots = romlayout::MACRO_SLOTS - 1));
        }
        Ok(())
    }

    fn check_icon(icon: u8) -> Result<()> {
        if !(1..=romlayout::ICON_COUNT).contains(&icon) {
            return invalid(tr!("error.icon_range", icon_count = romlayout::ICON_COUNT));
        }
        Ok(())
    }
}
