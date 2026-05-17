<!-- SPDX-License-Identifier: GPL-3.0-only -->

# RaidhOS shell-script examples

Top-level command flows — each script is a self-contained
example you can read first, then run. The library's
runnable Rust examples live under
[`crates/core/examples/`](../crates/core/examples/).

| File | What it shows |
|---|---|
| [`01-list-disks.sh`](01-list-disks.sh) | Plain `list-disks` + how to filter to removable. |
| [`02-dry-run-install.sh`](02-dry-run-install.sh) | A no-write rehearsal that exercises every validation guard. |
| [`03-real-install-linux.sh`](03-real-install-linux.sh) | Linux end-to-end with a Tails ISO. |
| [`04-catalog-verify.sh`](04-catalog-verify.sh) | Download an Ubuntu ISO and verify it against the bundled catalog. |
| [`05-persistence.sh`](05-persistence.sh) | Add a 4 GiB persistence overlay during install. |
| [`06-mok-enrol.sh`](06-mok-enrol.sh) | Enrol a RaidhOS MOK so Secure Boot accepts the signed `BOOTX64.EFI`. |

All scripts are `set -euo pipefail`-safe. They will refuse to
run if the prerequisites aren't on PATH.
