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
    Parse(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::UnsupportedPlatform => write!(f, "unsupported platform"),
            CoreError::Io(msg) => write!(f, "io error: {msg}"),
            CoreError::Validation(msg) => write!(f, "validation error: {msg}"),
            CoreError::NotImplemented(msg) => write!(f, "not implemented: {msg}"),
            CoreError::Parse(msg) => write!(f, "parse error: {msg}"),
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
    platform::list_disks()
}

pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
    Err(CoreError::NotImplemented(
        "install not implemented yet".to_string(),
    ))
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{CoreError, DiskInfo, Result};
    use serde::Deserialize;
    use std::process::Command;

    #[derive(Deserialize)]
    struct LsblkOutput {
        blockdevices: Vec<LsblkDevice>,
    }

    #[derive(Deserialize)]
    struct LsblkDevice {
        name: String,
        #[serde(default)]
        size: Option<String>,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        rm: Option<bool>,
        #[serde(default)]
        #[allow(dead_code)]
        type_field: Option<String>,
    }

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        let output = Command::new("lsblk")
            .args(["-b", "-J", "-o", "NAME,MODEL,SIZE,RM,TYPE"]) 
            .output()
            .map_err(|e| CoreError::Io(e.to_string()))?;

        if !output.status.success() {
            return Err(CoreError::Io("lsblk failed".to_string()));
        }

        let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| CoreError::Parse(e.to_string()))?;

        let mut disks = Vec::new();

        for dev in parsed.blockdevices {
            let is_disk = dev.type_field.as_deref() == Some("disk");
            if !is_disk {
                continue;
            }
            let size_bytes = dev
                .size
                .as_deref()
                .unwrap_or("0")
                .parse::<u64>()
                .unwrap_or(0);

            disks.push(DiskInfo {
                id: format!("/dev/{}", dev.name),
                model: dev.model.unwrap_or_else(|| "Unknown".to_string()),
                size_bytes,
                removable: dev.rm.unwrap_or(false),
            });
        }

        Ok(disks)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{CoreError, DiskInfo, Result};

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        Err(CoreError::NotImplemented(
            "macOS disk discovery not implemented yet".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{CoreError, DiskInfo, Result};

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        Err(CoreError::NotImplemented(
            "Windows disk discovery not implemented yet".to_string(),
        ))
    }
}
