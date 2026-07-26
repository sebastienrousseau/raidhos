//! `scan_isos` example.
//!
//! Walks the supplied directories looking for `.iso` files (one
//! directory deep) and prints what it found.
//!
//! ```bash
//! cargo run --example scan_isos -p raidhos-core -- /home /media /mnt
//! ```
//!
//! If no directories are given, scans the user's `~/Downloads`.

use raidhos_core as core;

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let dirs = if dirs.is_empty() {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        vec![format!("{home}/Downloads")]
    } else {
        dirs
    };

    println!("Scanning: {}", dirs.join(", "));
    let entries = match core::scan_isos(dirs) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("scan_isos failed: {e}");
            std::process::exit(1);
        }
    };

    if entries.is_empty() {
        println!("No ISOs found.");
        return;
    }

    println!("{:<40} {:>14}  PATH", "TITLE", "SIZE",);
    for e in entries {
        println!(
            "{:<40} {:>14}  {}",
            truncate(&e.title, 40),
            e.size_bytes,
            e.path
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
