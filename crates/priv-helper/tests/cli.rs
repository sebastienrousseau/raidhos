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
