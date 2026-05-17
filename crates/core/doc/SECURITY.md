<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-core` Security

`raidhos-core` is the trust boundary for the workspace. Every
destructive operation crosses this crate's API; the public
surface refuses unsafe input before it reaches the platform
layer.

The wider workspace threat model lives at
[`docs/THREAT_MODEL.md`](../../../docs/THREAT_MODEL.md). This
document indexes the *controls implemented in this crate* and
points to the code that enforces them.

---

## Controls

### C1 — Forbid `unsafe`

`#![forbid(unsafe_code)]` is set workspace-wide in
[`Cargo.toml:[workspace.lints.rust]`](../../../Cargo.toml).
A grep for `unsafe` in this crate's source returns zero hits
(verified by CI clippy `-D warnings`).

### C2 — Reject malformed device paths

[`lib.rs:validate_device_path`](../src/lib.rs) refuses:

- Empty or > 256 byte input
- Shell metacharacters (`;|&$\`<>"'*?()` plus `\n\r\t`)
- `..` (path traversal)
- The wrong per-OS shape (`/dev/…` on Linux, `/dev/disk…` on
  macOS, `\\.\PhysicalDriveN` on Windows)

### C3 — Refuse system / mounted / internal disks

The discovery parsers in
[`parsers.rs`](../src/parsers.rs) flag `is_system` based on the
OS's own classification:

- Linux: any mountpoint at `/`, `/boot`, `/boot/efi` (per
  `lsblk -o MOUNTPOINTS`)
- macOS: the `Internal == true` plist key from
  `diskutil list -plist`
- Windows: `IsSystem` or `IsBoot` from `Get-Disk`

`platform/*/validate_install` then refuses to operate on any
disk where `is_system` is true, has any partition mounted, or
has `removable == false` (when the caller is acting as a USB
installer).

### C4 — Double opt-in for destructive installs

Both `req.wipe == true` *and* `req.allow_write == true` are
required by every platform's `install_with` (see e.g.
[`platform/linux.rs`](../src/platform/linux.rs)). A dry-run
prints the events it would have emitted but writes nothing.

### C5 — Payload integrity

[`payload.rs::verify_payload`](../src/payload.rs) hashes the
payload tree against `manifest.json`. v0.0.1 emits an advisory
warning on mismatch; v0.1.0 will fail the install.

### C6 — ISO authenticity (GPG + SHA-256)

[`catalog.rs::verify_iso`](../src/catalog.rs) wraps `gpg(1)`
through the `Runtime` trait:

1. Imports the catalog-pinned `<fingerprint>.asc` into an
   ephemeral GNUPGHOME.
2. Verifies the detached signature on `SHA256SUMS`.
3. Looks up the expected hash for the entry's
   `iso_filename`.
4. SHA-256-hashes the ISO bytes and `eq_ignore_ascii_case`-
   compares.

Any failure surfaces as a `CatalogError::SignatureRejected`,
`MissingFilename`, `Sha256Mismatch`, or `KeyMissing` — each
with a clear `Display` string for the CLI / UI.

### C7 — No shell, no eval

Every subprocess is invoked via `std::process::Command` (under
`RealRuntime`), which does *not* spawn a shell. Arguments are
typed `&[&str]` and never concatenated into a command string
that an interpreter could expand.

### C8 — Testable subprocess paths

The `Runtime` trait makes every platform install path
unit-testable without touching real disks or binaries. Test
coverage on the install pipeline is currently 90%+; the
remaining gap is OS-specific code that needs a real device to
exercise.

---

## Defence-in-depth at the workspace edge

The library does not run privileged. Destructive installs go
through [`raidhos-priv-helper`](../../priv-helper/README.md),
which adds:

- **TOCTOU-safe device fd open** —
  [`crates/priv-helper/src/toctou.rs`](../../priv-helper/src/toctou.rs).
- **seccomp-bpf denylist** (Linux) —
  [`crates/priv-helper/src/seccomp.rs`](../../priv-helper/src/seccomp.rs).
- **polkit policy + pkexec wrapping** —
  [`packaging/polkit/org.raidhos.priv.policy`](../../../packaging/polkit/org.raidhos.priv.policy).
- **argv length cap** before clap parses —
  [`crates/priv-helper/src/main.rs`](../../priv-helper/src/main.rs).

The full list with citations is in
[`docs/HARDENING.md`](../../../docs/HARDENING.md).

---

## Supply chain

The crate participates in the workspace's signing pipeline:

- **cosign keyless** — every release artefact carries a Sigstore
  signature.
- **SLSA L3 provenance** — release builds publish a verifiable
  build attestation.
- **CycloneDX SBOM** — published alongside each release.
- **cargo-deny + cargo-audit** — both run on every PR, with the
  same advisory ignore-list (see
  [`deny.toml`](../../../deny.toml) and
  [`.github/workflows/audit.yml`](../../../.github/workflows/audit.yml)).
- **CodeQL + OpenSSF Scorecards** — weekly + per-push runs in
  [`.github/workflows/`](../../../.github/workflows/).

---

## Reporting a vulnerability

Email security@raidhos.dev — coordinated disclosure preferred.
Public PRs that include a fix are welcome once a tracking
advisory exists.
