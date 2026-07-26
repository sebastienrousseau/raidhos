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
//!     simulator: false,
//!     bios_compat: false,
//!     data_filesystem: Default::default(),
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
mod persistence_labels;
mod platform;
mod runtime;

pub use persistence_labels::{
    all_labels as persistence_labels, expected_persistence_label,
    DEFAULT_LABEL as DEFAULT_PERSISTENCE_LABEL,
};

pub use catalog::{
    find_entry, load_catalog, load_catalog_from, sha256sums_lookup_pub as sha256sums_lookup,
    verify_iso, verify_iso_companion_sha256, verify_iso_sha256, verify_iso_with, CatalogEntry,
    CatalogError, VerifiedIso,
};
pub use payload::{verify_payload, ManifestError, PayloadManifest};
pub use runtime::{Invocation, MockOutcome, MockRuntime, RealRuntime, Runtime};

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
///
/// # Examples
///
/// ```
/// use raidhos_core::CoreError;
/// let e = CoreError::Validation("device must be an absolute path".into());
/// assert_eq!(format!("{e}"), "validation error: device must be an absolute path");
/// ```
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
///
/// # Examples
///
/// ```
/// let d = raidhos_core::DiskInfo {
///     id: "/dev/sdb".into(), model: "USB".into(), size_bytes: 16_000_000_000,
///     removable: true, mountpoints: vec![], is_system: false,
/// };
/// assert!(d.removable && !d.is_system);
/// ```
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
///
/// # Examples
///
/// ```
/// let p = raidhos_core::PartitionInfo {
///     id: "/dev/sdb1".into(), label: "RAIDHOS_EFI".into(),
///     fstype: "vfat".into(), mountpoints: vec![],
/// };
/// assert_eq!(p.fstype, "vfat");
/// ```
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
///
/// # Examples
///
/// ```
/// // Safe defaults: dry-run with allow_write off.
/// let req = raidhos_core::InstallRequest {
///     device: "/dev/sdb".into(),
///     payload_version: "0.1.0".into(),
///     wipe: true,
///     dry_run: true,
///     allow_write: false,
///     simulator: false,
///     bios_compat: false,
///     data_filesystem: Default::default(),
/// };
/// assert!(req.dry_run && !req.allow_write);
/// ```
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
    /// If `true`, `device` is a sparse-file simulator (not a real
    /// block device). The per-OS path-shape check is skipped; the
    /// shell-metachar / length / `..` checks still apply. Used by
    /// `raidhos-cli install --simulator`.
    #[serde(default)]
    pub simulator: bool,
    /// If `true`, install a Legacy-BIOS-bootable layout in addition
    /// to UEFI: a protective MBR + hybrid GPT, with a BIOS Boot
    /// Partition (1 MiB, type `ef02`) at the front so `grub-install
    /// --target=i386-pc` has somewhere to embed.
    ///
    /// Closes Ventoy gap G1. v0.0.1 plumbs the request through the
    /// validate stage and the partition planner; the actual BIOS
    /// GRUB embedding is gated to v0.0.2 once we vendor an
    /// `i386-pc` GRUB build alongside the existing `x86_64-efi` one.
    /// In v0.0.1 this flag is accepted but produces a documented
    /// `CoreError::NotImplemented` when `dry_run` is false.
    #[serde(default)]
    pub bios_compat: bool,
    /// Filesystem to use on the DATA partition. Default `ExFat`
    /// matches v0.0.1 behaviour and the v0.0.1 GRUB build's loaded
    /// modules. Closes Ventoy gap G8 (NTFS) and the lower halves
    /// of G9 (ext4, Btrfs, XFS).
    ///
    /// Per-platform support, refused at validate time when the
    /// platform's install pipeline can't create the filesystem
    /// requested (e.g. `Ntfs` on macOS without ntfs-3g installed).
    #[serde(default)]
    pub data_filesystem: DataFilesystem,
}

