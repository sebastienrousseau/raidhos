//! End-to-end dry-run example.
//!
//! Calls `raidhos_core::install` with a synthetic `InstallRequest`
//! against a fake device. Prints every `ProgressEvent` emitted by the
//! pipeline. **No writes** — `dry_run` is `true` and `allow_write` is
//! `false`.
//!
//! ```bash
//! cargo run --example install_dry_run -p raidhos-core -- /dev/sdb
//! ```

use raidhos_core as core;

struct StdoutSink;
impl core::ProgressSink for StdoutSink {
    fn emit(&self, event: core::ProgressEvent) {
        let pct = event.percent.map(|p| format!("{p}%")).unwrap_or_default();
        println!("[{}] {} {}", event.phase, event.message, pct);
    }
}

fn main() {
    let device = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: install_dry_run <DEVICE>");
        std::process::exit(2);
    });

    let req = core::InstallRequest {
        device,
        payload_version: "0.0.0-dry-run".to_string(),
        wipe: true,
        dry_run: true,
        allow_write: false,
        simulator: false,
    };

    match core::install(req, &StdoutSink) {
        Ok(()) => println!("dry-run complete"),
        Err(e) => {
            eprintln!("dry-run failed: {e}");
            std::process::exit(1);
        }
    }
}
