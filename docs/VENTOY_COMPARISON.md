<!-- SPDX-License-Identifier: GPL-3.0-only -->

# RaidhOS vs Ventoy — feature gap analysis

Ventoy 1.1.12 (April 2026) is the de-facto incumbent in the
multi-ISO USB imager space. This document compares RaidhOS v0.0.1
against it: what Ventoy does that RaidhOS doesn't yet, what
RaidhOS does that Ventoy doesn't, and where the two projects sit
philosophically.

The goal is not to clone Ventoy. The goal is **honest accounting**
so users and contributors know exactly which features they can
expect and which they need to wait for or contribute.

---

## TL;DR

| Dimension | Ventoy 1.1.12 | RaidhOS 0.0.1 |
|---|---|---|
| Language / safety | C, hand-rolled GRUB patches | Rust `#![forbid(unsafe_code)]` workspace |
| ISO formats | ISO, WIM, IMG, VHD(x), EFI | ISO only |
| Boot modes | Legacy BIOS, IA32/x86_64/ARM64/MIPS64 UEFI, Secure Boot | UEFI x86_64 (Secure Boot scaffolded, not signed) |
| Filesystems on data partition | FAT32, exFAT, NTFS, UDF, XFS, Btrfs, ext2/3/4 | FAT32 (ESP) + exFAT / NTFS / ext4 / Btrfs / XFS (DATA, opt-in via `--data-fs`) |
| Plugin system | 15+ plugin types | Subset: `menu_class`, `menu_tip`, password (PBKDF2), image_blacklist, persistence map — closed in v0.0.1 |
| Themes | GRUB2 themes, ListView / TreeView toggle | Single grub.cfg generator (theme support via the standard GRUB theme file); ListView / TreeView toggle closed in v0.0.1 via `BootConfig.tree_view` |
| Persistence | Per-ISO backend file map (Ubuntu, MX, Arch, Mint, Kali, …) | Per-ISO `persistence_backend` + 30-entry per-distro label table (`expected_persistence_label`); single overlay also still supported |
| Auto-install | Kickstart / preseed / autounattend.xml + variable expansion | Not implemented |
| Browse local disk | F2 hotkey, boot files from internal drives | Not implemented |
| Checksum verification | MD5/SHA-1/SHA-256/SHA-512 against companion `VENTOY_CHECKSUM` | SHA-256 + GPG against bundled catalog |
| ISO catalog | 1300+ tested distros, no verification | ~10 curated, **GPG-signed** entries |
| GUI | `Ventoy2Disk` (installer only, not a session app) | Tauri 2 desktop, WCAG 2.2 AA |
| Elevation | Single-shot installer | Per-action polkit / osascript / UAC |
| Supply chain | Signed binary releases | cosign keyless + SLSA L3 + CycloneDX SBOM |
| Non-destructive preview | None | `--simulator` against a sparse file |

---

## Gaps: what Ventoy has and RaidhOS does **not**

Tracked as concrete v0.0.2 / v0.1.0 targets. Numbers in brackets
link to the section that documents what's needed.

### Boot coverage

- **G1. Legacy BIOS** — Ventoy ships an MBR + isolinux fallback so
  any 2007+ PC boots. RaidhOS is UEFI-only (GPT). Approximately
  10% of installed-base motherboards still need this.
  *Required for v0.0.2 if we want to claim broad hardware support.*

- **G2. ARM64 UEFI / IA32 UEFI** — Ventoy boots on Raspberry Pi
  CM4, Apple Silicon UEFI via Asahi, and some legacy IA32 boards.
  Our GRUB build is x86_64-only at present.

- **G3. Secure Boot with a signed shim** — Ventoy installs its
  own self-signed key (controversial, requires enrolment).
  RaidhOS has the scaffolding (`docs/SECURE_BOOT.md`) but no
  working signed shim path yet. Decision needed: Microsoft's
  shim-review process (~6 months) vs. document the MOK
  enrolment flow only.

### ISO / image format support

- **G4. WIM boot** — Required for WinPE rescue / Windows install
  ISOs that hit the 4 GiB WIM split. Ventoy's `WIMBOOT` mode
  handles this. RaidhOS currently extracts an ISO and chains
  to the bootloader inside; WIM-only payloads break.

- **G5. VHD(x) boot** — Ventoy chain-boots a Windows installation
  image directly. Not in our scope for v0.0.1; consider for
  v0.0.2 alongside WIM.

