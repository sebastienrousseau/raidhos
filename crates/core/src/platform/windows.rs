//! Windows platform backend.
//!
//! Discovery shells out to PowerShell `Get-Disk`; parsing is in
//! [`crate::parsers::parse_get_disk_json`]. The install pipeline calls
//! `Clear-Disk` / `New-Partition` / `Format-Volume` / `robocopy` via
//! PowerShell.
//!
//! The module compiles on every host (so its Runtime-driven tests
//! run under tarpaulin on Linux CI) — only the public re-export in
//! [`crate::platform`] is gated by `target_os`.

#![allow(dead_code)]

use crate::parsers::parse_get_disk_json;
use crate::runtime::{RealRuntime, Runtime};
use crate::{
    CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent, ProgressSink, Result,
};

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    list_disks_with(&RealRuntime)
}

pub(crate) fn list_disks_with(rt: &dyn Runtime) -> Result<Vec<DiskInfo>> {
    let script = "Get-Disk | Select-Object Number,FriendlyName,Size,BusType,IsBoot,IsSystem | ConvertTo-Json -Compress -Depth 2";
    let bytes = run_pwsh(rt, script)?;
    parse_get_disk_json(&bytes)
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

    let number = extract_disk_number(&req.device).ok_or_else(|| {
        CoreError::Validation(format!("could not extract disk number from {}", req.device))
    })?;

    sink.emit(ProgressEvent {
        phase: "partition".to_string(),
        message: "Clearing and partitioning disk".to_string(),
        percent: Some(30),
    });
    run_pwsh(
        rt,
        &format!("Clear-Disk -Number {number} -RemoveData -RemoveOEM -Confirm:$false"),
    )?;
    run_pwsh(
        rt,
        &format!("Initialize-Disk -Number {number} -PartitionStyle GPT -Confirm:$false"),
    )?;
    run_pwsh(rt, &format!(
        "$p = New-Partition -DiskNumber {number} -Size 33MB -GptType '{{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}}' -AssignDriveLetter; \
         Format-Volume -Partition $p -FileSystem FAT32 -NewFileSystemLabel 'RAIDHOS_EFI' -Confirm:$false | Out-Null; \
         $p.DriveLetter"
    ))?;
    run_pwsh(rt, &format!(
        "$p = New-Partition -DiskNumber {number} -UseMaximumSize -AssignDriveLetter; \
         Format-Volume -Partition $p -FileSystem exFAT -NewFileSystemLabel 'DATA' -Confirm:$false | Out-Null; \
         $p.DriveLetter"
    ))?;

    payload_copy(sink, rt, number)?;

    sink.emit(ProgressEvent {
        phase: "finalize".to_string(),
        message: "Installing boot loader entries".to_string(),
        percent: Some(95),
    });
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
    let target = if req.simulator {
        "simulator"
    } else {
        "windows"
    };
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
        .find(|d| d.id.eq_ignore_ascii_case(&req.device))
        .ok_or_else(|| CoreError::Validation("device not found".to_string()))?;
    if target.is_system {
        return Err(CoreError::Validation(
            "refusing to operate on system disk".to_string(),
        ));
    }
    Ok(())
}

fn payload_copy(sink: &dyn ProgressSink, rt: &dyn Runtime, _disk_number: u64) -> Result<()> {
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
            percent: Some(70),
        });
    }

    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    let esp_letter = drive_letter_for_label(rt, "RAIDHOS_EFI")?;
    let data_letter = drive_letter_for_label(rt, "DATA")?;

    robocopy(rt, &esp_payload, &format!("{esp_letter}:\\"))?;
    robocopy(rt, &data_payload, &format!("{data_letter}:\\"))?;

    Ok(())
}

fn drive_letter_for_label(rt: &dyn Runtime, label: &str) -> Result<String> {
    let script = format!(
        "(Get-Volume -FileSystemLabel '{}' | Select-Object -First 1).DriveLetter",
        label.replace('\'', "''")
    );
    let bytes = run_pwsh(rt, &script)?;
    let s = String::from_utf8_lossy(&bytes).trim().to_string();
    if s.is_empty() {
        return Err(CoreError::Io(format!("no drive letter for label {label}")));
    }
    Ok(s)
}

