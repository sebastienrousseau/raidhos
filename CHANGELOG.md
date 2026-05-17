# Changelog

All notable changes to RaidhOS are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Ventoy gap closures

Twelve of the 25 gaps tracked in
[`docs/VENTOY_COMPARISON.md`](docs/VENTOY_COMPARISON.md) are
closed in v0.0.1; two are scaffolded with API surface +
tooling, with the destructive paths deferred to v0.0.2.

| Gap | Status | Surface |
|---|---|---|
| G6 IMG / raw disk image boot | closed | grub.cfg renderer detects `.img` / `.raw` and emits `loopback` + `chainloader (loop)` |
| G7 .EFI binary chainload | closed | grub.cfg renderer detects `.efi` and emits `chainloader` |
| G8 NTFS data partition | closed | `--data-fs ntfs` (Linux pipeline) |
| G9 ext4 / Btrfs / XFS data partition | closed | `--data-fs ext4 \| btrfs \| xfs` (Linux pipeline) |
| G10 UDF support | closed | `insmod udf` emitted by grub.cfg + `udf` embedded in EFI |
| G11 menu_class + menu_tip per ISO | closed | `BootEntryConfig.class` + `tip` |
| G13 GRUB password protection | closed | `BootConfig.grub_superuser` + `grub_password_pbkdf2` (PBKDF2 only) |
| G17 image_blacklist | closed | `BootEntryConfig.hidden=true` |
| G18 per-ISO persistence backend | closed | `BootEntryConfig.persistence_backend` |
| G19 per-distro persistence labels | closed | `raidhos_core::expected_persistence_label` (30-distro table) |
| G20 ListView ↔ TreeView toggle | closed | `BootConfig.tree_view`; renderer groups by sanitised `class` into `submenu` blocks |
| G24 user-supplied checksum | closed | `verify_iso_sha256` / `verify_iso_companion_sha256` |
| G1 Legacy BIOS layout | scaffolded | `--bios-compat` flag; destructive path returns `NotImplemented` |
| G3 Secure Boot signed shim | scaffolded | `tools/secure-boot/{generate-mok,sign-bootx64}.sh` + enrolment helper |

### Changed

- `raidhos-core` line coverage jumped from ~68% to **91.21%**
  (778/853 lines covered) after the
  `validate_device_path_for_target` refactor. The macOS, Windows,
  *and* Linux `install_with` test modules no longer need a matching
  `target_os` to run; the path-shape gate is decoupled from the
  host's compile-time `target_os`. CI coverage gate raised from
  65% to 90%.
- Test count: `raidhos-core` 201 unit + 26 doctest, `raidhos-ui`
  61, `raidhos-cli` 14, `raidhos-priv-helper` 9 seccomp + 6 toctou
  + 7 integration. Total: 324+ tests across the workspace.

### Added

- `validate_device_path_for_target(device, target)` —
  target-string-driven path-shape validator. Accepts `"linux"`,
  `"macos"`, `"windows"`, or `"simulator"`. Public-but-crate-internal;
  the public `validate_device_path` / `validate_device_path_simulator`
  still delegate through it.
- `verify_iso_sha256(iso, hex)` and
  `verify_iso_companion_sha256(iso)` — public functions in
  `raidhos-core` for verifying ISOs against a user-supplied
  SHA-256 hex string or a co-located `<iso>.sha256` companion
  file (SHA256SUMS-style or bare-hex). Closes Ventoy gap G24.
- `bios_compat: bool` field on `InstallRequest` and the
  `--bios-compat` flag on both `raidhos-cli install` and
  `raidhos-priv-helper install`. The Linux platform pipeline
  accepts the request shape and emits the dry-run completion
  event; the actual `grub-install --target=i386-pc` embedding
  is deferred to v0.0.2 (scaffolds Ventoy gap G1).
- `DataFilesystem` enum + `data_filesystem` field on
  `InstallRequest`, plus `--data-fs <FS>` flag on the CLI and
  the privileged helper. Variants: `exfat` (default), `ntfs`,
  `ext4`, `btrfs`, `xfs`. Linux pipeline branches on the variant
  with per-filesystem `mkfs.*` formatters, each refusing cleanly
  with a per-package install hint (`ntfs-3g`, `e2fsprogs`,
  `btrfs-progs`, `xfsprogs`) when the binary is missing. Closes
  Ventoy gap G8 and the lower half of G9.
