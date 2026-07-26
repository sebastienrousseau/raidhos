<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `boot.json` reference

`boot.json` is the persisted form of the RaidhOS `BootConfig`.
It is read by the Tauri UI and (when written to the DATA
partition under `/raidhos/boot.json`) by the install pipeline,
which feeds it through `raidhos-ui::grub::render_grub_cfg` to
produce the final `grub.cfg`.

Every value that flows into the renderer passes through a
metachar filter (`sanitize` / `sanitize_username` /
`is_grub_pbkdf2_hash`) so a hostile ISO filename, boot-entry
title, or class name cannot escape its quoted context. See
[`THREAT_MODEL.md`](THREAT_MODEL.md) for the attacker model.

The machine-readable schema is at
[`BOOT_CONFIG_SCHEMA.json`](BOOT_CONFIG_SCHEMA.json). This
document is the human walkthrough.

---

## Where it lives

| Host | Path |
|---|---|
| Linux | `~/.config/raidhos/boot.json` |
| macOS | `~/Library/Application Support/org.raidhos.raidhos/boot.json` |
| Windows | `%APPDATA%\raidhos\config\boot.json` |
| On the USB | `/raidhos/boot.json` on the DATA partition |

The on-device copy is what the install pipeline emits and what
matters for the boot. The user-config copy is what the UI edits
between installs.

---

## Top-level fields

```jsonc
{
  "default_entry": "/boot/isos/ubuntu-24.04.iso",
  "entries":       [ … ],

  "tree_view":           false,
  "enable_disk_browser": false,
  "grub_superuser":      "",
  "grub_password_pbkdf2": ""
}
```

| Field | Type | Default | Purpose |
|---|---|---|---|
| `default_entry` | string \| null | `null` | Path of the entry that should boot by default. Matched against `BootEntryConfig.path`. |
| `entries` | array | `[]` | Boot menu entries — see below. |
| `tree_view` | bool | `false` | **G20.** Group entries by `class` into `submenu` blocks. Classless entries float to top level. |
| `enable_disk_browser` | bool | `false` | **G22.** Append an F2-hotkeyed menuentry that chains into a `grub.cfg` found on any local disk. |
| `grub_superuser` | string | `""` | **G13.** Superuser name. Sanitised to `[A-Za-z0-9_-]+`. Must be paired with a valid PBKDF2 hash to activate. |
| `grub_password_pbkdf2` | string | `""` | **G13.** Output of `grub-mkpasswd-pbkdf2`. Plaintext passwords are rejected. |

---

## Per-entry fields (`entries[]`)

```jsonc
{
  "title":  "Ubuntu 24.04 LTS Desktop",
  "path":   "/boot/isos/ubuntu-24.04.iso",
  "params": "quiet splash",
  "initrd": "",
  "kargs":  "iso-scan/filename=$isofile",

  "class":               "linux",
  "tip":                 "Long-term-support desktop",
  "hidden":              false,
  "persistence_backend": "/persistence/ubuntu.dat"
}
```

| Field | Type | Default | Purpose |
|---|---|---|---|
| `title` | string | required | Menu label. Sanitised. |
| `path` | string | required | Path on DATA partition. Dispatched by extension — see below. |
| `params` | string | required | Free-form kernel params appended to the `linux` line. |
| `initrd` | string | required (may be empty) | Override the auto-detected initrd. Empty → fall back to `(loop)/casper/initrd` (or `live/initrd.img`). |
| `kargs` | string | required (may be empty) | Additional kernel cmdline. Use this for `preseed=/cdrom/preseed.cfg`, `inst.ks=...`, `autoinstall`, etc. |
| `class` | string | `""` | **G11.** Emitted as `--class TAG` on the menuentry. Also the submenu label under TreeView. |
| `tip` | string | `""` | **G11.** Comment placed above the entry. |
| `hidden` | bool | `false` | **G17.** Skip this entry entirely — applies in both ListView and TreeView. |
| `persistence_backend` | string | `""` | **G18.** Path of a `.dat` persistence file. Appends `persistent persistent-path=<value>` to the linux line. Only applies to ISO entries; ignored for `.efi` / `.img`. |
| `autoinstall` | object | `{kind: "none", path: ""}` | **G12.** Typed auto-install descriptor — see below. |
| `conf_replace_path` | string | `""` | **G16 (partial).** External grub.cfg override on the DATA partition. Renderer emits `configfile ($root)<path>` as the first branch on the ISO entry; falls back to the auto-detect logic when the file is missing. Does *not* do Ventoy's in-ISO sed substitution. |

### `autoinstall` sub-object

```jsonc
{
  "kind": "kickstart",
  "path": "/ks/centos.ks"
}
```

| `kind` value | Karg emitted | Distro family |
|---|---|---|
| `none` (default) | — | none |
| `kickstart` | `inst.ks=hd:LABEL=<DATA>:<path>` | Fedora / RHEL / Rocky / Alma / Leap |
| `preseed` | `auto=install preseed/file=<path>` | Debian, Ubuntu live-installer |
| `autoinstall` | `autoinstall ds=nocloud;s=<path>` | Ubuntu 24.04+ subiquity |
| `autoyast` | `autoyast=<path>` | openSUSE / SLE |
| `cloud-init` | `ds=nocloud;s=<path>` | generic NoCloud datasource |

The DATA partition label is substituted at render time. Both
`path` and the label go through the `sanitize` metachar filter,
so a hostile config can't escape its quoted context on the
linux line. Empty `path` or `kind: none` emits no kargs at all,
which is the back-compat-safe default.

