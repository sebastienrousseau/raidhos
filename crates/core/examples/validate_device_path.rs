//! `validate_device_path` example.
//!
//! Demonstrates the per-OS device-path allowlist that gates every
//! destructive operation in raidhos-core.
//!
//! ```bash
//! cargo run --example validate_device_path -p raidhos-core
//! ```
//!
//! Output shows which inputs are accepted and which are refused, along
//! with the rejection reason — useful when wiring a new caller and
//! debugging why a path was rejected.

use raidhos_core::validate_device_path;

fn main() {
    let inputs: &[&str] = &[
        // ----- Linux -------------------------------------------------------
        "/dev/sdb",
        "/dev/nvme0n1",
        "/dev/mmcblk0",
        // ----- macOS -------------------------------------------------------
        "/dev/disk2",
        "/dev/disk10",
        // ----- Windows ----------------------------------------------------
        "\\\\.\\PhysicalDrive1",
        "\\\\?\\PhysicalDrive12",
        // ----- Refusals ---------------------------------------------------
        "",
        "sdb",                                // missing /dev/ prefix
        "/dev/sdb;rm -rf /",                  // shell metacharacter
        "/dev/sdb && echo pwned",             // shell metacharacter
        "/dev/sdb`whoami`",                   // backtick substitution
        "/dev/sdb$(id)",                      // dollar substitution
        "/dev/sdb|cat",                       // pipe
        "/dev/../etc/passwd",                 // path traversal
        &format!("/dev/{}", "a".repeat(300)), // overlong
    ];

    let widest = inputs.iter().map(|s| s.len()).max().unwrap_or(0);
    println!("{:<width$}  RESULT", "INPUT", width = widest);
    println!("{:-<width$}  {:-<60}", "", "", width = widest);
    for input in inputs {
        let result = match validate_device_path(input) {
            Ok(()) => "accepted".to_string(),
            Err(e) => format!("refused: {e}"),
        };
        println!("{:<width$}  {}", input, result, width = widest);
    }
}
