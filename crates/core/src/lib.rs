//! `raidhos-core` — disk discovery, validation, and install orchestration.
//!
//! This crate is the trust boundary between user input (a device path, a
//! payload directory, install flags) and the destructive operations that
//! act on raw block devices. Every public function on the install path
//! walks input through [`validate_device_path`] and the per-OS platform
//! backend in `crate::platform` before any byte is written.
//!
//! The crate forbids `unsafe` workspace-wide (see `Cargo.toml`). All
//! platform-specific code lives in `crate::platform`; the public
//! surface here is the same on every supported OS.
//!
//! # Examples
//!
//! Discover physical disks:
//!
//! ```no_run
//! let disks = raidhos_core::list_disks().expect("discovery failed");
//! for d in disks {
//!     println!("{} {} {} bytes", d.id, d.model, d.size_bytes);
//! }
//! ```
//!
//! Validate a device path before passing it anywhere risky:
//!
//! ```
//! # #[cfg(target_os = "linux")]
//! # {
//! assert!(raidhos_core::validate_device_path("/dev/sdb").is_ok());
//! assert!(raidhos_core::validate_device_path("/dev/sdb;rm -rf /").is_err());
//! # }
//! ```
//!
//! Dry-run the install pipeline (writes nothing):
//!
//! ```no_run
//! struct StdoutSink;
//! impl raidhos_core::ProgressSink for StdoutSink {
//!     fn emit(&self, event: raidhos_core::ProgressEvent) {
//!         println!("[{}] {}", event.phase, event.message);
//!     }
//! }
//!
//! let req = raidhos_core::InstallRequest {
//!     device: "/dev/sdb".to_string(),
//!     payload_version: "0.1.0".to_string(),
//!     wipe: true,
//!     dry_run: true,
//!     allow_write: false,
//! };
//! let _ = raidhos_core::install(req, &StdoutSink);
//! ```
//!
//! More runnable examples under [`examples/`](https://github.com/sebastienrousseau/raidhos/tree/main/crates/core/examples).

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

mod catalog;
mod parsers;
mod payload;
mod platform;

pub use catalog::{
    find_entry, load_catalog, load_catalog_from, sha256sums_lookup_pub as sha256sums_lookup,
    verify_iso, CatalogEntry, CatalogError, VerifiedIso,
};
pub use payload::{verify_payload, ManifestError, PayloadManifest};

/// Hand-rolled subprocess-output parsers, exposed for fuzzing.
///
/// Stability of this module is **not** guaranteed; it exists purely to
/// give the `fuzz/` harness a callable surface. Real code should call
/// the public functions like [`list_disks`] which wrap discovery
/// subprocess invocation around these parsers.
#[doc(hidden)]
pub mod __fuzz_api {
    pub use crate::parsers::{
        parse_disks_plist, parse_get_disk_json, parse_lsblk_disks, parse_lsblk_partitions,
    };
}

/// Convenience alias for results that fail with a [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;

/// Errors returned from public functions in this crate.
///
/// Kept variant-stable and `Display`-stable across patch releases — the
/// CLI and the privileged helper match on substrings of the rendered
/// form, and the Tauri UI surfaces these strings directly to users.
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum CoreError {
    /// Compiled for a target OS without a platform backend.
    #[error("unsupported platform")]
    UnsupportedPlatform,
    /// Subprocess or filesystem I/O failure. The string is the lossy
    /// rendering of the underlying error.
    #[error("io error: {0}")]
    Io(String),
    /// User-facing validation failure (bad device path, refused disk,
    /// missing opt-in flag).
    #[error("validation error: {0}")]
    Validation(String),
    /// The requested operation is not yet wired on this platform.
    #[error("not implemented: {0}")]
    NotImplemented(String),
    /// Parsing the output of a discovery subprocess failed.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Metadata about a physical disk discovered by the platform backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Platform-native device identifier (`/dev/sdX`, `/dev/diskN`,
    /// `\\.\PhysicalDriveN`).
    pub id: String,
    /// Human-readable model name as reported by the OS.
    pub model: String,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// `true` if the OS flags the device as removable / hot-pluggable.
    pub removable: bool,
    /// Mountpoints of any partitions on the disk. Non-empty means at
    /// least one partition is mounted right now.
    pub mountpoints: Vec<String>,
    /// `true` if the disk is mounted at a system path (`/`, `/boot`,
    /// `/boot/efi`) or flagged internal/system by the OS. Install will
    /// refuse to operate on such disks.
    pub is_system: bool,
}