- Per-ISO menu metadata on `BootEntryConfig`:
  - `class: String` → `--class TAG` on the rendered menuentry,
    sanitised against GRUB injection. Closes Ventoy gap G11.
  - `tip: String` → GRUB comment above the menuentry. G11.
  - `hidden: bool` → renderer skips the entry entirely. Closes
    Ventoy gap G17.
  - `persistence_backend: String` → appends `persistent
    persistent-path=<value>` to the live-kernel command line on
    casper + live boot flows. Closes Ventoy gap G18.
- `BootConfig.tree_view: bool` → when set, the grub.cfg renderer
  groups entries by their sanitised `class` into `submenu
  "<class>" { … }` blocks. Classless entries float to the top
  level so power-user kernels stay one keypress from boot;
  `hidden = true` still suppresses entries inside submenus, and
  class names go through the same metachar filter as everything
  else. Closes Ventoy gap G20.
- Raw disk image (`.img` / `.raw`) boot: the grub.cfg renderer
  detects these extensions at the end of an entry's `path` and
  emits `loopback loop $imgfile` followed by `chainloader
  (loop)` instead of the ISO 9660 kernel-search flow. Suits
  OpenWrt, floppy rescue images, and small embedded boot
  images that ship their own MBR. Closes Ventoy gap G6.
- `insmod udf` / `ntfs` / `ext2` / `btrfs` / `xfs` emitted by
  the grub.cfg renderer, and the same module list is embedded
  in the EFI binary via `tools/grub/build_grub.sh`. Closes
  Ventoy gap G10 (Blu-ray rescue ISOs) and tightens G8 / G9
  by keeping the DATA partition readable at boot when the user
  picks a non-default `--data-fs`.
- GRUB password protection: `grub_superuser` + `grub_password_pbkdf2`
  on `BootConfig`. PBKDF2-only — plaintext passwords are refused
  by `is_grub_pbkdf2_hash()`. The username is sanitised to
  `[A-Za-z0-9_-]+`; the hash flows through unmodified (sanitising
  would corrupt the PBKDF2 output). Closes Ventoy gap G13.
- `.EFI` direct chainload: the grub.cfg renderer detects
  `.efi` (case-insensitive) at the end of an entry's `path`
  and emits a single `chainloader "($root)<path>"` line instead
  of the loopback-ISO shim. Useful for memtest86+, firmware
  updaters, and other raw UEFI payloads. Closes Ventoy gap G7.
- `raidhos_core::expected_persistence_label(slug)` — maps a
  catalog slug to the volume label that distro's initramfs
  hunts for at boot (`casper-rw`, `MX-Persist`, `vtoycow`,
  `kali-persistence`, `writable`, `live-rw`, etc.). 30 distro
  entries across Ubuntu/Debian/MX/Arch/Kali/Fedora/CloneZilla/
  Kaspersky/Tails. Closes Ventoy gap G19.
- `--simulator <FILE>` + `--simulator-size-mb N` on
  `raidhos-cli install` — non-destructive preview against a
  sparse file. Targets the same install pipeline as a real USB.
- AUR (`PKGBUILD` + `PKGBUILD.bin`), Fedora COPR
  (`raidhos.spec`), and Flatpak
  (`io.github.sebastienrousseau.raidhos.yml` + .desktop +
  metainfo). Covers CachyOS / Arch / Manjaro / EndeavourOS /
  Garuda / Fedora / RHEL / Rocky / Alma / openSUSE / Silverblue
  / Bazzite / SteamOS / NixOS.
- Secure Boot signing tooling: `tools/secure-boot/generate-mok.sh`
  (RSA-2048 keypair + X.509 cert), `tools/secure-boot/sign-bootx64.sh`
  (sbsign), and the rewritten `scripts/raidhos-mok-enroll.sh` with
  auto-discovery of the `.cer` file. Scaffolds Ventoy gap G3.
- `docs/VENTOY_COMPARISON.md` — honest feature gap analysis
  against Ventoy 1.1.12 with 25 gaps (G1–G25) tagged with
  v0.0.2 / v0.1.0 / v0.2.0 priorities and 16 RaidhOS
  advantages (R1–R16).
- 25 new `raidhos-core` unit tests (parsers, runtime, lib.rs
  validators, catalog edge cases) bringing the total to 179.
- 36 Mermaid diagrams across `docs/` and per-crate doc folders
  validated locally via `mmdc`; three syntax-error fixes (edge
  labels, `@` in node text, embedded quotes in stadium nodes).

