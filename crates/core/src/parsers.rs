//! Platform-output parsers, lifted out of the `#[cfg]`-gated platform
//! backends so they compile on every host and can be fuzzed
//! independently.
//!
//! Each function takes raw bytes (the captured stdout of `lsblk`,
//! `diskutil`, or `Get-Disk`) and returns a `Vec<DiskInfo>`.

use crate::{CoreError, DiskInfo, PartitionInfo, Result};
use serde::{Deserialize, Deserializer};

/// `lsblk -b` returns SIZE as a JSON integer on util-linux ≥ 2.40 and as
/// a string on older versions. Accept either shape and normalise to
/// `u64`. Negative numbers are clamped to 0 so we never panic.
fn deserialize_size<'de, D>(deserializer: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Size {
        Number(i64),
        String(String),
        Null,
    }
    match Option::<Size>::deserialize(deserializer)?.unwrap_or(Size::Null) {
        Size::Number(n) => Ok(Some(n.max(0) as u64)),
        Size::String(s) => Ok(s.parse::<u64>().ok()),
        Size::Null => Ok(None),
    }
}

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
    #[serde(default, deserialize_with = "deserialize_size")]
    pub size: Option<u64>,
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
        let size_bytes = dev.size.unwrap_or(0);

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
    fn extract_plist_string_returns_none_when_close_before_open() {
        // The </string> appears BEFORE the <string> — pathological
        // ordering. Should refuse rather than panic or slice
        // backwards.
        let weird = "<key>K</key></string><string>";
        assert!(extract_plist_string(weird, "K").is_none());
    }

    #[test]
    fn extract_plist_string_returns_none_when_missing_close() {
        let no_close = "<key>K</key><string>val";
        assert!(extract_plist_string(no_close, "K").is_none());
    }

    #[test]
    fn extract_plist_integer_returns_none_when_close_before_open() {
        let weird = "<key>Size</key></integer><integer>";
        assert!(extract_plist_integer(weird, "Size").is_none());
    }

    #[test]
    fn extract_plist_integer_returns_none_when_missing_close() {
        let no_close = "<key>Size</key><integer>42";
        assert!(extract_plist_integer(no_close, "Size").is_none());
    }

    #[test]
    fn extract_plist_bool_returns_none_when_both_missing() {
        // Key present but neither <true/> nor <false/> follows
        // it — should be None, not unwrap or pick a default.
        let neither = "<key>Removable</key><integer>0</integer>";
        assert!(extract_plist_bool(neither, "Removable").is_none());
    }

    #[test]
    fn extract_plist_bool_prefers_first_marker() {
        // Both markers present — pick whichever appears first.
        let true_first = "<key>K</key><true/><false/>";
        assert_eq!(extract_plist_bool(true_first, "K"), Some(true));
        let false_first = "<key>K</key><false/><true/>";
        assert_eq!(extract_plist_bool(false_first, "K"), Some(false));
    }

    #[test]
    fn extract_plist_helpers_return_none_for_missing_key() {
        let no_key = "<plist><dict><key>Other</key><string>v</string></dict></plist>";
        assert!(extract_plist_string(no_key, "K").is_none());
        assert!(extract_plist_integer(no_key, "K").is_none());
        assert!(extract_plist_bool(no_key, "K").is_none());
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

    #[test]
    fn lsblk_collects_recursive_mounts_through_children() {
        // Disk with two-level children: disk → part → mounts.
        let raw = r#"{"blockdevices":[
            {"name":"sda","size":"500000000000","type":"disk","rm":false,
             "children":[
                {"name":"sda1","type":"part","mountpoints":["/boot"]},
                {"name":"sda2","type":"part","mountpoints":["/"]}
             ]}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert!(d.is_system); // /boot and / both flag system
        assert_eq!(d.mountpoints.len(), 2);
    }

    #[test]
    fn lsblk_handles_empty_mountpoint_strings() {
        // A null + empty-string mountpoint should be filtered out.
        let raw = r#"{"blockdevices":[
            {"name":"sdb","size":"16000000000","type":"disk","rm":true,
             "children":[
                {"name":"sdb1","type":"part","mountpoints":[null,""]}
             ]}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert!(disks[0].mountpoints.is_empty());
        assert!(!disks[0].is_system);
    }

    #[test]
    fn lsblk_skips_non_disk_entries() {
        // A loop device + a disk; non-disk should be skipped.
        let raw = r#"{"blockdevices":[
            {"name":"loop0","size":"10000","type":"loop","rm":false},
            {"name":"sdb","size":"16000000000","type":"disk","rm":true}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "/dev/sdb");
    }

    #[test]
    fn lsblk_handles_missing_size_field() {
        let raw = r#"{"blockdevices":[
            {"name":"sdc","type":"disk","rm":true}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].size_bytes, 0);
        assert_eq!(disks[0].model, "Unknown");
    }

    #[test]
    fn lsblk_accepts_size_as_integer() {
        // util-linux ≥ 2.40 emits SIZE as a JSON integer when -b is set.
        let raw = r#"{"blockdevices":[
            {"name":"loop0","size":268435456,"type":"disk","rm":false}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].size_bytes, 268_435_456);
    }

    #[test]
    fn lsblk_accepts_size_as_string() {
        // util-linux < 2.40 emits SIZE as a JSON string.
        let raw = r#"{"blockdevices":[
            {"name":"sdb","size":"16000000000","type":"disk","rm":true}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].size_bytes, 16_000_000_000);
    }

    #[test]
    fn lsblk_accepts_size_as_null() {
        // Some kernels report a null SIZE for empty card readers.
        let raw = r#"{"blockdevices":[
            {"name":"sdz","size":null,"type":"disk","rm":true}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].size_bytes, 0);
    }

    #[test]
    fn lsblk_clamps_negative_size_to_zero() {
        // Negative integer should clamp, not panic / underflow.
        let raw = r#"{"blockdevices":[
            {"name":"sdneg","size":-1,"type":"disk","rm":true}
        ]}"#;
        let disks = parse_lsblk_disks(raw.as_bytes()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].size_bytes, 0);
    }

    #[test]
    fn lsblk_partitions_filters_by_parent() {
        let raw = r#"{"blockdevices":[
            {"name":"sda","type":"disk","children":[
                {"name":"sda1","type":"part","pkname":"sda","label":"A","fstype":"ext4"}
            ]},
            {"name":"sdb","type":"disk","children":[
                {"name":"sdb1","type":"part","pkname":"sdb","label":"B","fstype":"vfat",
                 "mountpoints":["/mnt/usb"]}
            ]}
        ]}"#;
        let parts = parse_lsblk_partitions(raw.as_bytes(), "/dev/sdb").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].id, "/dev/sdb1");
        assert_eq!(parts[0].label, "B");
        assert_eq!(parts[0].fstype, "vfat");
        assert_eq!(parts[0].mountpoints, vec!["/mnt/usb"]);
    }

    #[test]
    fn lsblk_partitions_returns_empty_for_unknown_parent() {
        let raw = r#"{"blockdevices":[
            {"name":"sda","type":"disk","children":[
                {"name":"sda1","type":"part","pkname":"sda"}
            ]}
        ]}"#;
        let parts = parse_lsblk_partitions(raw.as_bytes(), "/dev/nonexistent").unwrap();
        assert!(parts.is_empty());
    }

    #[test]
    fn diskutil_skips_partition_slices() {
        let raw = r#"<plist><array>
            <dict><key>DeviceIdentifier</key><string>disk2</string>
                  <key>Size</key><integer>16000000000</integer>
                  <key>Removable</key><true/><key>Internal</key><false/></dict>
            <dict><key>DeviceIdentifier</key><string>disk2s1</string>
                  <key>Size</key><integer>33554432</integer></dict>
            <dict><key>DeviceIdentifier</key><string>disk2s2</string>
                  <key>Size</key><integer>15966445568</integer></dict>
        </array></plist>"#;
        let disks = parse_disks_plist(raw.as_bytes()).unwrap();
        // disk2 alone — slices skipped via looks_like_slice.
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "/dev/disk2");
    }

    #[test]
    fn diskutil_uses_ejectable_as_removable_fallback() {
        let raw = r#"<plist><array>
            <dict><key>DeviceIdentifier</key><string>disk3</string>
                  <key>Size</key><integer>1000</integer>
                  <key>Ejectable</key><true/><key>Internal</key><false/></dict>
        </array></plist>"#;
        let disks = parse_disks_plist(raw.as_bytes()).unwrap();
        assert!(disks[0].removable);
    }

    #[test]
    fn diskutil_defaults_to_internal_when_unspecified() {
        // No Internal/Removable key → defaults treat the disk as
        // internal/system, which is the safe default.
        let raw = r#"<plist><array>
            <dict><key>DeviceIdentifier</key><string>disk4</string>
                  <key>Size</key><integer>500</integer></dict>
        </array></plist>"#;
        let disks = parse_disks_plist(raw.as_bytes()).unwrap();
        assert!(disks[0].is_system);
    }

    #[test]
    fn diskutil_uses_unknown_model_when_media_name_missing() {
        let raw = r#"<plist><array>
            <dict><key>DeviceIdentifier</key><string>disk5</string>
                  <key>Size</key><integer>1</integer></dict>
        </array></plist>"#;
        let disks = parse_disks_plist(raw.as_bytes()).unwrap();
        assert_eq!(disks[0].model, "Unknown");
    }

    #[test]
    fn getdisk_treats_removable_via_busname() {
        let raw =
            br#"[{"Number":7,"FriendlyName":"X","Size":1,"BusType":"REMOVABLE","IsBoot":false,"IsSystem":false}]"#;
        let disks = parse_get_disk_json(raw).unwrap();
        assert!(disks[0].removable);
    }

    #[test]
    fn getdisk_handles_missing_fields() {
        // No Number → default 0; no FriendlyName → "Unknown"; etc.
        let raw = br#"[{}]"#;
        let disks = parse_get_disk_json(raw).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id, "\\\\.\\PhysicalDrive0");
        assert_eq!(disks[0].model, "Unknown");
        assert_eq!(disks[0].size_bytes, 0);
    }

    #[test]
    fn getdisk_treats_isboot_as_system() {
        let raw = br#"[{"Number":1,"BusType":"SATA","IsBoot":true,"IsSystem":false}]"#;
        let disks = parse_get_disk_json(raw).unwrap();
        assert!(disks[0].is_system);
    }

    #[test]
    fn looks_like_slice_handles_double_digit() {
        assert!(looks_like_slice("disk10s2"));
        assert!(looks_like_slice("disk99s99"));
        assert!(!looks_like_slice("s"));
        assert!(!looks_like_slice("a"));
    }
}
