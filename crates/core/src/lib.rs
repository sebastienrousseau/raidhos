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
    pub allow_write: bool,
}

#[derive(Clone, Debug)]
pub struct ProgressEvent {
    pub phase: String,
    pub message: String,
    pub percent: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct IsoEntry {
    pub title: String,
    pub path: String,
    pub size_bytes: u64,
    pub params: String,
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

pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>> {
    platform::scan_isos(dirs)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{CoreError, DiskInfo, InstallRequest, ProgressEvent, ProgressSink, Result};
    use serde::Deserialize;
    use std::process::Command;
    use std::{fs, path::PathBuf};

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
        if !req.allow_write {
            return Err(CoreError::Validation(
                "write blocked: set allow_write to proceed".to_string(),
            ));
        }

        sink.emit(ProgressEvent {
            phase: "partition".to_string(),
            message: "Creating GPT partitions".to_string(),
            percent: Some(30),
        });

        run("parted", &[&req.device, "-s", "mklabel", "gpt"])?;
        run(
            "parted",
            &[
                &req.device,
                "-s",
                "mkpart",
                "primary",
                "fat32",
                "1MiB",
                "33MiB",
            ],
        )?;
        run("parted", &[&req.device, "-s", "set", "1", "esp", "on"])?;
        run(
            "parted",
            &[
                &req.device,
                "-s",
                "mkpart",
                "primary",
                "33MiB",
                "100%",
            ],
        )?;
        run("parted", &[&req.device, "-s", "print"])?;

        sink.emit(ProgressEvent {
            phase: "format".to_string(),
            message: "Formatting partitions".to_string(),
            percent: Some(60),
        });

        let part1 = part_path(&req.device, 1);
        let part2 = part_path(&req.device, 2);
        run("mkfs.vfat", &["-F", "32", &part1])?;

        if has_cmd("mkfs.exfat") {
            run("mkfs.exfat", &[&part2])?;
        } else if has_cmd("mkexfatfs") {
            run("mkexfatfs", &[&part2])?;
        } else {
            return Err(CoreError::Io(
                "exFAT formatter not found (mkfs.exfat or mkexfatfs)".to_string(),
            ));
        }

        payload_copy(sink, &part1, &part2)?;

        sink.emit(ProgressEvent {
            phase: "complete".to_string(),
            message: "Install complete.".to_string(),
            percent: Some(100),
        });
        Ok(())
    }

    fn payload_copy(sink: &dyn ProgressSink, part1: &str, part2: &str) -> Result<()> {
        let payload_dir = std::env::var("RAIDHOS_PAYLOAD_DIR").ok();
        let Some(payload_dir) = payload_dir else {
            sink.emit(ProgressEvent {
                phase: "payload".to_string(),
                message: "Payload copy skipped (RAIDHOS_PAYLOAD_DIR not set).".to_string(),
                percent: Some(85),
            });
            return Ok(());
        };
        let payload = PathBuf::from(payload_dir);
        if !payload.exists() {
            sink.emit(ProgressEvent {
                phase: "payload".to_string(),
                message: "Payload copy skipped (directory not found).".to_string(),
                percent: Some(85),
            });
            return Ok(());
        }

        let esp_mount = PathBuf::from("/mnt/raidhos-esp");
        let data_mount = PathBuf::from("/mnt/raidhos-data");
        fs::create_dir_all(&esp_mount).map_err(|e| CoreError::Io(e.to_string()))?;
        fs::create_dir_all(&data_mount).map_err(|e| CoreError::Io(e.to_string()))?;

        run("mount", &[part1, esp_mount.to_str().unwrap_or("/mnt/raidhos-esp")])?;
        run("mount", &[part2, data_mount.to_str().unwrap_or("/mnt/raidhos-data")])?;

        sink.emit(ProgressEvent {
            phase: "payload".to_string(),
            message: "Copying payload files".to_string(),
            percent: Some(85),
        });

        let esp_payload = payload.join("esp");
        let data_payload = payload.join("data");
        if esp_payload.exists() {
            run("cp", &["-a", esp_payload.to_str().unwrap(), esp_mount.to_str().unwrap()])?;
        }
        if data_payload.exists() {
            run("cp", &["-a", data_payload.to_str().unwrap(), data_mount.to_str().unwrap()])?;
        } else {
            run("cp", &["-a", payload.to_str().unwrap(), data_mount.to_str().unwrap()])?;
        }

        let _ = run("umount", &[esp_mount.to_str().unwrap()]);
        let _ = run("umount", &[data_mount.to_str().unwrap()]);

        sink.emit(ProgressEvent {
            phase: "payload".to_string(),
            message: "Payload copy complete.".to_string(),
            percent: Some(90),
        });

        Ok(())
    }

    pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<super::IsoEntry>> {
        let mut results = Vec::new();
        for dir in dirs {
            let root = PathBuf::from(dir);
            if !root.exists() {
                continue;
            }
            let entries = fs::read_dir(&root).map_err(|e| CoreError::Io(e.to_string()))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    push_iso(&mut results, &path);
                } else if path.is_dir() {
                    if let Ok(subs) = fs::read_dir(&path) {
                        for sub in subs.flatten() {
                            let subpath = sub.path();
                            if subpath.is_file() {
                                push_iso(&mut results, &subpath);
                            }
                        }
                    }
                }
            }
        }
        results.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        Ok(results)
    }

    fn push_iso(results: &mut Vec<super::IsoEntry>, path: &PathBuf) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("iso") {
                if let Ok(meta) = fs::metadata(path) {
                    let title = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("ISO")
                        .to_string();
                    results.push(super::IsoEntry {
                        title,
                        path: path.display().to_string(),
                        size_bytes: meta.len(),
                        params: "quiet splash".to_string(),
                    });
                }
            }
        }
    }

    fn run(cmd: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(cmd)
            .args(args)
            .status()
            .map_err(|e| CoreError::Io(e.to_string()))?;
        if !status.success() {
            return Err(CoreError::Io(format!("command failed: {cmd}")));
        }
        Ok(())
    }

    fn has_cmd(cmd: &str) -> bool {
        Command::new("sh")
            .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn part_path(device: &str, idx: u8) -> String {
        if device.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            format!("{device}p{idx}")
        } else {
            format!("{device}{idx}")
        }
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

    pub fn scan_isos(_dirs: Vec<String>) -> Result<Vec<super::IsoEntry>> {
        Err(CoreError::NotImplemented(
            "macOS ISO scan not implemented yet".to_string(),
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

    pub fn scan_isos(_dirs: Vec<String>) -> Result<Vec<super::IsoEntry>> {
        Err(CoreError::NotImplemented(
            "Windows ISO scan not implemented yet".to_string(),
        ))
    }
}
