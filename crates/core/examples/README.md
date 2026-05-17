<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-core examples

Runnable examples for the public API. Each file is a complete,
self-contained program. Run with:

```bash
cargo run --example <name> -p raidhos-core -- [args]
```

| Example | What it shows |
|---|---|
| [`list_disks.rs`](list_disks.rs) | Enumerate physical disks via the per-OS discovery backend. |
| [`validate_device_path.rs`](validate_device_path.rs) | The per-OS device-path allowlist + every rejection reason. |
| [`scan_isos.rs`](scan_isos.rs) | One-level filesystem scan for `*.iso` files. |
| [`verify_payload.rs`](verify_payload.rs) | Walk a payload tree and verify it against `manifest.json`. |
| [`catalog_verify.rs`](catalog_verify.rs) | Catalog lookup + `gpg(1)` verification of a downloaded ISO. |
| [`install_dry_run.rs`](install_dry_run.rs) | Run the install pipeline with `dry_run = true` — no writes. |

All examples exit 0 on success and a non-zero code on failure;
they're safe to wire into shell scripts.

See the per-crate API documentation at
[`crates/core/doc/`](../doc/) and the architecture overview at
[`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md).
