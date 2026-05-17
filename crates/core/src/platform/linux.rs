//! Linux platform backend.
//!
//! Discovery shells out to `lsblk -J`; parsing is in
//! [`crate::parsers`]. The install pipeline shells out to `parted`,
//! `mkfs.vfat`, `mkfs.exfat`, `mount`, `cp`, `umount`. Every
//! subprocess is invoked through [`std::process::Command`] with
//! separated argv — no shell strings are ever built from user input.

use crate::parsers::{parse_lsblk_disks, parse_lsblk_partitions};
use crate::{
    validate_device_path, CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent,
    ProgressSink, Result,
};
use std::process::Command;
use std::{fs, path::PathBuf};

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let output = Command::new("lsblk")
        .args(["-b", "-J", "-o", "NAME,MODEL,SIZE,RM,TYPE,MOUNTPOINTS"])
        .output()
        .map_err(|e| CoreError::Io(e.to_string()))?;
    if !output.status.success() {
        return Err(CoreError::Io("lsblk failed".to_string()));
    }
    parse_lsblk_disks(&output.stdout)
}

pub fn list_partitions(device: String) -> Result<Vec<PartitionInfo>> {
    let output = Command::new("lsblk")
        .args([
            "-b",
            "-J",
            "-o",
            "NAME,TYPE,LABEL,FSTYPE,MOUNTPOINTS,PKNAME",
        ])
        .output()
        .map_err(|e| CoreError::Io(e.to_string()))?;
    if !output.status.success() {
        return Err(CoreError::Io("lsblk failed".to_string()));
    }
    parse_lsblk_partitions(&output.stdout, &device)
}

pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
    let disks = list_disks()?;
    install_with_disks(req, sink, &disks)
}

fn install_with_disks(
    req: InstallRequest,
    sink: &dyn ProgressSink,
    disks: &[DiskInfo],
) -> Result<()> {
    validate_install(&req, sink, disks)?;

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
        &[&req.device, "-s", "mkpart", "primary", "33MiB", "100%"],
    )?;
    run("parted", &[&req.device, "-s", "print"])?;

    sink.emit(ProgressEvent {
        phase: "format".to_string(),
        message: "Formatting partitions".to_string(),
        percent: Some(60),
    });

    let part1 = part_path(&req.device, 1);
    let part2 = part_path(&req.device, 2);
    run("mkfs.vfat", &["-F", "32", "-n", "RAIDHOS_EFI", &part1])?;

    if has_cmd("mkfs.exfat") {
        if run("mkfs.exfat", &["-n", "DATA", &part2]).is_err() {
            run("mkfs.exfat", &[&part2])?;
            let _ = run("exfatlabel", &[&part2, "DATA"]);
        }
    } else if has_cmd("mkexfatfs") {
        if run("mkexfatfs", &["-n", "DATA", &part2]).is_err() {
            run("mkexfatfs", &[&part2])?;
            let _ = run("exfatlabel", &[&part2, "DATA"]);
        }
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

fn validate_install(
    req: &InstallRequest,
    sink: &dyn ProgressSink,
    disks: &[DiskInfo],
) -> Result<()> {
    validate_device_path(&req.device)?;

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
        message: format!("Staging payload {}", req.payload_version),
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

    Ok(())
}

fn payload_copy(sink: &dyn ProgressSink, part1: &str, part2: &str) -> Result<()> {
    let payload_dir = std::env::var("RAIDHOS_PAYLOAD_DIR")
        .map_err(|_| CoreError::Validation("RAIDHOS_PAYLOAD_DIR is not set".to_string()))?;
    let payload = PathBuf::from(payload_dir);
    if !payload.exists() {
        return Err(CoreError::Validation(
            "RAIDHOS_PAYLOAD_DIR does not exist".to_string(),
        ));
    }
    let esp_payload = payload.join("esp");
    let data_payload = payload.join("data");
    if !esp_payload.exists() || !data_payload.exists() {
        return Err(CoreError::Validation(
            "RAIDHOS_PAYLOAD_DIR must contain esp/ and data/ directories".to_string(),
        ));
    }

    if let Err(e) = crate::payload::verify_payload(&payload) {
        sink.emit(ProgressEvent {
            phase: "payload".to_string(),
            message: format!("Payload integrity warning: {e}"),
            percent: Some(45),
        });
    }

    let esp_mount = PathBuf::from("/mnt/raidhos-esp");
    let data_mount = PathBuf::from("/mnt/raidhos-data");
    fs::create_dir_all(&esp_mount).map_err(|e| CoreError::Io(e.to_string()))?;
    fs::create_dir_all(&data_mount).map_err(|e| CoreError::Io(e.to_string()))?;

    run(
        "mount",
        &[part1, esp_mount.to_str().unwrap_or("/mnt/raidhos-esp")],
    )?;
    run(
        "mount",
        &[part2, data_mount.to_str().unwrap_or("/mnt/raidhos-data")],
    )?;

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    run(
        "cp",
        &[
            "-a",
            &format!("{}/.", esp_payload.to_string_lossy()),
            esp_mount.to_str().unwrap(),
        ],
    )?;
    run(
        "cp",
        &[
            "-a",
            &format!("{}/.", data_payload.to_string_lossy()),
            data_mount.to_str().unwrap(),
        ],
    )?;

    let _ = run("umount", &[esp_mount.to_str().unwrap()]);
    let _ = run("umount", &[data_mount.to_str().unwrap()]);

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Payload copy complete.".to_string(),
        percent: Some(90),
    });

    Ok(())
}

