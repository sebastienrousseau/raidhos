# Hardening summary

Every shipping security control, with file:line citations so reviewers
can verify them in seconds. Pair with `docs/THREAT_MODEL.md`.

## Compile-time

| Control                              | Citation |
| ------------------------------------ | -------- |
| Workspace-wide `unsafe_code = "forbid"` | `Cargo.toml` (`[workspace.lints.rust]`) |
| `cargo clippy ... -D warnings` in CI | `.github/workflows/ci.yml` |
| 95% coverage gate on `raidhos-core`  | `.github/workflows/ci.yml` (`cargo tarpaulin --fail-under 95`) |
| Pinned stable toolchain              | `rust-toolchain.toml` |
| Resolver v2 (strict feature unification) | `Cargo.toml` (`resolver = "2"`) |
| Release profile: LTO + strip + abort-on-panic + single codegen unit | `Cargo.toml` (`[profile.release]`) |

## Supply-chain

| Control                              | Citation |
| ------------------------------------ | -------- |
| Advisory database checked            | `deny.toml` (`[advisories]`) |
| License allowlist (12 permissive)    | `deny.toml` (`[licenses]`) |
| Unknown registry / git source denied | `deny.toml` (`[sources]`) |
| Yanked-crate warnings                | `deny.toml` (`yanked = "warn"`) |
| `Cargo.lock` committed (apps)        | repo root |
| CycloneDX SBOM on every release      | `.github/workflows/release.yml` |
| cosign keyless signatures on release artefacts | `.github/workflows/release.yml` |
| SLSA L3 provenance attestation       | `.github/workflows/release.yml` |
| CodeQL static analysis               | `.github/workflows/codeql.yml` |

## Input validation

| Control                              | Citation |
| ------------------------------------ | -------- |
| Device-path allowlist (per OS)       | `crates/core/src/lib.rs` (`validate_device_path`) |
| Shell-metacharacter rejection        | `crates/core/src/lib.rs` (constant `FORBIDDEN`) |
| Path-traversal (`..`) rejection      | `crates/core/src/lib.rs` |
| Length cap (256 bytes) on device path | `crates/core/src/lib.rs` |
| Length cap (64 KiB) on helper input  | `crates/priv-helper/src/main.rs` |
| Typed argv parsing in helper (`clap`) with unknown-flag rejection | `crates/priv-helper/src/main.rs` |
| Hand-rolled `diskutil` plist parser (no plist dep) | `crates/core/src/platform/macos.rs` |
| `serde`-based `lsblk` JSON parser    | `crates/core/src/platform/linux.rs` |
| `serde_json::Value` walk for `Get-Disk` output | `crates/core/src/platform/windows.rs` |
| Fuzz targets over every parser       | `fuzz/fuzz_targets/` |

## Runtime safety

| Control                              | Citation |
| ------------------------------------ | -------- |
| Refuse mounted partitions            | `crates/core/src/platform/linux.rs` (`validate_install`) |
| Refuse disks mounted at `/`, `/boot`, `/boot/efi` | `crates/core/src/platform/linux.rs` (`list_disks` builds `is_system`) |
| Refuse disks flagged internal (macOS) | `crates/core/src/platform/macos.rs` (`Internal` plist key) |
| Refuse disks flagged system or boot (Windows) | `crates/core/src/platform/windows.rs` (`IsBoot` / `IsSystem`) |
| Double opt-in: both `wipe` and `allow_write` required | `crates/core/src/platform/linux.rs` (`install_with_disks`) |
| Default dry-run = true, allow-write = false (CLI) | `crates/cli/src/main.rs` |
| No shell invocation in privileged path; `std::process::Command` with separated argv | `crates/core/src/platform/linux.rs` (`run`) |
| Payload presence check (`esp/`, `data/`) | `crates/core/src/platform/linux.rs` (`payload_copy`) |
| Payload SHA-256 verification before write | `crates/core/src/payload.rs` (`verify_payload`) |

## Privilege boundary

| Control                              | Citation |
| ------------------------------------ | -------- |
| Single privileged binary             | `crates/priv-helper/` (the only binary intended to run as root) |
| pkexec via polkit, `auth_admin_keep` | `packaging/linux/org.raidhos.policy` |
| `allow_any = no` (no remote elevation) | `packaging/linux/org.raidhos.policy` |
| `allow_inactive = no` (must have an active session) | `packaging/linux/org.raidhos.policy` |
| JSON-over-stdout output (machine-readable, never HTML/shell-injectable) | `crates/priv-helper/src/main.rs` |

## Tauri / UI

| Control                              | Citation |
| ------------------------------------ | -------- |
| Strict CSP: `default-src 'self'`     | `crates/ui-tauri/src-tauri/tauri.conf.json` |
| No `'unsafe-inline'` for scripts     | same |
| No `'unsafe-inline'` for styles      | same (after this release; styles in `frontend/styles.css`) |
| `frame-ancestors 'none'`             | same |
| `form-action 'none'`                 | same |
| `connect-src 'self' ipc: tauri:` only | same |
| Windows-subsystem = "windows" on non-debug Windows builds | `crates/ui-tauri/src-tauri/src/main.rs` |

## CI gates

| Control                              | Citation |
| ------------------------------------ | -------- |
| Build on every PR                    | `.github/workflows/ci.yml` |
| `cargo fmt --check`                  | same |
| `cargo clippy -D warnings`           | same |
| Coverage gate ≥ 95% on core          | same |
| Reproducible GRUB build via Docker   | `.github/workflows/ci.yml` (`grub-build`) + `tools/grub/build_grub.sh` |
| Pinned GitHub Actions by major version (Dependabot keeps current) | every workflow |

## Roadmap items not in this release

These are documented for honesty; they are not yet shipping controls.

- TOCTOU-safe device handle in helper (open fd, `fstat`, re-validate).
- `seccomp-bpf` filter in helper after parse.
- Curated GPG keyring for distro ISO signature verification.
- `BOOTX64.EFI` signed under a published RaidhOS key, with shim
  integration for Secure Boot.
- Docker base image pinning by digest in `tools/grub/Dockerfile`.

Track these in `docs/V0_0_1_STATUS.md`.