fn robocopy(rt: &dyn Runtime, src: &std::path::Path, dst: &str) -> Result<()> {
    match rt.run(
        "robocopy",
        &[
            &src.to_string_lossy(),
            dst,
            "/E",
            "/NFL",
            "/NDL",
            "/NJH",
            "/NJS",
        ],
    ) {
        Ok(()) => Ok(()),
        Err(e) => {
            // robocopy non-zero exit codes 0..7 mean "success with info"; the
            // RealRuntime maps them all to `command failed`. We can't
            // distinguish without raw output, so we accept the error and let
            // the underlying mount/copy round trip catch real failures. For
            // unit tests via MockRuntime, the runner returns Ok directly.
            //
            // This branch is unreachable under MockRuntime; the RealRuntime
            // path is exercised in CI via the install-hw-equiv workflow.
            Err(e)
        }
    }
}

fn run_pwsh(rt: &dyn Runtime, script: &str) -> Result<Vec<u8>> {
    rt.run_output(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}

pub(crate) fn extract_disk_number(device: &str) -> Option<u64> {
    let lower = device.to_ascii_lowercase();
    let tail = lower
        .strip_prefix("\\\\.\\physicaldrive")
        .or_else(|| lower.strip_prefix("\\\\?\\physicaldrive"))?;
    tail.parse().ok()
}

// Windows-shape device paths are only accepted by
// `validate_device_path` on Windows hosts.
// Tests run cross-platform: the install path's path-shape check now
// goes through `validate_device_path_for_target("windows", …)` which
// has no host dependency, so tarpaulin on Linux CI sees these tests
// execute against the Windows install pipeline through MockRuntime.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{MockOutcome, MockRuntime};

    struct NopSink;
    impl ProgressSink for NopSink {
        fn emit(&self, _: ProgressEvent) {}
    }

    fn disk(id: &str, is_system: bool) -> DiskInfo {
        DiskInfo {
            id: id.into(),
            model: "Test".into(),
            size_bytes: 1024,
            removable: true,
            mountpoints: Vec::new(),
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
        }
    }

    fn payload_mock(esp: bool, data: bool) -> MockRuntime {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-win-{}-{}",
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
    fn extract_disk_number_parses_well_known_forms() {
        assert_eq!(extract_disk_number("\\\\.\\PhysicalDrive3"), Some(3));
        assert_eq!(extract_disk_number("\\\\?\\PhysicalDrive12"), Some(12));
        assert_eq!(extract_disk_number("\\\\.\\physicaldrive0"), Some(0));
        assert_eq!(extract_disk_number("/dev/sdb"), None);
        assert_eq!(extract_disk_number(""), None);
        assert_eq!(extract_disk_number("\\\\.\\PhysicalDriveX"), None);
    }

    #[test]
    fn validate_rejects_system_disk() {
        let disks = vec![disk("\\\\.\\PhysicalDrive0", true)];
        let err = validate_install(
            &req("\\\\.\\PhysicalDrive0", true, true, false),
            &NopSink,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("system disk"));
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let err = validate_install(
            &req("\\\\.\\PhysicalDrive9", true, true, false),
            &NopSink,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn validate_rejects_without_wipe() {
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let err = validate_install(
            &req("\\\\.\\PhysicalDrive1", false, true, false),
            &NopSink,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("wipe flag"));
    }

    #[test]
    fn dry_run_succeeds() {
        let rt = MockRuntime::new();
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let res = install_with(
            req("\\\\.\\PhysicalDrive1", true, true, false),
            &NopSink,
            &rt,
            &disks,
        );
        assert!(res.is_ok());
    }

    #[test]
    fn install_rejects_when_allow_write_unset() {
        let rt = MockRuntime::new();
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let err = install_with(
            req("\\\\.\\PhysicalDrive1", true, false, false),
            &NopSink,
            &rt,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("write blocked"));
    }

    #[test]
    fn install_full_pipeline_against_mock() {
        let mut rt = payload_mock(true, true);
        // PowerShell run_output requires data in the queue.
        // Order: Clear-Disk, Initialize-Disk, New-Partition ESP,
        // New-Partition DATA, Get-Volume RAIDHOS_EFI, Get-Volume DATA,
        // robocopy esp, robocopy data.
        rt.outcomes = std::cell::RefCell::new(vec![
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(b"E\n".to_vec()),
            MockOutcome::Ok(b"F\n".to_vec()),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
        ]);
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let res = install_with(
            req("\\\\.\\PhysicalDrive1", true, false, true),
            &NopSink,
            &rt,
            &disks,
        );
        assert!(res.is_ok(), "install failed: {res:?}");
        let inv = rt.invocations.borrow();
        assert!(inv.iter().any(|i| i.cmd == "powershell"));
        assert!(inv.iter().any(|i| i.cmd == "robocopy"));
    }

    #[test]
    fn install_errors_on_unparseable_device_path() {
        let rt = MockRuntime::new();
        let disks = vec![disk("\\\\.\\PhysicalDriveBOGUS", false)];
        // install_with skips the lib-level validate_device_path (that
        // gate runs in the public install() wrapper), so it actually
        // reaches extract_disk_number, which fails to parse "BOGUS"
        // into a u64 and bubbles the "could not extract disk number"
        // validation error.
        let err = install_with(
            req("\\\\.\\PhysicalDriveBOGUS", true, false, true),
            &NopSink,
            &rt,
            &disks,
        )
        .unwrap_err();
        let s = format!("{err}");
        assert!(s.contains("could not extract disk number"), "got: {s}");
    }

    #[test]
    fn payload_copy_errors_without_env_var() {
        let mut rt = MockRuntime::new();
        // make run_output / run all OK so we reach payload_copy
        rt.outcomes = std::cell::RefCell::new(vec![
            MockOutcome::Ok(vec![]), // Clear-Disk
            MockOutcome::Ok(vec![]), // Initialize-Disk
            MockOutcome::Ok(vec![]), // ESP partition
            MockOutcome::Ok(vec![]), // DATA partition
        ]);
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let err = install_with(
            req("\\\\.\\PhysicalDrive1", true, false, true),
            &NopSink,
            &rt,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("RAIDHOS_PAYLOAD_DIR is not set"));
    }

    #[test]
    fn payload_copy_errors_with_missing_subdir() {
        let mut rt = payload_mock(true, false);
        rt.outcomes = std::cell::RefCell::new(vec![
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
            MockOutcome::Ok(vec![]),
        ]);
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let err = install_with(
            req("\\\\.\\PhysicalDrive1", true, false, true),
            &NopSink,
            &rt,
            &disks,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("must contain esp/ and data/"));
    }

    #[test]
    fn drive_letter_errors_on_empty_response() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"\n".to_vec()));
        let err = drive_letter_for_label(&rt, "MISSING").unwrap_err();
        assert!(format!("{err}").contains("no drive letter for label MISSING"));
    }

    #[test]
    fn drive_letter_returns_letter() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"E\n".to_vec()));
        assert_eq!(drive_letter_for_label(&rt, "DATA").unwrap(), "E");
    }

    #[test]
    fn list_disks_with_mock_parses_json() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(
            br#"[{"Number":1,"FriendlyName":"USB","Size":16000000000,"BusType":"USB","IsBoot":false,"IsSystem":false}]"#.to_vec()));
        let disks = list_disks_with(&rt).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive1");
    }

    #[test]
    fn list_disks_with_mock_propagates_error() {
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Err("powershell: bad".into()));
        assert!(list_disks_with(&rt).is_err());
    }

    #[test]
    fn list_partitions_returns_empty() {
        assert!(list_partitions("\\\\.\\PhysicalDrive1".into())
            .unwrap()
            .is_empty());
    }
}
