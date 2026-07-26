//! `verify_payload` example.
//!
//! Verifies that a payload tree on the filesystem matches the SHA-256
//! pinned in its `manifest.json`. Walks the directory, hashes every
//! regular file path-prefixed in sorted order, compares.
//!
//! ```bash
//! cargo run --example verify_payload -p raidhos-core -- /srv/raidhos/payload-1.1.10
//! ```
//!
//! Exit codes:
//! - `0` — match, or manifest's checksum is empty ("unpinned").
//! - `2` — mismatch (the bytes differ from what was pinned).
//! - `1` — couldn't read the manifest at all.

use raidhos_core::{verify_payload, ManifestError};
use std::path::PathBuf;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: verify_payload <RAIDHOS_PAYLOAD_DIR>");
            std::process::exit(2);
        }
    };

    match verify_payload(&path) {
        Ok(()) => {
            println!("OK: payload at {} matches manifest", path.display());
        }
        Err(ManifestError::Unpinned) => {
            println!(
                "ADVISORY: manifest.json:checksum is empty at {}; payload not yet pinned",
                path.display()
            );
            // exit 0 — non-fatal during development.
        }
        Err(ManifestError::Mismatch { manifest, computed }) => {
            eprintln!(
                "FATAL: payload mismatch at {}\n  manifest: {manifest}\n  computed: {computed}",
                path.display()
            );
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