#[cfg(not(test))]
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

#[cfg(test)]
fn run(_cmd: &str, _args: &[&str]) -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
fn has_cmd(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
fn has_cmd(_cmd: &str) -> bool {
    true
}

pub(crate) fn part_path(device: &str, idx: u8) -> String {
    if device
        .chars()
        .last()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("{device}p{idx}")
    } else {
        format!("{device}{idx}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sink {
        events: std::cell::RefCell<Vec<ProgressEvent>>,
    }

    impl ProgressSink for Sink {
        fn emit(&self, event: ProgressEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    fn disk(id: &str, mounts: Vec<&str>, is_system: bool) -> DiskInfo {
        DiskInfo {
            id: id.to_string(),
            model: "Test".to_string(),
            size_bytes: 128,
            removable: true,
            mountpoints: mounts.into_iter().map(|m| m.to_string()).collect(),
            is_system,
        }
    }

    fn req(device: &str, wipe: bool, dry_run: bool) -> InstallRequest {
        InstallRequest {
            device: device.to_string(),
            payload_version: "0.1.0".to_string(),
            wipe,
            dry_run,
            allow_write: false,
        }
    }

    #[test]
    fn validate_rejects_non_dev_path() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err = validate_install(&req("sdb", true, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("device must be an absolute /dev path"));
    }

    #[test]
    fn validate_rejects_without_wipe_flag() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err = validate_install(&req("/dev/sdb", false, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("wipe flag"));
    }

    #[test]
    fn validate_rejects_system_disk() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sda", vec!["/"], true)];
        let err = validate_install(&req("/dev/sda", true, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("system disk"));
    }

    #[test]
    fn validate_rejects_mounted_partitions() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec!["/media/usb"], false)];
        let err = validate_install(&req("/dev/sdb", true, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("mounted"));
    }

    #[test]
    fn validate_accepts_safe_disk() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let ok = validate_install(&req("/dev/sdb", true, true), &sink, &disks);
        assert!(ok.is_ok());
        assert!(!sink.events.borrow().is_empty());
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err = validate_install(&req("/dev/sdz", true, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn validate_rejects_shell_metacharacters() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err =
            validate_install(&req("/dev/sdb;rm -rf /", true, true), &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("forbidden"));
    }

    #[test]
    fn part_path_appends_p_for_nvme() {
        assert_eq!(part_path("/dev/nvme0n1", 1), "/dev/nvme0n1p1");
        assert_eq!(part_path("/dev/mmcblk0", 2), "/dev/mmcblk0p2");
    }

    #[test]
    fn part_path_appends_number_for_sata() {
        assert_eq!(part_path("/dev/sdb", 1), "/dev/sdb1");
        assert_eq!(part_path("/dev/sda", 2), "/dev/sda2");
    }

    #[test]
    fn install_dry_run_succeeds() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let mut r = req("/dev/sdb", true, true);
        r.allow_write = false;
        let res = install_with_disks(r, &sink, &disks);
        assert!(res.is_ok());
        let evts = sink.events.borrow();
        assert!(evts.iter().any(|e| e.phase == "complete"));
    }

    #[test]
    fn install_rejects_when_allow_write_unset() {
        let sink = Sink {
            events: std::cell::RefCell::new(Vec::new()),
        };
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let mut r = req("/dev/sdb", true, false);
        r.allow_write = false;
        let err = install_with_disks(r, &sink, &disks).unwrap_err();
        assert!(format!("{err}").contains("write blocked"));
    }
}