/// Filesystem options for the DATA partition.
///
/// `ExFat` is the v0.0.1 default — readable read/write from Linux,
/// macOS, and Windows out of the box, and supports > 4 GiB files
/// (unlike FAT32). Other variants close Ventoy gaps G8 and G9 for
/// users with specific compatibility needs.
///
/// Wire form is lowercase (`"exfat"`, `"ntfs"`, …) so the JSON
/// stays human-friendly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataFilesystem {
    /// exFAT — default. Cross-platform, > 4 GiB files supported.
    #[default]
    ExFat,
    /// NTFS — read/write on Windows native; readable on macOS via
    /// 3rd-party drivers; ntfs-3g on Linux. Closes Ventoy gap G8.
    Ntfs,
    /// ext4 — Linux-native, mountable read-write only on Linux.
    Ext4,
    /// Btrfs — Linux-native, supports snapshots and compression.
    Btrfs,
    /// XFS — Linux-native, optimised for large files.
    Xfs,
}

impl DataFilesystem {
    /// Lowercase wire-format name (`"exfat"`, `"ntfs"`, …).
    ///
    /// # Examples
    ///
    /// ```
    /// use raidhos_core::DataFilesystem;
    /// assert_eq!(DataFilesystem::ExFat.as_str(), "exfat");
    /// assert_eq!(DataFilesystem::Ntfs.as_str(), "ntfs");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            DataFilesystem::ExFat => "exfat",
            DataFilesystem::Ntfs => "ntfs",
            DataFilesystem::Ext4 => "ext4",
            DataFilesystem::Btrfs => "btrfs",
            DataFilesystem::Xfs => "xfs",
        }
    }

    /// Parse from a case-insensitive string. Used by the CLI's
    /// `--data-fs` flag and by JSON deserialisation.
    ///
    /// Named `parse` rather than `from_str` to avoid shadowing the
    /// `std::str::FromStr::from_str` trait method.
    ///
    /// # Examples
    ///
    /// ```
    /// use raidhos_core::DataFilesystem;
    /// assert_eq!(DataFilesystem::parse("ExFAT"), Some(DataFilesystem::ExFat));
    /// assert_eq!(DataFilesystem::parse("NTFS"),  Some(DataFilesystem::Ntfs));
    /// assert_eq!(DataFilesystem::parse("ext4"),  Some(DataFilesystem::Ext4));
    /// assert_eq!(DataFilesystem::parse("btrfs"), Some(DataFilesystem::Btrfs));
    /// assert_eq!(DataFilesystem::parse("xfs"),   Some(DataFilesystem::Xfs));
    /// assert_eq!(DataFilesystem::parse("zfs"),   None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "exfat" => Some(DataFilesystem::ExFat),
            "ntfs" => Some(DataFilesystem::Ntfs),
            "ext4" => Some(DataFilesystem::Ext4),
            "btrfs" => Some(DataFilesystem::Btrfs),
            "xfs" => Some(DataFilesystem::Xfs),
            _ => None,
        }
    }
}

/// Progress event emitted by the install pipeline.
///
/// # Examples
///
/// ```
/// let e = raidhos_core::ProgressEvent {
///     phase: "format".into(),
///     message: "Formatting FAT32 ESP".into(),
///     percent: Some(60),
/// };
/// assert_eq!(e.percent, Some(60));
/// ```
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
///
/// # Examples
///
/// ```
/// let iso = raidhos_core::IsoEntry {
///     title: "ubuntu".into(),
///     path: "/Downloads/ubuntu-24.04.iso".into(),
///     size_bytes: 4_500_000_000,
///     params: "quiet splash".into(),
/// };
/// assert!(iso.path.ends_with(".iso"));
/// ```
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
///
/// # Examples
///
/// ```
/// use raidhos_core::{ProgressEvent, ProgressSink};
/// struct StdoutSink;
/// impl ProgressSink for StdoutSink {
///     fn emit(&self, e: ProgressEvent) {
///         println!("[{}] {}", e.phase, e.message);
///     }
/// }
/// StdoutSink.emit(ProgressEvent {
///     phase: "validate".into(), message: "ok".into(), percent: Some(5),
/// });
/// ```
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
///     simulator: false,
///     bios_compat: false,
///     data_filesystem: Default::default(),
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
///
/// # Examples
///
/// ```no_run
/// let parts = raidhos_core::list_partitions("/dev/sdb".to_string()).unwrap();
/// for p in parts {
///     println!("{} {} {:?}", p.id, p.fstype, p.mountpoints);
/// }
/// ```
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
    validate_device_path_inner(device, false)
}

