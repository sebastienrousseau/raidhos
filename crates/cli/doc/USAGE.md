<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-cli usage

Full CLI reference. The man page (`raidhos-cli(1)`) and shell
completions are generated at build time from
[`crates/cli/src/cli.rs`](../src/cli.rs).

---

## Synopsis

```text
raidhos-cli <COMMAND> [OPTIONS]
```

Global flags: `--help`, `--version`. Subcommands have their own
`--help`.

---

## `list-disks`

```text
raidhos-cli list-disks
```

Enumerate physical disks visible to the current user. One line
per disk:

```text
<id> <model> <size_bytes> removable=<bool> system=<bool> mounts=<csv>
```

Suitable for `awk` / `cut` / `column -t`.

---

## `scan-isos`

```text
raidhos-cli scan-isos [--dirs <CSV>]
```

Walk the supplied directories (one level deep) for `*.iso`
files. Defaults to `/media,/mnt,/home` when no `--dirs` is
given. Output:

```text
<title> <abs-path> <size_bytes> <default-params>
```

---

## `install`

```text
raidhos-cli install --device <DEVICE>
                    [--payload-version <VER>]
                    [--wipe={true|false}]            # default: true
                    [--dry-run={true|false}]         # default: true
                    [--allow-write={true|false}]     # default: false
```

The default flags add up to a **safe no-op**: dry-run on,
allow-write off. Read the output, then re-invoke with
`--allow-write=true` once you've confirmed the target.

Real installs run through `raidhos-priv-helper`, not the CLI.

---

## `write-config`

```text
raidhos-cli write-config --mount-path <DIR> --config-path <FILE>
```

Write a previously-saved `boot.json` into the `raidhos/`
subdirectory of a mounted partition. Useful for editing the
boot menu without re-running the install pipeline.

---

## `catalog list`

```text
raidhos-cli catalog list
```

List bundled catalog entries:

```text
<slug>\t<name>
```

---

## `catalog verify`

```text
raidhos-cli catalog verify --slug <SLUG>
                           --iso  <FILE>
                           --sums <FILE>
                           --sig  <FILE>
                           [--key-dir <DIR>]    # default: catalog/keys
```

Verify a locally-downloaded ISO against the catalog:

1. Look up the entry by `--slug`.
2. Import the public key at `<key-dir>/<gpg_fingerprint>.asc`
   into an ephemeral GPG home.
3. `gpg --verify <sig> <sums>` — refuse if invalid.
4. Look up the ISO filename in `<sums>` — refuse if absent.
5. Compute SHA-256 of `<iso>` — refuse if it doesn't match.

Exit `0` on `ok`, `1` on any verification failure with the
specific reason on stderr.

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Runtime failure (validation refused, I/O error, command failed, verification mismatch) |
| `2` | Argument-parse error |

Stable across releases.

---

## Examples

See [`../../../examples/`](../../../examples/) for full shell
flows. Quick recipes:

```bash
raidhos-cli list-disks | column -t
raidhos-cli scan-isos --dirs /home,/Downloads
raidhos-cli install --device /dev/sdb --dry-run
raidhos-cli catalog list
raidhos-cli catalog verify --slug ubuntu-24.04-desktop-amd64 \
    --iso  ~/Downloads/ubuntu-24.04.3-desktop-amd64.iso \
    --sums ~/Downloads/SHA256SUMS \
    --sig  ~/Downloads/SHA256SUMS.gpg
```
