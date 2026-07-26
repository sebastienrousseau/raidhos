<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Using `raidhos-core` from your own crate

This document is the cookbook complement to [`API.md`](API.md).
It shows the most common flows end-to-end. Every snippet
compiles with the dependencies listed in
[`crates/core/Cargo.toml`](../Cargo.toml).

The corresponding runnable programs live in
[`crates/core/examples/`](../examples/). Run any of them with:

```bash
cargo run --example <name> -p raidhos-core
```

---

## 1. Enumerate disks

```rust
use raidhos_core::list_disks;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for d in list_disks()? {
        println!(
            "{:<20} {:<25} {:>15} bytes  removable={}  system={}",
            d.id, d.model, d.size_bytes, d.removable, d.is_system,
        );
    }
    Ok(())
}
```

The function shells out per host:

| Host | Subprocess |
|---|---|
| Linux | `lsblk -b -J -o NAME,MODEL,SIZE,RM,TYPE,MOUNTPOINTS` |
| macOS | `diskutil list -plist external` |
| Windows | `powershell Get-Disk \| ConvertTo-Json` |

Output is normalised into `DiskInfo`. See
[`examples/list_disks.rs`](../examples/list_disks.rs).

---

## 2. Validate a user-supplied device path

```rust
use raidhos_core::{validate_device_path, CoreError};

fn require_safe_device(s: &str) -> Result<(), CoreError> {
    validate_device_path(s)
}
```

`validate_device_path` checks:

1. Non-empty, length ≤ 256.
2. No shell metacharacters (`;|&$`…).
3. No `..` traversal.
4. Per-OS shape (`/dev/…` on Linux, `/dev/disk…` on macOS,
   `\\.\PhysicalDriveN` on Windows).

It does **not** stat the file or open it. Use it as an input
gate, then let `install()` handle the rest.

See [`examples/validate_device_path.rs`](../examples/validate_device_path.rs).

---

## 3. Dry-run the install pipeline

```rust
use raidhos_core::{install, InstallRequest, ProgressEvent, ProgressSink};

struct Quiet;
impl ProgressSink for Quiet {
    fn emit(&self, _e: ProgressEvent) {}
}

let req = InstallRequest {
    device: "/dev/sdb".into(),
    payload_version: "0.0.1".into(),
    wipe: true,
    dry_run: true,         // no bytes written
    allow_write: false,
};
let _ = install(req, &Quiet);
```

A dry-run runs every validation step (device discovery, mount
check, payload presence, partition layout planning) and prints
the events it would emit — but never touches the disk.

See [`examples/install_dry_run.rs`](../examples/install_dry_run.rs).

---

## 4. Verify a payload tree against its manifest

```rust
use raidhos_core::{verify_payload, ManifestError};
use std::path::Path;

let payload = Path::new("/srv/raidhos/payload");
match verify_payload(payload) {
    Ok(()) => println!("payload OK"),
    Err(ManifestError::Mismatch { path, expected, computed }) => {
        eprintln!("hash mismatch on {path}: expected {expected}, got {computed}");
    }
    Err(e) => eprintln!("verify failed: {e}"),
}
```

`verify_payload` reads `manifest.json` from the payload root,
walks the tree under `esp/` and `data/`, and SHA-256-hashes
every file. On mismatch it reports the first divergence so the
caller can pin-point a corrupted file.

See [`examples/verify_payload.rs`](../examples/verify_payload.rs).

---

## 5. Verify a downloaded ISO against the catalog

```rust
use raidhos_core::{find_entry, load_catalog, verify_iso, CatalogError};
use std::path::Path;

let catalog = load_catalog()?;
let entry = find_entry(&catalog, "ubuntu-24.04-desktop-amd64")
    .ok_or("not in catalog")?;

let result = verify_iso(
    entry,
    Path::new("/downloads/ubuntu.iso"),
    Path::new("/downloads/SHA256SUMS"),
    Path::new("/downloads/SHA256SUMS.gpg"),
    Path::new("catalog/keys"),  // bundled in the repo
);
match result {
    Ok(v) => println!("verified: sha256 = {}", v.computed_sha256),
    Err(CatalogError::GpgMissing) => eprintln!("install gpg(1) first"),
    Err(CatalogError::SignatureRejected(why)) => eprintln!("bad signature: {why}"),
    Err(e) => eprintln!("verify failed: {e}"),
}
```

`verify_iso` invokes `gpg(1)` through the `Runtime` abstraction
so the call is unit-testable via `MockRuntime`. The bundled
keyring lives at `catalog/keys/*.asc` in the repo.

See [`examples/catalog_verify.rs`](../examples/catalog_verify.rs).

---

## 6. Drive the install pipeline from a mocked runtime (unit testing)

`raidhos-core` exposes `Runtime` + `MockRuntime` so callers can
unit-test code that uses the install pipeline without spawning
real subprocesses:

```rust
use raidhos_core::{MockOutcome, MockRuntime, Runtime};

let mut rt = MockRuntime::new();
rt.outcomes = std::cell::RefCell::new(vec![
    MockOutcome::Ok(b"".to_vec()),      // first subprocess: parted
    MockOutcome::Ok(b"".to_vec()),      // second: mkfs.vfat
    MockOutcome::Ok(b"".to_vec()),      // third: mkfs.exfat
]);
let invocations_before = rt.invocations.borrow().len();
assert_eq!(invocations_before, 0);

// Then call any function that takes &dyn Runtime, e.g.
// verify_iso_with(&rt, …).
```

The full Runtime API is in [`API.md`](API.md#runtime-trait).

---

## Errors

All public functions return `Result<T, CoreError>` (or a
specialised error type with a `From<…>` into `CoreError`). The
five-variant enum is `Display`-stable across patch releases:

```rust
pub enum CoreError {
    UnsupportedPlatform,
    Io(String),
    Validation(String),
    NotImplemented(String),
    Parse(String),
}
```

The CLI greps on rendered strings, so don't reword the variants
between releases. New error contexts are added by extending the
inner `String`, not by introducing new variants.

---

## Re-exports for fuzzing

Parser entry-points are re-exported under
`raidhos_core::__fuzz_api::*` for the cargo-fuzz harness in
[`fuzz/`](../../../fuzz/). They are `#[doc(hidden)]` and not
guaranteed stable — call the public functions for production
use.

```rust
// in fuzz_targets/parse_lsblk_disks.rs
fuzz_target!(|data: &[u8]| {
    let _ = raidhos_core::__fuzz_api::parse_lsblk_disks(data);
});
```

---

## See also

- [`API.md`](API.md) — per-symbol reference
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — module map
- [`SECURITY.md`](SECURITY.md) — control inventory with citations
- [Top-level docs/](../../../docs/) — workspace-wide architecture,
  hardening, threat model.
