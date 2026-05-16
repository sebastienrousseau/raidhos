//! Privileged helper for RaidhOS.
//!
//! Invoked by the UI or CLI through an elevation path (`pkexec`, `sudo`,
//! UAC). Accepts a small set of subcommands and emits a single JSON
//! object on stdout so callers can parse results regardless of
//! language.
//!
//! This binary runs as root. Every input it parses must be
//! hostile-input-safe — see `docs/THREAT_MODEL.md`. Argument parsing
//! uses `clap` with `deny_hyphen_values`-style strictness so unknown
//! flags fail loudly rather than being silently ignored.

use clap::{Parser, Subcommand};
use raidhos_core as core;
use serde::Serialize;

/// Hard upper bound on combined argv byte length. Defence against
/// resource-exhaustion attacks; a real install request fits in a few
/// hundred bytes.
const MAX_ARGV_BYTES: usize = 64 * 1024;

#[derive(Parser)]
#[command(
    name = "raidhos-priv-helper",
    version,
    about = "RaidhOS privileged helper"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Enumerate physical disks.
    ListDisks,
    /// List partitions on a specific disk.
    ListPartitions {
        /// Device path (`/dev/sdX`, `/dev/diskN`, `\\.\PhysicalDriveN`).
        device: String,
    },
    /// Run the install pipeline.
    Install(InstallArgs),
}

#[derive(clap::Args)]
struct InstallArgs {
    /// Target device path.
    #[arg(long)]
    device: String,
    /// Payload version string (must match `payload/manifest.json`).
    #[arg(long, default_value = "0.1.0")]
    payload_version: String,
    /// Run every check but write nothing.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    /// Required to actually touch the device. Defaults to off.
    #[arg(long, default_value_t = false)]
    allow_write: bool,
    /// Set to disable the `wipe` opt-in (debug only — install will
    /// refuse).
    #[arg(long, default_value_t = false)]
    no_wipe: bool,
}

#[derive(Serialize)]
struct HelperResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> HelperResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

fn print_response<T: Serialize>(resp: &HelperResponse<T>) {
    match serde_json::to_string_pretty(resp) {
        Ok(s) => println!("{s}"),
        Err(e) => println!("{{\"ok\":false,\"data\":null,\"error\":\"serialize failed: {e}\"}}"),
    }
}

fn enforce_argv_budget() -> Result<(), String> {
    let total: usize = std::env::args().map(|a| a.len() + 1).sum();
    if total > MAX_ARGV_BYTES {
        return Err(format!(
            "combined argv length {total} exceeds limit of {MAX_ARGV_BYTES}"
        ));
    }
    Ok(())
}

fn main() {
    if let Err(e) = enforce_argv_budget() {
        print_response(&HelperResponse::<()>::err(e));
        std::process::exit(2);
    }

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // clap formats its own errors; surface them as JSON so the
            // calling UI/CLI doesn't have to parse stderr.
            print_response(&HelperResponse::<()>::err(e.to_string()));
            std::process::exit(2);
        },
    };

    match cli.command {
        Command::ListDisks => {
            let resp = match core::list_disks() {
                Ok(disks) => HelperResponse::ok(disks),
                Err(err) => HelperResponse::<Vec<core::DiskInfo>>::err(err.to_string()),
            };
            print_response(&resp);
        },
        Command::ListPartitions { device } => {
            let resp = match core::list_partitions(device) {
                Ok(parts) => HelperResponse::ok(parts),
                Err(err) => HelperResponse::<Vec<core::PartitionInfo>>::err(err.to_string()),
            };
            print_response(&resp);
        },
        Command::Install(args) => {
            let req = core::InstallRequest {
                device: args.device,
                payload_version: args.payload_version,
                wipe: !args.no_wipe,
                dry_run: args.dry_run,
                allow_write: args.allow_write,
            };
            let sink = StderrSink;
            let resp = match core::install(req, &sink) {
                Ok(_) => HelperResponse::ok(()),
                Err(err) => HelperResponse::<()>::err(err.to_string()),
            };
            print_response(&resp);
        },
    }
}

struct StderrSink;

impl core::ProgressSink for StderrSink {
    fn emit(&self, event: core::ProgressEvent) {
        eprintln!("{}: {}", event.phase, event.message);
    }
}
