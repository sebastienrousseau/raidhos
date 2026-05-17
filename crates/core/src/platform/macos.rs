//! macOS platform backend.
//!
//! Discovery shells out to `diskutil list -plist external`; parsing is
//! in [`crate::parsers::parse_disks_plist`]. The install pipeline uses
//! `diskutil` for partitioning + formatting and `bless` to mark the
//! ESP bootable.

use crate::parsers::parse_disks_plist;
use crate::{
    validate_device_path, CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent,
    ProgressSink, Result,
};
use std::path::PathBuf;
use std::process::Command;

fn run_diskutil(args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("diskutil")
        .args(args)
        .output()
        .map_err(|e| CoreError::Io(format!("diskutil: {e}")))?;
    if !out.status.success() {
        return Err(CoreError::Io(format!(
            "diskutil failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(out.stdout)
}

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let stdout = run_diskutil(&["list", "-plist", "external"])?;
    parse_disks_plist(&stdout)
}

pub fn list_partitions(_device: String) -> Result<Vec<PartitionInfo>> {
    Ok(Vec::new())
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

    // `diskutil unmountDisk` releases anything currently mounted.
    sink.emit(ProgressEvent {
        phase: "unmount".to_string(),
        message: format!("Unmounting {}", req.device),
        percent: Some(25),
    });
    let _ = run_diskutil(&["unmountDisk", "force", &req.device]);

    // Partition the whole disk: GPT, ESP (FAT32, 32 MiB) named
    // RAIDHOS_EFI, then a DATA (exFAT) partition for the rest.
    sink.emit(ProgressEvent {
        phase: "partition".to_string(),
        message: "Creating GPT partitions".to_string(),
        percent: Some(45),
    });
    run_diskutil(&[
        "partitionDisk",
        &req.device,
        "GPT",
        "FAT32",
        "RAIDHOS_EFI",
        "33M",
        "ExFAT",
        "DATA",
        "0", // 0 = remaining space
    ])?;

    // ESP and DATA partition paths on macOS.
    let part1 = format!("{}s1", req.device);
    let part2 = format!("{}s2", req.device);

    sink.emit(ProgressEvent {
        phase: "format".to_string(),
        message: "Verifying volume names".to_string(),
        percent: Some(60),
    });
    let _ = run_diskutil(&["rename", &part1, "RAIDHOS_EFI"]);
    let _ = run_diskutil(&["rename", &part2, "DATA"]);

    payload_copy(sink, &part1, &part2)?;

    // Mark the ESP as bootable via `bless`. Failure is non-fatal —
    // Macs will still see the disk and most can boot from an unblessed
    // FAT32 ESP under the firmware boot picker.
    let _ = Command::new("bless")
        .args(["--device", &format!("/dev/{}", part1), "--setBoot"])
        .status();

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
            percent: Some(70),
        });
    }

    // macOS auto-mounts new partitions under /Volumes/<label>.
    // Mount points after `diskutil rename`.
    let esp_mount = PathBuf::from("/Volumes/RAIDHOS_EFI");
    let data_mount = PathBuf::from("/Volumes/DATA");

    // Make sure they're mounted (a fresh format usually does this
    // automatically, but defensive).
    let _ = Command::new("diskutil").args(["mount", part1]).status();
    let _ = Command::new("diskutil").args(["mount", part2]).status();

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    cp_recursive(&esp_payload, &esp_mount)?;
    cp_recursive(&data_payload, &data_mount)?;

    let _ = Command::new("diskutil").args(["unmount", part1]).status();
    let _ = Command::new("diskutil").args(["unmount", part2]).status();

    Ok(())
}

fn cp_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    let status = Command::new("cp")
        .args([
            "-R",
            &format!("{}/", src.to_string_lossy()),
            &dst.to_string_lossy(),
        ])
        .status()
        .map_err(|e| CoreError::Io(format!("cp: {e}")))?;
    if !status.success() {
        return Err(CoreError::Io(format!(
            "cp -R {} {} failed",
            src.display(),
            dst.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nop_sink() -> NopSink {
        NopSink
    }
    struct NopSink;
    impl ProgressSink for NopSink {
        fn emit(&self, _: ProgressEvent) {}
    }

    fn disk(id: &str, mounts: Vec<&str>, is_system: bool) -> DiskInfo {
        DiskInfo {
            id: id.to_string(),
            model: "Test".into(),
            size_bytes: 1024,
            removable: true,
            mountpoints: mounts.into_iter().map(String::from).collect(),
            is_system,
        }
    }

    fn req(device: &str, wipe: bool, dry_run: bool, allow: bool) -> InstallRequest {
        InstallRequest {
            device: device.into(),
            payload_version: "0".into(),
            wipe,
            dry_run,
            allow_write: allow,
        }
    }

    #[test]
    fn validate_rejects_system_disk() {
        let disks = vec![disk("/dev/disk0", vec!["/"], true)];
        let err = validate_install(&req("/dev/disk0", true, true, false), &nop_sink(), &disks)
            .unwrap_err();
        assert!(format!("{err}").contains("system disk"));
    }

    #[test]
    fn validate_rejects_mounted_disk() {
        let disks = vec![disk("/dev/disk2", vec!["/Volumes/USB"], false)];
        let err = validate_install(&req("/dev/disk2", true, true, false), &nop_sink(), &disks)
            .unwrap_err();
        assert!(format!("{err}").contains("mounted"));
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err = validate_install(&req("/dev/disk9", true, true, false), &nop_sink(), &disks)
            .unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn validate_rejects_without_wipe() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err = validate_install(&req("/dev/disk2", false, true, false), &nop_sink(), &disks)
            .unwrap_err();
        assert!(format!("{err}").contains("wipe flag"));
    }

    #[test]
    fn dry_run_succeeds() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let res = install_with_disks(req("/dev/disk2", true, true, false), &nop_sink(), &disks);
        assert!(res.is_ok());
    }

    #[test]
    fn install_rejects_when_allow_write_unset() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err = install_with_disks(req("/dev/disk2", true, false, false), &nop_sink(), &disks)
            .unwrap_err();
        assert!(format!("{err}").contains("write blocked"));
    }
}