/// Variant of [`validate_device_path`] that accepts a sparse-file
/// simulator path. The shell-metachar / length / `..` checks still
/// apply, but the per-OS path-shape gate (`/dev/` on Linux,
/// `/dev/disk` on macOS, `\\.\PhysicalDrive` on Windows) is skipped.
///
/// Used by `raidhos-cli install --simulator` and by callers
/// constructing an [`InstallRequest`] with `simulator: true`.
pub fn validate_device_path_simulator(device: &str) -> Result<()> {
    validate_device_path_inner(device, true)
}

/// Target-OS-aware validator. Same checks as
/// [`validate_device_path`] but with the shape gate decoupled from
/// the host's compile-time `target_os`. Used by the per-platform
/// `install_with` implementations so they remain unit-testable
/// from any host (Linux CI tarpaulin in particular).
///
/// `target` accepts `"linux"`, `"macos"`, `"windows"`, or
/// `"simulator"` (= skip the shape gate).
pub(crate) fn validate_device_path_for_target(device: &str, target: &str) -> Result<()> {
    // Shared checks: empty / overlong / shell-metachar / path traversal.
    validate_device_path_common(device)?;
    match target {
        "simulator" => Ok(()),
        "linux" => {
            if device.starts_with("/dev/") {
                Ok(())
            } else {
                Err(CoreError::Validation(
                    "device must be an absolute /dev path".to_string(),
                ))
            }
        }
        "macos" => {
            if device.starts_with("/dev/disk") {
                Ok(())
            } else {
                Err(CoreError::Validation(
                    "device must be a /dev/diskN path".to_string(),
                ))
            }
        }
        "windows" => {
            let lower = device.to_ascii_lowercase();
            if lower.starts_with("\\\\.\\physicaldrive")
                || lower.starts_with("\\\\?\\physicaldrive")
            {
                Ok(())
            } else {
                Err(CoreError::Validation(
                    "device must be a \\\\.\\PhysicalDriveN path".to_string(),
                ))
            }
        }
        other => Err(CoreError::Validation(format!(
            "unknown validation target: {other}"
        ))),
    }
}

fn validate_device_path_common(device: &str) -> Result<()> {
    if device.is_empty() {
        return Err(CoreError::Validation("device path is empty".to_string()));
    }
    if device.len() > 256 {
        return Err(CoreError::Validation(
            "device path is unreasonably long".to_string(),
        ));
    }
    // `\\` is *not* in this list because Windows device paths
    // legitimately contain backslashes (`\\.\PhysicalDriveN`). The
    // per-OS shape check still rejects malformed paths on every
    // platform.
    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>', '"', '\'', '*', '?', '(', ')',
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
    Ok(())
}

