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
            let device_path = match (device, simulator) {
                (Some(d), None) => d,
                (None, Some(s)) => match prepare_simulator(&s, simulator_size_mb) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("simulator setup failed: {e}");
                        std::process::exit(1);
                    }
                },
                (Some(_), Some(_)) => {
                    eprintln!("--device and --simulator are mutually exclusive");
                    std::process::exit(2);
                }
                (None, None) => {
                    eprintln!("install: one of --device or --simulator is required");
                    std::process::exit(2);
                }
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
                match core::verify_iso(
                    &entry,
                    std::path::Path::new(&iso),
                    std::path::Path::new(&sums),
                    std::path::Path::new(&sig),
                    std::path::Path::new(&key_dir),
                ) {
                    Ok(v) => {
                        println!("ok\t{}\tsha256={}", v.entry.name, v.computed_sha256);
                    }
                    Err(e) => {
                        eprintln!("verify failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}
