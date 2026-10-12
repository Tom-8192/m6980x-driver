//! Befehlsschicht der GHOST M6980X (Lesebefehle plus einzeln am Geraet bestaetigte Schreibbefehle).
//!
//! Paket auf der Leitung (8 Bytes, keine Report-ID): `[Gruppe, Befehl, arg0..arg5]`.
//! Handshake aus der Original-Macro.exe (0x444aac):
//!   1. GET_FEATURE, bis Statusbit 0x80 (busy) aus ist
//!   2. SET_FEATURE mit dem Befehl
//!   3. GET_FEATURE, bis Statusbit 0x40 (done) gesetzt ist; Antwort = `[Status, data0..data6]`
//!
//! Antwortbytes, die ein Befehl nicht fuellt, behalten alte Werte - nur dokumentierte Bytes auswerten.
//! Belege zu jedem Befehl: docs/PROTOCOL.md.

use std::thread::sleep;
use std::time::Duration;

use crate::functions::{self as fnc, Function};
use crate::rom as romlayout;
use crate::transport::{REPORT_LEN, Transport};
use crate::{Error, Result, hex, invalid};

pub const STATUS_BUSY: u8 = 0x80;
pub const STATUS_DONE: u8 = 0x40;

pub const GROUP_SYSTEM: u8 = 0x00;
pub const GROUP_REGISTER: u8 = 0x01;
pub const GROUP_ROM: u8 = 0x02;

/// 8 KB, darueber gespiegelt (geprueft)
pub const ROM_SIZE: usize = 0x2000;
pub const ROM_READ_CHUNK: usize = 7;
/// Puffer-Bytes 0-5, 6-11, ..., 30-31
pub const ROM_FILL_CMDS: [u8; 6] = [0x03, 0x05, 0x06, 0x07, 0x08, 0x09];
pub const PROFILES: usize = 3;
pub const BUTTONS: usize = 10;
/// Eintraege in den Registern 03/04
pub const DPI_STAGES: usize = 4;
/// Die DPI-Taste schaltet 0 -> 1 -> 2 (beobachtet); die LEDs zeigen Stufe + 1.
pub const DPI_BUTTON_STAGES: usize = 3;
/// Mappingdefault.ini [6980DPIdefault] max
pub const MAX_DPI: u32 = 6030;
/// je Kanal: aus, halb, voll
pub const LED_LEVELS: [u8; 3] = [0, 1, 2];
/// konstantes letztes Byte im ROM und bei Set-Befehlen des Originals
pub const ROM_TRAILER: u8 = 0x12;
/// Macro.exe wartet nach einem Set-Befehl 10 ms.
pub const WRITE_DELAY: Duration = Duration::from_millis(10);
pub const SUPPORTED_MODEL: &str = "6980X";

/// Werkswerte der drei Stufen ([6980DPIdefault]; der 4. INI-Wert gehoert zu einer ungenutzten Stufe)
pub const FACTORY_DPI: [u32; 3] = [810, 1620, 3240];
/// 500 Hz: der Default-Knopf des Originals (0x44abbc) schreibt auch 01 8B 02
pub const FACTORY_INTERVAL_MS: u8 = 2;

pub type Reply = [u8; REPORT_LEN];

fn device<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Device(msg.into()))
}

/// Modellname wie im Original (0x42ebd8) aus den Identify-Antwortdaten (0C 12 ...):
/// Byte 6 = 0 -> 6980, 1 und Byte 2 = 9 -> 6980X, 1 sonst -> M8000X, 4 -> M8600, sonst unbekannt (None).
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

/// Wie Pythons `round(dpi / unit)` (bei genau .5 zur geraden Zahl), mit Bereichspruefung.
pub fn dpi_to_steps(dpi: i64, unit: u8) -> Result<u8> {
    let unit = i64::from(unit);
    let steps = if dpi < 0 {
        -1
    } else {
        let (q, r) = (dpi / unit, dpi % unit);
        if 2 * r > unit || (2 * r == unit && q % 2 == 1) { q + 1 } else { q }
    };
    if steps < 1 || steps * unit > i64::from(MAX_DPI) || steps > 0xFF {
        return invalid(format!("DPI muss zwischen {unit} und {MAX_DPI} liegen"));
    }
    Ok(steps as u8)
}

