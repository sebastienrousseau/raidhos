use clap::{Parser, Subcommand};
use raidhos_core as core;

#[derive(Parser)]
#[command(name = "raidhos-cli", version, about = "RaidhOS CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ListDisks,
    ScanIsos {
        #[arg(long, value_delimiter = ',', default_value = "/media,/mnt,/home")]
        dirs: Vec<String>,
    },
    Install {
        #[arg(long)]
        device: String,
        #[arg(long, default_value = "0.1.0")]
        payload_version: String,
        #[arg(long, default_value_t = true)]
        wipe: bool,
        #[arg(long, default_value_t = true)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        allow_write: bool,
    },
    WriteConfig {
        #[arg(long)]
        mount_path: String,
        #[arg(long)]
        config_path: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::ListDisks => {
            let disks = core::list_disks().expect("list_disks failed");
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
            let entries = core::scan_isos(dirs).expect("scan_isos failed");
            for e in entries {
                println!("{} {} {} {}", e.title, e.path, e.size_bytes, e.params);
            }
        }
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
            core::install(req, &StdoutSink).expect("install failed");
        }
        Commands::WriteConfig {
            mount_path,
            config_path,
        } => {
            let body = std::fs::read(&config_path).expect("read config");
            let dir = std::path::Path::new(&mount_path).join("raidhos");
            std::fs::create_dir_all(&dir).expect("create dir");
            let path = dir.join("boot.json");
            std::fs::write(path, body).expect("write config");
        }
    }
}
