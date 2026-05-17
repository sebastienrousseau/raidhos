<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-priv-helper` Hardening

This document is the **crate-local** hardening reference. The
workspace-wide picture is at
[`docs/HARDENING.md`](../../../docs/HARDENING.md), and the
attacker model the controls defend against is at
[`docs/THREAT_MODEL.md`](../../../docs/THREAT_MODEL.md).

The priv-helper is the *only* RaidhOS binary intended to run
elevated. Every layer below sits between the privilege
boundary and a destructive `Command::new(…)` call.

---

## L0 — Workspace lints

[`Cargo.toml:[workspace.lints.rust]`](../../../Cargo.toml):

```toml
unsafe_code = "forbid"
```

A grep for `unsafe` in this crate's `src/` returns zero hits.
CI runs `cargo clippy --workspace --exclude raidhos-ui
--all-targets -- -D warnings`, blocking any regression.

---

## L1 — argv

### L1.1 — clap derive parser

Source: [`src/main.rs`](../src/main.rs). Unknown flags exit 2
with clap's standard error text before any privileged work
starts. No positional-arg fallthrough.

### L1.2 — 64 KiB combined-byte cap

Source: [`src/main.rs::enforce_argv_budget`](../src/main.rs).
The sum of all `argv[i].len()` is checked before clap parses.
A request larger than 64 KiB exits 2 with
`{"ok":false,"error":"argv length …"}`.

Why: clap allocates on every argument; an attacker who can
spawn the helper (e.g. via a runaway script) shouldn't be able
to balloon its memory. The cap is comfortably above any real
invocation (longest seen in CI: ~400 bytes).

### L1.3 — Validation before disk open

Every request flows through
`raidhos_core::validate_device_path` before the helper opens
or stat()s the device. Refuses:

- Empty / > 256 byte path
- Shell metacharacters (`;|&$\`<>"'*?()` plus `\n\r\t`)
- `..` (path traversal)
- Wrong per-OS shape

---

## L2 — Kernel (Linux)

### L2.1 — seccomp-bpf denylist

Source: [`src/seccomp.rs`](../src/seccomp.rs). Applied after
argv parsing, before any privileged work. The filter is
**inherited** across `fork` / `clone` / `exec`, so children
(`parted`, `mkfs.vfat`, `cp`) run with the same restrictions.

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

Filter installation failures are **non-fatal** — the helper
logs and continues so the install still works on a kernel
without seccomp support. On systemd-nspawn and unprivileged
LXC the architecture target may be unrecognised; the helper
handles that gracefully (see the `riscv64` arm).

### L2.2 — TOCTOU-safe device fd

Source: [`src/toctou.rs`](../src/toctou.rs). Before any
destructive operation:

1. `OpenOptions::open(device)` — get a file descriptor.
2. `fstat(fd)` — snapshot `rdev` (major:minor) and size.
3. Hold the fd open for the duration of the install.

The kernel keeps the underlying object reachable even if the
`/dev/sdX` entry is removed mid-install. Any later `stat()`
on the same path that returns a different `rdev` triggers a
refusal in `assert_disk_identity()`.

---

## L3 — Privilege

### L3.1 — Never setuid

The helper has no `s` mode bit. It runs with whatever
privileges its parent has.

### L3.2 — pkexec / osascript / UAC

The OS-level elevation primitive is the only legitimate way to
elevate. Each one prompts the user for authentication:

| Host | Mechanism | Authoring |
|---|---|---|
| Linux | `pkexec` against [`org.raidhos.priv.policy`](../../../packaging/polkit/org.raidhos.priv.policy) | Installed by distro packages |
| macOS | `osascript -e 'do shell script "…" with administrator privileges'` | Built into the GUI |
| Windows | `Start-Process -Verb RunAs` triggers UAC | Built into the GUI |

The polkit rule allows interactive auth only. There is no
"always allow" flag.

---

## L4 — Output

### L4.1 — Single JSON line on stdout

Source: [`src/main.rs`](../src/main.rs). The helper prints
exactly one JSON object on stdout per invocation. No second
line, no progress chatter, no debug logs interleaved with the
machine-readable output. Callers (UI, CLI, scripts) parse the
shape directly.

### L4.2 — Progress events on stderr

Progress events from the install pipeline go to **stderr**
(typed, one event per line). This separates "what the helper
returned" from "what the helper is doing", which matters when
the helper is wrapped in pkexec / osascript output capture.

### L4.3 — Exit codes

| Code | Meaning |
|---|---|
| `0` | Success — `{ok: true}` on stdout |
| `1` | Runtime failure — `{ok: false, error: "..."}` on stdout |
| `2` | Argv parse error — `{ok: false}` plus clap's text on stdout |

Stable across releases.

---

## L5 — Build / supply chain

The helper participates in the workspace's build pipeline:

- `cargo deny check` and `cargo audit` block known-bad
  advisories per [`deny.toml`](../../../deny.toml).
- Releases are signed with cosign keyless + SLSA L3 provenance.
- CycloneDX SBOMs are attached to every release.
- CodeQL + OpenSSF Scorecards run on every push.

The full workflow set lives in
[`.github/workflows/`](../../../.github/workflows/).

---

## Testing the controls

```bash
# Argv cap fires:
cargo test -p raidhos-priv-helper argv_length_cap_rejects_huge_argv

# Validation refuses metachars:
cargo test -p raidhos-priv-helper invalid_device_path_fails_cleanly

# Integration tests in tests/cli.rs spawn the real binary:
cargo test -p raidhos-priv-helper --test cli
```

The seccomp filter is best tested under a kernel-capable
runner (CI runs Linux tests on `ubuntu-latest`). The TOCTOU
helpers are unit-tested with mock filesystem state.

---

## See also

- [`USAGE.md`](USAGE.md) — task-oriented cookbook.
- [`../README.md`](../README.md) — helper reference.
- [`../../core/doc/SECURITY.md`](../../core/doc/SECURITY.md) —
  library-level controls.
- [`../../../docs/HARDENING.md`](../../../docs/HARDENING.md) —
  workspace-wide citations.
- [`../../../docs/THREAT_MODEL.md`](../../../docs/THREAT_MODEL.md) —
  attacker model.
