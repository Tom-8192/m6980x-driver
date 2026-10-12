//! Linux: `/dev/hidraw*` direkt per ioctl (ohne hidapi), `flock` auf das Geraet als Sperre zwischen Prozessen.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::{Duration, Instant};

use super::{INTERFACE, LOCK_TIMEOUT_MS, PID, REPORT_LEN, Transport, VID, not_found, strip_report_id};
use crate::{Error, Result};

const fn ioc(direction: u64, kind: u8, nr: u64, size: u64) -> u64 {
    (direction << 30) | (size << 16) | ((kind as u64) << 8) | nr
}
const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;
pub const HIDIOCSFEATURE: u64 = ioc(IOC_WRITE | IOC_READ, b'H', 0x06, (REPORT_LEN + 1) as u64);
pub const HIDIOCGFEATURE: u64 = ioc(IOC_WRITE | IOC_READ, b'H', 0x07, (REPORT_LEN + 1) as u64);

/// /dev/hidrawN des Maus-Interfaces (USB-Interface 0).
pub fn find_hidraw() -> Option<String> {
    let hid_id = format!("HID_ID=0003:{:08X}:{:08X}", VID as u32, PID as u32);
    let mut nodes: Vec<_> = fs::read_dir("/sys/class/hidraw").ok()?.flatten().map(|e| e.path()).collect();
    nodes.sort();
    for node in nodes {
        let Ok(uevent) = fs::read_to_string(node.join("device").join("uevent")) else {
            continue;
        };
        if !uevent.to_uppercase().contains(&hid_id) {
            continue;
        }
        // .../3-2:1.0/0003:E0FF:0001.0007 -> das USB-Interface-Verzeichnis endet auf ".<ifnum>"
        let Ok(real) = fs::canonicalize(node.join("device")) else {
            continue;
        };
        let usb_if = real.parent().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
        if usb_if.is_some_and(|n| n.ends_with(&format!(".{INTERFACE}"))) {
            return Some(format!("/dev/{}", node.file_name()?.to_string_lossy()));
        }
    }
    None
}

pub struct HidrawTransport {
    file: File,
    path: String,
    locked: bool,
}

impl HidrawTransport {
    pub fn open(path: Option<&str>) -> Result<Self> {
        let path = match path {
            Some(p) => p.to_owned(),
            None => find_hidraw().ok_or_else(not_found)?,
        };
        let file = OpenOptions::new().read(true).write(true).open(&path).map_err(|e| {
            if e.kind() == ErrorKind::PermissionDenied {
                Error::Transport(format!("Kein Zugriff auf {path} - udev-Regel aus linux/ installieren"))
            } else {
                Error::Transport(format!("{path}: {e}"))
            }
        })?;
        Ok(Self { file, path, locked: false })
    }

    fn ioctl(&self, request: u64, buf: &mut [u8; REPORT_LEN + 1]) -> Result<usize> {
        // SAFETY: buf ist so gross, wie die Groesse im Request-Code angibt.
        let n = unsafe { libc::ioctl(self.file.as_raw_fd(), request as _, buf.as_mut_ptr()) };
        if n < 0 {
            return Err(Error::Transport(std::io::Error::last_os_error().to_string()));
        }
        Ok(n as usize)
    }
}

impl Transport for HidrawTransport {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..].copy_from_slice(data);
        self.ioctl(HIDIOCSFEATURE, &mut buf).map(|_| ())
    }

    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        let mut buf = [0u8; REPORT_LEN + 1];
        let n = self.ioctl(HIDIOCGFEATURE, &mut buf)?;
        // Die Maus liefert manchmal kurze Reports (z. B. beim Druecken der DPI-/Profiltaste) -> wie "busy" behandeln.
        if n < REPORT_LEN {
            return Err(Error::Transport(format!("unexpected feature report length {n}")));
        }
        strip_report_id(&buf)
    }

    fn lock(&mut self) -> Result<()> {
        let end = Instant::now() + Duration::from_millis(LOCK_TIMEOUT_MS as u64);
        loop {
            // SAFETY: gueltiger Dateideskriptor.
            if unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                self.locked = true;
                return Ok(());
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != ErrorKind::WouldBlock {
                return Err(Error::Transport(format!("Sperre fehlgeschlagen: {err}")));
            }
            if Instant::now() >= end {
                return Err(Error::Device("Maus wird gerade von einem anderen Programm benutzt".into()));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn unlock(&mut self) {
        if self.locked {
            // SAFETY: gueltiger Dateideskriptor.
            unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
            self.locked = false;
        }
    }

    fn path(&self) -> String {
        self.path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidraw_ioctl_numbers() {
        assert_eq!(HIDIOCSFEATURE, 0xC0094806);
        assert_eq!(HIDIOCGFEATURE, 0xC0094807);
    }
}
