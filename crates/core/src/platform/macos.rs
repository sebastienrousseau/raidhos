//! macOS platform backend.
//!
//! Discovery uses `diskutil list -plist external`. The plist is parsed
//! by a small, tolerant text walker rather than pulling a `plist` crate
//! — keeps the dependency tree minimal and the parser fuzz-targetable.
//!
//! **Install is not yet wired on macOS.** Returns
//! [`CoreError::NotImplemented`].

use crate::{CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressSink, Result};
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

pub(crate) fn parse_disks_plist(bytes: &[u8]) -> Result<Vec<DiskInfo>> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| CoreError::Parse(format!("non-utf8 plist: {e}")))?;
    let mut disks = Vec::new();
    for chunk in text.split("<dict>").skip(1) {
        let id = extract_plist_string(chunk, "DeviceIdentifier").unwrap_or_default();
        if id.is_empty() || looks_like_slice(&id) {
            continue;
        }
        let size_bytes = extract_plist_integer(chunk, "Size").unwrap_or(0);
        let removable = extract_plist_bool(chunk, "Removable").unwrap_or(false)
            || extract_plist_bool(chunk, "Ejectable").unwrap_or(false);
        let internal = extract_plist_bool(chunk, "Internal").unwrap_or(true);
        let model = extract_plist_string(chunk, "MediaName").unwrap_or_default();
        disks.push(DiskInfo {
            id: format!("/dev/{id}"),
            model: if model.is_empty() {
                "Unknown".to_string()
            } else {
                model
            },
            size_bytes,
            removable,
            mountpoints: Vec::new(),
            is_system: internal,
        });
    }
    Ok(disks)
}

/// macOS partition identifiers look like `disk0s1` — digit, `s`, digit.
/// A whole-disk identifier is just `diskN`. This check filters slice
/// entries out of the disk list.
fn looks_like_slice(id: &str) -> bool {
    let bytes = id.as_bytes();
    for i in 1..bytes.len().saturating_sub(1) {
        if bytes[i] == b's' && bytes[i - 1].is_ascii_digit() && bytes[i + 1].is_ascii_digit() {
            return true;
        }
    }
    false
}

fn extract_plist_string(chunk: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let start = chunk.find(&needle)?;
    let after = &chunk[start + needle.len()..];
    let open = after.find("<string>")?;
    let close = after.find("</string>")?;
    if close <= open {
        return None;
    }
    Some(after[open + "<string>".len()..close].to_string())
}

fn extract_plist_integer(chunk: &str, key: &str) -> Option<u64> {
    let needle = format!("<key>{key}</key>");
    let start = chunk.find(&needle)?;
    let after = &chunk[start + needle.len()..];
    let open = after.find("<integer>")?;
    let close = after.find("</integer>")?;
    if close <= open {
        return None;
    }
    after[open + "<integer>".len()..close].trim().parse().ok()
}

fn extract_plist_bool(chunk: &str, key: &str) -> Option<bool> {
    let needle = format!("<key>{key}</key>");
    let start = chunk.find(&needle)?;
    let after = &chunk[start + needle.len()..];
    let true_pos = after.find("<true/>");
    let false_pos = after.find("<false/>");
    match (true_pos, false_pos) {
        (Some(t), Some(f)) => Some(t < f),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    }
}

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let stdout = run_diskutil(&["list", "-plist", "external"])?;
    parse_disks_plist(&stdout)
}

pub fn list_partitions(_device: String) -> Result<Vec<PartitionInfo>> {
    Ok(Vec::new())
}

pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
    Err(CoreError::NotImplemented(
        "macOS installer is not yet wired up; use the Linux build for end-to-end install"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<plist version="1.0"><array>
<dict>
  <key>DeviceIdentifier</key><string>disk2</string>
  <key>Size</key><integer>16000000000</integer>
  <key>Removable</key><true/>
  <key>Internal</key><false/>
  <key>MediaName</key><string>USB SanDisk 3.2Gen1</string>
</dict>
<dict>
  <key>DeviceIdentifier</key><string>disk0</string>
  <key>Size</key><integer>500000000000</integer>
  <key>Removable</key><false/>
  <key>Internal</key><true/>
  <key>MediaName</key><string>APPLE SSD</string>
</dict>
</array></plist>"#;

    #[test]
    fn parses_external_usb_as_non_system() {
        let disks = parse_disks_plist(SAMPLE.as_bytes()).unwrap();
        let usb = disks.iter().find(|d| d.id == "/dev/disk2").unwrap();
        assert!(usb.removable);
        assert!(!usb.is_system);
        assert_eq!(usb.size_bytes, 16_000_000_000);
    }

    #[test]
    fn parses_internal_ssd_as_system() {
        let disks = parse_disks_plist(SAMPLE.as_bytes()).unwrap();
        let ssd = disks.iter().find(|d| d.id == "/dev/disk0").unwrap();
        assert!(!ssd.removable);
        assert!(ssd.is_system);
    }

    #[test]
    fn parser_tolerates_empty() {
        assert!(parse_disks_plist(b"<plist></plist>").unwrap().is_empty());
    }

    #[test]
    fn parser_rejects_non_utf8() {
        let bad = vec![0xff, 0xfe, 0xfd];
        assert!(parse_disks_plist(&bad).is_err());
    }

    #[test]
    fn slice_detector_recognises_partitions() {
        assert!(looks_like_slice("disk0s1"));
        assert!(looks_like_slice("disk2s5"));
        assert!(!looks_like_slice("disk0"));
        assert!(!looks_like_slice("disk10"));
        assert!(!looks_like_slice(""));
    }

    #[test]
    fn install_returns_not_implemented() {
        struct NopSink;
        impl ProgressSink for NopSink {
            fn emit(&self, _: crate::ProgressEvent) {}
        }
        let req = InstallRequest {
            device: "/dev/disk2".to_string(),
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
