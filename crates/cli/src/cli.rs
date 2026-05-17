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
        /// `\\.\PhysicalDriveN`). Mutually exclusive with `--simulator`.
        #[arg(long, conflicts_with = "simulator")]
        device: Option<String>,
        /// Simulate the install against a sparse file instead of a real
        /// block device. Same code path as the CI virtual-disk workflow.
        /// Creates the file if missing (default size 256 MiB; override
        /// with `--simulator-size-mb`).
        ///
        /// Examples:
        ///   raidhos-cli install --simulator /tmp/raidhos.img
        ///   raidhos-cli install --simulator /tmp/r.img --simulator-size-mb 1024
        #[arg(long, conflicts_with = "device")]
        simulator: Option<String>,
        /// Size in MiB for a newly-created `--simulator` file. Ignored
        /// if the file already exists.
        #[arg(long, default_value_t = 256)]
        simulator_size_mb: u64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_command_factory_renders_help() {
        let cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.clone().write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();
        assert!(help.contains("raidhos-cli"));
        assert!(help.contains("Discover, validate, and install RaidhOS"));
    }

    #[test]
    fn parses_list_disks() {
        let cli = Cli::try_parse_from(["raidhos-cli", "list-disks"]).unwrap();
        assert!(matches!(cli.command, Commands::ListDisks));
    }

    #[test]
    fn parses_scan_isos_with_default_dirs() {
        let cli = Cli::try_parse_from(["raidhos-cli", "scan-isos"]).unwrap();
        if let Commands::ScanIsos { dirs } = cli.command {
            assert!(dirs.contains(&"/home".to_string()));
        } else {
            panic!("expected ScanIsos");
        }
    }

    #[test]
    fn parses_scan_isos_with_custom_dirs() {
        let cli = Cli::try_parse_from(["raidhos-cli", "scan-isos", "--dirs", "/a,/b,/c"]).unwrap();
        if let Commands::ScanIsos { dirs } = cli.command {
            assert_eq!(
                dirs,
                vec!["/a".to_string(), "/b".to_string(), "/c".to_string()]
            );
        } else {
            panic!("expected ScanIsos");
        }
    }

    #[test]
    fn parses_install_defaults_to_dry_run_and_wipe() {
        let cli = Cli::try_parse_from(["raidhos-cli", "install", "--device", "/dev/sdb"]).unwrap();
        if let Commands::Install {
            device,
            simulator,
            payload_version,
            wipe,
            dry_run,
            allow_write,
            ..
        } = cli.command
        {
            assert_eq!(device.as_deref(), Some("/dev/sdb"));
            assert!(simulator.is_none());
            assert_eq!(payload_version, "0.1.0");
            assert!(wipe);
            assert!(dry_run);
            assert!(!allow_write);
        } else {
            panic!("expected Install");
        }
    }

    #[test]
    fn parses_install_with_simulator() {
        let cli = Cli::try_parse_from([
            "raidhos-cli",
            "install",
            "--simulator",
            "/tmp/raidhos-sim.img",
            "--simulator-size-mb",
            "512",
        ])
        .unwrap();
        if let Commands::Install {
            device,
            simulator,
            simulator_size_mb,
            ..
        } = cli.command
        {
            assert!(device.is_none());
            assert_eq!(simulator.as_deref(), Some("/tmp/raidhos-sim.img"));
            assert_eq!(simulator_size_mb, 512);
        } else {
            panic!("expected Install");
        }
    }

    #[test]
    fn install_device_and_simulator_are_mutually_exclusive() {
        let res = Cli::try_parse_from([
            "raidhos-cli",
            "install",
            "--device",
            "/dev/sdb",
            "--simulator",
            "/tmp/x.img",
        ]);
        assert!(res.is_err());
    }

    #[test]
    fn install_accepts_bare_then_requires_device_or_simulator_at_runtime() {
        // `--device` and `--simulator` are both Option<String> so clap
        // doesn't fail bare `install` parsing — main.rs gates this at
        // runtime with a clear error and exits 2. The test here just
        // confirms clap is happy; the runtime gate is covered by
        // tests/cli.rs spawning the binary.
        let cli = Cli::try_parse_from(["raidhos-cli", "install"]).unwrap();
        if let Commands::Install {
            device, simulator, ..
        } = cli.command
        {
            assert!(device.is_none());
            assert!(simulator.is_none());
        } else {
            panic!("expected Install");
        }
    }

    #[test]
    fn parses_write_config() {
        let cli = Cli::try_parse_from([
            "raidhos-cli",
            "write-config",
            "--mount-path",
            "/mnt/data",
            "--config-path",
            "/tmp/boot.json",
        ])
        .unwrap();
        if let Commands::WriteConfig {
            mount_path,
            config_path,
        } = cli.command
        {
            assert_eq!(mount_path, "/mnt/data");
            assert_eq!(config_path, "/tmp/boot.json");
        } else {
            panic!("expected WriteConfig");
        }
    }

    #[test]
    fn parses_catalog_list() {
        let cli = Cli::try_parse_from(["raidhos-cli", "catalog", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Catalog {
                action: CatalogAction::List
            }
        ));
    }

    #[test]
    fn parses_catalog_verify_with_explicit_args() {
        let cli = Cli::try_parse_from([
            "raidhos-cli",
            "catalog",
            "verify",
            "--slug",
            "ubuntu",
            "--iso",
            "/i.iso",
            "--sums",
            "/s.sums",
            "--sig",
            "/s.gpg",
        ])
        .unwrap();
        if let Commands::Catalog {
            action:
                CatalogAction::Verify {
                    slug,
                    iso,
                    sums,
                    sig,
                    key_dir,
                },
        } = cli.command
        {
            assert_eq!(slug, "ubuntu");
            assert_eq!(iso, "/i.iso");
            assert_eq!(sums, "/s.sums");
            assert_eq!(sig, "/s.gpg");
            assert_eq!(key_dir, "catalog/keys");
        } else {
            panic!("expected Catalog::Verify");
        }
    }

    #[test]
    fn parses_catalog_verify_with_override_key_dir() {
        let cli = Cli::try_parse_from([
            "raidhos-cli",
            "catalog",
            "verify",
            "--slug",
            "ubuntu",
            "--iso",
            "/i.iso",
            "--sums",
            "/s.sums",
            "--sig",
            "/s.gpg",
            "--key-dir",
            "/etc/keys",
        ])
        .unwrap();
        if let Commands::Catalog {
            action: CatalogAction::Verify { key_dir, .. },
        } = cli.command
        {
            assert_eq!(key_dir, "/etc/keys");
        } else {
            panic!("expected Catalog::Verify");
        }
    }

    #[test]
    fn rejects_unknown_subcommand() {
        let res = Cli::try_parse_from(["raidhos-cli", "definitely-not-a-subcommand"]);
        assert!(res.is_err());
    }

    #[test]
    fn install_payload_version_can_be_overridden() {
        let cli = Cli::try_parse_from([
            "raidhos-cli",
            "install",
            "--device",
            "/dev/sdb",
            "--payload-version",
            "0.5.0",
        ])
        .unwrap();
        if let Commands::Install {
            payload_version, ..
        } = cli.command
        {
            assert_eq!(payload_version, "0.5.0");
        } else {
            panic!("expected Install");
        }
    }
}
