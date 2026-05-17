<!-- SPDX-License-Identifier: GPL-3.0-only -->

<p align="center">
  <img src="https://raw.githubusercontent.com/sebastienrousseau/raidhos/main/docs/img/raidhos-logo.svg" alt="RaidhOS logo" width="128" />
</p>

<h1 align="center">raidhos-cli</h1>

<p align="center">
  Developer-facing command-line interface over
  <code>raidhos-core</code>. Same discovery and install pipeline
  as the Tauri UI, exposed as subcommands. Never runs elevated —
  destructive installs delegate to
  <code>raidhos-priv-helper</code>.
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/raidhos/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/raidhos/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/raidhos-cli"><img src="https://img.shields.io/crates/v/raidhos-cli.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Crates.io" /></a>
  <a href="https://docs.rs/raidhos-cli"><img src="https://img.shields.io/badge/docs.rs-raidhos--cli-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs.rs" /></a>
  <a href="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos"><img src="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos/badge" alt="OpenSSF Scorecard" /></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg?style=for-the-badge" alt="License" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — Homebrew, winget, Debian, Cargo, source
- [Quick Start](#quick-start) — list, scan, dry-run, verify

**CLI reference**

- [Subcommands](#subcommands) — one-line summary of every command
- [Examples](#examples) — runnable example index
- [Exit codes](#exit-codes) — stable across releases
- [Output formats](#output-formats) — column-separated, JSON
- [Safety defaults](#safety-defaults) — why a copy-paste never flashes

**Operational**

- [Configuration](#configuration) — environment, completions, man page
- [Testing](#testing) — unit, integration, CI
- [Documentation](#documentation) — all reference docs
- [License](#license)

---

## Install

### Package managers

| Channel | Install |
|---|---|
| Homebrew | `brew tap sebastienrousseau/tap && brew install raidhos` |
| winget | `winget install sebastienrousseau.RaidhOS` |
| Debian | `sudo dpkg -i raidhos_0.0.1_amd64.deb` |
| AppImage | `chmod +x raidhos-x86_64.AppImage && ./raidhos-x86_64.AppImage` |
| Cargo | `cargo install --git https://github.com/sebastienrousseau/raidhos --locked raidhos-cli` |
| Source | `cargo install --locked --path crates/cli` |

GitHub Releases publish pre-built tarballs for Linux (gnu+musl),
macOS (Intel + Apple Silicon + universal), and Windows
(x86_64). Each archive ships with the binary, man page
(`raidhos-cli.1`), shell completions, license bundle, and a
cosign keyless signature + SLSA L3 provenance.

### Build from source

```bash
git clone https://github.com/sebastienrousseau/raidhos.git
cd raidhos
cargo build --release -p raidhos-cli
./target/release/raidhos-cli --help
```

MSRV: 1.78. The `build.rs` emits a man page and shell completions
(bash / zsh / fish / elvish / PowerShell) into `OUT_DIR/dist/`
at compile time.

---

## Quick Start

```bash
# List the candidate disks. Removable + non-system only.
raidhos-cli list-disks

# Inventory ISOs you already downloaded.
raidhos-cli scan-isos --dirs /home,/media,/Downloads

# Dry-run an install. Validates every safety check, never writes.
raidhos-cli install --device /dev/sdX --dry-run

# Verify a downloaded Ubuntu ISO against the bundled catalog.
raidhos-cli catalog verify \
    --slug ubuntu-24.04-desktop-amd64 \
    --iso  ~/Downloads/ubuntu-24.04.3-desktop-amd64.iso \
    --sums ~/Downloads/SHA256SUMS \
    --sig  ~/Downloads/SHA256SUMS.gpg
```

A real install delegates to `raidhos-priv-helper` and asks for
authentication via `pkexec` (Linux), `osascript` (macOS), or
UAC (Windows). The CLI itself never asks for root.

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
  validate-device   Run validate_device_path() against a single string.
```

`--help` and `--version` are honoured on every subcommand.

---

## Examples

Runnable example scripts live in
[`crates/cli/examples/`](examples/). Each is a self-contained
shell wrapper around a single subcommand:

```bash
# 01 — list disks, then pretty-print as a table.
./examples/01-list-disks.sh

# 02 — scan a directory tree for ISOs, sorted alphabetically.
./examples/02-scan-isos.sh ~/Downloads /media

# 03 — dry-run an install against a fake device path.
./examples/03-install-dry-run.sh /dev/sdb

# 04 — show the bundled catalog and pick a slug to verify.
./examples/04-catalog-list.sh

# 05 — verify a downloaded ISO against the catalog.
./examples/05-catalog-verify.sh

# 06 — round-trip a boot.json into a mounted DATA partition.
./examples/06-write-config.sh
```

Example index: [`examples/README.md`](examples/README.md).

The top-level [`examples/`](../../examples/) directory holds
end-to-end flows that combine multiple CLI invocations (build
payload → write image → verify boot → recover).

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Runtime failure (validation refused, I/O error, command failed) |
| `2` | Argument parse error (clap rejected the input) |

Stable across releases. Wire into shell scripts.

---

## Output formats

### Column-separated (default)

`list-disks`, `scan-isos`, and `catalog list` produce
machine-readable lines:

```text
/dev/sdb USB DISK 3.0 32077825024 removable=true system=false mounts=
```

Pipe through `column -t` for human reading:

```bash
raidhos-cli list-disks | column -t
```

### JSON (`--json`)

Every command that produces output also accepts `--json`:

```bash
raidhos-cli list-disks --json | jq '.[] | select(.removable)'
```

### Progress events

`install` emits one line per progress phase on stdout:

```text
validate Validating target /dev/sdb 5%
prepare  Preparing partition layout 20%
format   Formatting FAT32 ESP 60%
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

The CLI itself never opens the device — destructive operations
go out to `raidhos-priv-helper` over a pkexec / osascript / UAC
prompt.

---

## Configuration

### Environment

| Variable | Read by | Purpose |
|---|---|---|
| `RAIDHOS_PAYLOAD_DIR` | `install` (passed through to helper) | Payload tree root |
| `RAIDHOS_CATALOG_PATH` | `catalog list/verify` | Override the bundled catalog.json |
| `RAIDHOS_KEY_DIR` | `catalog verify` | Override the bundled keyring directory |

### Shell completions

The `build.rs` emits completions for bash, zsh, fish, elvish,
and PowerShell at compile time. Installer scripts pick them up
from `target/release/build/raidhos-cli-*/out/dist/`. After
installing from a package manager, completions are placed under
the standard locations for your shell.

### Man page

A `raidhos-cli.1` man page is generated by `build.rs` and
installed alongside the binary.

---

## Testing

```bash
cargo test -p raidhos-cli --all-targets
```

The CLI has integration tests at
[`crates/priv-helper/tests/cli.rs`](../priv-helper/tests/cli.rs)
that spawn the actual binary and assert on exit codes, error
messages, and JSON output.

---

## Documentation

| Doc | What's in it |
|---|---|
| [`doc/USAGE.md`](doc/USAGE.md) | Cookbook for every subcommand |
| [`doc/SUBCOMMANDS.md`](doc/SUBCOMMANDS.md) | Per-subcommand reference |
| [`../../docs/USER_GUIDE.md`](../../docs/USER_GUIDE.md) | First-install walkthrough |
| [`../../docs/TROUBLESHOOTING.md`](../../docs/TROUBLESHOOTING.md) | Every error and its recovery |
| [`../core/README.md`](../core/README.md) | Library that powers the CLI |
| [`../priv-helper/README.md`](../priv-helper/README.md) | Binary that does the destructive bits |

---

## License

`raidhos-cli` is licensed under the **GPL-3.0-only**. See
[`LICENSE`](../../LICENSE) for the full text.
