//! `list_disks` example.
//!
//! Enumerates physical disks visible to the current user (per-OS:
//! `lsblk` on Linux, `diskutil` on macOS, `Get-Disk` on Windows) and
//! prints them in a human-readable table.
//!
//! ```bash
//! cargo run --example list_disks -p raidhos-core
//! ```

use raidhos_core as core;

fn main() {
    let disks = match core::list_disks() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("list_disks failed: {e}");
            std::process::exit(1);
        }
    };

    if disks.is_empty() {
        println!("No disks visible. Try plugging in a USB stick.");
        return;
    }

    println!(
        "{:<28} {:<24} {:>14} {:>10} {:>8} MOUNTS",
        "DEVICE", "MODEL", "SIZE", "REMOVABLE", "SYSTEM"
    );
    for d in disks {
        println!(
            "{:<28} {:<24} {:>14} {:>10} {:>8} {}",
            d.id,
            truncate(&d.model, 24),
            d.size_bytes,
            d.removable,
            d.is_system,
            d.mountpoints.join(",")
        );
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}
