//! Platform-output parsers, lifted out of the `#[cfg]`-gated platform
//! backends so they compile on every host and can be fuzzed
//! independently.
//!
//! Each function takes raw bytes (the captured stdout of `lsblk`,
//! `diskutil`, or `Get-Disk`) and returns a `Vec<DiskInfo>`.

use crate::{CoreError, DiskInfo, PartitionInfo, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------
// `lsblk -J` JSON output (Linux)
// ---------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct LsblkOutput {
    pub blockdevices: Vec<LsblkDevice>,
}

#[derive(Deserialize)]
pub(crate) struct LsblkDevice {
    pub name: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub rm: Option<bool>,
    #[serde(default, rename = "type")]
    pub type_field: Option<String>,
    #[serde(default)]
    pub mountpoints: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub children: Option<Vec<LsblkDevice>>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub fstype: Option<String>,
    #[serde(default)]
    pub pkname: Option<String>,
}

/// Parse `lsblk -b -J -o NAME,MODEL,SIZE,RM,TYPE,MOUNTPOINTS` output
/// into a list of whole-disk [`DiskInfo`]s.
#[doc(hidden)]
pub fn parse_lsblk_disks(bytes: &[u8]) -> Result<Vec<DiskInfo>> {
    let parsed: LsblkOutput =
        serde_json::from_slice(bytes).map_err(|e| CoreError::Parse(e.to_string()))?;

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
        let is_system = mounts
            .iter()
            .any(|m| m == "/" || m == "/boot" || m == "/boot/efi");

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

/// Parse `lsblk -b -J -o NAME,TYPE,LABEL,FSTYPE,MOUNTPOINTS,PKNAME`
/// output into partitions whose parent device matches `parent_dev`.
#[doc(hidden)]
pub fn parse_lsblk_partitions(bytes: &[u8], parent_dev: &str) -> Result<Vec<PartitionInfo>> {
    let parsed: LsblkOutput =
        serde_json::from_slice(bytes).map_err(|e| CoreError::Parse(e.to_string()))?;
    let parent = parent_dev.trim_start_matches("/dev/").to_string();
    let mut parts = Vec::new();
    for dev in parsed.blockdevices {
        collect_parts(&dev, &parent, &mut parts);
    }
    Ok(parts)
}

pub(crate) fn collect_mounts(dev: &LsblkDevice, mounts: &mut Vec<String>) {
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

pub(crate) fn collect_parts(dev: &LsblkDevice, parent: &str, parts: &mut Vec<PartitionInfo>) {
    if dev.type_field.as_deref() == Some("part") && dev.pkname.as_deref() == Some(parent) {
        let mut mounts = Vec::new();
        if let Some(mps) = &dev.mountpoints {
            for mp in mps.iter().flatten() {
                if !mp.is_empty() {
                    mounts.push(mp.clone());
                }
            }
        }
        parts.push(PartitionInfo {
            id: format!("/dev/{}", dev.name),
            label: dev.label.clone().unwrap_or_default(),
            fstype: dev.fstype.clone().unwrap_or_default(),
            mountpoints: mounts,
        });
    }
    if let Some(children) = &dev.children {
        for child in children {
            collect_parts(child, parent, parts);
        }
    }
}

// ---------------------------------------------------------------------
// `diskutil list -plist external` output (macOS)
// ---------------------------------------------------------------------

/// Parse the XML plist that `diskutil list -plist external` emits.
///
/// Hand-rolled rather than pulling a `plist` dependency — the format
/// we care about is a tiny subset.
#[doc(hidden)]
pub fn parse_disks_plist(bytes: &[u8]) -> Result<Vec<DiskInfo>> {
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
pub(crate) fn looks_like_slice(id: &str) -> bool {
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

// ---------------------------------------------------------------------
// PowerShell `Get-Disk | ConvertTo-Json` output (Windows)
// ---------------------------------------------------------------------

/// Parse the JSON `Get-Disk | ... | ConvertTo-Json` emits. Tolerant
/// of both single-object and array forms.
#[doc(hidden)]
pub fn parse_get_disk_json(bytes: &[u8]) -> Result<Vec<DiskInfo>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsblk_parses_minimal_output() {
        let raw = r#"{"blockdevices":[
            {"name":"sda","size":"500000000000","model":"OS","rm":false,"type":"disk",
             "children":[{"name":"sda1","type":"part","mountpoints":["/"]}]},
            {"name":"sdb","size":"16000000000","model":"USB","rm":true,"type":"disk",
             "children":[{"name":"sdb1","type":"part","mountpoints":[null]}]}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 2);
        let sda = disks.iter().find(|d| d.id == "/dev/sda").unwrap();
        assert!(sda.is_system);
        let sdb = disks.iter().find(|d| d.id == "/dev/sdb").unwrap();
        assert!(sdb.removable);
    }

    #[test]
    fn lsblk_rejects_garbage() {
        assert!(parse_lsblk_disks(b"not json").is_err());
    }

    #[test]
    fn diskutil_parses_external_and_internal() {
        let sample = r#"<plist><array>
<dict><key>DeviceIdentifier</key><string>disk2</string>
<key>Size</key><integer>16000000000</integer>
<key>Removable</key><true/><key>Internal</key><false/>
<key>MediaName</key><string>USB</string></dict>
<dict><key>DeviceIdentifier</key><string>disk0</string>
<key>Size</key><integer>500000000000</integer>
<key>Removable</key><false/><key>Internal</key><true/>
<key>MediaName</key><string>APPLE SSD</string></dict>
</array></plist>"#;
        let disks = parse_disks_plist(sample.as_bytes()).unwrap();
        assert!(disks.iter().any(|d| d.id == "/dev/disk2" && !d.is_system));
        assert!(disks.iter().any(|d| d.id == "/dev/disk0" && d.is_system));
    }

    #[test]
    fn diskutil_slice_detector() {
        assert!(looks_like_slice("disk0s1"));
        assert!(looks_like_slice("disk2s5"));
        assert!(!looks_like_slice("disk0"));
        assert!(!looks_like_slice("disk10"));
        assert!(!looks_like_slice(""));
    }

    #[test]
    fn diskutil_rejects_non_utf8() {
        assert!(parse_disks_plist(&[0xff, 0xfe, 0xfd]).is_err());
    }

    #[test]
    fn getdisk_parses_array() {
        let raw = r#"[{"Number":1,"FriendlyName":"SanDisk USB","Size":16000000000,"BusType":"USB","IsBoot":false,"IsSystem":false},
                      {"Number":0,"FriendlyName":"NVMe SSD","Size":512000000000,"BusType":"NVMe","IsBoot":true,"IsSystem":true}]"#;
        let disks = parse_get_disk_json(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive1");
        assert!(disks[0].removable);
        assert!(disks[1].is_system);
    }

    #[test]
    fn getdisk_parses_single_object() {
        let raw = r#"{"Number":2,"FriendlyName":"Solo","Size":1,"BusType":"USB","IsBoot":false,"IsSystem":false}"#;
        let disks = parse_get_disk_json(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive2");
    }

    #[test]
    fn getdisk_handles_empty() {
        assert!(parse_get_disk_json(b"").unwrap().is_empty());
    }

    #[test]
    fn getdisk_rejects_invalid_json() {
        assert!(parse_get_disk_json(b"not json").is_err());
    }
}
