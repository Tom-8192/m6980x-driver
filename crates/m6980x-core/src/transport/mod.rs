//! Feature report transport for the Gigabyte GHOST M6980X.
//!
//! The mouse is configured via an 8-byte HID feature report *without* report ID on USB interface 0
//! (the mouse interface itself).
//!
//! * Windows: hidapi.
//! * Linux: direct `/dev/hidraw*` ioctls - no dependency, and unlike hidapi's libusb backend
//!   the kernel driver is never detached (the mouse keeps working).
//!
//! Every command runs under a cross-process lock (`lock`/`unlock`) so that two programs
//! (e.g. the GUI with live display and the command line) do not take each other's replies.

use crate::{Error, Result, hex, tr};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

pub const VID: u16 = 0xE0FF;
pub const PID: u16 = 0x0001;
pub const INTERFACE: i32 = 0;
/// data bytes; the report ID 0 is not included.
pub const REPORT_LEN: usize = 8;
/// Longest time a command waits for another program that is currently talking to the mouse.
pub const LOCK_TIMEOUT_MS: u32 = 5000;

pub trait Transport {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()>;
    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]>;
    /// Lock for a whole command (busy poll, send, done poll).
    fn lock(&mut self) -> Result<()> {
        Ok(())
    }
    fn unlock(&mut self) {}
    /// Device path (hidapi path or /dev/hidrawN), for display only.
    fn path(&self) -> String {
        String::new()
    }
    /// Raw bytes of USB string descriptor `index`. The tilt mode switch is a sequence of such requests
    /// (`M6980X::set_tilt_mode`); not every platform can send them.
    fn string_descriptor(&mut self, index: u8) -> Result<Vec<u8>> {
        let _ = index;
        Err(Error::Transport(tr!("transport.string_unsupported")))
    }
    /// Serial number string request (part of the tilt mode switch; the M6980X has no serial number, so it fails).
    fn serial_number(&mut self) -> Result<Vec<u8>> {
        Err(Error::Transport(tr!("transport.string_unsupported")))
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
    fn string_descriptor(&mut self, index: u8) -> Result<Vec<u8>> {
        (**self).string_descriptor(index)
    }
    fn serial_number(&mut self) -> Result<Vec<u8>> {
        (**self).serial_number()
    }
}

/// hidapi and hidraw both return [report ID, data...]; normalise to 8 data bytes.
pub fn strip_report_id(buf: &[u8]) -> Result<[u8; REPORT_LEN]> {
    let data = match buf.len() {
        n if n == REPORT_LEN + 1 => &buf[1..],
        REPORT_LEN => buf,
        n => {
            return Err(Error::Transport(format!("unexpected feature report length {n}: {}", hex(buf))));
        }
    };
    Ok(data.try_into().expect("length checked"))
}

pub(crate) fn not_found() -> Error {
    Error::Transport(tr!(
        "transport.not_found",
        vid = format!("{VID:04X}"),
        pid = format!("{PID:04X}"),
        interface = INTERFACE
    ))
}

/// Opens the mouse (or the given path).
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
