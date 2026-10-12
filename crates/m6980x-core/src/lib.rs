//! M6980X Configuration Tool - core library for the Gigabyte GHOST M6980X (USB `E0FF:0001`).
//!
//! Port of the Python prototype (`prototype/python/m6980x_configtool`); the evidence for every command
//! is in `docs/PROTOCOL.md`.

pub mod backup;
pub mod functions;
pub mod i18n;
pub mod macro_steps;
pub mod names;
pub mod protocol;
pub mod rom;
pub mod transport;

#[cfg(any(test, feature = "fake"))]
pub mod fake;

pub use protocol::{DpiStage, Identify, M6980X};
pub use transport::{Transport, open_transport};

/// Error kinds as in the prototype: `Device` = M6980XError, `Transport` = TransportError, `Invalid` = ValueError.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Device(String),
    #[error("{0}")]
    Transport(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(msg.into()))
}

/// Bytes as "0c 12 09" (like Python `bytes.hex(" ")`).
pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod tests;
