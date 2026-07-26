//! Platform backends.
//!
//! Each backend module is compiled on every host (so its
//! Runtime-driven tests can run under tarpaulin on a Linux CI
//! runner), but only one set of public functions is **re-exported**
//! at any time — picked by `#[cfg(target_os = "...")]` at the
//! re-export level.

#![allow(missing_docs)]

use super::IsoEntry;
use crate::Result;

mod scan;

mod linux;
mod macos;
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{install, list_disks, list_partitions};

#[cfg(target_os = "macos")]
pub use macos::{install, list_disks, list_partitions};

#[cfg(target_os = "windows")]
pub use windows::{install, list_disks, list_partitions};

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use unsupported::{install, list_disks, list_partitions};

pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>> {
    scan::scan_isos_fs(dirs)
}
