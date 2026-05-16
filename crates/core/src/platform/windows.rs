//! Windows platform backend.
//!
//! Discovery uses PowerShell `Get-Disk`. Output is taken as JSON and
//! parsed via `serde_json::Value` rather than a typed struct because
//! `Get-Disk` returns a single object for one disk and an array for
//! many, and we want to tolerate either.
//!
//! **Install is not yet wired on Windows.** Returns
//! [`CoreError::NotImplemented`].

use crate::{CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressSink, Result};
use std::process::Command;

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let script = "Get-Disk | Select-Object Number,FriendlyName,Size,BusType,IsBoot,IsSystem | ConvertTo-Json -Compress -Depth 2";
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
            "Get-Disk failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    parse_get_disk_json(&out.stdout)
}

pub(crate) fn parse_get_disk_json(bytes: &[u8]) -> Result<Vec<DiskInfo>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| CoreError::Parse(e.to_string()))?;
    let array = match value {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    let mut disks = Vec::new();
    for item in array {
        let number = item
            .get("Number")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        let model = item
            .get("FriendlyName")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let size_bytes = item.get("Size").and_then(|v| v.as_u64()).unwrap_or(0);
        let bus = item
            .get("BusType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let removable = bus.contains("usb") || bus.contains("removable");
        let is_system = item
            .get("IsSystem")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || item
                .get("IsBoot")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        disks.push(DiskInfo {
            id: format!("\\\\.\\PhysicalDrive{number}"),
            model,
            size_bytes,
            removable,
            mountpoints: Vec::new(),
            is_system,
        });
    }
    Ok(disks)
}

pub fn list_partitions(_device: String) -> Result<Vec<PartitionInfo>> {
    Ok(Vec::new())
}

pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
    Err(CoreError::NotImplemented(
        "Windows installer is not yet wired up; use the Linux build for end-to-end install"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_disk_json_array() {
        let raw = r#"[{"Number":1,"FriendlyName":"SanDisk USB","Size":16000000000,"BusType":"USB","IsBoot":false,"IsSystem":false},
                      {"Number":0,"FriendlyName":"NVMe SSD","Size":512000000000,"BusType":"NVMe","IsBoot":true,"IsSystem":true}]"#;
        let disks = parse_get_disk_json(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive1");
        assert!(disks[0].removable);
        assert!(!disks[0].is_system);
        assert!(disks[1].is_system);
    }

    #[test]
    fn parses_get_disk_json_single_object() {
        let raw = r#"{"Number":2,"FriendlyName":"Solo","Size":1,"BusType":"USB","IsBoot":false,"IsSystem":false}"#;
        let disks = parse_get_disk_json(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive2");
    }

    #[test]
    fn parses_empty_input() {
        let disks = parse_get_disk_json(b"").unwrap();
        assert!(disks.is_empty());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_get_disk_json(b"not json").is_err());
    }

    #[test]
    fn install_returns_not_implemented() {
        struct NopSink;
        impl ProgressSink for NopSink {
            fn emit(&self, _: crate::ProgressEvent) {}
        }
        let req = InstallRequest {
            device: "\\\\.\\PhysicalDrive1".to_string(),
            payload_version: "x".to_string(),
            wipe: true,
            dry_run: false,
            allow_write: true,
        };
        assert!(matches!(
            install(req, &NopSink),
            Err(CoreError::NotImplemented(_))
        ));
    }
}
