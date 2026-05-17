<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-priv-helper` examples

The helper is the only RaidhOS binary intended to run elevated.
These scripts demonstrate its **exact** invocation contract:
single JSON line on stdout, progress events on stderr, exit
codes 0/1/2 only.

All scripts assume the helper is reachable as
`raidhos-priv-helper`. If you have built locally:

```bash
export PATH=$PWD/target/release:$PATH
```

| File | Subcommand | What it shows |
|---|---|---|
| [`01-list-disks.sh`](01-list-disks.sh) | `list-disks` | Disks visible to root, JSON-shape output. |
| [`02-list-partitions.sh`](02-list-partitions.sh) | `list-partitions <DEV>` | Partition layout of a chosen disk. |
| [`03-install-dry-run.sh`](03-install-dry-run.sh) | `install --dry-run` | Every safety check; no writes. |
| [`04-install-real.sh`](04-install-real.sh) | `install --allow-write` | The actually-destructive path. Read the script before running. |
| [`05-argv-cap.sh`](05-argv-cap.sh) | argv cap | Proves the 64 KiB combined-argv cap fires. |
| [`06-json-output.sh`](06-json-output.sh) | JSON shape | Shows both success and failure JSON shapes for callers. |

Larger flows that combine the helper with the CLI / UI live in
the top-level [`examples/`](../../../examples/) directory.

## Conventions

- All scripts are POSIX `sh`-compatible.
- Every script sets `set -euo pipefail`.
- Scripts that need root invoke `sudo` only on the helper
  itself, never on themselves — the script body always runs
  unprivileged.
- Destructive scripts print a confirmation prompt and the
  exact command they are about to run.

## See also

- [`../README.md`](../README.md) — the helper reference.
- [`../doc/USAGE.md`](../doc/USAGE.md) — task-oriented cookbook.
- [`../doc/HARDENING.md`](../doc/HARDENING.md) — what each
  defence layer covers.
