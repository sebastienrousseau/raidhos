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
    /// Opt-in Legacy-BIOS-bootable layout (Ventoy gap G1). v0.0.1
    /// plumbs the flag through validation only; actual BIOS GRUB
    /// embedding is v0.0.2 work.
    #[arg(long, default_value_t = false)]
    bios_compat: bool,
    /// Filesystem to format the DATA partition with. Ventoy gap G8
    /// (NTFS) + G9 (ext4/Btrfs/XFS).
    #[arg(long, default_value = "exfat")]
    data_fs: String,
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
    // `serde_json::to_string_pretty` cannot fail on any
    // `HelperResponse<T>` produced by this binary — T is always
    // one of our domain types (`Vec<DiskInfo>`, `Vec<PartitionInfo>`,
    // `()`), all of which have derived `Serialize` impls that
    // never error. Use `expect` so the panic surface is explicit;
    // a defensive `Err` fallback would be dead code and a coverage
    // hole that the previous version had.
    let body = serde_json::to_string_pretty(resp).expect("HelperResponse serialise");
    println!("{body}");
}

/// Build the `list-disks` response from a `core::list_disks()` result.
/// Returned tuple is `(response, ok_for_exit_code)`. Factored out so
/// the Ok/Err branches can be exercised by unit tests without
/// depending on the host's real disk inventory.
fn list_disks_response(
    res: core::Result<Vec<core::DiskInfo>>,
) -> (HelperResponse<Vec<core::DiskInfo>>, bool) {
    match res {
        Ok(disks) => (HelperResponse::ok(disks), true),
        Err(err) => (HelperResponse::err(err.to_string()), false),
    }
}

/// Build the `list-partitions` response from a `core::list_partitions()`
/// result. Same pattern as `list_disks_response` — see above.
fn list_partitions_response(
    res: core::Result<Vec<core::PartitionInfo>>,
) -> (HelperResponse<Vec<core::PartitionInfo>>, bool) {
    match res {
        Ok(parts) => (HelperResponse::ok(parts), true),
        Err(err) => (HelperResponse::err(err.to_string()), false),
    }
}

