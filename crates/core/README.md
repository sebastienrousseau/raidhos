<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-core

The trust-boundary library for RaidhOS. Disk discovery,
validation, install orchestration, payload integrity, and the
ISO catalog all live here.

`unsafe_code = "forbid"` (workspace-wide). The library has no
runtime dependencies that touch the network. Every destructive
operation crosses this crate's API.

---

## At a glance

| | |
|---|---|
| Crate name | `raidhos-core` |
| Public API | [`lib.rs`](src/lib.rs) — 8 functions, 6 types |
| MSRV | 1.78 |
| License | GPL-3.0-only |
| Lines of source | ~2,000 |
| Test count | 51 |
| Coverage (this crate) | 58.6% line, target ≥ 95% by v0.1.0 |

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
pub fn find_entry<'a>(catalog: &'a [CatalogEntry], slug: &str)
    -> Option<&'a CatalogEntry>;
pub fn verify_iso(
    entry: &CatalogEntry,
    iso: &Path, sums: &Path, sig: &Path, key_dir: &Path,
) -> Result<VerifiedIso, CatalogError>;
```

Types: `DiskInfo`, `PartitionInfo`, `IsoEntry`, `InstallRequest`,
`ProgressEvent`, `ProgressSink`, `CoreError`, `PayloadManifest`,
`ManifestError`, `CatalogEntry`, `CatalogError`, `VerifiedIso`.

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

---

## Architecture

```text
src/
├── lib.rs                  Public API + CoreError + validate_device_path
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
`#[cfg(target_os = "...")]`. The parsers are *not* gated — they
live in `parsers.rs` so cargo-fuzz can call them on any host
via `raidhos_core::__fuzz_api::*`.

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

Full attacker-model map: [`docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md).
Control citations: [`docs/HARDENING.md`](../../docs/HARDENING.md).

---

## Linking from your own crate

```toml
[dependencies]
raidhos-core = { git = "https://github.com/sebastienrousseau/raidhos", tag = "v0.0.1" }
```

The library has these deps:

- `serde` 1 + `serde_json` 1 (JSON / plist parsing)
- `sha2` 0.10 (payload manifest hash)
- `thiserror` 2 (typed errors)

Nothing else. No network, no async runtime, no procedural macros
beyond `serde_derive` / `thiserror_impl`.

---

## Testing

```bash
cargo test -p raidhos-core --all-targets
cargo tarpaulin -p raidhos-core --fail-under 55
```

The fuzz harness lives at [`fuzz/`](../../fuzz/) in the repo
root.

---

## See also

- [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) — where
  this crate fits in the workspace.
- [`docs/HARDENING.md`](../../docs/HARDENING.md) — every shipping
  control with `file:line` citations.
- [`docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md) — the
  attacker model this crate defends against.
- [`docs/DESIGN_NOTES.md`](../../docs/DESIGN_NOTES.md) — design
  decisions and trade-offs.
