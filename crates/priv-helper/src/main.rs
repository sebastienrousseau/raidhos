//! Privileged helper for RaidhOS.
//!
//! Invoked by the UI or CLI through an elevation path (`pkexec`,
//! `sudo`, UAC). Accepts a small set of subcommands and emits a
//! single JSON object on stdout so callers can parse results
//! regardless of language.
//!
//! This binary runs as root. Every input it parses must be
//! hostile-input-safe — see `docs/THREAT_MODEL.md`. Argument parsing
//! uses `clap` with `deny_hyphen_values`-style strictness so unknown
//! flags fail loudly rather than being silently ignored.

use clap::{Parser, Subcommand};
use raidhos_core as core;
use serde::Serialize;

#[cfg(target_os = "linux")]
mod seccomp;
#[cfg(target_os = "linux")]
mod toctou;

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
    /// Persistence overlay size in MiB. 0 disables persistence.
    #[arg(long, default_value_t = 0)]
    persistence_mb: u64,
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
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                // Treat help/version as success, not as a JSON error.
                print!("{e}");
                std::process::exit(0);
            }
            print_response(&HelperResponse::<()>::err(e.to_string()));
            std::process::exit(2);
        }
    };

    // Apply the seccomp denylist on Linux. Non-fatal if the kernel
    // rejects it — the rest of the safeguards still apply.
    #[cfg(target_os = "linux")]
    if let Err(e) = seccomp::install_denylist() {
        eprintln!("seccomp filter not installed: {e}");
    }

    // Final exit code derived from whether the operation succeeded.
    // Helper returns:
    //   0 — `{ok: true}`
    //   1 — `{ok: false, error: ...}`  (runtime / validation failure)
    //   2 — clap parse failure (already exited above)
    let mut ok = true;

    match cli.command {
        Command::ListDisks => {
            let resp = match core::list_disks() {
                Ok(disks) => HelperResponse::ok(disks),
                Err(err) => {
                    ok = false;
                    HelperResponse::<Vec<core::DiskInfo>>::err(err.to_string())
                }
            };
            print_response(&resp);
        }
        Command::ListPartitions { device } => {
            let resp = match core::list_partitions(device) {
                Ok(parts) => HelperResponse::ok(parts),
                Err(err) => {
                    ok = false;
                    HelperResponse::<Vec<core::PartitionInfo>>::err(err.to_string())
                }
            };
            print_response(&resp);
        }
        Command::Install(args) => {
            // TOCTOU defence on Linux: pin the device fd before we
            // hand off to the install pipeline.
            #[cfg(target_os = "linux")]
            let _pin = if args.allow_write && !args.dry_run {
                match toctou::pin_device(&args.device) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        print_response(&HelperResponse::<()>::err(format!(
                            "device pin failed: {e}"
                        )));
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            let req = core::InstallRequest {
                device: args.device,
                payload_version: args.payload_version,
                wipe: !args.no_wipe,
                dry_run: args.dry_run,
                allow_write: args.allow_write,
                // The privileged helper never accepts simulator paths.
                // Simulator mode is a CLI-only convenience for safe
                // preview against a sparse file; the helper exists
                // solely to write real block devices behind pkexec.
                simulator: false,
            };
            let sink = StderrSink;
            let install_result = core::install(req, &sink).map_err(|e| e.to_string());

            // Persistence overlay (Linux only, post-install).
            #[cfg(target_os = "linux")]
            let persistence_result = if install_result.is_ok()
                && args.persistence_mb > 0
                && args.allow_write
                && !args.dry_run
            {
                create_persistence(args.persistence_mb).err()
            } else {
                None
            };
            #[cfg(not(target_os = "linux"))]
            let persistence_result: Option<String> = None;

            let resp = match (install_result, persistence_result) {
                (Ok(_), None) => HelperResponse::ok(()),
                (Ok(_), Some(p_err)) => {
                    ok = false;
                    HelperResponse::<()>::err(format!(
                        "install succeeded but persistence creation failed: {p_err}"
                    ))
                }
                (Err(e), _) => {
                    ok = false;
                    HelperResponse::<()>::err(e)
                }
            };
            print_response(&resp);
        }
    }

    if !ok {
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn create_persistence(size_mb: u64) -> Result<(), String> {
    use std::process::Command;

    // After install, the DATA partition is unmounted (per
    // payload_copy). Find it again by label and remount.
    let mount = std::path::Path::new("/mnt/raidhos-data-persist");
    std::fs::create_dir_all(mount)
        .map_err(|e| format!("create_dir_all {}: {e}", mount.display()))?;

    let dev = Command::new("blkid")
        .args(["-L", "DATA"])
        .output()
        .map_err(|e| format!("blkid: {e}"))?;
    if !dev.status.success() {
        return Err("blkid: DATA label not found".to_string());
    }
    let dev = String::from_utf8_lossy(&dev.stdout).trim().to_string();

    let status = Command::new("mount")
        .args([&dev, mount.to_str().unwrap()])
        .status()
        .map_err(|e| format!("mount {dev}: {e}"))?;
    if !status.success() {
        return Err(format!("mount {dev} failed"));
    }

    let overlay = mount.join("persistence");
    let status = Command::new("dd")
        .args([
            "if=/dev/zero",
            &format!("of={}", overlay.to_string_lossy()),
            "bs=1M",
            &format!("count={size_mb}"),
            "status=none",
        ])
        .status()
        .map_err(|e| format!("dd: {e}"))?;
    if !status.success() {
        let _ = Command::new("umount").arg(mount).status();
        return Err("dd failed".to_string());
    }

    let status = Command::new("mkfs.ext4")
        .args(["-F", "-L", "persistence", overlay.to_str().unwrap()])
        .status()
        .map_err(|e| format!("mkfs.ext4: {e}"))?;
    if !status.success() {
        let _ = Command::new("umount").arg(mount).status();
        return Err("mkfs.ext4 failed".to_string());
    }

    // Write the persistence.conf that live-build expects.
    std::fs::write(mount.join("persistence.conf"), "/ union\n")
        .map_err(|e| format!("write persistence.conf: {e}"))?;

    let _ = Command::new("umount").arg(mount).status();
    Ok(())
}

struct StderrSink;

impl core::ProgressSink for StderrSink {
    fn emit(&self, event: core::ProgressEvent) {
        eprintln!("{}: {}", event.phase, event.message);
    }
}
