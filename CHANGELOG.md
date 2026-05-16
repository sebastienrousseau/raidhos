# Changelog

All notable changes to RaidhOS are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `docs/USER_GUIDE.md` — end-user walkthrough for first install.
- `docs/TROUBLESHOOTING.md` — common failures (pkexec, missing payload,
  Secure Boot) and how to recover.
- `docs/FAQ.md` — short answers to the questions that surface in issues.
- `docs/THREAT_MODEL.md` — explicit attacker model and which controls
  address each capability.
- `docs/HARDENING.md` — every shipping security control, with `file:line`
  citations into the source tree.
- `docs/SECURE_BOOT.md` — current state, why Secure Boot fails today, and
  the path to signed boot.
- `docs/V0_0_1_STATUS.md` — honest tracker of what is and is not in this
  release.
- `CODE_OF_CONDUCT.md` — Contributor Covenant 2.1.
- `CHANGELOG.md` — this file.
- Platform abstraction (`crates/core/src/platform/`) splitting Linux,
  macOS, and Windows backends behind a single trait.
- `thiserror`-based `CoreError` with `#[source]` preservation, replacing
  the hand-rolled enum. Wire format (JSON) is unchanged.
- `directories` crate for cross-platform user config (Linux `~/.config`,
  macOS `~/Library/Application Support`, Windows `%APPDATA%`).
- Payload integrity verification: `payload/manifest.json` carries a
  SHA-256 over the payload tree and is checked before any write.
- `clap`-based parsing in `raidhos-priv-helper` with
  `deny_unknown_fields`-equivalent strictness and a 64 KiB input cap.
- `clap_mangen` + `clap_complete` build artifacts: man pages and
  bash/zsh/fish/PowerShell completions.
- Fuzz scaffold (`fuzz/`) with targets for `validate_device_path`, the
  `lsblk`/`diskutil`/`Get-Disk` parsers, and the GRUB config renderer.
- `.github/workflows/release.yml` — multi-platform builds on tag push,
  cosign keyless signing, SLSA provenance, GitHub Releases upload.
- `.github/workflows/codeql.yml` — Rust + Actions static analysis.
- CycloneDX SBOM generation on every release.
- Strict CSP in `tauri.conf.json` — `'unsafe-inline'` removed from
  `style-src`; inline styles extracted to `frontend/styles.css`.
- `#![warn(missing_docs)]` on `raidhos-core` with rustdoc on every
  public item.

### Changed
- `crates/core/src/lib.rs` reduced to public API surface; platform code
  moved to `platform/{linux,macos,windows}.rs`.
- README rewritten with hero copy, screenshot placeholder, badges, and
  install paths (cargo + future signed binaries).

### Not yet shipped (tracked for next milestones)
- macOS install pipeline — discovery works; the destructive path is
  stubbed (`NotImplemented`).
- Windows install pipeline — discovery works; the destructive path is
  stubbed (`NotImplemented`).
- Tauri 2 migration.
- Secure Boot (signed shim + MOK enrolment).
- BIOS / legacy boot path.
- Persistence images.
- Curated ISO catalog with GPG signature verification.
- Drag-and-drop ISO management in the UI.
- seccomp-bpf restriction of `raidhos-priv-helper`.
- Frontend rewrite to a real framework (Svelte/Solid) with strict-CSP
  bundling.

See `docs/V0_0_1_STATUS.md` for the detailed breakdown.

[Unreleased]: https://github.com/sebastienrousseau/raidhos/compare/main...HEAD
