<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-cli` examples

Each script in this directory is a self-contained demonstration
of one `raidhos-cli` subcommand. They are kept deliberately
short so the *command-line shape* is what stands out, not the
shell plumbing.

The scripts assume a release build of the CLI is on `PATH`:

```bash
cargo install --locked --path crates/cli
# or run them against the local build:
export PATH=$PWD/target/release:$PATH
```

| File | Subcommand | What it shows |
|---|---|---|
| [`01-list-disks.sh`](01-list-disks.sh) | `list-disks` | Enumerate physical disks; pretty-print with `column -t`. |
| [`02-scan-isos.sh`](02-scan-isos.sh) | `scan-isos` | Walk one or more directories for `*.iso` files. |
| [`03-install-dry-run.sh`](03-install-dry-run.sh) | `install --dry-run` | Validate every safety check, write nothing. |
| [`04-catalog-list.sh`](04-catalog-list.sh) | `catalog list` | Inspect the bundled distro catalog. |
| [`05-catalog-verify.sh`](05-catalog-verify.sh) | `catalog verify` | GPG-verify + SHA-256-check a downloaded ISO. |
| [`06-write-config.sh`](06-write-config.sh) | `write-config` | Push a saved `boot.json` into a mounted partition. |
| [`07-validate-device.sh`](07-validate-device.sh) | `validate-device` | Probe the `validate_device_path()` gate. |

Larger end-to-end flows that combine multiple subcommands live
in the top-level [`examples/`](../../../examples/) directory.

## Conventions

- All scripts are POSIX `sh`-compatible (no bashisms).
- Every script sets `set -euo pipefail` at the top.
- Every script prints what it is about to run before it runs it,
  so the output reads like a transcript.
- Scripts never assume root. The real install path goes through
  `raidhos-priv-helper`, never `sudo`.

## See also

- [`../README.md`](../README.md) — the CLI reference.
- [`../doc/USAGE.md`](../doc/USAGE.md) — task-oriented cookbook.
- [`../doc/SUBCOMMANDS.md`](../doc/SUBCOMMANDS.md) — per-subcommand reference.