/// 0x445900: kleinster unterstuetzter Sensorschritt >= value, sonst der groesste unterstuetzte.
pub fn snap_step(value: u8, supported: &[u8]) -> Option<u8> {
    supported.iter().copied().filter(|&s| s >= value).min().or_else(|| supported.iter().copied().max())
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Port von 0x445900: jede Achse bekommt einen unterstuetzten Sensorschritt plus den Bruch
/// gewuenscht/Sensor (gekuerzt wie 0x445640), mit dem die Firmware skaliert.
/// Liefert [(Achse, Sensorschritt, Zaehler, Nenner)] fuer X (0) und Y (1).
pub fn dpi_write_plan(x_steps: u8, y_steps: u8, supported: &[u8]) -> Result<Vec<(u8, u8, u8, u8)>> {
    let mut plan = Vec::new();
    for (axis, wanted) in [(0u8, x_steps), (1, y_steps)] {
        let Some(step) = snap_step(wanted, supported) else {
            return device("Maus meldet keine DPI-Schritte");
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

    // -- unterste Ebene --------------------------------------------------------------------
    /// Ein Befehl mit Handshake. `trailer` landet in Byte 7, `delay` ist die Wartezeit nach dem Senden.
    pub fn xfer(&mut self, group: u8, cmd: u8, args: &[u8], trailer: Option<u8>, delay: Duration) -> Result<Reply> {
        if args.len() > 6 {
            return invalid("max. 6 Argument-Bytes");
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
            return device("Maus meldet dauerhaft 'beschaeftigt'");
        }

        self.t.set_feature(pkt)?;
        self.pause(delay);

        for _ in 0..self.done_tries {
            if let Some(reply) = self.poll().filter(|r| r[0] & STATUS_DONE != 0) {
                return Ok(reply);
            }
            self.pause(self.poll_interval);
        }
        device(format!("keine Antwort auf {}", hex(pkt)))
    }

    fn pause(&self, d: Duration) {
        if !d.is_zero() {
            sleep(d);
        }
    }

    /// Das Original behandelt ein fehlgeschlagenes GET_FEATURE wie "nicht bereit" und versucht es erneut;
    /// die Maus liefert z. B. waehrend eines Drucks auf die DPI-/Profiltaste kurze Reports.
    fn poll(&mut self) -> Option<Reply> {
        self.t.get_feature().ok()
    }

    pub fn get_register(&mut self, reg: u8, args: &[u8]) -> Result<Reply> {
        if reg & 0x80 != 0 {
            return invalid("Bit 0x80 waere ein Schreibbefehl");
        }
        self.xfer(GROUP_REGISTER, reg, args, None, Duration::ZERO)
    }

    /// Set-Befehle des Originals warten 10 ms; die meisten enden auf 0x12.
    fn set_register(&mut self, reg: u8, args: &[u8], trailer: Option<u8>) -> Result<Reply> {
        self.xfer(GROUP_REGISTER, reg | 0x80, args, trailer, WRITE_DELAY)
    }

    // -- System -----------------------------------------------------------------------------
    /// Das Original akzeptiert das Geraet nur, wenn Antwortbytes 1-2 = 0C 12 sind (0x44407c).
    pub fn identify(&mut self) -> Result<Identify> {
        let r = self.xfer(GROUP_SYSTEM, 0x00, &[], None, Duration::ZERO)?;
        Ok(Identify { signature_ok: r[1..3] == [0x0C, 0x12], raw: hex(&r[1..]), model: model_of(&r[1..]) })
    }

    /// Nur die 6980X zulassen: die anderen GHOST-Modelle haben einen anderen Speicheraufbau
    /// (M8000X/M8600: 32 KB, 5 Profile, 70 Makros; 6980: anderer DPI-Bereich), unsere Adressen waeren falsch.
    pub fn check_model(&mut self) -> Result<Identify> {
        let ident = self.identify()?;
        if !ident.signature_ok {
            return device(format!("Geraet antwortet nicht wie eine GHOST-Maus (Identifizieren: {})", ident.raw));
        }
        if ident.model != Some(SUPPORTED_MODEL) {
            return device(format!(
                "Modell {} wird nicht unterstuetzt, nur {SUPPORTED_MODEL} (Identifizieren: {})",
                ident.model.unwrap_or("unbekannt"),
                ident.raw
            ));
        }
        Ok(ident)
    }

    pub fn name(&mut self) -> Result<String> {
        let r = self.xfer(GROUP_SYSTEM, 0x01, &[], None, Duration::ZERO)?;
        let raw = r[1..7].split(|&c| c == 0).next().unwrap_or_default();
        Ok(raw.iter().map(|&c| if c.is_ascii() { c as char } else { '\u{FFFD}' }).collect())
    }

    // -- Register ---------------------------------------------------------------------------
    pub fn active_profile(&mut self) -> Result<u8> {
        Ok(self.get_register(0x08, &[])?[1])
    }

    /// Aktives Hardware-Profil (0-2) umschalten und durch Zuruecklesen pruefen.
    pub fn set_active_profile(&mut self, profile: u8) -> Result<()> {
        if usize::from(profile) >= PROFILES {
            return invalid(format!("Profil muss 0..{} sein", PROFILES - 1));
        }
        self.set_register(0x08, &[profile], Some(ROM_TRAILER))?;
        let actual = self.active_profile()?;
        if actual != profile {
            return device(format!("Profilwechsel nicht uebernommen: Maus meldet Profil {actual}"));
        }
        Ok(())
    }

    /// Register 07: aktive DPI-Stufe, 0-basiert (1 = Mitte, 2 von 3 LEDs an - am Geraet gesehen).
    pub fn active_dpi_stage(&mut self) -> Result<u8> {
        Ok(self.get_register(0x07, &[])?[1])
    }

    pub fn register_0a(&mut self) -> Result<u16> {
        let r = self.get_register(0x0A, &[])?;
        Ok(u16::from(r[1]) | u16::from(r[2]) << 8)
    }

    /// Register 02 (01 02 00 .. 12, 4 ms), vom Original beim Verlassen der Einstellungsseite gesendet.
    /// Ohne gehen Abtastrate und DPI beim Abstecken verloren (getestet); LED-Farben bleiben auch so.
    pub fn save_settings(&mut self) -> Result<()> {
        self.xfer(GROUP_REGISTER, 0x02, &[0], Some(ROM_TRAILER), Duration::from_millis(4)).map(|_| ())
    }

    /// Register 0B: USB-Abfrageintervall in ms (1-8); Abtastrate = 1000 / Wert Hz.
    pub fn report_interval(&mut self) -> Result<u8> {
        Ok(self.get_register(0x0B, &[])?[1])
    }

    /// Wie das Original (Mitschnitt: 500 -> 333 Hz = 01 8B 02 -> 01 8B 03).
    pub fn set_report_interval(&mut self, ms: u8) -> Result<()> {
        if !(1..=8).contains(&ms) {
            return invalid("Abfrageintervall muss 1-8 ms sein");
        }
        self.set_register(0x0B, &[ms], None)?;
        let actual = self.report_interval()?;
        if actual != ms {
            return device(format!("Abtastrate nicht uebernommen: Maus meldet {actual} ms"));
        }
        Ok(())
    }

    pub fn led_color(&mut self, profile: u8) -> Result<[u8; 3]> {
        let r = self.get_register(0x06, &[profile])?;
        Ok([r[1], r[2], r[3]])
    }

    /// LED-Farbe eines Profils setzen. Jeder Kanal hat drei Stufen: 0 aus, 1 halb, 2 voll.
    /// Wie das Original (0x445ba8) ohne Umschalten des aktiven Profils.
    pub fn set_led_color(&mut self, profile: u8, rgb: [u8; 3]) -> Result<()> {
        if usize::from(profile) >= PROFILES {
            return invalid(format!("Profil muss 0..{} sein", PROFILES - 1));
        }
        if rgb.iter().any(|c| !LED_LEVELS.contains(c)) {
            return invalid("LED-Stufen muessen 0 (aus), 1 (mittel) oder 2 (voll) sein");
        }
        self.set_register(0x06, &[profile, rgb[0], rgb[1], rgb[2]], None)?;
        let actual = self.led_color(profile)?;
        if actual != rgb {
            return device(format!("LED-Farbe nicht uebernommen: Maus meldet {actual:?}"));
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
        // wie das Original (0x4457fc): nur Nenner 0 werden durch 1 ersetzt
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

    /// Aktive DPI-Stufe waehlen wie das Original (0x445280). Software kann auch Stufe 3 waehlen,
    /// die die DPI-Taste nie erreicht.
    pub fn set_active_dpi_stage(&mut self, stage: u8) -> Result<()> {
        if usize::from(stage) >= DPI_STAGES {
            return invalid(format!("DPI-Stufe muss 0..{} sein", DPI_STAGES - 1));
        }
        self.set_register(0x07, &[stage], Some(ROM_TRAILER))?;
        let actual = self.active_dpi_stage()?;
        if actual != stage {
            return device(format!("DPI-Stufe nicht uebernommen: Maus meldet {actual}"));
        }
        Ok(())
    }

    /// DPI einer Stufe setzen, gerundet auf die Geraete-Einheit (90). Liefert die zurueckgelesene Stufe.
    pub fn set_dpi(&mut self, stage: u8, x_dpi: i64, y_dpi: Option<i64>) -> Result<DpiStage> {
        if usize::from(stage) >= DPI_STAGES {
            return invalid(format!("DPI-Stufe muss 0..{} sein", DPI_STAGES - 1));
        }
        let unit = self.dpi_stage(stage)?.unit;
        let x = dpi_to_steps(x_dpi, unit)?;
        let y = match y_dpi {
            Some(y) => dpi_to_steps(y, unit)?,
            None => x,
        };
        let supported = self.dpi_supported_steps()?;
        let plan = dpi_write_plan(x, y, &supported)?;
        // gleiche Reihenfolge wie das Original: Sensorschritte X, Y, dann Brueche X, Y
        for &(axis, step, _, _) in &plan {
            self.set_register(0x03, &[axis, stage, step], Some(ROM_TRAILER))?;
        }
        for &(axis, _, num, den) in &plan {
            self.set_register(0x04, &[axis, stage, num, den], Some(ROM_TRAILER))?;
        }
        let actual = self.dpi_stage(stage)?;
        let unit = u32::from(unit);
        if (actual.x_dpi(), actual.y_dpi()) != (u32::from(x) * unit, u32::from(y) * unit) {
            return device(format!("DPI nicht uebernommen: Maus meldet {}/{}", actual.x_dpi(), actual.y_dpi()));
        }
        Ok(actual)
    }

    /// Wie der Default-Knopf des Originals: DPI aus [6980DPIdefault], dann Abtastrate 500 Hz.
    /// Liefert die neuen DPI-Stufen.
    pub fn apply_factory_defaults(&mut self) -> Result<Vec<DpiStage>> {
        let stages = FACTORY_DPI
            .iter()
            .enumerate()
            .map(|(stage, &dpi)| self.set_dpi(stage as u8, i64::from(dpi), None))
            .collect::<Result<Vec<_>>>()?;
        self.set_report_interval(FACTORY_INTERVAL_MS)?;
        Ok(stages)
    }

    /// ROM-Adresse des Datensatzes einer Geraetetaste oder None fuer ihre Grundfunktion.
    /// Geraetenummern (an der Maus ermittelt): 0 links, 1 rechts, 2 Mitte, 3 hinten, 4 vorne,
    /// 5 keine Taste, 6 Kipp links, 7 Kipp rechts, 8 Rad hoch, 9 Rad runter.
    pub fn button(&mut self, profile: u8, button: u8) -> Result<Option<u16>> {
        let r = self.get_register(0x05, &[profile, button])?;
        if r[1] & 0x80 == 0 {
            return Ok(None);
        }
        Ok(Some(u16::from(r[2]) | u16::from(r[3]) << 8))
    }

    fn check_button(profile: u8, button: u8) -> Result<()> {
        if usize::from(profile) >= PROFILES || usize::from(button) >= BUTTONS {
            return invalid("Profil oder Taste ausserhalb des Bereichs");
        }
        Ok(())
    }

    /// Taste auf einen Funktionsdatensatz oder Makro-Slot legen wie das Original
    /// (01 85 <Profil> <Taste> 80 <lo> <hi> 12, danach das aktive Profil neu aktivieren).
    pub fn set_button(&mut self, profile: u8, button: u8, address: u16) -> Result<()> {
        Self::check_button(profile, button)?;
        if usize::from(address) >= ROM_SIZE {
            return invalid("Adresse ausserhalb des ROM");
        }
        let active = self.active_profile()?;
        let [lo, hi] = address.to_le_bytes();
        self.set_register(0x05, &[profile, button, 0x80, lo, hi], Some(ROM_TRAILER))?;
        self.set_active_profile(active)?;
        let actual = self.button(profile, button)?;
        if actual != Some(address) {
            return device(format!("Tastenbelegung nicht uebernommen: Maus meldet {actual:?}"));
        }
        Ok(())
    }

    // -- ROM --------------------------------------------------------------------------------
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

    /// 02 01 <lo> <hi> 00 <Wert> 00 12 - vom Makro-Editor des Originals benutzt (1 ms).
    pub fn write_rom_byte(&mut self, addr: u16, value: u8) -> Result<()> {
        let [lo, hi] = addr.to_le_bytes();
        self.xfer(GROUP_ROM, 0x01, &[lo, hi, 0, value, 0], Some(ROM_TRAILER), Duration::from_millis(1)).map(|_| ())
    }

    /// 02 02 <lo> <hi> 00 <b0> <b1> 12 - 2-Byte-Schreiben des Originals (0x444ba8, 1 ms).
    pub fn write_rom_word(&mut self, addr: u16, data: [u8; 2]) -> Result<()> {
        let [lo, hi] = addr.to_le_bytes();
        self.xfer(GROUP_ROM, 0x02, &[lo, hi, 0, data[0], data[1]], Some(ROM_TRAILER), Duration::from_millis(1))
            .map(|_| ())
    }

    /// Bis 32 Bytes ueber den Transferpuffer wie das Original (0x444c30): Fuellbefehle
    /// 03/05/06/07/08/09 mit je 6 Bytes, dann 02 04 <lo> <hi> 00 <Laenge> 00 12.
    pub fn write_rom_block(&mut self, addr: u16, data: &[u8]) -> Result<()> {
        if !(1..=32).contains(&data.len()) {
            return invalid("Block 1-32 Bytes");
        }
        for (cmd, chunk) in ROM_FILL_CMDS.iter().zip(data.chunks(6)) {
            self.xfer(GROUP_ROM, *cmd, chunk, None, Duration::from_millis(1))?;
        }
        // Wartezeit waechst mit der Laenge genau wie [ebp-4] im Original
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

    /// Schreiben und pruefen. Bloecke ueberschreiten nie eine 32-Byte-Grenze.
    pub fn write_rom(&mut self, addr: u16, data: &[u8], bytewise: bool) -> Result<()> {
        if usize::from(addr) + data.len() > ROM_SIZE {
            return invalid("Schreibbereich ausserhalb des ROM");
        }
        if bytewise {
            for (i, &b) in data.iter().enumerate() {
                self.write_rom_byte(addr + i as u16, b)?;
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
            }
        }
        let actual = self.read_rom(addr, data.len())?;
        if actual != data {
            return device(format!("ROM 0x{addr:04X} nicht korrekt geschrieben: {}", hex(&actual)));
        }
        Ok(())
    }

    // -- obere Ebene: Funktionen und Makros (Formate in docs/PROTOCOL.md) ---------------------
    /// Zurueck zur eingebauten Funktion der Taste. Das Original hat dafuer keine Option; geschrieben wird
    /// der Zustand, den die Maus fuer unberuehrte Tasten meldet (Flags 00, Adresse FFFF).
    pub fn reset_button(&mut self, profile: u8, button: u8) -> Result<()> {
        Self::check_button(profile, button)?;
        let active = self.active_profile()?;
        self.set_register(0x05, &[profile, button, 0x00, 0xFF, 0xFF], Some(ROM_TRAILER))?;
        self.set_active_profile(active)?;
        if let Some(actual) = self.button(profile, button)? {
            return device(format!("Taste nicht auf Standard: Maus meldet 0x{actual:04X}"));
        }
        Ok(())
    }

    /// Wie das Original: Funktionsdatensatz an seine feste Adresse (neu) schreiben, dann die Taste darauf legen.
    /// Maustasten-Funktionen auf den Radtasten bekommen den Klick-Datensatz des Originals (functions::placement).
    pub fn assign_function(&mut self, profile: u8, button: u8, function: &Function) -> Result<()> {
        if function.size == 0 {
            return invalid(format!(
                "'{}' hat keinen Datensatz - es ist die Grundfunktion des Kipprads (Taste dort auf 'standard' setzen)",
                function.key
            ));
        }
        if function.record.is_empty() {
            return invalid(format!("Inhalt von '{}' ist unbekannt - nicht zuweisbar", function.key));
        }
        let (address, record) = fnc::placement(function, usize::from(button));
        self.write_rom(address, &record, false)?;
        self.set_button(profile, button, address)
    }

    pub fn assign_macro(&mut self, profile: u8, button: u8, slot: usize) -> Result<()> {
        Self::check_slot(slot)?;
        self.set_button(profile, button, romlayout::macro_slot_addr(slot))
    }

    /// Makro (Modusbyte + Ereignisse + 00) in einen Slot schreiben, dann seinen Info-Eintrag
    /// (Bild-Byte, Name max. 9 Zeichen) - gleiche Reihenfolge wie das Original.
    /// icon: Bild 1-100; None behaelt ein fuer diesen Slot schon gewaehltes Bild.
    pub fn write_macro(&mut self, slot: usize, record: &[u8], name: &str, icon: Option<u8>) -> Result<()> {
        Self::check_slot(slot)?;
        if !(2..=romlayout::MACRO_SLOT_SIZE).contains(&record.len()) || record.last() != Some(&0x00) {
            return invalid(format!(
                "Makro muss mit 00 enden und hoechstens {} Bytes lang sein",
                romlayout::MACRO_SLOT_SIZE
            ));
        }
        let Some(raw_name) = name.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect::<Option<Vec<u8>>>() else {
            return invalid("Name nur mit Latin-1-Zeichen");
        };
        if !(1..=romlayout::MACRO_NAME_LEN).contains(&raw_name.len()) {
            return invalid(format!("Name 1-{} Zeichen", romlayout::MACRO_NAME_LEN));
        }
        if let Some(icon) = icon {
            Self::check_icon(icon)?;
        }
        let info = romlayout::macro_info_addr(slot);
        let icon = match icon {
            Some(i) => Some(i),
            None => romlayout::macro_icon(&self.read_rom(info, romlayout::MACRO_INFO_SIZE)?),
        };
        self.write_rom(romlayout::macro_slot_addr(slot), record, true)?;
        self.set_macro_icon(slot, icon)?;
        let mut padded = raw_name;
        padded.resize(romlayout::MACRO_NAME_LEN, 0);
        self.write_rom(info, &padded, false)
    }

    /// Wie "Freigeben" im Original: Byte 0 des Makronamens = 00, die Makrodaten bleiben.
    pub fn clear_macro(&mut self, slot: usize) -> Result<()> {
        Self::check_slot(slot)?;
        self.write_rom(romlayout::macro_info_addr(slot), &[0x00], true)
    }

    /// "Freigeben" des Originals (Mitschnitt 4): alle Makronamen leeren, alle Tasten aller Profile auf Standard.
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

    /// Nur das Bild-Byte wie "Change Icon" im Original (100 + n); None = kein Bild (Slot + 1).
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

    /// Die 20 Makronamen aus der Info-Tabelle.
    pub fn macro_names(&mut self) -> Result<Vec<String>> {
        let table = self.read_rom(romlayout::MACRO_INFO_BASE, romlayout::MACRO_SLOTS * romlayout::MACRO_INFO_SIZE)?;
        Ok(romlayout::macro_names(&table))
    }

    fn check_slot(slot: usize) -> Result<()> {
        if slot >= romlayout::MACRO_SLOTS {
            return invalid(format!("Makro-Slot muss 0..{} sein", romlayout::MACRO_SLOTS - 1));
        }
        Ok(())
    }

    fn check_icon(icon: u8) -> Result<()> {
        if !(1..=romlayout::ICON_COUNT).contains(&icon) {
            return invalid(format!("Bildnummer 1-{}", romlayout::ICON_COUNT));
        }
        Ok(())
    }
}
