//! Linux platform backend.
//!
//! Discovery shells out to `lsblk -J`; parsing is in
//! [`crate::parsers`]. The install pipeline shells out to `parted`,
//! `mkfs.vfat`, `mkfs.exfat`, `mount`, `cp`, `umount`. Every
//! subprocess goes through the [`Runtime`] trait so the pipeline is
//! fully unit-testable.

// The module compiles on every host so tests run under tarpaulin
// regardless of the runner's OS.
#![allow(dead_code)]

use crate::parsers::{parse_lsblk_disks, parse_lsblk_partitions};
use crate::runtime::{RealRuntime, Runtime};
use crate::{
    CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent, ProgressSink, Result,
};

/// Discovery wrapper using the real runtime.
pub fn list_disks() -> Result<Vec<DiskInfo>> {
    list_disks_with(&RealRuntime)
}

pub(crate) fn list_disks_with(rt: &dyn Runtime) -> Result<Vec<DiskInfo>> {
    let bytes = rt.run_output(
        "lsblk",
        &["-b", "-J", "-o", "NAME,MODEL,SIZE,RM,TYPE,MOUNTPOINTS"],
    )?;
    parse_lsblk_disks(&bytes)
}

/// Partition enumeration via `lsblk`.
pub fn list_partitions(device: String) -> Result<Vec<PartitionInfo>> {
    list_partitions_with(&RealRuntime, &device)
}

pub(crate) fn list_partitions_with(rt: &dyn Runtime, device: &str) -> Result<Vec<PartitionInfo>> {
    let bytes = rt.run_output(
        "lsblk",
        &[
            "-b",
            "-J",
            "-o",
            "NAME,TYPE,LABEL,FSTYPE,MOUNTPOINTS,PKNAME",
        ],
    )?;
    parse_lsblk_partitions(&bytes, device)
}

/// Run the install pipeline using the real runtime.
pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
    let rt = RealRuntime;
    let disks = list_disks_with(&rt)?;
    install_with(req, sink, &rt, &disks)
}

