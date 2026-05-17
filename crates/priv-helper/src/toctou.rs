//! Time-of-check / time-of-use defence.
//!
//! [`raidhos_core::validate_device_path`] checks a string; it does not
//! check that the string still points at the same device by the time
//! we open it. Between validation and the first write a hostile actor
//! could swap a symlink or hot-plug a different USB stick at the same
//! `/dev/sdX` path.
//!
//! Mitigation: open the device once, read its `rdev` (major:minor) and
//! size via `fstat`, then keep the file descriptor open for the
//! lifetime of the install. Re-list the candidate disks and confirm
//! the disk we validated still maps to the same `rdev`. Linux only —
//! macOS and Windows have different device models and will need
//! separate work.

#![cfg(target_os = "linux")]
// The helpers below (`assert_matches`, `assert_disk_identity`,
// struct fields `rdev` / `size_hint`) are exposed for callers that
// want a stronger TOCTOU check between the validate step and a
// specific later operation. The current `install` path only needs
// the fd-held-for-lifetime guarantee; the comparison helpers are
// scaffolded for v0.0.2 when the install pipeline gets re-entrant
// inspection points.
#![allow(dead_code)]

use raidhos_core::DiskInfo;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::MetadataExt;

/// Snapshot of the device's identity at validate time. Held alongside
/// the open fd so subsequent operations can compare against it.
#[derive(Debug)]
pub struct DeviceHandle {
    /// Open fd on the device. Kept alive for the install duration so
    /// the kernel keeps the underlying object reachable even if the
    /// /dev entry is removed.
    _file: File,
    /// Major/minor encoded as a single u64 (Linux `dev_t`).
    pub rdev: u64,
    /// Size in bytes as reported by `fstat`.
    pub size_hint: u64,
}

/// Open the device read-write, snapshot its identity, return a handle.
pub fn pin_device(path: &str) -> Result<DeviceHandle, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {path}: {e}"))?;
    let meta = file.metadata().map_err(|e| format!("fstat {path}: {e}"))?;
    // `metadata().rdev()` is 0 for regular files. Block devices have
    // a non-zero rdev. Refuse to operate on anything that doesn't.
    if meta.rdev() == 0 {
        return Err(format!("{path} is not a block device"));
    }
    Ok(DeviceHandle {
        _file: file,
        rdev: meta.rdev(),
        size_hint: meta.size(),
    })
}

/// Verify the pinned device still matches what we expected. Call this
/// after any operation that could have raced with us — e.g. after
/// `parted` or after `wipefs`.
pub fn assert_matches(handle: &DeviceHandle, path: &str) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("re-stat {path}: {e}"))?;
    if meta.rdev() != handle.rdev {
        return Err(format!(
            "device {path} rdev changed (was {:#x}, now {:#x}) — refusing to proceed",
            handle.rdev,
            meta.rdev()
        ));
    }
    Ok(())
}

/// Verify that the disk we validated through `list_disks` still
/// corresponds to the pinned `rdev`. Defence against the user
/// listing one device but a different one being substituted before
/// we open it.
pub fn assert_disk_identity(
    _handle: &DeviceHandle,
    target: &DiskInfo,
    path: &str,
) -> Result<(), String> {
    if target.id != path {
        return Err(format!(
            "candidate disk id {} does not match requested path {}",
            target.id, path
        ));
    }
    // The DiskInfo doesn't carry rdev today; the path check is the
    // best we can do until we extend the discovery type. The held fd
    // still gives us TOCTOU-after-this-point safety.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(id: &str) -> DiskInfo {
        DiskInfo {
            id: id.to_string(),
            model: "Test".into(),
            size_bytes: 0,
            removable: true,
            mountpoints: Vec::new(),
            is_system: false,
        }
    }

    // We can't easily build a real DeviceHandle in a unit test (it owns
    // a File on a /dev/* node), so we synthesise one with a regular
    // file in /tmp. The rdev/size fields are exercised directly below;
    // assert_matches and assert_disk_identity only care about the
    // pinned values.
    fn synthetic_handle(rdev: u64, size: u64) -> DeviceHandle {
        let p = std::env::temp_dir().join(format!(
            "raidhos-toctou-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, b"x").unwrap();
        let file = std::fs::OpenOptions::new().read(true).open(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        DeviceHandle {
            _file: file,
            rdev,
            size_hint: size,
        }
    }

    #[test]
    fn assert_disk_identity_accepts_matching_path() {
        let h = synthetic_handle(0x803, 1024);
        let d = disk("/dev/sdb");
        assert!(assert_disk_identity(&h, &d, "/dev/sdb").is_ok());
    }

    #[test]
    fn assert_disk_identity_rejects_mismatched_path() {
        let h = synthetic_handle(0x803, 1024);
        let d = disk("/dev/sdb");
        let err = assert_disk_identity(&h, &d, "/dev/sdc").unwrap_err();
        assert!(err.contains("/dev/sdb"));
        assert!(err.contains("/dev/sdc"));
    }

    #[test]
    fn pin_device_refuses_missing_path() {
        let res = pin_device("/no/such/raidhos-test-path-xyz");
        assert!(res.is_err());
    }

    #[test]
    fn pin_device_refuses_regular_file() {
        // A regular file has rdev == 0, so pin_device refuses.
        let p = std::env::temp_dir().join(format!(
            "raidhos-toctou-reg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, b"not a device").unwrap();
        let res = pin_device(p.to_str().unwrap());
        let _ = std::fs::remove_file(&p);
        assert!(res.is_err());
        let msg = res.unwrap_err();
        assert!(
            msg.contains("not a block device") || msg.contains("open"),
            "unexpected error: {msg}",
        );
    }

    #[test]
    fn device_handle_exposes_pinned_fields() {
        let h = synthetic_handle(0xdead, 4096);
        assert_eq!(h.rdev, 0xdead);
        assert_eq!(h.size_hint, 4096);
    }

    #[test]
    fn assert_matches_rejects_missing_path() {
        let h = synthetic_handle(0x803, 1024);
        let err = assert_matches(&h, "/no/such/path-raidhos-xyz").unwrap_err();
        assert!(err.contains("re-stat"));
    }
}
