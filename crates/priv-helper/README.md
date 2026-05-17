<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-priv-helper

The single RaidhOS binary intended to run elevated. Everything
else — `raidhos-cli`, `raidhos-ui` — is unprivileged and
delegates the destructive step to this binary.

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

Citations: [`docs/HARDENING.md`](../../docs/HARDENING.md).

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

## Examples

```bash
# 1. List disks. (Run as root to see all of them.)
sudo raidhos-priv-helper list-disks | jq .

# 2. List partitions of a specific disk.
sudo raidhos-priv-helper list-partitions /dev/sdb | jq .

# 3. Dry-run an install — safe, no writes.
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
  raidhos-priv-helper install --device /dev/sdb --dry-run

# 4. Real install with a 4 GiB persistence overlay.
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
  raidhos-priv-helper install \
    --device /dev/sdb \
    --allow-write \
    --persistence-mb 4096
```

The UI calls the helper via `pkexec` (Linux), `osascript with
administrator privileges` (macOS), or `Start-Process -Verb
RunAs` (Windows). Direct invocation is fine for power users and
CI tests.

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

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success — `{ok: true}` on stdout |
| `1` | Runtime failure — `{ok: false, error: "..."}` on stdout |
| `2` | Argv parse error — `{ok: false}` plus clap's text on stdout |

---

## See also

- [`raidhos-core` README](../core/README.md) — the library the
  helper drives.
- [`docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md) — what
  this binary's hardening defends against.
- [`docs/HARDENING.md`](../../docs/HARDENING.md) — every
  shipping control with `file:line` citations.
- [`packaging/linux/org.raidhos.policy`](../../packaging/linux/org.raidhos.policy)
  — the polkit rule.