/// Test-visible install entry point. `disks` is the already-resolved
/// disk inventory (so unit tests can pass synthetic disks without
/// running `lsblk`).
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
            message: if req.bios_compat {
                "Dry-run complete. Hybrid BIOS+UEFI layout will be requested.".to_string()
            } else {
                "Dry-run complete. No changes made.".to_string()
            },
            percent: Some(100),
        });
        return Ok(());
    }
    if !req.allow_write {
        return Err(CoreError::Validation(
            "write blocked: set allow_write to proceed".to_string(),
        ));
    }

    // Ventoy gap G1: Legacy-BIOS-bootable layout requires an
    // `i386-pc` GRUB build alongside the existing `x86_64-efi` one,
    // plus a BIOS Boot Partition at the front of the disk. The
    // partition planner is wired below behind `bios_compat`, but the
    // actual `grub-install --target=i386-pc` embedding is v0.0.2
    // work. Surface a clear NotImplemented to callers in v0.0.1.
    if req.bios_compat {
        return Err(CoreError::NotImplemented(
            "Legacy-BIOS install (--bios-compat) is scaffolded but the \
             i386-pc GRUB embedding lands in v0.0.2; use --dry-run or \
             --simulator to validate the request shape until then"
                .to_string(),
        ));
    }

    sink.emit(ProgressEvent {
        phase: "partition".to_string(),
        message: "Creating GPT partitions".to_string(),
        percent: Some(30),
    });

    rt.run("parted", &[&req.device, "-s", "mklabel", "gpt"])?;
    rt.run(
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
    rt.run("parted", &[&req.device, "-s", "set", "1", "esp", "on"])?;
    rt.run(
        "parted",
        &[&req.device, "-s", "mkpart", "primary", "33MiB", "100%"],
    )?;
    rt.run("parted", &[&req.device, "-s", "print"])?;

    sink.emit(ProgressEvent {
        phase: "format".to_string(),
        message: "Formatting partitions".to_string(),
        percent: Some(60),
    });

    let part1 = part_path(&req.device, 1);
    let part2 = part_path(&req.device, 2);
    rt.run("mkfs.vfat", &["-F", "32", "-n", "RAIDHOS_EFI", &part1])?;

    if rt.has_cmd("mkfs.exfat") {
        if rt.run("mkfs.exfat", &["-n", "DATA", &part2]).is_err() {
            rt.run("mkfs.exfat", &[&part2])?;
            let _ = rt.run("exfatlabel", &[&part2, "DATA"]);
        }
    } else if rt.has_cmd("mkexfatfs") {
        if rt.run("mkexfatfs", &["-n", "DATA", &part2]).is_err() {
            rt.run("mkexfatfs", &[&part2])?;
            let _ = rt.run("exfatlabel", &[&part2, "DATA"]);
        }
    } else {
        return Err(CoreError::Io(
            "exFAT formatter not found (mkfs.exfat or mkexfatfs)".to_string(),
        ));
    }

    payload_copy(sink, rt, &part1, &part2)?;

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
    // testable from any host. Linux callers in production still hit
    // the same shape check as before.
    let target = if req.simulator { "simulator" } else { "linux" };
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

    // Simulator mode: the target is a sparse file the user provided.
    // Skip the list_disks lookup (the file isn't in lsblk) and the
    // is_system / mounted checks (they don't apply to a file).
    if !req.simulator {
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

fn payload_copy(sink: &dyn ProgressSink, rt: &dyn Runtime, part1: &str, part2: &str) -> Result<()> {
    let payload_dir = rt
        .env_var("RAIDHOS_PAYLOAD_DIR")
        .ok_or_else(|| CoreError::Validation("RAIDHOS_PAYLOAD_DIR is not set".to_string()))?;
    let payload = std::path::PathBuf::from(payload_dir);
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
            percent: Some(45),
        });
    }

    let esp_mount = rt.mount_base().join("raidhos-esp");
    let data_mount = rt.mount_base().join("raidhos-data");
    rt.create_dir_all(&esp_mount)?;
    rt.create_dir_all(&data_mount)?;

    rt.run(
        "mount",
        &[part1, esp_mount.to_str().unwrap_or("/mnt/raidhos-esp")],
    )?;
    rt.run(
        "mount",
        &[part2, data_mount.to_str().unwrap_or("/mnt/raidhos-data")],
    )?;

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    rt.run(
        "cp",
        &[
            "-a",
            &format!("{}/.", esp_payload.to_string_lossy()),
            esp_mount.to_str().unwrap(),
        ],
    )?;
    rt.run(
        "cp",
        &[
            "-a",
            &format!("{}/.", data_payload.to_string_lossy()),
            data_mount.to_str().unwrap(),
        ],
    )?;

    let _ = rt.run("umount", &[esp_mount.to_str().unwrap()]);
    let _ = rt.run("umount", &[data_mount.to_str().unwrap()]);

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Payload copy complete.".to_string(),
        percent: Some(90),
    });

    Ok(())
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

