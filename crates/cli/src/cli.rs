//! CLI argument schema, shared between the binary and `build.rs` so
//! man pages and shell completions stay in sync with the runtime
//! parser.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "raidhos-cli",
    version,
    about = "RaidhOS CLI — Ventoy-style multi-ISO USB installer",
    long_about = "Discover, validate, and install RaidhOS to a USB stick.\n\
                  All destructive operations require both `--allow-write=true` and\n\
                  `--wipe=true`; both default to safe values. Start with `--dry-run`."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Enumerate physical disks visible to the current user.
    ListDisks,
    /// Walk directories for *.iso files (one level deep).
    ScanIsos {
        /// Comma-separated list of root directories to scan.
        #[arg(long, value_delimiter = ',', default_value = "/media,/mnt,/home")]
        dirs: Vec<String>,
    },
    /// Run the install pipeline. Defaults to dry-run.
    Install {
        /// Target device path (`/dev/sdX`, `/dev/diskN`,
        /// `\\.\PhysicalDriveN`).
        #[arg(long)]
        device: String,
        /// Payload version string (must match `payload/manifest.json`).
        #[arg(long, default_value = "0.1.0")]
        payload_version: String,
        /// Required for destructive install. Defaults to true.
        #[arg(long, default_value_t = true)]
        wipe: bool,
        /// Run validation only, never write. Defaults to true.
        #[arg(long, default_value_t = true)]
        dry_run: bool,
        /// Required to actually touch the device. Defaults to false.
        #[arg(long, default_value_t = false)]
        allow_write: bool,
    },
    /// Write a previously-saved boot.json into a mounted partition.
    WriteConfig {
        /// Mountpoint of the target partition.
        #[arg(long)]
        mount_path: String,
        /// Path to the boot.json on the host.
        #[arg(long)]
        config_path: String,
    },
    /// Curated ISO catalog operations: list known distros and verify
    /// a local ISO against the bundled GPG-signed checksums.
    Catalog {
        #[command(subcommand)]
        action: CatalogAction,
    },
}

#[derive(Subcommand)]
pub enum CatalogAction {
    /// List bundled catalog entries.
    List,
    /// Verify a locally downloaded ISO against the catalog.
    Verify {
        /// Catalog slug (use `raidhos-cli catalog list` to discover).
        #[arg(long)]
        slug: String,
        /// Path to the local ISO file.
        #[arg(long)]
        iso: String,
        /// Path to the local SHA256SUMS-equivalent file.
        #[arg(long)]
        sums: String,
        /// Path to the detached signature.
        #[arg(long)]
        sig: String,
        /// Directory containing `<fingerprint>.asc` keys (defaults to
        /// `catalog/keys/`).
        #[arg(long, default_value = "catalog/keys")]
        key_dir: String,
    },
}