fn validate_device_path_inner(device: &str, simulator: bool) -> Result<()> {
    if simulator {
        return validate_device_path_for_target(device, "simulator");
    }
    // Compile-time target selection so each host build only sees its
    // own arm — keeps coverage at 100% per host (rather than running
    // a `cfg!()` runtime check whose dead arms always show uncovered).
    #[cfg(target_os = "linux")]
    {
        validate_device_path_for_target(device, "linux")
    }
    #[cfg(target_os = "macos")]
    {
        validate_device_path_for_target(device, "macos")
    }
    #[cfg(target_os = "windows")]
    {
        validate_device_path_for_target(device, "windows")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Unsupported host: only the common checks apply.
        validate_device_path_common(device)
    }
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

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_relative_path_on_linux() {
        assert!(matches!(
            validate_device_path("sdb"),
            Err(CoreError::Validation(_))
        ));
        assert!(matches!(
            validate_device_path("dev/sdb"),
            Err(CoreError::Validation(_))
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accepts_typical_macos_device() {
        assert!(validate_device_path("/dev/disk2").is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_non_disk_path_on_macos() {
        assert!(matches!(
            validate_device_path("/dev/null"),
            Err(CoreError::Validation(_))
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn accepts_typical_windows_device() {
        assert!(validate_device_path("\\\\.\\PhysicalDrive1").is_ok());
        // Note: the long-path form `\\?\PhysicalDrive1` exists in Win32
        // but is rejected here because `?` is in FORBIDDEN as a
        // shell-metachar defence. Callers should use the `\\.\…` form.
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_drive_letter_path_on_windows() {
        // C:\… is not a physical-drive handle; refuse.
        assert!(matches!(
            validate_device_path("C:\\Windows"),
            Err(CoreError::Validation(_))
        ));
    }

    #[test]
    fn rejects_each_forbidden_metacharacter() {
        for ch in [
            ';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>', '"', '\'', '*', '?', '(', ')',
        ] {
            let s = format!("/dev/sdb{ch}");
            assert!(
                matches!(validate_device_path(&s), Err(CoreError::Validation(_))),
                "expected rejection for {ch:?}",
            );
        }
    }

    #[test]
    fn validate_device_path_for_target_linux_accepts_dev_path() {
        assert!(validate_device_path_for_target("/dev/sdb", "linux").is_ok());
        assert!(validate_device_path_for_target("/dev/nvme0n1", "linux").is_ok());
        assert!(validate_device_path_for_target("sdb", "linux").is_err());
    }

    #[test]
    fn validate_device_path_for_target_macos_accepts_dev_disk() {
        assert!(validate_device_path_for_target("/dev/disk2", "macos").is_ok());
        assert!(validate_device_path_for_target("/dev/sdb", "macos").is_err());
        assert!(validate_device_path_for_target("/Users/me/x", "macos").is_err());
    }

    #[test]
    fn validate_device_path_for_target_windows_accepts_physicaldrive() {
        assert!(validate_device_path_for_target("\\\\.\\PhysicalDrive1", "windows").is_ok());
        assert!(validate_device_path_for_target("\\\\.\\physicaldrive9", "windows").is_ok());
        assert!(validate_device_path_for_target("/dev/sdb", "windows").is_err());
        assert!(validate_device_path_for_target("C:\\Windows", "windows").is_err());
    }

    #[test]
    fn validate_device_path_for_target_simulator_accepts_anything_safe() {
        assert!(validate_device_path_for_target("/tmp/raidhos.img", "simulator").is_ok());
        assert!(validate_device_path_for_target("./out/test.img", "simulator").is_ok());
        // Even simulator mode rejects shell metachars.
        assert!(validate_device_path_for_target("/tmp/a;rm", "simulator").is_err());
        assert!(validate_device_path_for_target("/tmp/../etc", "simulator").is_err());
        assert!(validate_device_path_for_target("", "simulator").is_err());
    }

    #[test]
    fn validate_device_path_for_target_rejects_unknown_target_string() {
        let err = validate_device_path_for_target("/dev/sdb", "freebsd").unwrap_err();
        assert!(
            matches!(&err, CoreError::Validation(s) if s.contains("unknown validation target")),
            "got: {err:?}",
        );
    }

    #[test]
    fn coreerror_display_renders_each_variant() {
        // Lock in the wire-stable text so the CLI / UI grep paths keep
        // matching across releases.
        assert_eq!(
            format!("{}", CoreError::UnsupportedPlatform),
            "unsupported platform"
        );
        assert_eq!(
            format!("{}", CoreError::Io("boom".into())),
            "io error: boom"
        );
        assert_eq!(
            format!("{}", CoreError::Validation("x".into())),
            "validation error: x"
        );
        assert_eq!(
            format!("{}", CoreError::NotImplemented("y".into())),
            "not implemented: y"
        );
        assert_eq!(
            format!("{}", CoreError::Parse("z".into())),
            "parse error: z"
        );
    }

    #[test]
    fn install_request_round_trips_through_json() {
        let req = InstallRequest {
            device: "/dev/sdb".into(),
            payload_version: "0.1.0".into(),
            wipe: true,
            dry_run: true,
            allow_write: false,
            simulator: false,
            bios_compat: false,
            data_filesystem: Default::default(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: InstallRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.device, "/dev/sdb");
        assert!(back.wipe);
        assert!(back.dry_run);
        assert!(!back.allow_write);
    }

    #[test]
    fn progress_event_round_trips_through_json() {
        let ev = ProgressEvent {
            phase: "format".into(),
            message: "exFAT".into(),
            percent: Some(42),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: ProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.phase, "format");
        assert_eq!(back.percent, Some(42));
    }

    #[test]
    fn disk_info_round_trips_through_json() {
        let d = DiskInfo {
            id: "/dev/sdb".into(),
            model: "USB".into(),
            size_bytes: 16_000_000_000,
            removable: true,
            mountpoints: vec!["/mnt/x".into()],
            is_system: false,
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: DiskInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "/dev/sdb");
        assert_eq!(back.size_bytes, 16_000_000_000);
        assert!(back.removable);
    }

    #[test]
    fn iso_entry_round_trips_through_json() {
        let e = IsoEntry {
            title: "ubuntu".into(),
            path: "/iso/ubuntu.iso".into(),
            size_bytes: 4096,
            params: "boot=casper".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: IsoEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back.title, "ubuntu");
        assert_eq!(back.params, "boot=casper");
    }

    // ---------------------------------------------------------------
    // DataFilesystem — round-trip + parse coverage
    // ---------------------------------------------------------------

    #[test]
    fn data_filesystem_as_str_round_trip() {
        for fs in [
            DataFilesystem::ExFat,
            DataFilesystem::Ntfs,
            DataFilesystem::Ext4,
            DataFilesystem::Btrfs,
            DataFilesystem::Xfs,
        ] {
            assert_eq!(DataFilesystem::parse(fs.as_str()), Some(fs));
        }
    }

    #[test]
    fn data_filesystem_parse_is_case_insensitive() {
        for (s, fs) in [
            ("EXFAT", DataFilesystem::ExFat),
            ("NTFS", DataFilesystem::Ntfs),
            ("Ext4", DataFilesystem::Ext4),
            ("BTRFS", DataFilesystem::Btrfs),
            ("xfs", DataFilesystem::Xfs),
        ] {
            assert_eq!(DataFilesystem::parse(s), Some(fs));
        }
    }

    #[test]
    fn data_filesystem_parse_rejects_unknown() {
        for s in ["", "zfs", "hfs", "fat32", "ntfs3", " ntfs"] {
            assert_eq!(DataFilesystem::parse(s), None, "{s} should fail");
        }
    }

    #[test]
    fn data_filesystem_default_is_exfat() {
        assert_eq!(DataFilesystem::default(), DataFilesystem::ExFat);
    }

    // ---------------------------------------------------------------
    // Platform-dispatch one-liners — exercise the call so coverage
    // doesn't count them as dead. Errors are fine; we just want the
    // dispatch line to fire.
    // ---------------------------------------------------------------

    #[test]
    fn dispatch_list_disks_returns_or_errors_cleanly() {
        // On every CI host the call returns Ok(_) or Err(_); a panic
        // would be a regression.
        let _ = list_disks();
    }

    #[test]
    fn dispatch_scan_isos_returns_for_empty_input() {
        let res = scan_isos(vec![]).expect("empty dirs is not an error");
        assert!(res.is_empty());
    }

    #[test]
    fn dispatch_list_partitions_handles_nonexistent_device() {
        // The device doesn't exist; the call should return cleanly
        // (Ok(empty) on hosts without the device, or Err(_) on hosts
        // that try to probe). Either way: no panic.
        let _ = list_partitions("/dev/nonexistent-raidhos-test".to_string());
    }

    #[test]
    fn dispatch_install_dry_run_returns() {
        struct Quiet;
        impl ProgressSink for Quiet {
            fn emit(&self, _: ProgressEvent) {}
        }
        let req = InstallRequest {
            device: "/dev/nonexistent-raidhos-test".to_string(),
            payload_version: "0.1.0".to_string(),
            wipe: true,
            dry_run: true,
            allow_write: false,
            simulator: false,
            bios_compat: false,
            data_filesystem: DataFilesystem::ExFat,
        };
        // Validate / dispatch must not panic. Refusal is fine.
        let _ = install(req, &Quiet);
    }

    // ---------------------------------------------------------------
    // validate_device_path_simulator — covers the "simulator" branch
    // of validate_device_path_inner (lib.rs:584)
    // ---------------------------------------------------------------

    #[test]
    fn simulator_validator_accepts_sparse_file_path() {
        // Any reasonable filesystem path is accepted under the
        // simulator gate — only the metachar / length / `..` checks
        // apply.
        assert!(validate_device_path_simulator("/tmp/raidhos-sim.img").is_ok());
        assert!(validate_device_path_simulator("./sim.img").is_ok());
        assert!(validate_device_path_simulator("sim.img").is_ok());
    }

    #[test]
    fn simulator_validator_still_rejects_metachars() {
        assert!(validate_device_path_simulator("/tmp/sim; rm -rf /").is_err());
        assert!(validate_device_path_simulator("/tmp/sim`whoami`").is_err());
        assert!(validate_device_path_simulator("/tmp/../etc/passwd").is_err());
        assert!(validate_device_path_simulator("").is_err());
    }
}
