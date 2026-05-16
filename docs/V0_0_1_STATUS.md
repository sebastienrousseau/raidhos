# v0.0.1 status report

What is in `feat/v0.0.1`, what is not, and what we deliberately deferred.
This document is the source of truth — issue trackers and the
`CHANGELOG.md` summarise from here.

Audience: anyone reviewing the branch before it lands, and anyone
deciding whether to ship a v0.0.1 tag from it.

## Scope decision (2026-05-17)

v0.0.1 is positioned as a **Linux preview** with honest scope. It is
not yet "cross-platform USB imager"; macOS and Windows can list and
validate disks, but the destructive path is stubbed.

The alternative — wait until macOS and Windows installers are
end-to-end — would have meant another two to four weeks of work on
hardware-bound code, with no shippable artefact in between. v0.0.1 is
the smallest tagged release that meaningfully documents and locks down
the Linux path. Honest readme copy ("Linux only in v0.0.1") is the
cost.

## What landed in this branch

### Documentation (new)
- `CHANGELOG.md` — Keep-a-Changelog format.
- `docs/USER_GUIDE.md` — end-to-end walkthrough of the first install.
- `docs/TROUBLESHOOTING.md` — pkexec, missing payload, Secure Boot,
  exFAT formatter, Wayland UI bugs.
- `docs/FAQ.md` — short answers to recurring questions.
- `docs/THREAT_MODEL.md` — explicit attacker model, in-scope vs
  out-of-scope, boundary→control mapping.
- `docs/HARDENING.md` — every shipping control, with `file:line`
  citations.
- `docs/SECURE_BOOT.md` — why it fails today, three credible paths to
  signed boot, step-by-step disable instructions.
- `docs/V0_0_1_STATUS.md` — this document.
- `docs/screenshots/` — placeholder.

### Code (refactors)
- **Platform abstraction.** `crates/core/src/lib.rs` reduced from
  ~1.1K lines to public-surface dispatch. Per-OS implementations moved
  to `crates/core/src/platform/{linux,macos,windows,unsupported}.rs`.
  Shared ISO scan moved to `crates/core/src/platform/scan.rs`. Tests
  preserved and reorganised by platform.
- **`thiserror`-based `CoreError`** with `Serialize`/`Deserialize`
  preserved, message text stable so existing substring-match tests
  pass.
- **Payload integrity verification.** New `crates/core/src/payload.rs`
  with `PayloadManifest` parser and `verify_payload()` that walks the
  payload tree, hashes content (path-prefixed, order-stable SHA-256),
  and compares against `payload/manifest.json:checksum`. Wired into
  Linux install path as a non-fatal warning (the manifest's checksum
  field is empty for now, so the path emits `ManifestError::Unpinned`).
  Fatal once `checksum` is populated.
- **`crates/cli/src/cli.rs`** — schema split out so `build.rs` can
  reuse it for man pages and completions.
- **`crates/cli/build.rs`** — emits `raidhos-cli.1`, plus bash, zsh,
  fish, elvish, and PowerShell completions, into the build OUT_DIR's
  `dist/` subdirectory. Release workflow picks these up.
- **`crates/priv-helper/src/main.rs`** rewritten on `clap` derive.
  Unknown flags fail loudly; 64 KiB argv length cap defends against
  resource-exhaustion. Errors emitted as JSON (machine-readable).
- **`crates/ui-tauri/src-tauri/src/main.rs`** — `save_boot_config`
  switched from `$HOME`-prefixed Linux path to
  `directories::ProjectDirs`, so the UI persists config correctly on
  macOS (`~/Library/Application Support`) and Windows (`%APPDATA%`)
  too.
- `BootConfig` and `BootEntryConfig` made `pub` and gained `Serialize`
  derive — required by the `to_vec_pretty` call site that was
  previously a dormant compile error.
- `#![warn(missing_docs)]` on `raidhos-core`; rustdoc on every public
  item.
- **macOS slice detector bug fix.** The original
  `id.contains('s')` heuristic caused every `diskN` identifier
  starting with `disk` to be filtered out. New `looks_like_slice`
  checks for `digit-'s'-digit` only. CI never caught this because
  workflows only ran Ubuntu; new CI matrix runs all three.

### Code (frontend hardening)
- Inline `<style>` (455 lines of CSS) extracted to
  `crates/ui-tauri/frontend/styles.css`.
- Inline `<script>` (736 lines of JS) extracted to
  `crates/ui-tauri/frontend/app.js`.
- Two inline `style=` attributes replaced with utility classes
  (`.actions--mb-3`, `.actions--mt-2`).
- `tauri.conf.json` CSP tightened:
  - Removed `'unsafe-inline'` from `style-src`.
  - Added `object-src 'none'`.

### CI / supply chain
- `.github/workflows/ci.yml` — now matrix-builds on
  `ubuntu-latest`, `macos-latest`, `windows-latest`. Coverage gate
  (`raidhos-core ≥95%`) split into its own Ubuntu-only job. GRUB build
  unchanged.
