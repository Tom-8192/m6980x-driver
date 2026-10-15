//! Windows: hidapi for the feature reports, a named mutex as the lock between processes.

use std::ffi::CString;

use hidapi::{HidApi, HidDevice};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{HidD_GetIndexedString, HidD_GetSerialNumberString};
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

use super::{INTERFACE, LOCK_TIMEOUT_MS, PID, REPORT_LEN, Transport, VID, not_found, strip_report_id};
use crate::{Error, Result, tr};

const MUTEX_NAME: &str = "Local\\m6980x_configtool";
/// buffer size Tilt.exe passes to HidD_GetIndexedString
const STRING_BUFFER: usize = 0x200;

pub struct HidapiTransport {
    dev: HidDevice,
    _api: HidApi,
    path: String,
    mutex: HANDLE,
    locked: bool,
    /// second handle for the string descriptor requests, opened on first use
    strings: Option<HANDLE>,
}

fn hid_err(e: hidapi::HidError) -> Error {
    Error::Transport(e.to_string())
}

impl HidapiTransport {
    pub fn open(path: Option<&str>) -> Result<Self> {
        let api = HidApi::new().map_err(hid_err)?;
        let path = match path {
            Some(p) => CString::new(p).map_err(|_| Error::Transport(tr!("transport.invalid_device_path")))?,
            None => api
                .device_list()
                .find(|d| d.vendor_id() == VID && d.product_id() == PID && d.interface_number() == INTERFACE)
                .map(|d| d.path().to_owned())
                .ok_or_else(not_found)?,
        };
        let dev = api.open_path(&path).map_err(hid_err)?;
        let name: Vec<u16> = MUTEX_NAME.encode_utf16().chain(Some(0)).collect();
        // SAFETY: valid, null-terminated name; the handle is closed in Drop.
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if mutex.is_null() {
            return Err(Error::Transport(tr!("transport.lock_create_failed", error = std::io::Error::last_os_error())));
        }
        Ok(Self { dev, _api: api, path: path.to_string_lossy().into_owned(), mutex, locked: false, strings: None })
    }
}

impl Transport for HidapiTransport {
    fn set_feature(&mut self, data: &[u8; REPORT_LEN]) -> Result<()> {
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..].copy_from_slice(data);
        self.dev.send_feature_report(&buf).map_err(|e| Error::Transport(tr!("transport.send_failed", e = e)))
    }

    fn get_feature(&mut self) -> Result<[u8; REPORT_LEN]> {
        let mut buf = [0u8; REPORT_LEN + 1];
        let n = self.dev.get_feature_report(&mut buf).map_err(hid_err)?;
        strip_report_id(&buf[..n.min(buf.len())])
    }

    fn lock(&mut self) -> Result<()> {
        // SAFETY: handle from CreateMutexW, lives as long as self.
        match unsafe { WaitForSingleObject(self.mutex, LOCK_TIMEOUT_MS) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => {
                self.locked = true;
                Ok(())
            }
            WAIT_TIMEOUT => Err(Error::Device(tr!("transport.busy_elsewhere"))),
            _ => Err(Error::Transport(tr!("transport.lock_failed", error = std::io::Error::last_os_error()))),
        }
    }

    fn unlock(&mut self) {
        if self.locked {
            // SAFETY: this thread owns the mutex (lock succeeded).
            unsafe { ReleaseMutex(self.mutex) };
            self.locked = false;
        }
    }

    fn path(&self) -> String {
        self.path.clone()
    }

    fn string_descriptor(&mut self, index: u8) -> Result<Vec<u8>> {
        let handle = self.string_handle()?;
        let mut buf = vec![0u8; STRING_BUFFER];
        // SAFETY: buffer and length match; the handle lives as long as self.
        let ok =
            unsafe { HidD_GetIndexedString(handle, u32::from(index), buf.as_mut_ptr().cast(), STRING_BUFFER as u32) };
        if !ok {
            let e = std::io::Error::last_os_error();
            return Err(Error::Transport(tr!("transport.string_failed", index = index, e = e)));
        }
        Ok(buf)
    }

    fn serial_number(&mut self) -> Result<Vec<u8>> {
        let handle = self.string_handle()?;
        let mut buf = vec![0u8; STRING_BUFFER];
        // SAFETY: as above.
        let ok = unsafe { HidD_GetSerialNumberString(handle, buf.as_mut_ptr().cast(), STRING_BUFFER as u32) };
        if !ok {
            return Err(Error::Transport(tr!("transport.serial_failed", e = std::io::Error::last_os_error())));
        }
        Ok(buf)
    }
}

impl HidapiTransport {
    /// Handle for the string requests like Tilt.exe opens it (`0x405200`: no access rights, overlapped). All requests
    /// of the tilt mode switch go through the same handle, as in Tilt.exe; it stays open until the transport is dropped.
    fn string_handle(&mut self) -> Result<HANDLE> {
        if let Some(h) = self.strings {
            return Ok(h);
        }
        let path: Vec<u16> = self.path.encode_utf16().chain(Some(0)).collect();
        // SAFETY: null-terminated path; the handle is closed in Drop.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(Error::Transport(tr!("transport.string_open_failed", e = std::io::Error::last_os_error())));
        }
        self.strings = Some(handle);
        Ok(handle)
    }
}

impl Drop for HidapiTransport {
    fn drop(&mut self) {
        self.unlock();
        // SAFETY: the handles are ours.
        unsafe {
            CloseHandle(self.mutex);
            if let Some(h) = self.strings {
                CloseHandle(h);
            }
        }
    }
}