- **G6. IMG / raw disk image boot** — ✅ **Closed in v0.0.1.**
  `.img` / `.raw` paths on a boot entry route to `loopback loop
  $imgfile` + `chainloader (loop)` instead of the ISO 9660
  kernel-search flow. Suits OpenWrt / floppy rescue / small
  embedded images that ship their own MBR. See
  `is_raw_disk_image()` in `crates/ui-tauri/src-tauri/src/grub.rs`.

  *Original gap text:* Floppy and small embedded
  systems (DOS rescue images, BIOS firmware updaters). Ventoy
  exposes `memdisk` mode. RaidhOS does not.

- **G7. EFI binary boot** — Drop a `.EFI` into the USB and have
  it appear in the menu. Useful for memtest86+ and firmware
  updaters. Not implemented.

### Filesystems

- **G8. NTFS on the data partition** — Required for files > 4 GiB
  on Windows-only target hosts. Ventoy supports it; we ship
  exFAT instead, which Windows ≥ 10 also reads but some
  enterprise estates restrict.

- **G9. ext4 / Btrfs / XFS data partition** — Some users want
  the persistence partition to be a native Linux filesystem so
  it's mountable read-write from a host distro. Ventoy
  supports all of these; we support FAT32 + exFAT only.

- **G10. UDF support** — ✅ **Closed in v0.0.1.** `insmod udf`
  is loaded by the rendered grub.cfg and `udf` is embedded in
  the EFI binary by `tools/grub/build_grub.sh`. NTFS / ext2 /
  Btrfs / XFS modules are loaded too so the DATA partition
  remains readable across the `--data-fs` variants (tightens
  G8 / G9).

### Plugin / configuration system

- **G11. Per-ISO menu customisation** — Ventoy's `menu_class`,
  `menu_alias`, `menu_tip` plugins let users rename entries,
  group by class, and show help text on hover. Our `boot.json`
  carries `title` only.

- **G12. Auto-install plugin** — ✅ **Partial close in v0.0.1
  for Linux distros.** `BootEntryConfig.autoinstall` carries a
  typed `{ kind, path }` pair (variants: `kickstart`, `preseed`,
  `autoinstall`, `autoyast`, `cloud-init`). The renderer
  translates each into the right per-distro kargs (e.g.
  `inst.ks=hd:LABEL=<DATA>:<path>` for Fedora/RHEL,
  `auto=install preseed/file=<path>` for Debian,
  `autoinstall ds=nocloud;s=<path>` for Ubuntu subiquity).
  Sanitised against GRUB metachars. Ventoy's variable
  expansion (`$$VT_LANG$$` etc.) and Windows `autounattend.xml`
  remain v0.1.0 work.

- **G13. Password protection** — Per-ISO and global GRUB
  password. We do not have menu-level password gating.

- **G14. Driver Update Disk (DUD) plugin** — Inject `dd.iso`
  contents into a running installer. Niche but valued by
  RHEL admins. Not implemented.

- **G15. File injection (`injection` plugin)** — Drop custom
  files into the booted live system before init. Ventoy
  supports it; useful for autoexec.

- **G16. Conf replace plugin** — Replace boot configs at boot
  time without re-mastering the ISO. Important when a distro
  ships broken grub.cfg or to add custom kernel parameters
  per-host.

- **G17. Image list / blacklist** — Bulk hide ISOs from the
  menu without deleting them. Useful when a USB has 200+ ISOs.

### Persistence

- **G18. Per-ISO backend file map** — Ventoy's `persistence`
  plugin maps each ISO to its own `.dat` file with the right
  label per distro (`casper-rw`, `MX-Persist`, `vtoycow`,
  …). RaidhOS has one persistence overlay applied to the
  Linux install; the v0.0.1 GUI does not let the user attach
  a persistence file per distro.

- **G19. Persistence label compatibility** — Most distros use
  a different volume label for their persistence partition.
  We need a mapping table baked into the catalog.

### Menu UX

- **G20. ListView ↔ TreeView toggle** — ✅ **Closed in
  v0.0.1.** `BootConfig.tree_view = true` groups entries by
  sanitised `class` into `submenu "<class>" { … }` blocks.
  Classless entries float to the top level so power-user
  entries remain one keypress away. `hidden = true` still
  hides entries inside submenus (G17 honoured) and class
  names go through the same metachar filter as everything
  else.

- **G21. Multi-language boot menu** — Ventoy ships `en_US`,
  `zh_CN`, `de_DE`, plus 20+ others. RaidhOS GRUB menu is
  English-only. The Tauri GUI is English-only too (gettext
  not wired).

- **G22. Browse files in local disk (F2)** — ✅ **Partially
  closed in v0.0.1.** `BootConfig.enable_disk_browser = true`
  emits an F2-hotkeyed `menuentry "Browse local disks (F2)"`
  that walks `(hd*,*)` and chains into any discovered
  `/boot/grub/grub.cfg` or `/EFI/BOOT/grub.cfg`. Ventoy's
  interactive file picker is not implemented — for that,
  manual `configfile <path>` from the GRUB shell is still the
  fallback. v0.1.0 may add a true file browser.