### Path dispatch

The extension at the end of `path` (case-insensitive) selects the
boot strategy:

| Extension | Strategy | Closes |
|---|---|---|
| `.efi` | `chainloader "($root)<path>"` — direct UEFI binary load | G7 |
| `.img` / `.raw` | `loopback loop $imgfile` + `chainloader (loop)` | G6 |
| anything else (`.iso`, `.udf`) | `loopback loop $isofile` + kernel search (casper / live / configfile) | — |

The renderer applies these in order; `.efi` wins on collision.

---

## End-to-end example

A real-world configuration that exercises every feature added
across the v0.0.1 Ventoy-gap closures:

```json
{
  "default_entry": "/boot/isos/ubuntu-24.04.iso",
  "entries": [
    {
      "title":  "Ubuntu 24.04 LTS Desktop",
      "path":   "/boot/isos/ubuntu-24.04.iso",
      "params": "quiet splash",
      "initrd": "",
      "kargs":  "",
      "class":  "linux",
      "tip":    "Long-term-support desktop",
      "hidden": false,
      "persistence_backend": "/persistence/ubuntu.dat"
    },
    {
      "title":  "Fedora Workstation 41",
      "path":   "/boot/isos/fedora-41.iso",
      "params": "",
      "initrd": "",
      "kargs":  "",
      "class":  "linux",
      "tip":    "Wayland-first desktop",
      "hidden": false,
      "persistence_backend": ""
    },
    {
      "title":  "Windows 10 Setup",
      "path":   "/boot/isos/windows10.iso",
      "params": "",
      "initrd": "",
      "kargs":  "",
      "class":  "windows",
      "tip":    "Install Windows 10",
      "hidden": false,
      "persistence_backend": ""
    },
    {
      "title":  "Memtest86+",
      "path":   "/boot/efi/memtest86plus.efi",
      "params": "",
      "initrd": "",
      "kargs":  "",
      "class":  "",
      "tip":    "RAM check — direct EFI chainload",
      "hidden": false,
      "persistence_backend": ""
    },
    {
      "title":  "OpenWrt sysupgrade",
      "path":   "/boot/imgs/openwrt-x86_64.img",
      "params": "",
      "initrd": "",
      "kargs":  "",
      "class":  "",
      "tip":    "Raw disk image — loopback + chainload",
      "hidden": false,
      "persistence_backend": ""
    },
    {
      "title":  "[ARCHIVED] CentOS 7 net-install",
      "path":   "/boot/isos/centos-7.iso",
      "params": "",
      "initrd": "",
      "kargs":  "",
      "class":  "linux",
      "tip":    "Kept for compatibility — hidden",
      "hidden": true,
      "persistence_backend": ""
    }
  ],
  "tree_view": true,
  "enable_disk_browser": true,
  "grub_superuser": "admin",
  "grub_password_pbkdf2": "grub.pbkdf2.sha512.10000.deadbeef.cafef00d"
}
```

What you get in the rendered `grub.cfg`:

- The two `class: "linux"` entries (Ubuntu, Fedora) and the
  `class: "windows"` entry (Windows 10) are grouped into
  `submenu "linux" { … }` / `submenu "windows" { … }` blocks
  (G20).
- Memtest86+ and OpenWrt have no `class`, so they appear at
  the top level — one keypress from boot.
- Memtest is `chainloader`ed because of the `.efi` extension
  (G7); OpenWrt is loopback + chainload because of `.img`
  (G6); Ubuntu / Fedora / Windows go through the standard
  ISO 9660 loopback flow.
- Ubuntu's `persistent persistent-path=/persistence/ubuntu.dat`
  is appended to its `linux` line (G18).
- CentOS 7 doesn't appear in the rendered menu at all (G17).
- A boot-menu password gate is active because both
  `grub_superuser` and `grub_password_pbkdf2` are set with a
  valid PBKDF2 hash (G13). The hash is the literal output of
  `grub-mkpasswd-pbkdf2` — plaintext is refused.
- An F2-hotkeyed "Browse local disks" entry is appended at
  the bottom (G22).

---

## Generating a PBKDF2 hash

```bash
grub-mkpasswd-pbkdf2
# Enter password: …
# Reenter password: …
# PBKDF2 hash of your password is grub.pbkdf2.sha512.10000.<salt>.<hash>
```

Paste the `grub.pbkdf2.sha512.…` line into `grub_password_pbkdf2`.

---

## Backward compatibility

Every field added across the v0.0.1 gap closures carries
`#[serde(default)]`. An older `boot.json` that predates G11 /
G13 / G17 / G18 / G19 / G20 / G22 still loads — the missing
fields fill in with safe defaults (empty string, `false`).

Two backend tests pin this:

- `boot_config_round_trips_frontend_json_shape` — round-trips
  a fully-populated payload.
- `boot_config_legacy_payload_back_compat` — accepts a
  pre-v0.0.1 payload that omits every new field.

---

## See also

- [`BOOT_CONFIG_SCHEMA.json`](BOOT_CONFIG_SCHEMA.json) —
  machine-readable schema for IDE / `ajv` validation.
- [`VENTOY_COMPARISON.md`](VENTOY_COMPARISON.md) — what each
  field maps to in the Ventoy gap matrix.
- [`HARDENING.md`](HARDENING.md) — the `sanitize` /
  `sanitize_username` / `is_grub_pbkdf2_hash` defences.
- [`crates/ui-tauri/src-tauri/src/grub.rs`](../crates/ui-tauri/src-tauri/src/grub.rs)
  — the renderer source.
