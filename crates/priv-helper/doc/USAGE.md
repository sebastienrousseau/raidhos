<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-priv-helper usage

The single RaidhOS binary intended to run elevated. Spawned by
the CLI or the UI through `pkexec` (Linux), `osascript`
(macOS), or `Start-Process -Verb RunAs` (Windows).

Direct invocation under `sudo` is fine for power users and CI
tests; see the examples below.

---

## Synopsis

```text
raidhos-priv-helper <COMMAND> [ARGS...]
```

Output is always a single JSON object on stdout:

```json
{ "ok": true | false, "data": <T> | null, "error": <string> | null }
```

Stderr carries human-readable progress chatter from the install
pipeline. Don't parse stderr.

---

## `list-disks`

```text
raidhos-priv-helper list-disks
```

Same as the CLI's `list-disks`, but emits a JSON `data: [...]`
shape instead of plain-text columns. Useful when the UI needs
disks for the picker.

---

## `list-partitions <DEVICE>`

```text
raidhos-priv-helper list-partitions /dev/sdb
```

Partition metadata for a disk.

---

## `install`

```text
raidhos-priv-helper install
    --device <DEV>
    [--payload-version <VER>]      # default: 0.1.0
    [--dry-run]                    # default: off
    [--allow-write]                # default: off
    [--no-wipe]                    # debug only; install will refuse
    [--persistence-mb <N>]         # default: 0; Linux only
```

The destructive path requires **both** `wipe == true` (the
default) and `--allow-write`. The CLI gives both safe defaults;
the helper requires them to be explicit.

`RAIDHOS_PAYLOAD_DIR` must be set in the environment.

---

## Hardening (Linux)

The helper applies two defences on Linux before any privileged
work:

### Seccomp-bpf denylist

Installed post-parse, pre-write. Inherited across
`fork`/`clone`/`exec` so children (`parted`, `mkfs.vfat`, `cp`)
run under the same restrictions.

Denied syscalls return `EPERM`:

`ptrace`, `bpf`, `perf_event_open`, `userfaultfd`, `modify_ldt`,
`kexec_load`, `kexec_file_load`, `init_module`, `finit_module`,
`delete_module`, `create_module`, `process_vm_readv`,
`process_vm_writev`.

### TOCTOU device-fd pin

The helper opens the device once at validate time and snapshots
`rdev` via `fstat`. The fd is held for the install lifetime —
even if the `/dev/sdX` entry is removed mid-install, the kernel
keeps the underlying object reachable.

A later `stat` returning a different `rdev` is rejected as a
mid-install device swap.

Filter installation failures are non-fatal (the install still
runs without seccomp on a kernel that rejects the filter). TOCTOU
pin failure is fatal (we don't write to a device we can't pin).

---

## Examples

```bash
# 1. List disks (root sees all)
sudo raidhos-priv-helper list-disks | jq .

# 2. List partitions of a specific disk
sudo raidhos-priv-helper list-partitions /dev/sdb | jq .

# 3. Dry-run an install
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
  raidhos-priv-helper install --device /dev/sdb --dry-run

# 4. Real install with persistence
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
  raidhos-priv-helper install \
    --device /dev/sdb \
    --allow-write \
    --persistence-mb 4096

# 5. Programmatic — check exit + parse JSON
JSON=$(sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload \
       raidhos-priv-helper install --device /dev/sdb --dry-run)
if [[ $(echo "$JSON" | jq -r .ok) != "true" ]]; then
    echo "install refused: $(echo "$JSON" | jq -r .error)" >&2
    exit 1
fi
```

---

## Exit codes

| Code | Meaning | JSON shape |
|---|---|---|
| `0` | Success | `{ok: true, data: <T>, error: null}` |
| `1` | Runtime failure | `{ok: false, error: "<msg>"}` |
| `2` | Argv parse error | `{ok: false, error: "<clap msg>"}` |

`--help` and `--version` exit `0` with clap's text on stdout,
not wrapped in JSON.

---

## See also

- [`raidhos-cli` USAGE](../../cli/doc/USAGE.md) — the
  unprivileged front-end.
- [`docs/THREAT_MODEL.md`](../../../docs/THREAT_MODEL.md) — what
  the helper's hardening defends against.
- [`docs/HARDENING.md`](../../../docs/HARDENING.md) — every
  control with `file:line` citations.
