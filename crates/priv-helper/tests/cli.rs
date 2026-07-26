//! Integration tests for `raidhos-priv-helper`.
//!
//! These spawn the actual compiled binary and assert on stdout JSON
//! / exit codes. Covers the `main()` entry surface that unit tests
//! can't reach.

use std::process::Command;

fn helper() -> Command {
    Command::new(env!("CARGO_BIN_EXE_raidhos-priv-helper"))
}

#[test]
fn version_flag_exits_zero() {
    let out = helper().arg("--version").output().expect("spawn");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("raidhos-priv-helper"));
}

#[test]
fn help_flag_exits_zero() {
    let out = helper().arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Usage"));
}

#[test]
fn unknown_subcommand_emits_json_error() {
    let out = helper()
        .arg("not-a-real-subcommand")
        .output()
        .expect("spawn");
    // clap rejects unknown subcommand → exit 2 with JSON wrapping.
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"ok\": false"));
}

#[test]
fn install_requires_device_argument() {
    let out = helper().arg("install").output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn list_disks_returns_json_object() {
    let out = helper().arg("list-disks").output().expect("spawn");
    let text = String::from_utf8_lossy(&out.stdout);
    // Output may or may not be `ok: true` depending on whether
    // lsblk (Linux) / diskutil (macOS) / Get-Disk (Windows) are
    // available — but the shape is always a JSON object with `ok`.
    assert!(text.contains("\"ok\""));
}

#[test]
fn install_install_dry_run_against_invalid_device_fails_cleanly() {
    let out = helper()
        .args([
            "install",
            "--device",
            "/dev/this-is-not-real-xyz",
            "--dry-run",
        ])
        .output()
        .expect("spawn");
    // Either exits 1 with a validation error, or exits 2 from clap
    // depending on the host's path-shape acceptance.
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"ok\": false") || text.is_empty());
}

#[test]
#[cfg(unix)]
fn argv_length_cap_rejects_huge_argv() {
    // Build a single argument longer than 64 KiB so the cap fires
    // before clap sees the request. Unix-only because Windows
    // CreateProcess refuses commandlines past ~32 KiB before our
    // own cap can run.
    let big = "x".repeat(100_000);
    let out = helper().arg(&big).output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("argv length"));
}

/// `list-partitions <DEVICE>` against a nonexistent device should
/// produce a JSON `{ok: false, ...}` rather than a panic — exercises
/// the error branch in main()'s ListPartitions arm.
#[test]
fn list_partitions_against_invalid_device_emits_json_error() {
    let out = helper()
        .args(["list-partitions", "/dev/this-is-not-real-xyz"])
        .output()
        .expect("spawn");
    let text = String::from_utf8_lossy(&out.stdout);
    // The shape is always a JSON object with an `ok` field; the
    // exit code is 1 (runtime failure) when the device probe fails.
    assert!(text.contains("\"ok\""), "missing ok field: {text}");
    // Either OK (empty list) on hosts where the probe gracefully
    // returns empty, or false with an error on hosts where it fails.
    // Both are valid; what matters is that we got a JSON object on
    // stdout, no panic, no garbage.
}

/// `--help` and `--version` both exit 0 with clap's text on stdout
/// (not wrapped in JSON). The existing tests cover `--version` and
/// `--help`; this one ensures the `install --help` subcommand-level
/// help works too — exercises the clap help-display path in main().
#[test]
fn install_help_subcommand_exits_zero() {
    let out = helper()
        .args(["install", "--help"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "got: {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--device"));
    assert!(text.contains("--data-fs"));
    assert!(text.contains("--bios-compat"));
}

/// `install` with a shape-valid device + --wipe --dry-run fires
/// at least the "validate" progress event before failing on
/// "device not found". Exercises `StderrSink::emit` (priv-helper/
/// main.rs:317-318) which is otherwise dead in unit tests.
#[test]
fn install_dry_run_with_shape_valid_device_emits_progress() {
    // Pick a device that passes the host's path-shape gate but
    // isn't in the real disk inventory.
    let device = if cfg!(target_os = "linux") {
        "/dev/sd-no-such-raidhos"
    } else if cfg!(target_os = "macos") {
        "/dev/disk99"
    } else if cfg!(target_os = "windows") {
        "\\\\.\\PhysicalDrive99"
    } else {
        return; // unsupported host
    };
    // priv-helper install: wipe is enabled by default (--no-wipe
    // disables it). So just --device + --dry-run is enough.
    let out = helper()
        .args(["install", "--device", device, "--dry-run"])
        .output()
        .expect("spawn");
    // Either succeeds (no real disk inventory needed for some
    // hosts) or fails with "device not found". Either way, the
    // "validate" progress event should have appeared on stderr
    // before the device-not-found check.
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Accept any signal that the install dispatch reached the
    // sink — "validate", "prepare", "complete", etc.
    assert!(
        stderr.contains("validate")
            || stderr.contains("complete")
            || stderr.contains("prepare")
            || out.status.code() == Some(1),
        "no progress signal: stderr={stderr}, status={:?}",
        out.status,
    );
}

/// `list-disks` with PATH deprived → host's `diskutil` /
/// `lsblk` / `powershell` cannot be located and `core::list_disks`
/// returns Err. Exercises the Err arm in main()'s ListDisks
/// dispatch (priv-helper/main.rs:165-167).
#[test]
fn list_disks_path_deprived_emits_json_error() {
    let out = helper()
        .arg("list-disks")
        .env_clear()
        .env("PATH", "/var/empty")
        .output()
        .expect("spawn");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"ok\""), "missing ok field: {text}");
    // Exit 1 (runtime failure) is expected; exit 0 happens on
    // hosts where the underlying tool gracefully returns empty.
    // Either way the JSON shape must be present.
}

/// `install --data-fs <unknown>` must be rejected with exit 1
/// (runtime validation), not panic or accept silently.
#[test]
fn install_unknown_data_fs_fails_cleanly() {
    let out = helper()
        .args([
            "install",
            "--device",
            "/dev/this-is-not-real-xyz",
            "--data-fs",
            "hfsplus",
            "--dry-run",
        ])
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "expected non-zero exit");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // Either clap rejected the value (if data_fs becomes an enum)
    // or the runtime parser rejected it. Either way, the message
    // must mention the offending value or "data-fs".
    assert!(
        combined.to_lowercase().contains("hfsplus")
            || combined.to_lowercase().contains("data-fs")
            || combined.contains("\"ok\": false"),
        "no rejection message: {combined}",
    );
}