/// Build the install-subcommand response from the two underlying
/// results: the core install pipeline outcome, and (on Linux only)
/// the optional persistence-creation error. Three reachable cases:
/// - install Ok, no persistence step run / succeeded → `ok` response.
/// - install Ok, persistence step ran and failed → wrap the
///   persistence error so the operator can re-run just the
///   persistence step.
/// - install Err → propagate the install error.
fn install_response(
    install_result: std::result::Result<(), String>,
    persistence_result: Option<String>,
) -> (HelperResponse<()>, bool) {
    match (install_result, persistence_result) {
        (Ok(()), None) => (HelperResponse::ok(()), true),
        (Ok(()), Some(p_err)) => (
            HelperResponse::err(format!(
                "install succeeded but persistence creation failed: {p_err}"
            )),
            false,
        ),
        (Err(e), _) => (HelperResponse::err(e), false),
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
    let ok = match cli.command {
        Command::ListDisks => {
            let (resp, dispatch_ok) = list_disks_response(core::list_disks());
            print_response(&resp);
            dispatch_ok
        }
        Command::ListPartitions { device } => {
            let (resp, dispatch_ok) = list_partitions_response(core::list_partitions(device));
            print_response(&resp);
            dispatch_ok
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
                bios_compat: args.bios_compat,
                data_filesystem: core::DataFilesystem::parse(&args.data_fs)
                    .unwrap_or(core::DataFilesystem::ExFat),
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

            let (resp, dispatch_ok) = install_response(install_result, persistence_result);
            print_response(&resp);
            dispatch_ok
        }
    };

    if !ok {
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn create_persistence(size_mb: u64) -> Result<(), String> {
    create_persistence_with(
        &core::RealRuntime,
        size_mb,
        std::path::Path::new("/mnt/raidhos-data-persist"),
    )
}

/// Persistence-overlay builder, parametric on the runtime so unit
/// tests can drive every subprocess call through a `MockRuntime`
/// without actually invoking `dd` / `mkfs.ext4` / `mount` / `umount`.
/// The `mount` path is also a parameter so tests can point at a
/// scratch directory instead of `/mnt/raidhos-data-persist`.
#[cfg(target_os = "linux")]
pub(crate) fn create_persistence_with(
    rt: &dyn core::Runtime,
    size_mb: u64,
    mount: &std::path::Path,
) -> Result<(), String> {
    rt.create_dir_all(mount)
        .map_err(|e| format!("create_dir_all {}: {e}", mount.display()))?;

    let dev_bytes = rt
        .run_output("blkid", &["-L", "DATA"])
        .map_err(|e| format!("blkid: {e}"))?;
    let dev = String::from_utf8_lossy(&dev_bytes).trim().to_string();
    if dev.is_empty() {
        return Err("blkid: DATA label not found".to_string());
    }

    let mount_str = mount.to_string_lossy().into_owned();
    rt.run("mount", &[&dev, &mount_str])
        .map_err(|e| format!("mount {dev}: {e}"))?;

    let overlay = mount.join("persistence");
    let overlay_str = overlay.to_string_lossy().into_owned();
    let count_arg = format!("count={size_mb}");
    let of_arg = format!("of={overlay_str}");
    if let Err(e) = rt.run(
        "dd",
        &["if=/dev/zero", &of_arg, "bs=1M", &count_arg, "status=none"],
    ) {
        let _ = rt.run("umount", &[&mount_str]);
        return Err(format!("dd failed: {e}"));
    }

    if let Err(e) = rt.run("mkfs.ext4", &["-F", "-L", "persistence", &overlay_str]) {
        let _ = rt.run("umount", &[&mount_str]);
        return Err(format!("mkfs.ext4 failed: {e}"));
    }

    // Write the persistence.conf that live-build expects. The path
    // is real even under MockRuntime — tests pass a tmp dir as the
    // `mount` parameter so this write lands somewhere writable.
    std::fs::write(mount.join("persistence.conf"), "/ union\n")
        .map_err(|e| format!("write persistence.conf: {e}"))?;

    let _ = rt.run("umount", &[&mount_str]);
    Ok(())
}

struct StderrSink;

impl core::ProgressSink for StderrSink {
    fn emit(&self, event: core::ProgressEvent) {
        eprintln!("{}: {}", event.phase, event.message);
    }
}

#[cfg(test)]
mod response_tests {
    use super::*;

    #[test]
    fn list_disks_response_wraps_ok_payload() {
        let disks = vec![core::DiskInfo {
            id: "/dev/sdb".into(),
            model: "TestStick".into(),
            size_bytes: 1024,
            removable: true,
            mountpoints: vec![],
            is_system: false,
        }];
        let (resp, ok) = list_disks_response(Ok(disks));
        assert!(ok);
        assert!(resp.ok);
        assert!(resp.error.is_none());
        assert_eq!(resp.data.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn list_disks_response_propagates_err_to_helper_response() {
        let (resp, ok) = list_disks_response(Err(core::CoreError::Io("simulated".into())));
        assert!(!ok);
        assert!(!resp.ok);
        assert!(resp.error.as_deref().unwrap().contains("simulated"));
    }

    #[test]
    fn list_partitions_response_wraps_ok_payload() {
        let parts = vec![core::PartitionInfo {
            id: "/dev/sdb1".into(),
            label: "RAIDHOS_EFI".into(),
            fstype: "vfat".into(),
            mountpoints: vec![],
        }];
        let (resp, ok) = list_partitions_response(Ok(parts));
        assert!(ok);
        assert!(resp.ok);
        assert_eq!(resp.data.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn list_partitions_response_propagates_err_to_helper_response() {
        let (resp, ok) =
            list_partitions_response(Err(core::CoreError::Io("lsblk fell over".into())));
        assert!(!ok);
        assert!(!resp.ok);
        assert!(resp.error.as_deref().unwrap().contains("lsblk fell over"));
    }

    #[test]
    fn install_response_ok_no_persistence() {
        let (resp, ok) = install_response(Ok(()), None);
        assert!(ok);
        assert!(resp.ok);
        assert!(resp.error.is_none());
    }

    #[test]
    fn install_response_ok_with_persistence_error() {
        let (resp, ok) = install_response(Ok(()), Some("dd failed".into()));
        assert!(!ok);
        assert!(!resp.ok);
        let err = resp.error.unwrap();
        assert!(err.contains("install succeeded"), "got: {err}");
        assert!(err.contains("persistence creation failed"), "got: {err}");
        assert!(err.contains("dd failed"), "got: {err}");
    }

    #[test]
    fn install_response_install_failed() {
        let (resp, ok) = install_response(Err("device not found".into()), None);
        assert!(!ok);
        assert!(!resp.ok);
        assert_eq!(resp.error.as_deref(), Some("device not found"));
    }

    #[test]
    fn install_response_install_failed_ignores_persistence() {
        // When install errors, persistence_result is ignored — the
        // install error takes precedence.
        let (resp, ok) = install_response(
            Err("partition failed".into()),
            Some("persistence would have failed too".into()),
        );
        assert!(!ok);
        assert_eq!(resp.error.as_deref(), Some("partition failed"));
    }

    /// `print_response` cannot fail on any HelperResponse<T> that this
    /// binary constructs — covered structurally via the `.expect()` call
    /// after the previous refactor. This test just confirms the
    /// serialisation produces the expected JSON shape.
    #[test]
    fn helper_response_json_shape() {
        let resp = HelperResponse::<()>::ok(());
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"data\":null"));

        let resp = HelperResponse::<Vec<core::DiskInfo>>::err("boom".to_string());
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"boom\""));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod persistence_tests {
    use super::*;
    use core::{MockOutcome, MockRuntime, Runtime};

    fn tmp_mount() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "raidhos-persist-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Happy path: blkid finds DATA, mount succeeds, dd succeeds,
    /// mkfs.ext4 succeeds, persistence.conf gets written, umount
    /// fires. Verifies every recorded invocation in order.
    #[test]
    fn create_persistence_happy_path() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"/dev/sdb2\n".to_vec())); // blkid
        rt.push_outcome(MockOutcome::Ok(vec![])); // mount
        rt.push_outcome(MockOutcome::Ok(vec![])); // dd
        rt.push_outcome(MockOutcome::Ok(vec![])); // mkfs.ext4
        rt.push_outcome(MockOutcome::Ok(vec![])); // umount (fire-and-forget)

        let res = create_persistence_with(&rt, 256, &mount);
        assert!(res.is_ok(), "got: {res:?}");

        // persistence.conf must land in the mount dir.
        let conf = mount.join("persistence.conf");
        assert!(conf.exists(), "persistence.conf not written");
        let body = std::fs::read_to_string(&conf).unwrap();
        assert_eq!(body, "/ union\n");

        let inv = rt.invocations.borrow();
        let cmds: Vec<_> = inv.iter().map(|i| i.cmd.as_str()).collect();
        assert!(cmds.contains(&"blkid"));
        assert!(cmds.contains(&"mount"));
        assert!(cmds.contains(&"dd"));
        assert!(cmds.contains(&"mkfs.ext4"));
        assert!(cmds.contains(&"umount"));

        let _ = std::fs::remove_dir_all(&mount);
    }

    /// blkid stdout is empty → "DATA label not found".
    #[test]
    fn create_persistence_errors_when_blkid_empty() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(vec![])); // blkid stdout empty
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("DATA label not found"), "got: {err}");
        let _ = std::fs::remove_dir_all(&mount);
    }

    /// blkid itself errors → propagates as "blkid: ...".
    #[test]
    fn create_persistence_errors_when_blkid_fails() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Err("not installed".into()));
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("blkid"), "got: {err}");
        let _ = std::fs::remove_dir_all(&mount);
    }

    /// mount fails → propagates as "mount /dev/sdb2: ...".
    #[test]
    fn create_persistence_errors_when_mount_fails() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"/dev/sdb2\n".to_vec())); // blkid
        rt.push_outcome(MockOutcome::Err("EBUSY".into())); // mount
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("mount /dev/sdb2"), "got: {err}");
        let _ = std::fs::remove_dir_all(&mount);
    }

    /// dd fails → umount is fired in cleanup before the error
    /// propagates.
    #[test]
    fn create_persistence_errors_when_dd_fails() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"/dev/sdb2\n".to_vec())); // blkid
        rt.push_outcome(MockOutcome::Ok(vec![])); // mount
        rt.push_outcome(MockOutcome::Err("no space".into())); // dd
        rt.push_outcome(MockOutcome::Ok(vec![])); // umount (cleanup)
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("dd failed"), "got: {err}");
        assert!(rt.invocations.borrow().iter().any(|i| i.cmd == "umount"));
        let _ = std::fs::remove_dir_all(&mount);
    }

    /// mkfs.ext4 fails → umount cleanup also fires.
    #[test]
    fn create_persistence_errors_when_mkfs_fails() {
        let mount = tmp_mount();
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"/dev/sdb2\n".to_vec())); // blkid
        rt.push_outcome(MockOutcome::Ok(vec![])); // mount
        rt.push_outcome(MockOutcome::Ok(vec![])); // dd
        rt.push_outcome(MockOutcome::Err("bad fs".into())); // mkfs.ext4
        rt.push_outcome(MockOutcome::Ok(vec![])); // umount (cleanup)
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("mkfs.ext4 failed"), "got: {err}");
        let _ = std::fs::remove_dir_all(&mount);
    }

    /// persistence.conf write fails when the mount dir doesn't
    /// exist. We pass a mount path that we never create — the
    /// `rt.create_dir_all` call goes through MockRuntime (which is
    /// `fake_mkdir = true` by default and does no real I/O), so the
    /// dir really doesn't exist on disk when fs::write tries it.
    #[test]
    fn create_persistence_errors_when_conf_write_fails() {
        let mount = std::path::PathBuf::from("/dev/null-this-is-not-a-real-directory-raidhos-test");
        let rt = MockRuntime::new();
        rt.push_outcome(MockOutcome::Ok(b"/dev/sdb2\n".to_vec()));
        rt.push_outcome(MockOutcome::Ok(vec![]));
        rt.push_outcome(MockOutcome::Ok(vec![]));
        rt.push_outcome(MockOutcome::Ok(vec![]));
        let err = create_persistence_with(&rt, 256, &mount).unwrap_err();
        assert!(err.contains("write persistence.conf"), "got: {err}");
    }

    /// create_dir_all failure (e.g. backing path is a file) →
    /// propagates with the mount path in the error.
    #[test]
    fn create_persistence_errors_when_mkdir_fails() {
        // Real-filesystem-backed mock so create_dir_all hits the
        // OS. Point at a path that exists as a regular file so
        // mkdir refuses.
        use std::cell::RefCell;
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-persist-mkdir-fail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&scratch, b"this is a file").unwrap();

        let mut rt = MockRuntime::new();
        rt.fake_mkdir = false; // route create_dir_all through the real fs
        rt.outcomes = RefCell::new(vec![]); // no subprocess calls expected
        let err = create_persistence_with(&rt, 256, &scratch).unwrap_err();
        assert!(err.contains("create_dir_all"), "got: {err}");
        let _ = std::fs::remove_file(&scratch);
    }
}
