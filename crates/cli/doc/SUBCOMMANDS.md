<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-cli` — per-subcommand reference

Every subcommand is described here with: argv shape, defaults,
exit codes specific to the command, output shape, and the
[`raidhos-core`](../../core/README.md) function it delegates
to. For task-oriented examples see [`USAGE.md`](USAGE.md).

---

## `list-disks`

```text
raidhos-cli list-disks [--json]
```

Enumerate physical disks visible to the current user.

| Output | Form |
|---|---|
| Default | Column-separated text lines (one disk per row) |
| `--json` | JSON array of `DiskInfo` objects |

| Exit | Meaning |
|---|---|
| 0 | At least one disk listed |
| 1 | Discovery subprocess failed |
| 2 | Argv parse error |

Delegates to: `raidhos_core::list_disks`.

---

## `list-partitions`

```text
raidhos-cli list-partitions --device <DEV> [--json]
```

List partitions on a specific disk.

| Argument | Required | Notes |
|---|---|---|
| `--device <DEV>` | yes | Per-OS path (`/dev/sdb`, `/dev/disk2`, `\\.\PhysicalDriveN`) |

Delegates to: `raidhos_core::list_partitions`.

---

## `scan-isos`

```text
raidhos-cli scan-isos --dirs <DIR>[,<DIR>...] [--json]
```

Walk the supplied directories looking for `*.iso` files (one
level deep — matches the on-USB layout).

Delegates to: `raidhos_core::scan_isos`.

---

## `install`

```text
raidhos-cli install [--device <DEV> | --simulator <FILE>]
                    [--simulator-size-mb <N>]
                    [--payload-version <VER>]
                    [--dry-run]
                    [--allow-write]
                    [--no-wipe]
                    [--persistence-mb <N>]
                    [--data-fs <FS>]
                    [--bios-compat]
```

Run the install pipeline. Defaults are safe — without
`--allow-write`, the CLI dry-runs.

| Flag | Default | Purpose |
|---|---|---|
| `--device <DEV>` | one-of required | Target device path |
| `--simulator <FILE>` | one-of required | Sparse-file preview target (mutually exclusive with `--device`). Closes Ventoy gap G24's preview need; no real hardware touched. |
| `--simulator-size-mb <N>` | 256 | Size of a newly-created `--simulator` file |
| `--payload-version <VER>` | from `manifest.json` | Payload version pin |
| `--dry-run` | on (until `--allow-write` is set) | No writes |
| `--allow-write` | off | Required for destructive run |
| `--no-wipe` | off | Refuses install; debug-only |
| `--persistence-mb <N>` | 0 | Persistence image size (Linux only) |
| `--data-fs <FS>` | `exfat` | DATA partition filesystem. One of `exfat`, `ntfs`, `ext4`, `btrfs`, `xfs`. Closes Ventoy gaps G8 (NTFS) and G9 (ext4/Btrfs/XFS). Linux pipeline only today; macOS / Windows install paths use the platform-native equivalents. |
| `--bios-compat` | off | Opt-in Legacy-BIOS-bootable layout. Scaffolded in v0.0.1: the request is accepted and dry-run validates the shape; the destructive write path returns `CoreError::NotImplemented` until v0.0.2 ships the i386-pc GRUB binary. Closes Ventoy gap G1 (scaffold). |

Delegates to: `raidhos_core::install` (dry-run) or
`raidhos-priv-helper install` (destructive run, via the OS's
elevation primitive).

---

## `write-config`

```text
raidhos-cli write-config --mount-path <MOUNT> --config-path <PATH>
```

Write a previously-saved `boot.json` into a mounted partition.
Used to update an existing RaidhOS USB without re-flashing.

---

## `catalog list`

```text
raidhos-cli catalog list [--json]
```

Print the bundled distro catalog. Each entry has a slug
(machine-friendly identifier) plus metadata used by `catalog
verify`.

Delegates to: `raidhos_core::load_catalog`.

---

## `catalog verify`

```text
raidhos-cli catalog verify --slug <SLUG>
                           --iso  <PATH>
                           --sums <PATH>
                           --sig  <PATH>
                           [--key-dir <PATH>]
```

Verify a locally-downloaded ISO against the catalog. Requires
`gpg(1)` on `PATH`.

| Argument | Required | Notes |
|---|---|---|
| `--slug <SLUG>` | yes | Identifies the catalog entry |
| `--iso <PATH>` | yes | The downloaded ISO |
| `--sums <PATH>` | yes | The downloaded `SHA256SUMS` file |
| `--sig <PATH>` | yes | The downloaded `SHA256SUMS.gpg` |
| `--key-dir <PATH>` | no, defaults to bundled | Override the keyring directory |

Delegates to: `raidhos_core::verify_iso`.

---

## `validate-device`

```text
raidhos-cli validate-device <DEV>
```

Run `validate_device_path()` against a single string. Exits 0
on accept, 1 on refuse. Useful for shell-script gating.

Delegates to: `raidhos_core::validate_device_path`.

---

## Global flags

| Flag | Purpose |
|---|---|
| `--help` | Per-subcommand help text |
| `--version` | Print the binary version + commit hash |
| `--json` | Where the command produces structured output, emit JSON instead of columns |

---

## See also

- [`USAGE.md`](USAGE.md) — task-oriented cookbook.
- [`../README.md`](../README.md) — CLI reference (Quick Start,
  install channels).
- [`../../core/doc/API.md`](../../core/doc/API.md) — per-symbol
  reference for the underlying library.
