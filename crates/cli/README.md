<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-cli

Developer-facing command-line interface over `raidhos-core`.
Same discovery and install pipeline as the Tauri UI, exposed as
subcommands. Never runs elevated — destructive installs
delegate to `raidhos-priv-helper`.

---

## Install

| Channel | Install |
|---|---|
| Homebrew | `brew tap sebastienrousseau/tap && brew install raidhos` |
| winget | `winget install sebastienrousseau.RaidhOS` |
| Debian | `sudo dpkg -i raidhos_0.0.1_amd64.deb` |
| Source | `cargo install --git https://github.com/sebastienrousseau/raidhos --locked raidhos-cli` |

Build artefacts shipped alongside the binary: man page
(`raidhos-cli.1`), bash / zsh / fish / elvish / PowerShell
completions, emitted by [`build.rs`](build.rs) at compile time
into `OUT_DIR/dist/`.

---

## Subcommands

```text
raidhos-cli <COMMAND>

Commands:
  list-disks        Enumerate physical disks visible to the current user.
  scan-isos         Walk directories for *.iso files (one level deep).
  install           Run the install pipeline (defaults to --dry-run).
  write-config      Write a previously-saved boot.json into a mounted
                    partition.
  catalog list      List bundled catalog entries.
  catalog verify    Verify a locally-downloaded ISO against the catalog.
```

`--help` and `--version` are honoured on every subcommand.

---

## Examples

```bash
# 1. List candidate disks. Shows ID, model, size, removable / system
#    flags, and mountpoints.
raidhos-cli list-disks

# 2. Inventory ISOs you already downloaded.
raidhos-cli scan-isos --dirs /home,/media,/Downloads

# 3. Dry-run an install. Validates every safety check, never writes.
raidhos-cli install --device /dev/sdX --dry-run

# 4. List bundled distros + their slugs.
raidhos-cli catalog list

# 5. Verify a downloaded Ubuntu ISO against the catalog (needs gpg).
raidhos-cli catalog verify \
    --slug ubuntu-24.04-desktop-amd64 \
    --iso  ~/Downloads/ubuntu-24.04.3-desktop-amd64.iso \
    --sums ~/Downloads/SHA256SUMS \
    --sig  ~/Downloads/SHA256SUMS.gpg

# 6. Write a saved boot.json into a mounted DATA partition.
raidhos-cli write-config \
    --mount-path /mnt/raidhos-data \
    --config-path ~/.config/raidhos/boot.json
```

Full command flows: [`../../examples/`](../../examples/).

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Runtime failure (validation refused, I/O error, command failed) |
| `2` | Argument parse error (clap rejected the input) |

Stable across releases. Wire into shell scripts.

---

## Output

`list-disks`, `scan-isos`, and `catalog list` produce
column-separated machine-readable output:

```text
/dev/sdb USB DISK 3.0 32077825024 removable=true system=false mounts=
```

For human consumption, pipe through `column -t`:

```bash
raidhos-cli list-disks | column -t
```

`install` emits progress events as lines on stdout:

```text
validate Validating target /dev/sdb 5%
prepare  Preparing partition layout 20%
…
complete Dry-run complete. No changes made. 100%
```

---

## Safety defaults

```bash
# Equivalent to running with no flags after --device:
raidhos-cli install --device /dev/sdX \
    --dry-run=true --wipe=true --allow-write=false
```

A user who copy-pastes from the docs and forgets to set
`--allow-write=true` does **not** flash anything by accident.

---

## See also

- [`raidhos-core` README](../core/README.md) — the library
  behind the CLI.
- [`raidhos-priv-helper` README](../priv-helper/README.md) —
  the binary that actually runs elevated.
- [`docs/USER_GUIDE.md`](../../docs/USER_GUIDE.md) — first
  install walkthrough.
- [`docs/TROUBLESHOOTING.md`](../../docs/TROUBLESHOOTING.md) —
  every error and recovery.
