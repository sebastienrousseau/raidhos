<!-- SPDX-License-Identifier: GPL-3.0-only -->

<p align="center">
  <img src="https://raw.githubusercontent.com/sebastienrousseau/raidhos/main/docs/img/raidhos-logo.svg" alt="RaidhOS logo" width="128" />
</p>

<h1 align="center">raidhos-core</h1>

<p align="center">
  The trust-boundary library for RaidhOS — disk discovery,
  validation, install orchestration, payload integrity, and the
  GPG-verified ISO catalog. <code>#![forbid(unsafe_code)]</code>.
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/raidhos/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/raidhos/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/raidhos-core"><img src="https://img.shields.io/crates/v/raidhos-core.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Crates.io" /></a>
  <a href="https://docs.rs/raidhos-core"><img src="https://img.shields.io/badge/docs.rs-raidhos--core-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs.rs" /></a>
  <a href="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos"><img src="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos/badge" alt="OpenSSF Scorecard" /></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg?style=for-the-badge" alt="License" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — Cargo, source
- [Quick Start](#quick-start) — discover disks and dry-run install in twelve lines

**Library reference**

- [Public API](#public-api) — every `pub` item, one screen
- [Examples](#examples) — runnable example index
- [Safety model](#safety-model) — every refusal, with citations
- [Architecture](#architecture) — module map
- [Configuration](#configuration) — environment, features, MSRV

**Operational**

- [Testing](#testing) — unit, integration, fuzz, coverage
- [Documentation](#documentation) — all reference docs
- [License](#license)

---

## Install

### As a Rust library

```toml
[dependencies]
raidhos-core = { git = "https://github.com/sebastienrousseau/raidhos", tag = "v0.0.1" }
```

`raidhos-core` carries no binaries. The CLI ships in the sister
crate [`raidhos-cli`](../cli/README.md); destructive installs run
out-of-process via [`raidhos-priv-helper`](../priv-helper/README.md).

### Build from source

```bash
git clone https://github.com/sebastienrousseau/raidhos.git
cd raidhos
cargo test -p raidhos-core --all-targets
```

**MSRV**: 1.85. The library compiles on every supported host
(Linux, macOS, Windows) with the same source — platform code is
behind `#[cfg(target_os = "…")]`.

### Dependencies

Library deps are deliberately minimal — no network stack, no
async runtime, no procedural macros beyond `serde_derive` /
`thiserror_impl`.

| Crate | Why |
|---|---|
| `serde` 1 + `serde_json` 1 | JSON / plist parsing |
| `sha2` 0.10 | Payload manifest hashing |
| `thiserror` 2 | Typed error rendering |

---

## Quick Start

```rust
use raidhos_core::{install, list_disks, InstallRequest, ProgressEvent, ProgressSink};

struct StdoutSink;
impl ProgressSink for StdoutSink {
    fn emit(&self, e: ProgressEvent) {
        println!("[{}] {}", e.phase, e.message);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for d in list_disks()? {
        println!("{} {} {} bytes (removable={})", d.id, d.model, d.size_bytes, d.removable);
    }
    let req = InstallRequest {
        device: "/dev/sdb".into(),
        payload_version: "0.0.1".into(),
        wipe: true,
        dry_run: true,        // writes nothing
        allow_write: false,
    };
    install(req, &StdoutSink)?;
    Ok(())
}
```

Live runnable variants are in [`examples/`](examples/) — see
[Examples](#examples).

---

## Public API

```rust
// Discovery
pub fn list_disks() -> Result<Vec<DiskInfo>>;
pub fn list_partitions(device: String) -> Result<Vec<PartitionInfo>>;
pub fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>>;

// Validation
pub fn validate_device_path(device: &str) -> Result<()>;

// Install pipeline
pub fn install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()>;

// Payload integrity
pub fn verify_payload(payload_dir: &Path) -> Result<(), ManifestError>;

// ISO catalog
pub fn load_catalog() -> Result<Vec<CatalogEntry>>;
pub fn load_catalog_from(path: &Path) -> Result<Vec<CatalogEntry>>;
pub fn find_entry<'a>(catalog: &'a [CatalogEntry], slug: &str) -> Option<&'a CatalogEntry>;
pub fn verify_iso(
    entry: &CatalogEntry,
    iso: &Path, sums: &Path, sig: &Path, key_dir: &Path,
) -> Result<VerifiedIso, CatalogError>;
pub fn verify_iso_with(
    rt: &dyn Runtime,
    entry: &CatalogEntry,
    iso: &Path, sums: &Path, sig: &Path, key_dir: &Path,
) -> Result<VerifiedIso, CatalogError>;
pub fn sha256sums_lookup(body: &str, filename: &str) -> Option<String>;

// Runtime abstraction (testable subprocess layer)
pub trait Runtime { /* run, run_output, has_cmd, env_var, … */ }
pub struct RealRuntime;
pub struct MockRuntime { /* … */ }
```

Types: `DiskInfo`, `PartitionInfo`, `IsoEntry`, `InstallRequest`,
`ProgressEvent`, `ProgressSink`, `CoreError`, `PayloadManifest`,
`ManifestError`, `CatalogEntry`, `CatalogError`, `VerifiedIso`,
`Invocation`, `MockOutcome`.

Full per-symbol reference: [`doc/API.md`](doc/API.md).

---

## Examples

Runnable examples live in [`examples/`](examples/). Each one is
a complete program with no extra setup:

```bash
# List physical disks visible to your user.
cargo run --example list_disks -p raidhos-core

# Show which device paths the validator accepts vs refuses.
cargo run --example validate_device_path -p raidhos-core

# Scan the filesystem for *.iso files.
cargo run --example scan_isos -p raidhos-core -- /home /media

# Verify a payload tree against its manifest.json.
cargo run --example verify_payload -p raidhos-core -- /srv/raidhos/payload

# Dry-run the install pipeline against a fake device.
cargo run --example install_dry_run -p raidhos-core -- /dev/sdb

# Verify a downloaded ISO against the bundled catalog (needs gpg(1)).
cargo run --example catalog_verify -p raidhos-core -- \
    ubuntu-24.04-desktop-amd64 ~/Downloads/ubuntu.iso \
    ~/Downloads/SHA256SUMS ~/Downloads/SHA256SUMS.gpg
```

Example index: [`examples/README.md`](examples/README.md).

---

## Safety model

The crate's job is to **refuse to do unsafe things**. Refusals
that fire before any byte is written:

| Check | Where |
|---|---|
| Empty / >256-byte device path | [`lib.rs:validate_device_path`](src/lib.rs) |
| Shell metacharacters in path | same (`FORBIDDEN` constant) |
| Path traversal (`..`) | same |
| Wrong per-OS path shape | same |
| Disk mounted at `/`, `/boot`, `/boot/efi` (Linux) | `platform/linux.rs::list_disks` |
| Disk flagged internal (macOS) | `parsers.rs::parse_disks_plist` |
| Disk flagged system/boot (Windows) | `parsers.rs::parse_get_disk_json` |
| Any partition mounted | `platform/{linux,macos}.rs::validate_install` |
| Missing `wipe == true` opt-in | same |
| Missing `allow_write == true` opt-in | same |
| `RAIDHOS_PAYLOAD_DIR` missing | `platform/*::payload_copy` |
| `esp/` or `data/` missing in payload | same |
| Payload SHA-256 mismatch | `payload.rs::verify_payload` (advisory in v0.0.1) |
| ISO GPG signature mismatch | `catalog.rs::verify_iso` |
| SHA256SUMS hash mismatch | same |

Full attacker-model map: [`docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md).
Control citations: [`docs/HARDENING.md`](../../docs/HARDENING.md).

---

## Architecture

```text
src/
├── lib.rs                  Public API + CoreError + validate_device_path
├── runtime.rs              Runtime trait (subprocess abstraction),
│                           RealRuntime + MockRuntime
├── parsers.rs              Hand-rolled lsblk / diskutil / Get-Disk parsers,
│                           re-exposed via __fuzz_api for cargo-fuzz.
├── payload.rs              PayloadManifest + verify_payload (SHA-256 tree).
├── catalog.rs              ISO catalog + verify_iso (gpg(1) wrapper).
└── platform/
    ├── mod.rs              cfg-based platform dispatch.
    ├── scan.rs             Shared one-level ISO filesystem scanner.
    ├── linux.rs            parted / mkfs.vfat / mkfs.exfat / mount / cp.
    ├── macos.rs            diskutil partitionDisk / rename / mount / cp / bless.
    ├── windows.rs          Clear-Disk / New-Partition / Format-Volume / robocopy.
    └── unsupported.rs      CoreError::UnsupportedPlatform fallback.
```

Each platform module exposes the same three functions
(`list_disks`, `list_partitions`, `install`), gated by
`#[cfg(target_os = "…")]`. The parsers are *not* gated — they
live in `parsers.rs` so cargo-fuzz can call them on any host
via `raidhos_core::__fuzz_api::*`.

The `Runtime` trait separates subprocess intent from execution,
letting every install path be unit-tested via `MockRuntime`
without touching real disks.

---

## Configuration

### Environment

| Variable | Read by | Purpose |
|---|---|---|
| `RAIDHOS_PAYLOAD_DIR` | `platform/*::payload_copy` | Where to read `esp/`, `data/`, and `manifest.json` |
| `RAIDHOS_KEY_DIR` | `catalog.rs::verify_iso` (caller) | Override for the bundled keyring directory |

### Features

The crate has no Cargo features in v0.0.1 — everything is `default`.

### Public re-exports

```rust
pub use catalog::{find_entry, load_catalog, load_catalog_from,
                  sha256sums_lookup, verify_iso, verify_iso_with,
                  CatalogEntry, CatalogError, VerifiedIso};
pub use payload::{verify_payload, ManifestError, PayloadManifest};
pub use runtime::{Invocation, MockOutcome, MockRuntime, RealRuntime, Runtime};
```

---

## Testing

```bash
# Unit + integration tests (136+ tests + 22 doctests at time of writing)
cargo test -p raidhos-core --all-targets
cargo test -p raidhos-core --doc

# Coverage (line coverage is ~91.6% on Linux CI; gate set to 90%)
cargo tarpaulin -p raidhos-core --fail-under 90

# Fuzz (afl + libfuzzer). Targets re-exported via __fuzz_api.
cargo fuzz run parse_lsblk_disks   -- -max_total_time=30
cargo fuzz run parse_disks_plist   -- -max_total_time=30
cargo fuzz run parse_get_disk_json -- -max_total_time=30
cargo fuzz run sha256sums_lookup   -- -max_total_time=30
```

The fuzz harness lives at [`fuzz/`](../../fuzz/) in the repo
root. Each public parser also has a runnable example in
[`examples/`](examples/) so the contract is testable from
outside the crate.

---

## Documentation

| Doc | What's in it |
|---|---|
| [`doc/API.md`](doc/API.md) | Per-symbol reference for every `pub` item |
| [`doc/USAGE.md`](doc/USAGE.md) | Cookbook: how to use the library from your own code |
| [`doc/ARCHITECTURE.md`](doc/ARCHITECTURE.md) | Module-level architecture diagram |
| [`doc/SECURITY.md`](doc/SECURITY.md) | Per-control citations and threat-model linkages |
| [`../../docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md) | Attacker model the crate defends against |
| [`../../docs/HARDENING.md`](../../docs/HARDENING.md) | Shipping controls with `file:line` citations |
| [`../../docs/DESIGN_NOTES.md`](../../docs/DESIGN_NOTES.md) | Design decisions and trade-offs |

---

## License

`raidhos-core` is licensed under the **GPL-3.0-only**. See
[`LICENSE`](../../LICENSE) for the full text. The workspace
license declaration is asserted via
`workspace.package.license = "GPL-3.0-only"`.
