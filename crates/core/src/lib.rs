//! RaidhOS core library.
//!
//! Provides disk discovery, safety checks, and installation orchestration.

use std::fmt;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug)]
pub enum CoreError {
    UnsupportedPlatform,
    Io(String),
    Validation(String),
    NotImplemented(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::UnsupportedPlatform => write!(f, "unsupported platform"),
            CoreError::Io(msg) => write!(f, "io error: {msg}"),
            CoreError::Validation(msg) => write!(f, "validation error: {msg}"),
            CoreError::NotImplemented(msg) => write!(f, "not implemented: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}

#[derive(Clone, Debug)]
pub struct DiskInfo {
    pub id: String,
    pub model: String,
    pub size_bytes: u64,
    pub removable: bool,
}

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub device: String,
    pub payload_version: String,
    pub wipe: bool,
}

#[derive(Clone, Debug)]
pub struct ProgressEvent {
    pub phase: String,
    pub message: String,
    pub percent: Option<u8>,
}

pub trait ProgressSink {
    fn emit(&self, event: ProgressEvent);
}

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    Err(CoreError::NotImplemented(
        "disk discovery not implemented yet".to_string(),
    ))
}

pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
    Err(CoreError::NotImplemented(
        "install not implemented yet".to_string(),
    ))
}

#[cfg(target_os = "linux")]
mod platform {
    // TODO: Linux disk discovery and installer implementation.
}

#[cfg(target_os = "macos")]
mod platform {
    // TODO: macOS disk discovery and installer implementation.
}

#[cfg(target_os = "windows")]
mod platform {
    // TODO: Windows disk discovery and installer implementation.
}