- `.github/workflows/release.yml` — new. Tag-driven, builds
  `x86_64`/`aarch64` × Linux/macOS/Windows, packages, hashes
  (SHA-256), signs with cosign (OIDC keyless), attests SLSA L3 build
  provenance, generates CycloneDX SBOM, publishes GitHub Release with
  a `SHA256SUMS` manifest.
- `.github/workflows/codeql.yml` — new. Static analysis on
  `actions` + `javascript`.
- `.github/workflows/audit.yml` — new. `cargo-deny` +
  `cargo-audit` on every PR and weekly cron.
- `.github/workflows/fuzz.yml` — new. 60-second smoke fuzz per
  trust-boundary target on every push.

### Fuzz scaffold
- `fuzz/Cargo.toml` plus four targets in `fuzz/fuzz_targets/`:
  - `validate_device_path` (fully active).
  - `parse_lsblk_disks`, `parse_disks_plist`,
    `parse_get_disk_json` (placeholders pending a `doc(hidden)`
    export from `raidhos-core`).
- `fuzz/README.md` explains local runs and CI smoke.

## What did not land (and why)

| Item | Why deferred | Estimated effort |
|---|---|---|
| macOS install pipeline (`diskutil unmountDisk` + `asr` + `bless`) | Requires Mac hardware to validate and a careful test plan against APFS-formatted disks. | 1-2 weeks |
| Windows install pipeline (`Clear-Disk` + `New-Partition` + `Format-Volume` + `bcdboot`) | Requires Windows hardware; PowerShell+raw-device interaction is fiddly to test in CI. | 1-2 weeks |
| Tauri 1 → 2 migration | Every command signature changes; the new capabilities model is a security upgrade but a multi-day refactor. | 3-5 days |
| Secure Boot (signed shim + MOK enrolment) | Needs a published signing key with rotation policy, a MOK enrolment helper, and either Microsoft UEFI CA sign-off or a documented MOK flow. | 1-3 weeks |
| BIOS / legacy boot path (`--legacy`) | Needs isolinux/syslinux integration and dual partition table. | 1 week |
| Persistence images | New feature: persistence overlay creation, GRUB chain into casper-rw / persistence.conf. | 2 weeks |
| Curated ISO catalog + GPG verification | Needs keyring curation, key-rotation policy, and a download UI. | 2-3 weeks |
| Drag-and-drop ISO management | Tauri webview drag-and-drop has platform quirks; would also want filesystem permissions tightening simultaneously. | 1 week |
| Svelte/Solid frontend rewrite | Days-long project; the existing static HTML/CSS/JS works well enough for v0.0.1. | 1-2 weeks |
| seccomp-bpf filter in priv-helper | Linux-only; needs accurate syscall whitelist that survives glibc/musl variance. Better landed alongside TOCTOU work. | 3-5 days |
| TOCTOU-safe device fd (open, fstat, re-validate) | Touches the install hot path; needs careful Linux integration testing. | 3-5 days |
| Docker base image pinning by digest in `tools/grub/Dockerfile` | One-line change, but needs a reproducible-build run to seed the digest. | 1 hour |
| Mac+Win bootstrap script audit | Currently best-effort. Worth re-validating once their install paths exist. | 1 day |
| i18n scaffolding (`fluent-rs`) | English-only fine for v0.0.1; structure now so retrofitting is cheap later. | 2 days |
| Accessibility audit (WCAG 2.2 AA, axe-core in CI) | Will require keyboard-flow rework. Worth landing once we have a real design system. | 1 week |
| Light theme + system theme detection | Cosmetic; the design system needs love first. | 2 days |
| Curated user-onboarding screen on first launch | Belongs with the UI design pass. | 1 day |
| Push-based progress events (Tauri `emit_all`) instead of polling `AppState::last_events` | Tauri 2 migration is a forcing function. | 1 day after Tauri 2 |
| BLAKE3 alongside SHA-256 for internal hashes | Speed win, low priority. | 1 day |
| Streaming SHA-256 during ISO copy (single-pass) | Tied to copy refactor in install pipeline. | 1-2 days |
| `cargo-mutants` (mutation testing) | High signal but expensive in CI minutes. Land after coverage stabilises. | 2 days |

## Verifying the branch locally

```bash
cargo build --workspace --exclude raidhos-ui
cargo clippy --workspace --exclude raidhos-ui --all-targets -- -D warnings
cargo test  --workspace --exclude raidhos-ui --all-targets
cargo tarpaulin -p raidhos-core --fail-under 95
./tools/grub/build_grub.sh    # Docker required, one-off
```

For the UI (requires Tauri native deps on your OS):

```bash
cargo build --workspace
./target/debug/raidhos-ui
```

For fuzz:

```bash
cd fuzz && cargo +nightly fuzz run validate_device_path -- -max_total_time=60
```

## Path to v0.0.2

In order of priority:

1. macOS installer (week 1-2).
2. Windows installer (week 3-4).
3. Tauri 2 migration (week 5).
4. Secure Boot with MOK enrolment (week 6-8).
5. ISO catalog with GPG verification (week 9-10).

Each of those is its own design doc before the code lands; pull
requests will reference these slots so reviewers can keep scope.