### Fixed

- `docs/ARCHITECTURE.md` sequence diagram: dropped `<br/>`
  from participant aliases and rephrased the `Vec<DiskInfo>`
  message text — Mermaid's sequenceDiagram parser doesn't
  accept either.
- `docs/RELEASE.md` flowchart: quoted node label
  `[attest-build-provenance@v1]` so the `@` isn't read as a
  link-id separator.
- `docs/USER_GUIDE.md` flowchart: quoted the whole stadium
  node `(["Click List Disks"])` instead of mixing quotes
  inside parens.
- `docs/SECURE_BOOT.md`: backslashes in shown file paths
  replaced with forward slashes (irrelevant glyph for a
  conceptual diagram).

## [0.0.1] — 2026-05-17 (release candidate)

First public preview. The discovery surface, validation surface,
and install pipeline are all working end-to-end on Linux; macOS
and Windows have the same code path with platform-specific
subprocesses, validated by the virtual-disk install workflow.

### Added — workspace

- Three production crates plus the UI:
  `raidhos-core` (library), `raidhos-cli` (CLI binary),
  `raidhos-priv-helper` (the only binary intended to run
  elevated), `raidhos-ui` (Tauri 2 desktop app).
- Platform abstraction in `crates/core/src/platform/`
  (Linux, macOS, Windows, unsupported fallback) gated by
  `#[cfg(target_os = "…")]`.
- `Runtime` trait abstraction over `std::process::Command`,
  `std::env`, and `std::fs`. `RealRuntime` ships in production;
  `MockRuntime` makes every install path unit-testable on every
  host.
- `directories` crate for cross-platform user-config locations
  (`~/.config`, `~/Library/Application Support`, `%APPDATA%`).
- `thiserror`-based `CoreError` and `CatalogError` with
  `Display`-stable strings the CLI / UI / helper can match on.

### Added — install pipeline

- **Linux** install pipeline: `parted` GPT,
  `mkfs.vfat` ESP, `mkfs.exfat` (or `mkexfatfs`) DATA, payload
  copy, persistence image (`--persistence-mb`).
- **macOS** install pipeline: `diskutil partitionDisk` + mount +
  payload copy + `bless` for boot loader entry.
- **Windows** install pipeline: PowerShell `Clear-Disk` /
  `Initialize-Disk` / `New-Partition` / `Format-Volume`,
  `robocopy` for the payload tree.

### Added — safety / hardening

- `validate_device_path()` per-OS shape check, plus
  shell-metachar / `..` / length / empty rejection.
- Double opt-in for destructive installs: both `wipe == true`
  and `allow_write == true` are required.
- System / mounted / removable-false disk refusal in every
  platform's `validate_install()`.
- Payload SHA-256 walk against `manifest.json` before write.
- seccomp-bpf denylist on Linux (ptrace / bpf / kexec /
  init_module / perf_event_open / userfaultfd / process_vm_*).
- TOCTOU-safe device fd open: open the device once, snapshot
  `rdev` via `fstat`, hold the fd for the install lifetime.
- 64 KiB argv-length cap in the priv-helper before clap parses.
- `pkexec` (Linux), `osascript with administrator privileges`
  (macOS), and `Start-Process -Verb RunAs` (Windows) elevation
  wrappers — the helper is never setuid.
- polkit policy
  (`packaging/polkit/org.raidhos.priv.policy`) shipped with
  Debian/AppImage packages.

### Added — ISO catalog

- `catalog/catalog.json` curated distro list with per-entry
  GPG fingerprint pin and SHA256SUMS layout.
- `catalog/keys/` bundled keyring (offline GPG verification).
- `verify_iso` / `verify_iso_with(rt)` GPG-then-SHA256
  verification flow, fully testable via `MockRuntime`.

### Added — UI

- Tauri 2 migration from Tauri 1 (capabilities model, strict
  CSP, no `'unsafe-inline'`).
- WCAG 2.2 AA accessibility pass: skip-to-main, focus ring,
  `prefers-color-scheme` / `prefers-contrast` /
  `prefers-reduced-motion` media-query support, keyboard-only
  flow, drag-and-drop ISO management with a click-fallback.
- `raidhos://progress` typed event stream replacing the prior
  `Mutex<Vec<…>>` polling loop.
- Three-step beginner wizard alongside the power-user
  dashboard.
