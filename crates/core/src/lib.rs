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
    pub mountpoints: Vec<String>,
    pub is_system: bool,
}

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub device: String,
    pub payload_version: String,
    pub wipe: bool,
    pub dry_run: bool,
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

pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
    platform::install(req, sink)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{CoreError, DiskInfo, InstallRequest, ProgressEvent, ProgressSink, Result};
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
        #[serde(default)]
        mountpoints: Option<Vec<Option<String>>>,
        #[serde(default)]
        children: Option<Vec<LsblkDevice>>,
    }

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        let output = Command::new("lsblk")
            .args(["-b", "-J", "-o", "NAME,MODEL,SIZE,RM,TYPE,MOUNTPOINTS"])
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

            let mut mounts = Vec::new();
            collect_mounts(&dev, &mut mounts);
            let is_system = mounts.iter().any(|m| m == "/" || m == "/boot" || m == "/boot/efi");

            disks.push(DiskInfo {
                id: format!("/dev/{}", dev.name),
                model: dev.model.unwrap_or_else(|| "Unknown".to_string()),
                size_bytes,
                removable: dev.rm.unwrap_or(false),
                mountpoints: mounts,
                is_system,
            });
        }

        Ok(disks)
    }

    fn collect_mounts(dev: &LsblkDevice, mounts: &mut Vec<String>) {
        if let Some(mps) = &dev.mountpoints {
            for mp in mps.iter().flatten() {
                if !mp.is_empty() {
                    mounts.push(mp.clone());
                }
            }
        }
        if let Some(children) = &dev.children {
            for child in children {
                collect_mounts(child, mounts);
            }
        }
    }

    pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
        if !req.device.starts_with("/dev/") {
            return Err(CoreError::Validation(
                "device must be an absolute /dev path".to_string(),
            ));
        }

        sink.emit(ProgressEvent {
            phase: "validate".to_string(),
            message: format!("Validating target {}", req.device),
            percent: Some(5),
        });

        if !req.wipe {
            return Err(CoreError::Validation(
                "wipe flag must be set for destructive install".to_string(),
            ));
        }

        let disks = list_disks()?;
        let target = disks
            .iter()
            .find(|d| d.id == req.device)
            .ok_or_else(|| CoreError::Validation("device not found".to_string()))?;

        if target.is_system {
            return Err(CoreError::Validation(
                "refusing to operate on system disk".to_string(),
            ));
        }

        if !target.mountpoints.is_empty() {
            return Err(CoreError::Validation(
                "device has mounted partitions; unmount first".to_string(),
            ));
        }

        sink.emit(ProgressEvent {
            phase: "prepare".to_string(),
            message: "Preparing partition layout".to_string(),
            percent: Some(20),
        });

        sink.emit(ProgressEvent {
            phase: "payload".to_string(),
            message: format!("Staging Ventoy payload {}", req.payload_version),
            percent: Some(45),
        });

        sink.emit(ProgressEvent {
            phase: "write".to_string(),
            message: "Writing boot structures".to_string(),
            percent: Some(70),
        });

        sink.emit(ProgressEvent {
            phase: "finalize".to_string(),
            message: "Final checks".to_string(),
            percent: Some(90),
        });

        if req.dry_run {
            sink.emit(ProgressEvent {
                phase: "complete".to_string(),
                message: "Dry-run complete. No changes made.".to_string(),
                percent: Some(100),
            });
            return Ok(());
        }

        Err(CoreError::NotImplemented(
            "installer not wired yet; use dry_run".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{CoreError, DiskInfo, InstallRequest, ProgressSink, Result};

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        Err(CoreError::NotImplemented(
            "macOS disk discovery not implemented yet".to_string(),
        ))
    }

    pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
        Err(CoreError::NotImplemented(
            "macOS installer not implemented yet".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{CoreError, DiskInfo, InstallRequest, ProgressSink, Result};

    pub fn list_disks() -> Result<Vec<DiskInfo>> {
        Err(CoreError::NotImplemented(
            "Windows disk discovery not implemented yet".to_string(),
        ))
    }

    pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
        Err(CoreError::NotImplemented(
            "Windows installer not implemented yet".to_string(),
        ))
    }
}
