//! Simulierte Maus fuer Tests und den Probelauf der Oberflaeche (Port von `FakeMouse` aus
//! prototype/python/tests/test_protocol.py). Antwortet wie das Geraet: nach jedem Befehl einmal "busy", dann "done".

use std::collections::HashMap;

use crate::Result;
use crate::transport::{REPORT_LEN, Transport};

const FILL_OFFSETS: [(u8, usize); 6] = [(0x03, 0), (0x05, 6), (0x06, 12), (0x07, 18), (0x08, 24), (0x09, 30)];

fn hx(text: &str) -> [u8; REPORT_LEN] {
    let v: Vec<u8> = text.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
    v.try_into().unwrap()
}

/// Antworten der echten Maus (Statusbyte + 7 Datenbytes)
pub fn default_replies() -> HashMap<Vec<u8>, [u8; REPORT_LEN]> {
    HashMap::from([
        (vec![0x00, 0x00], hx("40 0c 12 09 2d 00 03 01")),
        (vec![0x00, 0x01], hx("40 47 33 00 00 00 00 00")),
        (vec![0x01, 0x03, 0x01], hx("41 ff ff ff ff ff ff ff")),
    ])
}

#[derive(Debug, Clone)]
pub struct FakeMouse {
    pub sent: Vec<[u8; REPORT_LEN]>,
    pub state: [u8; REPORT_LEN],
    pub replies: HashMap<Vec<u8>, [u8; REPORT_LEN]>,
    pub busy_polls: u32,
    pub pending: u32,
    pub profile: u8,
    pub accept_profile_switch: bool,
    pub leds: [[u8; 3]; 3],
    pub accept_led: bool,
    /// je DPI-Stufe [x_step, y_step] und [x_num, x_den, y_num, y_den] - Werte der echten Maus
    pub dpi_steps: [[u8; 2]; 4],
    pub dpi_ratio: [[u8; 4]; 4],
    pub dpi_stage: u8,
    pub rom: Vec<u8>,
    pub buffer: [u8; 32],
    /// (Profil, Geraetetaste) -> (Flags, Adresse)
    pub buttons: HashMap<(u8, u8), (u8, u16)>,
    pub interval: u8,
    pub saved: bool,
}

impl Default for FakeMouse {
    fn default() -> Self {
        Self {
            sent: Vec::new(),
            state: hx("41 00 00 00 00 00 00 00"),
            replies: default_replies(),
            busy_polls: 1,
            pending: 0,
            profile: 0,
            accept_profile_switch: true,
            leds: [[2, 2, 0], [0, 2, 2], [2, 1, 0]],
            accept_led: true,
            dpi_steps: [[18, 18], [42, 42], [56, 56], [56, 56]],
            dpi_ratio: [[1, 1, 1, 1], [1, 1, 1, 1], [65, 56, 65, 56], [67, 56, 67, 56]],
            dpi_stage: 1,
            rom: (0..0x2000usize).map(|i| (i & 0xFF) as u8).collect(),
            buffer: [0; 32],
            buttons: HashMap::from([((0, 4), (0x80, 0x1D98))]),
            interval: 2,
            saved: false,
        }
    }
}

impl FakeMouse {
    pub fn new() -> Self {
        Self::default()
    }

    fn with_status(status: u8, data: &[u8], rest: &[u8]) -> [u8; REPORT_LEN] {
        let mut out = [0u8; REPORT_LEN];
        out[0] = status;
        out[1..1 + data.len()].copy_from_slice(data);
        out[1 + data.len()..].copy_from_slice(&rest[1 + data.len()..]);
        out
    }

    fn reply_key(&self, data: &[u8]) -> Option<Vec<u8>> {
        let mut keys: Vec<&Vec<u8>> = self.replies.keys().filter(|k| data.starts_with(k)).collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        keys.first().map(|k| (*k).clone())
    }
}

