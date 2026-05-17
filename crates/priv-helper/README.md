<!-- SPDX-License-Identifier: GPL-3.0-only -->

<p align="center">
  <img src="https://raw.githubusercontent.com/sebastienrousseau/raidhos/main/docs/img/raidhos-logo.svg" alt="RaidhOS logo" width="128" />
</p>

<h1 align="center">raidhos-priv-helper</h1>

<p align="center">
  The single RaidhOS binary intended to run elevated. Everything
  else — <code>raidhos-cli</code>, <code>raidhos-ui</code> — is
  unprivileged and delegates the destructive step to this
  binary.
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/raidhos/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/raidhos/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/raidhos-priv-helper"><img src="https://img.shields.io/crates/v/raidhos-priv-helper.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Crates.io" /></a>
  <a href="https://docs.rs/raidhos-priv-helper"><img src="https://img.shields.io/badge/docs.rs-raidhos--priv--helper-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs.rs" /></a>
  <a href="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos"><img src="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos/badge" alt="OpenSSF Scorecard" /></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg?style=for-the-badge" alt="License" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — bundled with the desktop app; also on Cargo
- [Quick Start](#quick-start) — list, dry-run, real install

**Helper reference**

- [Subcommands](#subcommands) — argv shape
- [Hardening](#hardening) — every defence layer with citations
- [Examples](#examples) — runnable example index
- [Output shape](#output-shape) — single JSON line on stdout
- [Exit codes](#exit-codes) — stable across releases

**Operational**

- [Privilege escalation](#privilege-escalation) — pkexec, osascript, UAC
- [Seccomp denylist](#seccomp-denylist) — Linux
- [TOCTOU defence](#toctou-defence) — Linux
- [Testing](#testing) — unit, integration, fuzz
- [Documentation](#documentation) — all reference docs
- [License](#license)

---

## Install

The helper is bundled with the desktop application's installer:

| Channel | What you get |
|---|---|
| Homebrew | `brew install raidhos` ships both CLI + helper |
| winget | `winget install sebastienrousseau.RaidhOS` ships the helper alongside the UI |
| Debian | `raidhos.deb` installs the helper to `/usr/libexec/raidhos-priv-helper` and the polkit policy to `/usr/share/polkit-1/actions/` |
| AppImage | The AppImage bundles the helper; you need a separate polkit policy install for true elevation |
| Cargo (advanced) | `cargo install --git https://github.com/sebastienrousseau/raidhos --locked raidhos-priv-helper` |

The polkit / launchd / scheduled-task plumbing only fires when
the helper is invoked through the OS-specific elevation
wrapper — see [Privilege escalation](#privilege-escalation).

### Build from source

```bash
git clone https://github.com/sebastienrousseau/raidhos.git
cd raidhos
cargo build --release -p raidhos-priv-helper
sudo ./target/release/raidhos-priv-helper list-disks | jq .
```

MSRV: 1.78.

---

## Quick Start

```bash
# 1. Inventory disks visible to root.
sudo raidhos-priv-helper list-disks | jq .

# 2. Dry-run an install. Every safety check runs, nothing is written.
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
    raidhos-priv-helper install --device /dev/sdb --dry-run

# 3. Real install with a 4 GiB persistence overlay (Linux only).
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
    raidhos-priv-helper install \
        --device /dev/sdb \
        --allow-write \
        --persistence-mb 4096
```

In production this binary is never invoked directly. It runs
behind `pkexec` / `osascript` / `Start-Process -Verb RunAs`, and
the GUI / CLI wraps it.

---

## Subcommands

```text
raidhos-priv-helper <COMMAND>

Commands:
  list-disks                  Enumerate physical disks.
  list-partitions <DEVICE>    List partitions on a specific disk.
  install [OPTIONS]           Run the install pipeline.
```

### `install` arguments

| Flag | Default | Purpose |
|---|---|---|
| `--device <DEV>` | (required) | Target device path |
| `--payload-version <VER>` | `0.1.0` | Payload version string |
| `--dry-run` | off | Run every check; never write |
| `--allow-write` | off | Required to actually touch the device |
| `--no-wipe` | off | Debug-only; install will refuse |
| `--persistence-mb <N>` | `0` | Persistence overlay size in MiB (Linux only) |

The destructive path requires **both** `--allow-write` and
`wipe == true`. The CLI gives both safe defaults; the helper
requires them to be explicit.

---

## Hardening

| Layer | Control |
|---|---|
| Argv | `clap` derive parser; unknown flags fail loudly |
| Argv | 64 KiB combined-byte cap (`enforce_argv_budget`) |
| Kernel (Linux) | seccomp-bpf denylist — ptrace, bpf, kexec_*, module load/unload/create, perf_event_open, userfaultfd, modify_ldt, process_vm_readv/writev |
| Filesystem (Linux) | TOCTOU-safe device fd pin — open + fstat(rdev) + hold fd for the install lifetime |
| Privilege | No setuid. Always invoked through `pkexec` (Linux), `osascript with administrator privileges` (macOS), or `Start-Process -Verb RunAs` (Windows) |
| Output | JSON-over-stdout — machine-readable, never HTML/shell-injectable |
| Exit | `--help` / `--version` exit 0 cleanly, not wrapped in a JSON error |
| Trust boundary | Every byte that crosses into a destructive operation goes through `raidhos-core` first |

Full citations: [`docs/HARDENING.md`](../../docs/HARDENING.md).

---

## Examples

Runnable examples live in [`examples/`](examples/). Each one
exercises one subcommand or one hardening layer:

```bash
# 01 — list-disks under sudo, with JSON pretty-printing.
./examples/01-list-disks.sh

# 02 — list-partitions for the device picked by 01.
./examples/02-list-partitions.sh /dev/sdb

# 03 — dry-run install, no writes.
./examples/03-install-dry-run.sh /dev/sdb

# 04 — real install, requires --allow-write.
./examples/04-install-real.sh /dev/sdb

# 05 — argv length cap: prove it bounces a >64 KiB argument.
./examples/05-argv-cap.sh

# 06 — show the helper's JSON output shape on success and failure.
./examples/06-json-output.sh
```

Example index: [`examples/README.md`](examples/README.md).

---

## Output shape

Every invocation prints a single JSON object on stdout:

```json
{
  "ok": true,
  "data": null,
  "error": null
}
```

```json
{
  "ok": false,
  "data": null,
  "error": "validation: device must be an absolute /dev path"
}
```

Callers (the UI's `install_elevated` Tauri command, the CLI's
elevation wrapper, your own scripts) parse exactly that shape
— there is no second line, no progress chatter on stdout,
nothing to scrape.

Progress events from the install pipeline go to **stderr**.

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success — `{ok: true}` on stdout |
| `1` | Runtime failure — `{ok: false, error: "..."}` on stdout |
| `2` | Argv parse error — `{ok: false}` plus clap's text on stdout |

---

## Privilege escalation

The helper is **not** setuid. Elevation is delegated per host:

| Host | Mechanism | Authoring |
|---|---|---|
| Linux | `pkexec` against [`org.raidhos.priv.policy`](../../packaging/polkit/org.raidhos.priv.policy) | Distros install the policy to `/usr/share/polkit-1/actions/` |
| macOS | `osascript -e 'do shell script "…" with administrator privileges'` | Built into the GUI; no preinstalled policy |
| Windows | `Start-Process -Verb RunAs` triggers UAC | Built into the GUI; no preinstalled policy |

The UI never asks for a password directly — that is the OS's
job. The polkit policy on Linux can be tightened per-user / per-
group via standard polkit `rules.d/*.rules` files.

---

## Seccomp denylist

Linux only. Applied **after** argv parsing and **before** any
privileged work:

| Syscall | Why denied |
|---|---|
| `ptrace` | Debugger attach |
| `bpf` | eBPF program load |
| `perf_event_open` | Perf side channels |
| `userfaultfd` | Userspace fault handling for race-window widening |
| `modify_ldt` | x86 LDT tricks |
| `kexec_load`, `kexec_file_load` | Replace running kernel |
| `init_module`, `finit_module`, `delete_module`, `create_module` | Kernel module load/unload |
| `process_vm_readv`, `process_vm_writev` | Cross-process memory |

The filter is **inherited** across `fork` / `clone` / `exec`,
so the children (`parted`, `mkfs.vfat`, `cp`) run under the
same restrictions.

Filter installation failures are non-fatal — the helper logs
and continues so the install still works on a kernel without
seccomp support. Source: [`src/seccomp.rs`](src/seccomp.rs).

---

## TOCTOU defence

Linux only. Before any partition / format / copy:

1. `OpenOptions::open(device)` — get a file descriptor.
2. `fstat(fd)` — snapshot `rdev` (major:minor) and size.
3. Hold the fd open for the duration of the install.

The kernel keeps the underlying object reachable even if the
`/dev/sdX` entry is removed mid-install, and any later `stat`
on the same path that returns a different `rdev` is rejected.
Source: [`src/toctou.rs`](src/toctou.rs).

---

## Testing

```bash
cargo test -p raidhos-priv-helper --all-targets
```

Integration tests in [`tests/cli.rs`](tests/cli.rs) spawn the
real binary and assert on:

- `--version` and `--help` exit cleanly with exit code 0
- Unknown subcommand emits a `{ok: false}` JSON error and
  exits 2
- `install` requires `--device`
- `list-disks` returns a JSON object
- An invalid device path fails cleanly with exit 1
- The 64 KiB argv length cap fires (Unix only — Windows's
  CreateProcess refuses the input before our cap can run)

---

## Documentation

| Doc | What's in it |
|---|---|
| [`doc/USAGE.md`](doc/USAGE.md) | Cookbook for the helper subcommands |
| [`doc/HARDENING.md`](doc/HARDENING.md) | Crate-local hardening reference |
| [`../../docs/HARDENING.md`](../../docs/HARDENING.md) | Workspace-wide hardening citations |
| [`../../docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md) | Attacker model this binary defends against |
| [`../core/README.md`](../core/README.md) | Library the helper drives |
| [`packaging/polkit/org.raidhos.priv.policy`](../../packaging/polkit/org.raidhos.priv.policy) | Linux polkit rule |

---

## License

`raidhos-priv-helper` is licensed under the **GPL-3.0-only**.
See [`LICENSE`](../../LICENSE) for the full text.
