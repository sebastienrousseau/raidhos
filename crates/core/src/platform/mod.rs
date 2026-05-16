//! Platform backends.
//!
//! Each `target_os` selects exactly one submodule. The public API
//! surfaced by [`super::list_disks`] / [`super::install`] /
//! [`super::scan_isos`] / [`super::list_partitions`] is identical across
//! platforms, but the implementations are wildly different — `lsblk` vs
//! `diskutil` vs `Get-Disk`, etc.

#![allow(missing_docs)] // internal module; public surface is re-exported through lib.rs

use super::IsoEntry;
use crate::Result;

mod scan;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{install, list_disks, list_partitions};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{install, list_disks, list_partitions};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{install, list_disks, list_partitions};

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use unsupported::{install, list_disks, list_partitions};

pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>> {
    scan::scan_isos_fs(dirs)
}
