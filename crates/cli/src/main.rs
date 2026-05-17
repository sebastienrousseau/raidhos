//! `raidhos-cli` — developer-facing CLI over `raidhos-core`. Same
//! discovery and install pipeline as the Tauri UI, exposed as commands.

use clap::Parser;
use raidhos_core as core;

mod cli;
use cli::{CatalogAction, Cli, Commands};

/// Create or reuse a sparse file at `path` and return it as a device
/// path the install pipeline can target. The file is sized to
/// `size_mb` MiB if it does not exist; otherwise its existing size
/// is preserved. The path is returned as-is so the install pipeline's
/// validator sees it the same way it would see `/dev/sdX`.
///
/// This is the "non-destructive preview" the README points at — same
/// install code path as the CI virtual-disk workflow, but the
/// destruction happens to a file the user owns.
fn prepare_simulator(path: &str, size_mb: u64) -> Result<String, String> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
    use std::path::Path;

    let p = Path::new(path);
    if !p.exists() {
        let bytes = size_mb.saturating_mul(1024).saturating_mul(1024);
        if bytes == 0 {
            return Err("simulator size must be > 0 MiB".to_string());
        }
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(p)
            .map_err(|e| format!("create {path}: {e}"))?;
        // Seek + single-byte write produces a sparse file on every
        // supported filesystem.
        f.seek(SeekFrom::Start(bytes - 1))
            .map_err(|e| format!("seek {path}: {e}"))?;
        f.write_all(&[0u8])
            .map_err(|e| format!("write {path}: {e}"))?;
        eprintln!("simulator: created sparse file {path} ({size_mb} MiB)");
    } else {
        eprintln!("simulator: reusing existing file {path}");
    }
    Ok(path.to_string())
}

/// Print one line per disk — matches the documented contract of
/// `raidhos-cli list-disks`. Extracted out of `main()` so the
/// format string is exercised by unit tests with synthetic
/// `DiskInfo` fixtures (the real `core::list_disks()` only
/// returns disks on hosts where lsblk / diskutil / Get-Disk
/// actually sees one).
fn print_disks(disks: &[core::DiskInfo]) {
    for d in disks {
        println!(
            "{} {} {} removable={} system={} mounts={}",
            d.id,
            d.model,
            d.size_bytes,
            d.removable,
            d.is_system,
            d.mountpoints.join(",")
        );
    }
}

/// Print the success line for `raidhos-cli catalog verify`.
/// Extracted out of `main()` so the format string is exercised
/// by unit tests with a synthetic [`core::VerifiedIso`] fixture
/// (the Ok arm of the real `core::verify_iso` needs a complete
/// GPG + SHA256SUMS + ISO chain which is impractical to stage
/// in process-spawn integration tests).
fn print_verified(v: &core::VerifiedIso) {
    println!("ok\t{}\tsha256={}", v.entry.name, v.computed_sha256);
}