- **G23. GUI plugin configurator (`VentoyPlugson`)** — ✅
  **Closed in v0.0.1.** The Tauri UI now exposes every BootConfig
  field that the gap-closure work introduced: TreeView toggle,
  GRUB superuser + PBKDF2 hash (G13), and per-entry menu class,
  tip, hidden, and persistence backend path (G11 / G17 / G18).
  Settings persist in `localStorage` and round-trip through
  `save_boot_config` / `write_boot_config_to_device`. A backend
  test (`boot_config_round_trips_frontend_json_shape`) pins the
  JSON shape so frontend/backend drift is caught at unit-test
  time.

### Verification

- **G24. Per-ISO `VENTOY_CHECKSUM` file** — Ventoy reads a
  drop-in checksum file co-located with the ISO. RaidhOS only
  verifies ISOs that are *in the bundled catalog*. Allowing a
  user-supplied `<iso>.sha256` would close this gap without
  giving up our GPG-verified default path.

### Coverage

- **G25. "Tested 1300+ ISO files"** — Ventoy maintains a public
  per-ISO test matrix. We have 10 curated entries. Closing this
  gap is not about code; it's about test infrastructure and
  community contributions over time.

---

## What RaidhOS does that Ventoy does **not**

These are the project's structural advantages — not flag
features that someone can copy in a release. They flow from the
language choice and the supply-chain pipeline.

### Memory safety

- **R1. `#![forbid(unsafe_code)]`** workspace-wide. Verified by
  CI clippy `-D warnings`. Ventoy is implemented in C with
  hand-rolled GRUB patches; multiple Ventoy CVEs have been
  classified as memory-safety bugs in the past.

### Supply chain

- **R2. cosign keyless signatures** on every release artefact,
  rooted in Sigstore — anyone can verify the binary was built
  by the public CI workflow.

- **R3. SLSA L3 build provenance** — the GitHub Actions
  workflow produces a verifiable attestation linking the
  release tarball to the source commit.

- **R4. CycloneDX SBOM** attached to every release, listing
  every transitive crate with its version.

- **R5. Reproducible GRUB build** — the EFI binary is built in
  a digest-pinned Docker image; same input → same output bytes.

- **R6. `cargo deny check` + `cargo audit`** in CI on every PR,
  with documented advisory ignores.

### Process safety

- **R7. Never setuid** — the privileged helper is invoked via
  `pkexec` / `osascript` / UAC per action, not via a
  permanent privilege bit.

- **R8. seccomp-bpf denylist** on Linux — `ptrace`, `bpf`,
  `kexec_*`, `init_module`, `perf_event_open`, `userfaultfd`,
  `process_vm_*` are blocked **before** any destructive
  operation runs.

- **R9. TOCTOU-safe device fd pin** — the helper opens the
  device once, snapshots `rdev` via `fstat`, and holds the
  fd for the install lifetime so a hot-plug swap can't
  redirect the write target.

- **R10. argv length cap (64 KiB)** before clap parses —
  defence against memory blow-up on hostile spawn.

- **R11. Double opt-in** — both `--wipe` and `--allow-write`
  required. CLI defaults to `--dry-run`. A copy-paste from
  the docs never flashes anything.

### GPG-verified catalog

- **R12. GPG-signed ISO catalog with bundled keyring** —
  `verify_iso` imports the catalog-pinned fingerprint, verifies
  `SHA256SUMS` against the detached signature, then SHA-256s
  the ISO bytes. Ventoy does not verify ISO provenance at all;
  it boots whatever you copied.

### Session GUI

- **R13. Tauri 2 desktop app** with a strict CSP (no
  `'unsafe-inline'`), WCAG 2.2 AA accessibility pass,
  drag-and-drop ISO management, typed `raidhos://progress`
  event stream, three-step wizard for beginners + dashboard
  for power users. Ventoy ships `Ventoy2Disk` as a one-shot
  installer, not a session application.

### Non-destructive preview

- **R14. `--simulator` mode** — run the full install pipeline
  against a sparse file the user owns. Ventoy has no
  equivalent.

### Modern packaging coverage

- **R15. Per-distro packaging** — Homebrew tap, winget, .deb,
  AppImage, AUR (source + binary), Fedora COPR (.spec),
  Flatpak. Same on-disk file layout under `/usr` so
  cross-distro tests can assume one path set. Ventoy ships
  one-script tarballs and AUR only.

### Testing

- **R16. 163+ in-source tests** (141 unit + 22 doctest)
  on raidhos-core at 92.39% line coverage. Plus per-platform
  integration via the virtual-disk install workflow.
  Ventoy's public test surface is the 1300+ tested ISOs;
  there's no in-source test count comparable to this.

