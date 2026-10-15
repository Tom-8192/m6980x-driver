//! Settings that live in the PC, not in the mouse: the Windows scroll speed (what the original changes on its DPI
//! page, see docs/PROTOCOL.md "Scroll settings") and whether programs of the original software are running.
//!
//! Only Windows has a system-wide scroll speed; on other systems the scroll functions return an error and
//! `original_software()` finds nothing.

use crate::Result;
#[cfg(not(windows))]
use crate::{Error, tr};

/// Range of both sliders. The original allows 1-100 lines; for characters it had no slider (Tilt.exe used 1-40).
pub const SCROLL_MIN: u32 = 1;
pub const SCROLL_MAX: u32 = 100;
/// Windows default for lines per wheel notch and characters per tilt step.
pub const WINDOWS_DEFAULT: u32 = 3;
/// `WHEEL_PAGESCROLL`: "one screen per notch" instead of a number of lines.
pub const PAGE_SCROLL: u32 = u32::MAX;

/// Whether this system has a scroll speed the tool can set.
pub fn scroll_supported() -> bool {
    cfg!(windows)
}

/// Lines per wheel notch (`SPI_GETWHEELSCROLLLINES`, the original's vertical slider). `PAGE_SCROLL` = one screen.
pub fn scroll_lines() -> Result<u32> {
    imp::get(imp::SPI_GETWHEELSCROLLLINES)
}

/// Lines per wheel notch for all programs, stored for the user (`SPIF_UPDATEINIFILE | SPIF_SENDCHANGE` like the
/// original, `0x44D994`).
pub fn set_scroll_lines(lines: u32) -> Result<()> {
    check_range(lines)?;
    imp::set(imp::SPI_SETWHEELSCROLLLINES, lines)
}

/// Characters per tilt step (`SPI_GETWHEELSCROLLCHARS`). The original never sets it; it is the Windows setting for
/// horizontal scrolling in native tilt mode (Tilt.exe's own speed only works in its tilt mode).
pub fn scroll_chars() -> Result<u32> {
    imp::get(imp::SPI_GETWHEELSCROLLCHARS)
}

pub fn set_scroll_chars(chars: u32) -> Result<()> {
    check_range(chars)?;
    imp::set(imp::SPI_SETWHEELSCROLLCHARS, chars)
}

fn check_range(value: u32) -> Result<()> {
    if (SCROLL_MIN..=SCROLL_MAX).contains(&value) {
        Ok(())
    } else {
        crate::invalid(crate::tr!("pc.scroll_range", min = SCROLL_MIN, max = SCROLL_MAX))
    }
}

/// Programs of the original software that are running right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OriginalSoftware {
    /// `Tilt.exe`: switches the mouse into tilt mode when it starts (with its default setting) and then does the
    /// horizontal scrolling itself; the Windows setting for characters per tilt step has no effect then.
    pub tilt: bool,
    /// `Macro.exe`: talks to the mouse over the same channel without our lock between programs.
    pub configurator: bool,
}

/// Looks for `Tilt.exe` and `Macro.exe` whose path contains "GHOST" (the original installs to
/// `Documents\GIGABYTE\GHOST(6980X)`). If the path of a process cannot be read, its name alone counts.
pub fn original_software() -> OriginalSoftware {
    imp::original_software()
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    pub use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES, SPI_SETWHEELSCROLLCHARS, SPI_SETWHEELSCROLLLINES,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SystemParametersInfoW};

    use super::OriginalSoftware;
    use crate::{Error, Result, tr};

    pub fn get(action: u32) -> Result<u32> {
        let mut value = 0u32;
        // SAFETY: the GET actions write one UINT to pvParam.
        let ok = unsafe { SystemParametersInfoW(action, 0, (&raw mut value).cast(), 0) };
        if ok == 0 {
            return Err(Error::Transport(tr!("pc.read_failed", e = std::io::Error::last_os_error())));
        }
        Ok(value)
    }

    pub fn set(action: u32, value: u32) -> Result<()> {
        // SAFETY: the SET actions take the value in uiParam, pvParam is unused.
        let ok =
            unsafe { SystemParametersInfoW(action, value, std::ptr::null_mut(), SPIF_UPDATEINIFILE | SPIF_SENDCHANGE) };
        if ok == 0 {
            return Err(Error::Transport(tr!("pc.write_failed", e = std::io::Error::last_os_error())));
        }
        Ok(())
    }

    fn wide_to_string(wide: &[u16]) -> String {
        let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
        String::from_utf16_lossy(&wide[..end])
    }

    /// Full path of a process, None if Windows does not give it (e.g. a process of another user).
    fn process_path(pid: u32) -> Option<String> {
        // SAFETY: plain handle query; the handle is closed below.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: buffer and length match; len receives the number of characters written.
        let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
        // SAFETY: handle from OpenProcess.
        unsafe { CloseHandle(handle) };
        (ok != 0).then(|| wide_to_string(&buf[..len as usize]))
    }

    pub fn original_software() -> OriginalSoftware {
        let mut found = OriginalSoftware::default();
        // SAFETY: snapshot handle is closed below.
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap == INVALID_HANDLE_VALUE {
            return found;
        }
        // SAFETY: zeroed PROCESSENTRY32W with dwSize set is the documented start state.
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        // SAFETY: valid snapshot and entry.
        let mut more = unsafe { Process32FirstW(snap, &mut entry) } != 0;
        while more {
            let name = wide_to_string(&entry.szExeFile).to_lowercase();
            if name == "tilt.exe" || name == "macro.exe" {
                let original = process_path(entry.th32ProcessID).is_none_or(|p| p.to_uppercase().contains("GHOST"));
                if original {
                    if name == "tilt.exe" {
                        found.tilt = true;
                    } else {
                        found.configurator = true;
                    }
                }
            }
            // SAFETY: as above.
            more = unsafe { Process32NextW(snap, &mut entry) } != 0;
        }
        // SAFETY: handle from CreateToolhelp32Snapshot.
        unsafe { CloseHandle(snap) };
        found
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub const SPI_GETWHEELSCROLLLINES: u32 = 0x68;
    pub const SPI_SETWHEELSCROLLLINES: u32 = 0x69;
    pub const SPI_GETWHEELSCROLLCHARS: u32 = 0x6C;
    pub const SPI_SETWHEELSCROLLCHARS: u32 = 0x6D;

    pub fn get(_action: u32) -> Result<u32> {
        Err(Error::Invalid(tr!("pc.windows_only")))
    }

    pub fn set(_action: u32, _value: u32) -> Result<()> {
        Err(Error::Invalid(tr!("pc.windows_only")))
    }

    pub fn original_software() -> OriginalSoftware {
        OriginalSoftware::default()
    }
}
