//! `catalog::verify_iso` example.
//!
//! Verifies a locally-downloaded ISO against a bundled-catalog entry.
//! The catalog (`catalog/catalog.json`) names the SHA256SUMS URL,
//! the detached signature URL, and the GPG signing key fingerprint
//! for several major Linux distros.
//!
//! ```bash
//! # First, download the ISO + its SHA256SUMS + signature by hand
//! # (or any other way), then point this example at the local files.
//! cargo run --example catalog_verify -p raidhos-core -- \
//!     ubuntu-24.04-desktop-amd64 \
//!     ~/Downloads/ubuntu-24.04.3-desktop-amd64.iso \
//!     ~/Downloads/SHA256SUMS \
//!     ~/Downloads/SHA256SUMS.gpg
//! ```

use raidhos_core::{find_entry, load_catalog, verify_iso};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!(
            "usage: catalog_verify <slug> <iso-path> <sums-path> <sig-path> [key-dir]\n\
             example: catalog_verify ubuntu-24.04-desktop-amd64 \
             ~/Downloads/ubuntu.iso ~/Downloads/SHA256SUMS \
             ~/Downloads/SHA256SUMS.gpg catalog/keys"
        );
        std::process::exit(2);
    }
    let slug = &args[0];
    let iso = Path::new(&args[1]);
    let sums = Path::new(&args[2]);
    let sig = Path::new(&args[3]);
    let key_dir = args
        .get(4)
        .map(|s| Path::new(s.as_str()))
        .unwrap_or_else(|| Path::new("catalog/keys"));

    let catalog = match load_catalog() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("load_catalog failed: {e}");
            std::process::exit(1);
        }
    };
    let entry = match find_entry(&catalog, slug) {
        Some(e) => e.clone(),
        None => {
            eprintln!("no catalog entry for slug '{slug}'");
            std::process::exit(1);
        }
    };

    match verify_iso(&entry, iso, sums, sig, key_dir) {
        Ok(v) => {
            println!(
                "OK: {} verified (sha256={})",
                v.entry.name, v.computed_sha256
            );
        }
        Err(e) => {
            eprintln!("verification failed: {e}");
            std::process::exit(1);
        }
    }
}