/// Dispatch the result of `core::verify_iso`: print on Ok, return
/// an error message on Err. Separating this out of `main()` lets
/// the Ok arm be unit-tested without provisioning a full GPG
/// keyring + SHA256SUMS + signed ISO fixture.
fn handle_verify_result(
    res: std::result::Result<core::VerifiedIso, core::CatalogError>,
) -> std::result::Result<(), String> {
    match res {
        Ok(v) => {
            print_verified(&v);
            Ok(())
        }
        Err(e) => Err(format!("verify failed: {e}")),
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::ListDisks => {
            let disks = match core::list_disks() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("list-disks failed: {e}");
                    std::process::exit(1);
                }
            };
            print_disks(&disks);
        }
        Commands::ScanIsos { dirs } => {
            let entries = match core::scan_isos(dirs) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("scan-isos failed: {e}");
                    std::process::exit(1);
                }
            };
            for e in entries {
                println!("{} {} {} {}", e.title, e.path, e.size_bytes, e.params);
            }
        }
        Commands::Install {
            device,
            simulator,
            simulator_size_mb,
            payload_version,
            wipe,
            dry_run,
            allow_write,
            bios_compat,
            data_fs,
        } => {
            struct StdoutSink;
            impl core::ProgressSink for StdoutSink {
                fn emit(&self, event: core::ProgressEvent) {
                    let pct = event.percent.map(|p| format!("{p}%")).unwrap_or_default();
                    println!("{} {} {}", event.phase, event.message, pct);
                }
            }

            // Resolve a real device path or set up a sparse-file simulator.
            let simulator_mode = simulator.is_some();
            // clap's `conflicts_with` on the args struct already
            // rejects the (Some, Some) case at parse time, so we
            // only have to handle the three reachable states:
            // device-only, simulator-only, or neither.
            let device_path = if let Some(d) = device {
                d
            } else if let Some(s) = simulator {
                match prepare_simulator(&s, simulator_size_mb) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("simulator setup failed: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("install: one of --device or --simulator is required");
                std::process::exit(2);
            };

            let data_filesystem = match core::DataFilesystem::parse(&data_fs) {
                Some(fs) => fs,
                None => {
                    eprintln!(
                        "unknown --data-fs value: {data_fs} (try exfat, ntfs, ext4, btrfs, xfs)",
                    );
                    std::process::exit(2);
                }
            };
            let req = core::InstallRequest {
                device: device_path,
                payload_version,
                wipe,
                dry_run,
                allow_write,
                simulator: simulator_mode,
                bios_compat,
                data_filesystem,
            };
            if let Err(e) = core::install(req, &StdoutSink) {
                eprintln!("install failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::WriteConfig {
            mount_path,
            config_path,
        } => {
            let body = match std::fs::read(&config_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read {config_path}: {e}");
                    std::process::exit(1);
                }
            };
            let dir = std::path::Path::new(&mount_path).join("raidhos");
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("create_dir_all {}: {e}", dir.display());
                std::process::exit(1);
            }
            let path = dir.join("boot.json");
            if let Err(e) = std::fs::write(&path, body) {
                eprintln!("write {}: {e}", path.display());
                std::process::exit(1);
            }
        }
        Commands::Catalog { action } => match action {
            CatalogAction::List => {
                let catalog = match core::load_catalog() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("catalog: {e}");
                        std::process::exit(1);
                    }
                };
                for entry in catalog {
                    println!("{}\t{}", entry.slug, entry.name);
                }
            }
            CatalogAction::Verify {
                slug,
                iso,
                sums,
                sig,
                key_dir,
            } => {
                let catalog = match core::load_catalog() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("catalog: {e}");
                        std::process::exit(1);
                    }
                };
                let entry = match core::find_entry(&catalog, &slug) {
                    Some(e) => e.clone(),
                    None => {
                        eprintln!("no catalog entry with slug {slug}");
                        std::process::exit(1);
                    }
                };
                let res = core::verify_iso(
                    &entry,
                    std::path::Path::new(&iso),
                    std::path::Path::new(&sums),
                    std::path::Path::new(&sig),
                    std::path::Path::new(&key_dir),
                );
                if let Err(msg) = handle_verify_result(res) {
                    eprintln!("{msg}");
                    std::process::exit(1);
                }
            }
        },
    }
}

#[cfg(test)]
mod print_tests {
    use super::*;

    fn disk_fixture(id: &str) -> core::DiskInfo {
        core::DiskInfo {
            id: id.into(),
            model: "Test".into(),
            size_bytes: 1024,
            removable: true,
            mountpoints: vec!["/mnt/u".into()],
            is_system: false,
        }
    }

    /// `print_disks` runs without panicking on every reasonable
    /// fixture — empty slice, single disk, multiple disks. The
    /// println output is dropped on the floor but the line
    /// coverage of the format string is what matters.
    #[test]
    fn print_disks_handles_empty_slice() {
        print_disks(&[]);
    }

    #[test]
    fn print_disks_iterates_each_disk() {
        let disks = vec![
            disk_fixture("/dev/sdb"),
            disk_fixture("/dev/sdc"),
            disk_fixture("/dev/sdd"),
        ];
        print_disks(&disks);
    }

    /// `print_verified` similarly: just runs the format string
    /// against a fixture VerifiedIso. Exercises lines that the
    /// integration tests can't reach (the Ok arm of verify_iso
    /// needs a real GPG + sums + sig + ISO chain).
    fn entry_fixture() -> core::CatalogEntry {
        core::CatalogEntry {
            name: "Test 1.0".into(),
            slug: "test-1.0".into(),
            iso_url: String::new(),
            sha256sums_url: String::new(),
            sha256sums_sig_url: String::new(),
            gpg_fingerprint: String::new(),
            iso_filename: "test.iso".into(),
        }
    }

    #[test]
    fn print_verified_prints_ok_line() {
        let v = core::VerifiedIso {
            entry: entry_fixture(),
            computed_sha256: "deadbeef".repeat(8),
        };
        print_verified(&v);
    }

    #[test]
    fn handle_verify_result_ok_invokes_print_and_returns_ok() {
        let v = core::VerifiedIso {
            entry: entry_fixture(),
            computed_sha256: "deadbeef".repeat(8),
        };
        let res = handle_verify_result(Ok(v));
        assert!(res.is_ok());
    }

    #[test]
    fn handle_verify_result_err_wraps_message() {
        let res = handle_verify_result(Err(core::CatalogError::Io("simulated".into())));
        let msg = res.unwrap_err();
        assert!(msg.starts_with("verify failed: "), "got: {msg}");
        assert!(msg.contains("simulated"), "got: {msg}");
    }
}
