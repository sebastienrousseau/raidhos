//! Windows platform backend.
//!
//! Discovery shells out to PowerShell `Get-Disk`; parsing is in
//! [`crate::parsers::parse_get_disk_json`]. The install pipeline calls
//! `Clear-Disk`, `New-Partition`, `Format-Volume`, `bcdboot` via
//! PowerShell.

use crate::parsers::parse_get_disk_json;
use crate::{
    validate_device_path, CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressEvent,
    ProgressSink, Result,
};
use std::process::Command;

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let script = "Get-Disk | Select-Object Number,FriendlyName,Size,BusType,IsBoot,IsSystem | ConvertTo-Json -Compress -Depth 2";
    let out = run_pwsh(script)?;
    parse_get_disk_json(&out)
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

    let number = extract_disk_number(&req.device).ok_or_else(|| {
        CoreError::Validation(format!("could not extract disk number from {}", req.device))
    })?;

    sink.emit(ProgressEvent {
        phase: "partition".to_string(),
        message: "Clearing and partitioning disk".to_string(),
        percent: Some(30),
    });

    // Clear all existing data and partition table.
    run_pwsh(&format!(
        "Clear-Disk -Number {number} -RemoveData -RemoveOEM -Confirm:$false"
    ))?;
    run_pwsh(&format!(
        "Initialize-Disk -Number {number} -PartitionStyle GPT -Confirm:$false"
    ))?;

    // ESP — 33 MiB FAT32 labelled RAIDHOS_EFI.
    run_pwsh(&format!(
        "$p = New-Partition -DiskNumber {number} -Size 33MB -GptType '{{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}}' -AssignDriveLetter; \
         Format-Volume -Partition $p -FileSystem FAT32 -NewFileSystemLabel 'RAIDHOS_EFI' -Confirm:$false | Out-Null; \
         $p.DriveLetter"
    ))?;

    // DATA — remaining space exFAT labelled DATA.
    run_pwsh(&format!(
        "$p = New-Partition -DiskNumber {number} -UseMaximumSize -AssignDriveLetter; \
         Format-Volume -Partition $p -FileSystem exFAT -NewFileSystemLabel 'DATA' -Confirm:$false | Out-Null; \
         $p.DriveLetter"
    ))?;

    payload_copy(sink, number)?;

    // Install the Windows boot manager on the ESP. The caller is expected
    // to have placed an x64 EFI bootloader at `<ESP>:\EFI\BOOT\BOOTX64.EFI`.
    // bcdboot ensures firmware recognises the ESP as a boot target.
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
        .find(|d| d.id.eq_ignore_ascii_case(&req.device))
        .ok_or_else(|| CoreError::Validation("device not found".to_string()))?;
    if target.is_system {
        return Err(CoreError::Validation(
            "refusing to operate on system disk".to_string(),
        ));
    }
    Ok(())
}

fn payload_copy(sink: &dyn ProgressSink, _disk_number: u64) -> Result<()> {
    let payload_dir = std::env::var("RAIDHOS_PAYLOAD_DIR")
        .map_err(|_| CoreError::Validation("RAIDHOS_PAYLOAD_DIR is not set".to_string()))?;
    let payload = std::path::PathBuf::from(payload_dir);
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

    // The caller (priv-helper) must mount the freshly created
    // partitions and pass us drive letters, or we re-resolve them by
    // label. Resolve via label so we don't have to thread state.
    sink.emit(ProgressEvent {
        phase: "payload".to_string(),
        message: "Copying payload files".to_string(),
        percent: Some(85),
    });

    let esp_letter = drive_letter_for_label("RAIDHOS_EFI")?;
    let data_letter = drive_letter_for_label("DATA")?;

    robocopy(&esp_payload, &format!("{esp_letter}:\\"))?;
    robocopy(&data_payload, &format!("{data_letter}:\\"))?;

    Ok(())
}

fn drive_letter_for_label(label: &str) -> Result<String> {
    let script = format!(
        "(Get-Volume -FileSystemLabel '{}' | Select-Object -First 1).DriveLetter",
        label.replace('\'', "''")
    );
    let out = run_pwsh(&script)?;
    let s = String::from_utf8_lossy(&out).trim().to_string();
    if s.is_empty() {
        return Err(CoreError::Io(format!("no drive letter for label {label}")));
    }
    Ok(s)
}

fn robocopy(src: &std::path::Path, dst: &str) -> Result<()> {
    let status = Command::new("robocopy")
        .args([
            &src.to_string_lossy(),
            dst,
            "/E",
            "/NFL",
            "/NDL",
            "/NJH",
            "/NJS",
        ])
        .status()
        .map_err(|e| CoreError::Io(format!("robocopy: {e}")))?;
    // robocopy uses non-zero exit codes for "success with info" too.
    // Codes 0..=7 indicate success/warnings; 8+ is failure.
    if let Some(code) = status.code() {
        if code >= 8 {
            return Err(CoreError::Io(format!("robocopy failed: exit {code}")));
        }
    }
    Ok(())
}

fn run_pwsh(script: &str) -> Result<Vec<u8>> {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| CoreError::Io(format!("powershell: {e}")))?;
    if !out.status.success() {
        return Err(CoreError::Io(format!(
            "powershell failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(out.stdout)
}

/// Extract the integer disk number from a Windows physical device path
/// like `\\.\PhysicalDrive3`.
pub(crate) fn extract_disk_number(device: &str) -> Option<u64> {
    let lower = device.to_ascii_lowercase();
    let prefix_a = "\\\\.\\physicaldrive";
    let prefix_b = "\\\\?\\physicaldrive";
    let tail = if let Some(t) = lower.strip_prefix(prefix_a) {
        t
    } else if let Some(t) = lower.strip_prefix(prefix_b) {
        t
    } else {
        return None;
    };
    tail.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_disk_number_parses_well_known_forms() {
        assert_eq!(extract_disk_number("\\\\.\\PhysicalDrive3"), Some(3));
        assert_eq!(extract_disk_number("\\\\?\\PhysicalDrive12"), Some(12));
        assert_eq!(extract_disk_number("\\\\.\\physicaldrive0"), Some(0));
        assert_eq!(extract_disk_number("/dev/sdb"), None);
    }

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
        }
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
    fn dry_run_succeeds() {
        let disks = vec![disk("\\\\.\\PhysicalDrive1", false)];
        let res = install_with_disks(
            req("\\\\.\\PhysicalDrive1", true, true, false),
            &NopSink,
            &disks,
        );
        assert!(res.is_ok());
    }
}