/// Metadata about a single partition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionInfo {
    /// Partition device identifier (e.g. `/dev/sdb1`).
    pub id: String,
    /// Filesystem label, or empty if none.
    pub label: String,
    /// Filesystem type as reported by the OS (e.g. `vfat`, `exfat`).
    pub fstype: String,
    /// Current mountpoints, if any.
    pub mountpoints: Vec<String>,
}

/// User-supplied install request. Built by the CLI, the UI, and the
/// privileged helper from their respective argument surfaces and handed
/// to [`install`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallRequest {
    /// Device path. Validated against the per-OS allowlist.
    pub device: String,
    /// Payload version string (matched against `payload/manifest.json`).
    pub payload_version: String,
    /// Must be `true` for a destructive install. Defence in depth — both
    /// `wipe` and `allow_write` are required.
    pub wipe: bool,
    /// If `true`, runs every check but writes nothing.
    pub dry_run: bool,
    /// Must be `true` to actually touch the device. Defaults to `false`.
    pub allow_write: bool,
}

/// Progress event emitted by the install pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    /// Coarse phase tag: `validate`, `prepare`, `partition`, `format`,
    /// `payload`, `write`, `finalize`, `complete`.
    pub phase: String,
    /// Human-readable detail.
    pub message: String,
    /// Optional progress percentage 0..=100.
    pub percent: Option<u8>,
}

/// One ISO discovered by the [`scan_isos`] helper.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IsoEntry {
    /// Filename stem, used as the menu title in GRUB.
    pub title: String,
    /// Absolute path on the host filesystem.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Default kernel parameters for this entry.
    pub params: String,
}

/// Trait for sinks that consume [`ProgressEvent`]s emitted by the
/// install pipeline. The CLI prints them to stdout; the Tauri UI
/// accumulates them in a `Mutex<Vec<…>>` and ships them back over IPC.
pub trait ProgressSink {
    /// Called once per emitted event. Implementations should not block
    /// for long — the install path emits events synchronously.
    fn emit(&self, event: ProgressEvent);
}

/// Enumerate physical disks visible to the current user. Returns an
/// error if the platform discovery subprocess fails.
///
/// # Examples
///
/// ```no_run
/// for d in raidhos_core::list_disks().unwrap() {
///     println!("{} ({} bytes, removable={})", d.id, d.size_bytes, d.removable);
/// }
/// ```
pub fn list_disks() -> Result<Vec<DiskInfo>> {
    platform::list_disks()
}

/// Run the install pipeline. Validates the request, refuses unsafe
/// targets, then either dry-runs or executes per `req`.
///
/// # Examples
///
/// ```no_run
/// # struct Quiet;
/// # impl raidhos_core::ProgressSink for Quiet { fn emit(&self, _: raidhos_core::ProgressEvent) {} }
/// let req = raidhos_core::InstallRequest {
///     device: "/dev/sdb".to_string(),
///     payload_version: "0.1.0".to_string(),
///     wipe: true,
///     dry_run: true,     // never writes
///     allow_write: false,
/// };
/// let _ = raidhos_core::install(req, &Quiet);
/// ```
pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()> {
    platform::install(req, sink)
}

/// Walk the supplied directories looking for `*.iso` files (one level
/// deep). Used by the UI's "Scan ISOs" action.
///
/// # Examples
///
/// ```no_run
/// let isos = raidhos_core::scan_isos(vec!["/home".into(), "/Downloads".into()])
///     .unwrap();
/// for e in isos {
///     println!("{} ({} bytes)", e.title, e.size_bytes);
/// }
/// ```
pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>> {
    platform::scan_isos(dirs)
}

