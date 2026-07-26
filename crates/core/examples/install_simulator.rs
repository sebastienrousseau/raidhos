//! Non-destructive install preview against a sparse file.
//!
//! Runs the full install pipeline — partition, format, payload
//! verify — against a sparse file the user owns instead of a real
//! block device. The destruction happens to the file, never to the
//! host's `/dev/sdX`.
//!
//! This is the answer to "I want to try RaidhOS before trusting it
//! with my USB stick." It exercises exactly the same code path the
//! CI virtual-disk workflow uses (Linux loop device, macOS hdiutil
//! sparse image, Windows VHD).
//!
//! ```bash
//! # Default: 256 MiB sparse file at /tmp/raidhos-sim.img
//! cargo run --example install_simulator -p raidhos-core
//!
//! # Custom path + size
//! cargo run --example install_simulator -p raidhos-core -- /tmp/big.img 1024
//! ```
//!
//! After the run, inspect with `parted`, attach via `losetup` /
//! `hdiutil attach`, or just delete the file.
//!
//! The privileged helper does **not** accept simulator paths —
//! simulator mode is a CLI / library convenience, never a path
//! through the elevated binary.

use raidhos_core as core;

struct StdoutSink;
impl core::ProgressSink for StdoutSink {
    fn emit(&self, event: core::ProgressEvent) {
        let pct = event.percent.map(|p| format!(" {p}%")).unwrap_or_default();
        println!("[{}]{} {}", event.phase, pct, event.message);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "/tmp/raidhos-sim.img".to_string());
    let size_mb: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(256);

    if let Err(e) = ensure_sparse_file(&path, size_mb) {
        eprintln!("simulator setup failed: {e}");
        std::process::exit(1);
    }

    let req = core::InstallRequest {
        device: path.clone(),
        payload_version: "0.0.0-simulator".to_string(),
        wipe: true,
        dry_run: false,    // we actually want to write — to the file, not a device
        allow_write: true, // safe because simulator mode bypasses /dev/* targeting
        simulator: true,
        bios_compat: false,
        data_filesystem: Default::default(),
    };

    println!("Running install pipeline against {path} ({size_mb} MiB sparse)…");
    match core::install(req, &StdoutSink) {
        Ok(()) => {
            println!();
            println!("Done. Inspect with:");
            println!("  parted -m {path} print");
            println!("  sudo losetup -fP {path}   # Linux — attach as loop");
            println!("  hdiutil attach -nomount {path}   # macOS");
            println!();
            println!("Or just delete it:");
            println!("  rm {path}");
        }
        Err(e) => {
            eprintln!("install failed: {e}");
            eprintln!();
            eprintln!("If you see 'RAIDHOS_PAYLOAD_DIR is not set', the");
            eprintln!("install pipeline needs a payload tree to copy. Set");
            eprintln!("the env var before running:");
            eprintln!(
                "  RAIDHOS_PAYLOAD_DIR=/path/to/payload cargo run --example install_simulator"
            );
            std::process::exit(1);
        }
    }
}

/// Create a sparse file at `path` sized to `size_mb` MiB. No-op if
/// the file already exists at the requested size.
fn ensure_sparse_file(path: &str, size_mb: u64) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    let p = std::path::Path::new(path);
    if p.exists() {
        println!("simulator: reusing existing file {path}");
        return Ok(());
    }

    let bytes = size_mb.saturating_mul(1024).saturating_mul(1024);
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(p)?;
    // Single-byte write at the end produces a sparse file on every
    // supported filesystem.
    f.seek(SeekFrom::Start(bytes - 1))?;
    f.write_all(&[0u8])?;
    println!("simulator: created sparse file {path} ({size_mb} MiB)");
    Ok(())
}
