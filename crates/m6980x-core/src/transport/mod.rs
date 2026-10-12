//! Feature-Report-Transport fuer die Gigabyte GHOST M6980X.
//!
//! Die Maus wird ueber einen 8-Byte-HID-Feature-Report *ohne* Report-ID auf USB-Interface 0
//! (dem Maus-Interface selbst) konfiguriert.
//!
//! * Windows: hidapi.
//! * Linux: direkte `/dev/hidraw*`-ioctls - ohne Abhaengigkeit, und anders als hidapis libusb-Backend
//!   wird der Kernel-Treiber nie geloest (die Maus funktioniert weiter).
//!
//! Jeder Befehl laeuft unter einer prozessuebergreifenden Sperre (`lock`/`unlock`), damit sich zwei
//! Programme (z. B. Oberflaeche mit Live-Anzeige und Kommandozeile) nicht die Antworten wegnehmen.

use crate::{Error, Result, hex, tr};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

pub const VID: u16 = 0xE0FF;
pub const PID: u16 = 0x0001;
pub const INTERFACE: i32 = 0;
/// Datenbytes; die Report-ID 0 gehoert nicht dazu.
pub const REPORT_LEN: usize = 8;
/// So lange wartet ein Befehl hoechstens auf ein anderes Programm, das gerade mit der Maus spricht.
pub const LOCK_TIMEOUT_MS: u32 = 5000;

pub trait Transport {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()>;
    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]>;
    /// Sperre fuer einen ganzen Befehl (Busy-Abfrage, Senden, Done-Abfrage).
    fn lock(&mut self) -> Result<()> {
        Ok(())
    }
    fn unlock(&mut self) {}
    /// Geraetepfad (hidapi-Pfad oder /dev/hidrawN), nur zur Anzeige.
    fn path(&self) -> String {
        String::new()
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        (**self).set_feature(data)
    }
    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        (**self).get_feature()
    }
    fn lock(&mut self) -> Result<()> {
        (**self).lock()
    }
    fn unlock(&mut self) {
        (**self).unlock()
    }
    fn path(&self) -> String {
        (**self).path()
    }
}

/// hidapi und hidraw liefern beide [Report-ID, Daten...]; auf 8 Datenbytes normalisieren.
pub fn strip_report_id(buf: &[u8]) -> Result<[u8; REPORT_LEN]> {
    let data = match buf.len() {
        n if n == REPORT_LEN + 1 => &buf[1..],
        REPORT_LEN => buf,
        n => {
            return Err(Error::Transport(format!("unexpected feature report length {n}: {}", hex(buf))));
        }
    };
    Ok(data.try_into().expect("Laenge geprueft"))
}

pub(crate) fn not_found() -> Error {
    Error::Transport(tr!(
        "transport.not_found",
        vid = format!("{VID:04X}"),
        pid = format!("{PID:04X}"),
        interface = INTERFACE
    ))
}

/// Oeffnet die Maus (oder den angegebenen Pfad).
pub fn open_transport(path: Option<&str>) -> Result<Box<dyn Transport>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::HidrawTransport::open(path)?))
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows::HidapiTransport::open(path)?))
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = path;
        Err(Error::Transport(crate::i18n::tr!("transport.unsupported_os")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_report_id_accepts_both_lengths() {
        let with_id: Vec<u8> = std::iter::once(0).chain(0..8).collect();
        assert_eq!(strip_report_id(&with_id).unwrap(), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(strip_report_id(&[0, 1, 2, 3, 4, 5, 6, 7]).unwrap(), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(matches!(strip_report_id(&[0]), Err(Error::Transport(_))));
    }
}