/// List partitions on the given device. Returns an empty list on
/// platforms where partition enumeration is not yet wired.
pub fn list_partitions(device: String) -> Result<Vec<PartitionInfo>> {
    platform::list_partitions(device)
}

/// Validate that a device identifier looks plausible for the current
/// platform and contains no shell metacharacters.
///
/// This is defence in depth — `Command::new(...).arg(...)` does not
/// invoke a shell, but rejecting the obvious cases early gives clearer
/// errors and prevents accidental misuse from API callers who later
/// pass these strings into shell snippets.
///
/// # Examples
///
/// ```
/// # #[cfg(target_os = "linux")]
/// # {
/// assert!(raidhos_core::validate_device_path("/dev/sdb").is_ok());
/// assert!(raidhos_core::validate_device_path("/dev/nvme0n1").is_ok());
/// // Rejections:
/// assert!(raidhos_core::validate_device_path("").is_err());
/// assert!(raidhos_core::validate_device_path("sdb").is_err()); // missing /dev/ prefix
/// assert!(raidhos_core::validate_device_path("/dev/sdb;rm -rf /").is_err());
/// assert!(raidhos_core::validate_device_path("/dev/../etc/passwd").is_err());
/// # }
/// ```
pub fn validate_device_path(device: &str) -> Result<()> {
    if device.is_empty() {
        return Err(CoreError::Validation("device path is empty".to_string()));
    }
    if device.len() > 256 {
        return Err(CoreError::Validation(
            "device path is unreasonably long".to_string(),
        ));
    }

    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>', '"', '\'', '*', '?', '(', ')', '\\',
    ];
    if device.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Err(CoreError::Validation(
            "device path contains forbidden character".to_string(),
        ));
    }

    if device.contains("..") {
        return Err(CoreError::Validation(
            "device path may not contain '..'".to_string(),
        ));
    }

    #[cfg(target_os = "linux")]
    {
        if !device.starts_with("/dev/") {
            return Err(CoreError::Validation(
                "device must be an absolute /dev path".to_string(),
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if !device.starts_with("/dev/disk") {
            return Err(CoreError::Validation(
                "device must be a /dev/diskN path".to_string(),
            ));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let lower = device.to_ascii_lowercase();
        let looks_physical =
            lower.starts_with("\\\\.\\physicaldrive") || lower.starts_with("\\\\?\\physicaldrive");
        if !looks_physical {
            return Err(CoreError::Validation(
                "device must be a \\\\.\\PhysicalDriveN path".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod cross_platform_tests {
    use super::*;

    #[test]
    fn rejects_empty_device() {
        assert!(matches!(
            validate_device_path(""),
            Err(CoreError::Validation(_))
        ));
    }

    #[test]
    fn rejects_overlong_device() {
        let long = "/dev/".to_string() + &"a".repeat(300);
        assert!(matches!(
            validate_device_path(&long),
            Err(CoreError::Validation(_))
        ));
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for s in [
            "/dev/sdb;rm -rf /",
            "/dev/sdb|cat",
            "/dev/sdb`whoami`",
            "/dev/sdb$(id)",
            "/dev/sdb && echo",
            "/dev/sdb\nrm",
        ] {
            assert!(
                matches!(validate_device_path(s), Err(CoreError::Validation(_))),
                "expected rejection for {s}"
            );
        }
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(matches!(
            validate_device_path("/dev/../etc/passwd"),
            Err(CoreError::Validation(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn accepts_typical_linux_device() {
        assert!(validate_device_path("/dev/sdb").is_ok());
        assert!(validate_device_path("/dev/nvme0n1").is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accepts_typical_macos_device() {
        assert!(validate_device_path("/dev/disk2").is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn accepts_typical_windows_device() {
        assert!(validate_device_path("\\\\.\\PhysicalDrive1").is_ok());
    }
}