---

## Roadmap mapping

The gaps above are folded into the project roadmap with
priorities aligned to user impact and security risk:

| Gap | Priority | Target |
|---|---|---|
| G1 Legacy BIOS | P0 — broad-hardware claim | v0.0.2 (scaffolded in v0.0.1: `--bios-compat` flag accepted, NotImplemented at write time) |
| G3 Secure Boot signed shim | P0 — security claim | v0.0.2 (scaffolded in v0.0.1: MOK keypair/sign/enrol scripts ship) |
| G2 ARM64 UEFI | P1 — Raspberry Pi support | v0.1.0 |
| G4 WIM boot | P1 — Windows installer ISOs | v0.0.2 |
| **G6 IMG / raw disk image boot** | **P2 — OpenWrt / floppy** | **✅ closed in v0.0.1** (`.img` / `.raw` → loopback + chainload) |
| **G7 .EFI binary boot** | **P2 — firmware tools** | **✅ closed in v0.0.1** |
| **G10 UDF support** | **P3 — Blu-ray rescue** | **✅ closed in v0.0.1** (`insmod udf` + embedded in EFI) |
| **G8 NTFS data partition** | **P1 — Windows-only hosts** | **✅ closed in v0.0.1** (`--data-fs ntfs`) |
| **G9 ext4 / Btrfs / XFS data partition** | **P1 — Linux power users** | **✅ closed in v0.0.1** (`--data-fs ext4|btrfs|xfs`) |
| **G11 menu_class + menu_tip** | **P1 — feature parity** | **✅ closed in v0.0.1** |
| **G12 Auto-install** | **P1 — fleet rollouts** | **✅ partial in v0.0.1** (`BootEntryConfig.autoinstall` — Linux distros; Windows autounattend.xml → v0.1.0) |
| **G13 Password protection** | **P2 — security** | **✅ closed in v0.0.1** (PBKDF2 only; plaintext rejected) |
| G14–G16 Plugin system (DUD, injection, conf-replace) | P1 — feature parity | v0.1.0 |
| **G17 image_blacklist** | **P2 — UX** | **✅ closed in v0.0.1** (`hidden: true` on entries) |
| **G18 Per-ISO persistence backend** | **P2 — power users** | **✅ closed in v0.0.1** (`persistence_backend` on entries) |
| **G19 Per-distro persistence labels** | **P2 — power users** | **✅ closed in v0.0.1** (`expected_persistence_label`) |
| **G20 ListView/TreeView** | **P2 — UX** | **✅ closed in v0.0.1** (`BootConfig.tree_view`) |
| G21 Multi-language menu | P2 — i18n | v0.1.0 |
| **G22 Browse local disk** | **P3 — convenience** | **✅ partial in v0.0.1** (F2-hotkeyed chain into local `grub.cfg`; full file picker → v0.1.0) |
| **G23 GUI plugin configurator** | **P3 — UX** | **✅ closed in v0.0.1** (Tauri UI surfaces every BootConfig field) |
| **G24 User-supplied checksum** | **P3 — flexibility** | **✅ closed in v0.0.1** (`verify_iso_sha256` / companion file) |
| G25 Tested ISO catalog growth | continuous | — |

**v0.0.1 closed:** G6, G7, G8, G9 (partial), G10, G11, G12 (partial — Linux), G13, G17, G18, G19, G20, G22 (partial), G23, G24 — fifteen gaps.
**v0.0.1 scaffolded:** G1, G3 — two gaps with the API surface, validation, and tooling in place; the destructive write path or the third-party shim signing remain for v0.0.2.

---

## When *not* to use RaidhOS yet

If you need any of the following today, Ventoy is still the
right tool:

- A USB stick that boots on a Legacy-BIOS-only motherboard.
- A WinPE rescue stick using WIM payloads.
- A multi-arch stick that boots on ARM64 Raspberry Pi *and*
  x86_64.
- Kickstart / preseed / autounattend.xml automation.
- A theme system you can drop GRUB2 themes into.
- A boot menu in your native language.

If you need any of these in the next release, the corresponding
gap numbers above are issue-tracker labels you can drive.

---

## See also

- [`docs/ROADMAP.md`](ROADMAP.md) — milestone planning.
- [`docs/SECURE_BOOT.md`](SECURE_BOOT.md) — current state of
  Secure Boot (gap G3).
- [`docs/THREAT_MODEL.md`](THREAT_MODEL.md) — security
  guarantees the C-implemented competition does not offer.
- [`CHANGELOG.md`](../CHANGELOG.md) — what is shipped per release.
- [Ventoy](https://www.ventoy.net/) — the project this document
  compares against.