// These tests use Linux-shape device paths (`/dev/sdb`) which the
// `validate_device_path` cross-platform check rejects on macOS /
// Windows hosts. Coverage on the Linux CI runner is what matters
// for tarpaulin; gate accordingly.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::runtime::{MockOutcome, MockRuntime};

    struct CollectSink {
        events: std::cell::RefCell<Vec<ProgressEvent>>,
    }
    impl ProgressSink for CollectSink {
        fn emit(&self, event: ProgressEvent) {
            self.events.borrow_mut().push(event);
        }
    }
    fn sink() -> CollectSink {
        CollectSink {
            events: std::cell::RefCell::new(Vec::new()),
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

    fn req(device: &str, wipe: bool, dry_run: bool, allow: bool) -> InstallRequest {
        InstallRequest {
            device: device.to_string(),
            payload_version: "0.1.0".to_string(),
            wipe,
            dry_run,
            allow_write: allow,
            simulator: false,
            bios_compat: false,
        }
    }

    fn payload_dir_with(esp: bool, data: bool) -> (std::path::PathBuf, MockRuntime) {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-linux-{}-{}",
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
        // Make the formatters appear present.
        rt.available_cmds.push("mkfs.exfat");
        (tmp, rt)
    }

    #[test]
    fn validate_rejects_non_dev_path() {
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err = validate_install(&req("sdb", true, true, false), &sink(), &disks).unwrap_err();
        assert!(format!("{err}").contains("device must be an absolute /dev path"));
    }

    #[test]
    fn validate_rejects_without_wipe_flag() {
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err =
            validate_install(&req("/dev/sdb", false, true, false), &sink(), &disks).unwrap_err();
        assert!(format!("{err}").contains("wipe flag"));
    }

    #[test]
    fn validate_rejects_system_disk() {
        let disks = vec![disk("/dev/sda", vec!["/"], true)];
        let err =
            validate_install(&req("/dev/sda", true, true, false), &sink(), &disks).unwrap_err();
        assert!(format!("{err}").contains("system disk"));
    }

    #[test]
    fn validate_rejects_mounted_partitions() {
        let disks = vec![disk("/dev/sdb", vec!["/media/usb"], false)];
        let err =
            validate_install(&req("/dev/sdb", true, true, false), &sink(), &disks).unwrap_err();
        assert!(format!("{err}").contains("mounted"));
    }

    #[test]
    fn validate_accepts_safe_disk() {
        let s = sink();
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let ok = validate_install(&req("/dev/sdb", true, true, false), &s, &disks);
        assert!(ok.is_ok());
        assert!(!s.events.borrow().is_empty());
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err =
            validate_install(&req("/dev/sdz", true, true, false), &sink(), &disks).unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn validate_rejects_shell_metacharacters() {
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let err = validate_install(
            &req("/dev/sdb;rm -rf /", true, true, false),
            &sink(),
            &disks,
        )
        .unwrap_err();
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
        let s = sink();
        let rt = MockRuntime::new();
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, true, false);
        assert!(install_with(r, &s, &rt, &disks).is_ok());
        assert!(s.events.borrow().iter().any(|e| e.phase == "complete"));
    }

    #[test]
    fn install_rejects_when_allow_write_unset() {
        let rt = MockRuntime::new();
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, false);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("write blocked"));
    }

    #[test]
    fn install_full_pipeline_against_mock_exfat() {
        let (_dir, mut rt) = payload_dir_with(true, true);
        rt.available_cmds = vec!["mkfs.exfat"];
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let ok = install_with(r, &sink(), &rt, &disks);
        assert!(ok.is_ok(), "install failed: {:?}", ok);
        let inv = rt.invocations.borrow();
        // parted x5 + mkfs.vfat + mkfs.exfat + 2 mount + 2 cp + 2 umount + 2 mkdir = 15
        assert!(inv.iter().any(|i| i.cmd == "parted"));
        assert!(inv.iter().any(|i| i.cmd == "mkfs.vfat"));
        assert!(inv.iter().any(|i| i.cmd == "mkfs.exfat"));
        assert!(inv.iter().any(|i| i.cmd == "mount"));
        assert!(inv.iter().any(|i| i.cmd == "cp"));
        assert!(inv.iter().any(|i| i.cmd == "umount"));
    }

    #[test]
    fn install_full_pipeline_falls_back_to_mkexfatfs() {
        let (_dir, mut rt) = payload_dir_with(true, true);
        rt.available_cmds = vec!["mkexfatfs"]; // mkfs.exfat NOT available
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let ok = install_with(r, &sink(), &rt, &disks);
        assert!(ok.is_ok());
        let inv = rt.invocations.borrow();
        assert!(inv.iter().any(|i| i.cmd == "mkexfatfs"));
        // mkfs.exfat must NOT have been invoked.
        assert!(!inv.iter().any(|i| i.cmd == "mkfs.exfat"));
    }

    #[test]
    fn install_fails_when_no_exfat_formatter_present() {
        let (_dir, mut rt) = payload_dir_with(true, true);
        rt.available_cmds = vec![]; // neither formatter available
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("exFAT formatter not found"));
    }

    #[test]
    fn install_exfat_label_fallback_after_initial_failure() {
        // First mkfs.exfat call fails; helper retries without -n.
        let (_dir, mut rt) = payload_dir_with(true, true);
        rt.available_cmds = vec!["mkfs.exfat"];
        // 5x parted Ok + mkfs.vfat Ok + mkfs.exfat Err + mkfs.exfat (retry) Ok + exfatlabel Ok + ...
        rt.outcomes = std::cell::RefCell::new(vec![
            MockOutcome::Ok(vec![]),                        // parted mklabel
            MockOutcome::Ok(vec![]),                        // parted mkpart ESP
            MockOutcome::Ok(vec![]),                        // parted set esp on
            MockOutcome::Ok(vec![]),                        // parted mkpart DATA
            MockOutcome::Ok(vec![]),                        // parted print
            MockOutcome::Ok(vec![]),                        // mkfs.vfat
            MockOutcome::Err("first attempt fails".into()), // mkfs.exfat -n DATA
            MockOutcome::Ok(vec![]),                        // mkfs.exfat (no label) retry
            MockOutcome::Ok(vec![]),                        // exfatlabel
            MockOutcome::Ok(vec![]),                        // mount esp
            MockOutcome::Ok(vec![]),                        // mount data
            MockOutcome::Ok(vec![]),                        // cp esp/
            MockOutcome::Ok(vec![]),                        // cp data/
            MockOutcome::Ok(vec![]),                        // umount esp
            MockOutcome::Ok(vec![]),                        // umount data
        ]);
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        assert!(install_with(r, &sink(), &rt, &disks).is_ok());
        let inv = rt.invocations.borrow();
        assert_eq!(
            inv.iter().filter(|i| i.cmd == "mkfs.exfat").count(),
            2,
            "expected mkfs.exfat to be retried"
        );
        assert!(inv.iter().any(|i| i.cmd == "exfatlabel"));
    }

    #[test]
    fn payload_copy_errors_without_env_var() {
        let mut rt = MockRuntime::new();
        rt.available_cmds.push("mkfs.exfat");
        // No env var set.
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("RAIDHOS_PAYLOAD_DIR is not set"));
    }

    #[test]
    fn payload_copy_errors_when_dir_missing() {
        let mut rt = MockRuntime::new();
        rt.available_cmds.push("mkfs.exfat");
        rt.env
            .push(("RAIDHOS_PAYLOAD_DIR".into(), "/missing".into()));
        // path_exists for /missing returns false (not in existing_paths).
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("does not exist"));
    }

    #[test]
    fn payload_copy_errors_without_esp_or_data() {
        let (_dir, rt) = payload_dir_with(true, false); // missing data/
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("must contain esp/ and data/"));
    }

    #[test]
    fn parted_failure_aborts_install() {
        let (_dir, mut rt) = payload_dir_with(true, true);
        rt.available_cmds = vec!["mkfs.exfat"];
        rt.outcomes = std::cell::RefCell::new(vec![MockOutcome::Err("parted boom".into())]);
        let disks = vec![disk("/dev/sdb", vec![], false)];
        let r = req("/dev/sdb", true, false, true);
        let err = install_with(r, &sink(), &rt, &disks).unwrap_err();
        assert!(format!("{err}").contains("parted boom"));
    }

    #[test]
    fn list_disks_with_mock_passes_through_parser() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(
            br#"{"blockdevices":[{"name":"sdb","size":"16000000000","type":"disk","rm":true}]}"#
                .to_vec(),
        ));
        let disks = list_disks_with(&rt).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "/dev/sdb");
    }

    #[test]
    fn list_disks_with_mock_propagates_io_error() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Err("lsblk: not found".into()));
        let err = list_disks_with(&rt).unwrap_err();
        assert!(format!("{err}").contains("lsblk"));
    }

    #[test]
    fn list_partitions_with_mock() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(
            br#"{"blockdevices":[{"name":"sdb","type":"disk","children":[{"name":"sdb1","type":"part","pkname":"sdb","label":"X"}]}]}"#.to_vec(),
        ));
        let parts = list_partitions_with(&rt, "/dev/sdb").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].id, "/dev/sdb1");
        assert_eq!(parts[0].label, "X");
    }

    #[test]
    fn list_partitions_with_mock_propagates_error() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Err("lsblk: bad".into()));
        assert!(list_partitions_with(&rt, "/dev/sdb").is_err());
    }
}
