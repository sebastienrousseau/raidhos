//! Generates a man page and shell completions at build time.
//!
//! Output lands under `OUT_DIR/dist/` so packaging scripts can pick
//! them up without re-running the binary. Failures here are
//! non-fatal — we never want a missing completion file to block the
//! main build.

use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use std::env;
use std::path::PathBuf;

#[path = "src/cli.rs"]
mod cli;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = match env::var_os("OUT_DIR") {
        Some(v) => PathBuf::from(v),
        None => return,
    };
    let dist = out_dir.join("dist");
    if std::fs::create_dir_all(&dist).is_err() {
        return;
    }

    let mut cmd = cli::Cli::command();

    // Man page. Failure here is non-fatal: missing docs don't block
    // the build.
    let mut buf = Vec::new();
    if clap_mangen::Man::new(cmd.clone()).render(&mut buf).is_ok() {
        let _ = std::fs::write(dist.join("raidhos-cli.1"), buf);
    }

    // Completions.
    for shell in [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Elvish,
    ] {
        let _ = generate_to(shell, &mut cmd, "raidhos-cli", &dist);
    }
}
