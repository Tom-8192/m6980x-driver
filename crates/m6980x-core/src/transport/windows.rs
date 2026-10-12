//! Windows: hidapi fuer die Feature-Reports, benannter Mutex als Sperre zwischen Prozessen.

use std::ffi::CString;

use hidapi::{HidApi, HidDevice};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

use super::{INTERFACE, LOCK_TIMEOUT_MS, PID, REPORT_LEN, Transport, VID, not_found, strip_report_id};
use crate::{Error, Result, tr};

const MUTEX_NAME: &str = "Local\\m6980x_configtool";

pub struct HidapiTransport {
    dev: HidDevice,
    _api: HidApi,
    path: String,
    mutex: HANDLE,
    locked: bool,
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
        // SAFETY: gueltiger, nullterminierter Name; das Handle wird in Drop geschlossen.
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if mutex.is_null() {
            return Err(Error::Transport(tr!("transport.lock_create_failed", error = std::io::Error::last_os_error())));
        }
        Ok(Self { dev, _api: api, path: path.to_string_lossy().into_owned(), mutex, locked: false })
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
        // SAFETY: Handle aus CreateMutexW, lebt so lange wie self.
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
            // SAFETY: dieser Thread besitzt den Mutex (lock war erfolgreich).
            unsafe { ReleaseMutex(self.mutex) };
            self.locked = false;
        }
    }

    fn path(&self) -> String {
        self.path.clone()
    }
}

impl Drop for HidapiTransport {
    fn drop(&mut self) {
        self.unlock();
        // SAFETY: Handle gehoert uns.
        unsafe { CloseHandle(self.mutex) };
    }
}