impl Transport for FakeMouse {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        let d = *data;
        self.sent.push(d);
        let key = self.reply_key(&d);
        let addr = usize::from(d[2]) | usize::from(d[3]) << 8;
        match (d[0], d[1]) {
            (0x01, 0x88) => {
                if self.accept_profile_switch {
                    self.profile = d[2];
                }
            }
            (0x01, 0x86) => {
                if self.accept_led {
                    self.leds[usize::from(d[2])] = [d[3], d[4], d[5]];
                }
            }
            (0x01, 0x06) => self.state = Self::with_status(0x41, &self.leds[usize::from(d[2])], &self.state),
            (0x01, 0x87) => self.dpi_stage = d[2],
            (0x01, 0x07) => self.state = Self::with_status(0x41, &[self.dpi_stage], &self.state),
            (0x01, 0x83) => self.dpi_steps[usize::from(d[3])][usize::from(d[2])] = d[4],
            (0x01, 0x84) => {
                let i = 2 * usize::from(d[2]);
                self.dpi_ratio[usize::from(d[3])][i..i + 2].copy_from_slice(&d[4..6]);
            }
            (0x01, 0x03) if d[2] == 0 => {
                let [x, y] = self.dpi_steps[usize::from(d[3])];
                self.state = [0x41, x, y, 0x5A, 0x01, 0xFF, 0xFF, 0xFF];
            }
            (0x01, 0x04) if d[2] == 0 => {
                let [a, b, c, e] = self.dpi_ratio[usize::from(d[3])];
                self.state = [0x41, a, b, c, e, 0xFF, 0xFF, 0xFF];
            }
            // altes Byte 2 wie bei der echten Maus
            (0x01, 0x08) => self.state = Self::with_status(0x41, &[self.profile, 0x1F], &self.state),
            (0x01, 0x02) => self.saved = true,
            (0x01, 0x85) => {
                self.buttons.insert((d[2], d[3]), (d[4], u16::from(d[5]) | u16::from(d[6]) << 8));
            }
            (0x01, 0x05) => {
                let (flags, a) = self.buttons.get(&(d[2], d[3])).copied().unwrap_or((0x00, 0xFFFF));
                self.state = [0x41, flags, (a & 0xFF) as u8, (a >> 8) as u8, 0x00, 0x80, 0x80, 0x00];
            }
            (0x01, 0x8B) => self.interval = d[2],
            (0x01, 0x0B) => self.state = Self::with_status(0x41, &[self.interval], &self.state),
            (0x02, cmd) if FILL_OFFSETS.iter().any(|(c, _)| *c == cmd) => {
                let off = FILL_OFFSETS.iter().find(|(c, _)| *c == cmd).unwrap().1;
                let n = 6.min(32 - off);
                self.buffer[off..off + n].copy_from_slice(&d[2..2 + n]);
            }
            (0x02, 0x04) => {
                let n = usize::from(d[5]);
                self.rom[addr..addr + n].copy_from_slice(&self.buffer[..n]);
            }
            (0x02, 0x02) => self.rom[addr..addr + 2].copy_from_slice(&d[5..7]),
            (0x02, 0x01) => self.rom[addr] = d[5],
            (0x02, 0x00) if key.is_none() => {
                // ueber 0x2000 gespiegelt wie die echte Maus
                let mut s = [0x42u8; REPORT_LEN];
                for (i, b) in s[1..].iter_mut().enumerate() {
                    *b = self.rom[(addr + i) % self.rom.len()];
                }
                self.state = s;
            }
            _ => match key {
                Some(k) => self.state = self.replies[&k],
                None => panic!("unerwarteter Befehl {}", crate::hex(&d)),
            },
        }
        self.pending = self.busy_polls;
        Ok(())
    }

    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        if self.pending > 0 {
            self.pending -= 1;
            let mut s = self.state;
            s[0] = 0x80;
            return Ok(s);
        }
        Ok(self.state)
    }

    fn path(&self) -> String {
        "simuliert".into()
    }
}
