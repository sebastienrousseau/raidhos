//! macOS platform backend.
//!
//! Discovery shells out to `diskutil list -plist external`; parsing is
//! in [`crate::parsers::parse_disks_plist`]. The install pipeline uses
//! `diskutil` for partitioning + formatting and `bless` to mark the
//! ESP bootable.
//!
//! The module compiles on every host (so its Runtime-driven tests
//! run under tarpaulin) — only the public re-export in
//! [`crate::platform`] is gated by `target_os`.

#![allow(dead_code)]

use crate::parsers::parse_disks_plist;
use crate::runtime::{RealRuntime, Runtime};
use crate::{
    CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent, ProgressSink, Result,
};
use std::path::PathBuf;

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    list_disks_with(&RealRuntime)
}

pub(crate) fn list_disks_with(rt: &dyn Runtime) -> Result<Vec<DiskInfo>> {
    let bytes = rt.run_output("diskutil", &["list", "-plist", "external"])?;
    parse_disks_plist(&bytes)
}

pub fn list_partitions(_device: String) -> Result<Vec<PartitionInfo>> {
    Ok(Vec::new())
}

pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
    let rt = RealRuntime;
    let disks = list_disks_with(&rt)?;
    install_with(req, sink, &rt, &disks)
}

pub(crate) fn install_with(
    req: InstallRequest,
    sink: &dyn ProgressSink,
    rt: &dyn Runtime,
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
        phase: "unmount".to_string(),
        message: format!("Unmounting {}", req.device),
        percent: Some(25),
    });
    let _ = rt.run("diskutil", &["unmountDisk", "force", &req.device]);

    sink.emit(ProgressEvent {
        phase: "partition".to_string(),
        message: "Creating GPT partitions".to_string(),
        percent: Some(45),
    });
    rt.run(
        "diskutil",
        &[
            "partitionDisk",
            &req.device,
            "GPT",
            "FAT32",
            "RAIDHOS_EFI",
            "33M",
            "ExFAT",
            "DATA",
            "0",
        ],
    )?;

    let part1 = format!("{}s1", req.device);
    let part2 = format!("{}s2", req.device);

    sink.emit(ProgressEvent {
        phase: "format".to_string(),
        message: "Verifying volume names".to_string(),
        percent: Some(60),
    });
    let _ = rt.run("diskutil", &["rename", &part1, "RAIDHOS_EFI"]);
    let _ = rt.run("diskutil", &["rename", &part2, "DATA"]);

    payload_copy(sink, rt, &part1, &part2)?;

    // bless is non-fatal — Macs will still boot from an unblessed
    // FAT32 ESP via the firmware boot picker.
    let _ = rt.run(
        "bless",
        &["--device", &format!("/dev/{}", part1), "--setBoot"],
    );

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
    // Use the target-specific validator so this code path is unit-
    // testable from any host (Linux CI in particular).
    let target = if req.simulator { "simulator" } else { "macos" };
    crate::validate_device_path_for_target(&req.device, target)?;

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
    if req.simulator {
        // Simulator targets a regular file; not in the disk inventory.
        return Ok(());
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

fn payload_copy(sink: &dyn ProgressSink, rt: &dyn Runtime, part1: &str, part2: &str) -> Result<()> {
    let payload_dir = rt
        .env_var("RAIDHOS_PAYLOAD_DIR")
        .ok_or_else(|| CoreError::Validation("RAIDHOS_PAYLOAD_DIR is not set".to_string()))?;
    let payload = PathBuf::from(payload_dir);
    if !rt.path_exists(&payload) {
        return Err(CoreError::Validation(
            "RAIDHOS_PAYLOAD_DIR does not exist".to_string(),
        ));
    }
    let esp_payload = payload.join("esp");
    let data_payload = payload.join("data");
    if !rt.path_exists(&esp_payload) || !rt.path_exists(&data_payload) {
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

    let esp_mount = PathBuf::from("/Volumes/RAIDHOS_EFI");
    let data_mount = PathBuf::from("/Volumes/DATA");
    let _ = rt.run("diskutil", &["mount", part1]);
    let _ = rt.run("diskutil", &["mount", part2]);

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    cp_recursive(rt, &esp_payload, &esp_mount)?;
    cp_recursive(rt, &data_payload, &data_mount)?;

    let _ = rt.run("diskutil", &["unmount", part1]);
    let _ = rt.run("diskutil", &["unmount", part2]);

    Ok(())
}

fn cp_recursive(rt: &dyn Runtime, src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    rt.run(
        "cp",
        &[
            "-R",
            &format!("{}/", src.to_string_lossy()),
            &dst.to_string_lossy(),
        ],
    )
}

// macOS-shape device paths are only accepted by `validate_device_path`
// on macOS hosts; gate to keep these tests Mac-only.
// Tests run cross-platform: the install path's path-shape check now
// goes through `validate_device_path_for_target("macos", …)` which
// has no host dependency, so tarpaulin on Linux CI sees these tests
// execute against the macOS install pipeline through MockRuntime.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{MockOutcome, MockRuntime};

    struct NopSink;
    impl ProgressSink for NopSink {
        fn emit(&self, _: ProgressEvent) {}
    }

    fn disk(id: &str, mounts: Vec<&str>, is_system: bool) -> DiskInfo {
        DiskInfo {
            id: id.into(),
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
            simulator: false,
            bios_compat: false,
        }
    }

    fn payload_dir_mock(esp: bool, data: bool) -> MockRuntime {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-macos-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut rt = MockRuntime::new();
        rt.env
            .push(("RAIDHOS_PAYLOAD_DIR".into(), tmp.display().to_string()));
        rt.existing_paths.push(tmp.clone());
        if esp {
            rt.existing_paths.push(tmp.join("esp"));
        }
        if data {
            rt.existing_paths.push(tmp.join("data"));
        }
        rt
    }

    #[test]
    fn validate_rejects_system_disk() {
        let disks = vec![disk("/dev/disk0", vec!["/"], true)];
        let err =
            validate_install(&req("/dev/disk0", true, true, false), &NopSink, &disks).unwrap_err();
        assert!(format!("{err}").contains("system disk"));
    }

    #[test]
    fn validate_rejects_mounted_disk() {
        let disks = vec![disk("/dev/disk2", vec!["/Volumes/USB"], false)];
        let err =
            validate_install(&req("/dev/disk2", true, true, false), &NopSink, &disks).unwrap_err();
        assert!(format!("{err}").contains("mounted"));
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            validate_install(&req("/dev/disk9", true, true, false), &NopSink, &disks).unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn validate_rejects_without_wipe() {
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            validate_install(&req("/dev/disk2", false, true, false), &NopSink, &disks).unwrap_err();
        assert!(format!("{err}").contains("wipe flag"));
    }

    #[test]
    fn dry_run_succeeds() {
        let rt = MockRuntime::new();
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let res = install_with(req("/dev/disk2", true, true, false), &NopSink, &rt, &disks);
        assert!(res.is_ok());
    }

    #[test]
    fn install_rejects_when_allow_write_unset() {
        let rt = MockRuntime::new();
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            install_with(req("/dev/disk2", true, false, false), &NopSink, &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("write blocked"));
    }

    #[test]
    fn install_full_pipeline_against_mock() {
        let rt = payload_dir_mock(true, true);
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let res = install_with(req("/dev/disk2", true, false, true), &NopSink, &rt, &disks);
        assert!(res.is_ok(), "install failed: {res:?}");
        let inv = rt.invocations.borrow();
        assert!(inv
            .iter()
            .any(|i| i.cmd == "diskutil" && i.args.iter().any(|a| a == "partitionDisk")));
        assert!(inv.iter().any(|i| i.cmd == "cp"));
        assert!(inv.iter().any(|i| i.cmd == "bless"));
    }

    #[test]
    fn install_propagates_partition_failure() {
        let mut rt = payload_dir_mock(true, true);
        // First mock call is `diskutil unmountDisk` (non-fatal),
        // second is `diskutil partitionDisk` — return Err.
        rt.outcomes = std::cell::RefCell::new(vec![
            MockOutcome::Ok(vec![]),                   // unmountDisk (non-fatal)
            MockOutcome::Err("partition boom".into()), // partitionDisk
        ]);
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            install_with(req("/dev/disk2", true, false, true), &NopSink, &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("partition boom"));
    }

    #[test]
    fn payload_copy_errors_without_env_var() {
        let rt = MockRuntime::new();
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            install_with(req("/dev/disk2", true, false, true), &NopSink, &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("RAIDHOS_PAYLOAD_DIR is not set"));
    }

    #[test]
    fn payload_copy_errors_with_missing_subdirs() {
        let rt = payload_dir_mock(true, false); // missing data/
        let disks = vec![disk("/dev/disk2", vec![], false)];
        let err =
            install_with(req("/dev/disk2", true, false, true), &NopSink, &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("must contain esp/ and data/"));
    }

    #[test]
    fn list_disks_with_mock_parses_plist() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(
            br#"<plist><array><dict><key>DeviceIdentifier</key><string>disk2</string><key>Size</key><integer>16000000000</integer><key>Removable</key><true/><key>Internal</key><false/></dict></array></plist>"#.to_vec()));
        let disks = list_disks_with(&rt).unwrap();
        assert!(disks.iter().any(|d| d.id == "/dev/disk2"));
    }

    #[test]
    fn list_disks_with_mock_propagates_io_error() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Err("diskutil: not found".into()));
        let err = list_disks_with(&rt).unwrap_err();
        assert!(format!("{err}").contains("diskutil"));
    }

    #[test]
    fn list_partitions_returns_empty() {
        // macOS returns empty for now (per implementation note).
        assert!(list_partitions("/dev/disk2".into()).unwrap().is_empty());
    }
}