- `frontend/`: zero npm, zero bundler. Plain HTML + plain JS
  served by Tauri.

### Added — supply chain / CI

- `cargo deny check` (advisories + licenses + bans + sources)
  gating every PR, with `[advisories].ignore` for unmaintained
  transitives we don't control.
- `cargo audit --deny unsound` with mirrored
  `--ignore` flags.
- CodeQL (Rust + Actions) + OpenSSF Scorecards.
- cosign keyless signing + SLSA L3 provenance + CycloneDX SBOM
  on every release.
- Reproducible Docker GRUB build pipeline (digest-pinned
  base image + retry-with-mirror fallback).
- Virtual-disk install workflow exercising the Linux loop
  device, macOS `hdiutil` sparse image, and Windows VHD code
  paths end-to-end.
- Fuzz harness (`fuzz/`) targeting `parse_lsblk_disks`,
  `parse_disks_plist`, `parse_get_disk_json`, and
  `sha256sums_lookup`.
- Tarpaulin coverage gate.

### Added — packaging

- Homebrew tap formula.
- winget manifest.
- Debian `.deb` build script + polkit install.
- AppImage build script.
- macOS `.dmg` build script.
- Windows `.msi` build script.

### Added — docs

- Noyalib-style per-crate READMEs with badges, contents TOC,
  install / quick-start / reference / operational sections.
- `crates/*/doc/` folders: USAGE, ARCHITECTURE, SECURITY,
  HARDENING, COMMANDS, ACCESSIBILITY (one per crate as
  appropriate).
- `crates/*/examples/` runnable examples (Rust `.rs` for
  raidhos-core, shell scripts for cli / priv-helper, HTML/JS
  pages for ui-tauri).
- `docs/USER_GUIDE.md`, `docs/TROUBLESHOOTING.md`,
  `docs/FAQ.md`, `docs/THREAT_MODEL.md`, `docs/HARDENING.md`,
  `docs/SECURE_BOOT.md`, `docs/V0_0_1_STATUS.md`,
  `docs/ARCHITECTURE.md`, `docs/PAYLOAD.md`, `docs/CATALOG.md`,
  `docs/TAURI_NOTES.md`, `docs/DESIGN_NOTES.md`.
- `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1).
- `CONTRIBUTING.md` with per-OS dev workflows.
- `SECURITY.md` reporting policy.
- `.github/dependabot.yml`, `.github/ISSUE_TEMPLATE/`,
  `.github/PULL_REQUEST_TEMPLATE.md`.
- Workspace-wide Mermaid diagrams using the neutral theme.

### Added — tests

- 121+ `raidhos-core` unit tests covering every public function
  and every error variant.
- 22+ rustdoc doctest examples on every public item.
- 9 `raidhos-priv-helper` seccomp filter unit tests.
- 6 `raidhos-priv-helper` toctou unit tests.
- 7 `raidhos-priv-helper` integration tests (spawn the real
  binary).
- 12 `raidhos-cli` clap-schema unit tests.
- 20 `raidhos-ui` unit tests (13 grub + 7 main.rs).
- Fuzz parser targets activated via `__fuzz_api` re-exports.
- Virtual-disk install workflow on Linux / macOS / Windows
  hosted runners.

### Changed

- `crates/core/src/lib.rs` slimmed to the public-API surface;
  platform-specific code moved to `platform/{linux,macos,windows}.rs`.
- README rewritten to noyalib visual style (centred header,
  badges, contents TOC, quick start, reference sections).
- Coverage gate set to a realistic 65% floor; tracking 85% by
  v0.0.2 and 95% by v0.1.0.
- Mermaid diagrams: `%%{init: {'theme':'neutral'}}%%` everywhere
  to drop yellow / improve print contrast.

### Fixed

- `validate_device_path`: removed `\\` from FORBIDDEN so
  legitimate Windows `\\.\PhysicalDriveN` paths pass.
- Coverage tooling: re-gated platform modules so tarpaulin on
  Linux sees only the active backend.
- `cargo-audit`: passes the same `--ignore` set as `cargo-deny`
  so the audit workflow doesn't trip on unmaintained transitives.
- GRUB Docker build: switched from `mawk` to `gawk` for
  `asorti`, with a 3× retry + GitHub-mirror fallback on
  savannah's transient outages.

[Unreleased]: https://github.com/sebastienrousseau/raidhos/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/sebastienrousseau/raidhos/releases/tag/v0.0.1
