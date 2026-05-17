//! `raidhos-cli` — developer-facing CLI over `raidhos-core`. Same
//! discovery and install pipeline as the Tauri UI, exposed as commands.

use clap::Parser;
use raidhos_core as core;

mod cli;
use cli::{CatalogAction, Cli, Commands};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::ListDisks => {
            let disks = match core::list_disks() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("list-disks failed: {e}");
                    std::process::exit(1);
                },
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
        },
        Commands::ScanIsos { dirs } => {
            let entries = match core::scan_isos(dirs) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("scan-isos failed: {e}");
                    std::process::exit(1);
                },
            };
            for e in entries {
                println!("{} {} {} {}", e.title, e.path, e.size_bytes, e.params);
            }
        },
        Commands::Install {
            device,
            payload_version,
            wipe,
            dry_run,
            allow_write,
        } => {
            struct StdoutSink;
            impl core::ProgressSink for StdoutSink {
                fn emit(&self, event: core::ProgressEvent) {
                    let pct = event.percent.map(|p| format!("{p}%")).unwrap_or_default();
                    println!("{} {} {}", event.phase, event.message, pct);
                }
            }

            let req = core::InstallRequest {
                device,
                payload_version,
                wipe,
                dry_run,
                allow_write,
            };
            if let Err(e) = core::install(req, &StdoutSink) {
                eprintln!("install failed: {e}");
                std::process::exit(1);
            }
        },
        Commands::WriteConfig {
            mount_path,
            config_path,
        } => {
            let body = match std::fs::read(&config_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read {config_path}: {e}");
                    std::process::exit(1);
                },
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
        },
        Commands::Catalog { action } => match action {
            CatalogAction::List => {
                let catalog = match core::load_catalog() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("catalog: {e}");
                        std::process::exit(1);
                    },
                };
                for entry in catalog {
                    println!("{}\t{}", entry.slug, entry.name);
                }
            },
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
                    },
                };
                let entry = match core::find_entry(&catalog, &slug) {
                    Some(e) => e.clone(),
                    None => {
                        eprintln!("no catalog entry with slug {slug}");
                        std::process::exit(1);
                    },
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
                    },
                    Err(e) => {
                        eprintln!("verify failed: {e}");
                        std::process::exit(1);
                    },
                }
            },
        },
    }
}
