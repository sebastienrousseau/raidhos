# Threat model

RaidhOS reads and writes raw block devices. The blast radius of a bug is
"the user's data on the targeted disk, and possibly their system disk."
This document is the explicit attacker model the code defends against.

## Assets

1. **The user's system disk.** A flash that lands on the wrong device
   loses an OS.
2. **The bytes booted into.** A tampered ISO or tampered GRUB binary can
   pivot the next reboot into anything.
3. **The user's polkit / sudo credentials.** Cached briefly by
   `auth_admin_keep`; a malicious process could try to ride that window.
4. **The integrity of `raidhos-priv-helper` while elevated.** Anything it
   parses while running as root must be hostile-input-safe.

## Attackers in scope

| Attacker                        | Capabilities                                              | Key controls |
| ------------------------------- | --------------------------------------------------------- | ------------ |
| Local unprivileged user         | Reads world-readable files, can run any unprivileged tool | Polkit policy with `allow_any=no`, `allow_inactive=no`; no setuid binaries |
| Local user with shell access invoking RaidhOS | Provides device paths and ISO paths      | Per-OS device path allowlist; system disk refusal; mount-state refusal; double opt-in (`wipe` + `allow_write`) |
| Adversary controlling an ISO    | Crafts an ISO with a hostile layout, hostile kernel       | ISO never executes during install; GRUB chains into it later — Secure Boot is the long-term mitigation (see `docs/SECURE_BOOT.md`) |
| Adversary on the payload mirror | Replaces `payload/` files between download and install    | Payload manifest SHA-256 verification before any write (`payload/manifest.json` + `verify_payload()`) |
| Adversary on the supply chain   | Injects a malicious dependency                            | `cargo deny` (advisories + licenses + unknown sources); reproducible build flags (`Cargo.lock`, pinned toolchain, single-codegen-unit release profile); SLSA L3 provenance on releases |
| Adversary controlling subprocess output | Plants malicious bytes in `lsblk`/`diskutil`/`Get-Disk` output | Hand-rolled parsers; fuzzing scaffold (`fuzz/`) over all three; strict `deny_unknown_fields`-equivalent on helper inputs |
| Network attacker between user and GitHub | Replaces the release binary                      | cosign keyless signatures on every release; SBOM and SLSA attestations published alongside binaries |
| Tauri webview content (XSS via filename) | Renders attacker-controlled string in DOM         | Strict CSP (no `'unsafe-inline'` for scripts; styles extracted to file); `frame-ancestors 'none'`; `form-action 'none'` |
| TOCTOU between validate and write | Swaps a symlink or unplugs USB between checks            | Helper opens the device fd and `fstat`-checks against the validated metadata before issuing writes (planned; v0.0.2) |

## Attackers explicitly out of scope

- Anyone who already has root or Administrator on the host machine.
  RaidhOS is not a rootkit-defeating tool.
- Hardware attackers (cold-boot, physical USB-mux MITM, evil-USB
  controllers presenting different geometry to host vs target).
- Side-channel attackers on the host (Spectre-class, RowHammer).
- Compromised firmware (UEFI implants, hostile Option ROMs). Secure Boot
  helps once we ship it, but the bar is "firmware trusts what it
  trusts."

## Boundaries and controls — quick map

| Boundary                              | Control                                              | Location |
| ------------------------------------- | ---------------------------------------------------- | -------- |
| User-supplied device path → core      | `validate_device_path`                               | `crates/core/src/lib.rs` |
| Subprocess output → core              | Typed parsers, fuzz targets                          | `crates/core/src/lib.rs` (lsblk, plist, get-disk) |
| UI command → core                     | `#[tauri::command]` with typed args, strict CSP      | `crates/ui-tauri/src-tauri/src/main.rs` |
| Stdin/argv → priv-helper              | `clap` typed parsing, length cap                     | `crates/priv-helper/src/main.rs` |
| Payload on disk → install             | SHA-256 verification against manifest                | `payload/manifest.json` + `verify_payload()` |
| Network → user's machine              | cosign signatures, SBOM, SLSA L3                     | `.github/workflows/release.yml` |
| Webview → user data                   | CSP `default-src 'self'`; no `connect-src` to remote | `crates/ui-tauri/src-tauri/tauri.conf.json` |

## What "secure" means here

We do not claim 100% secure. We claim:

1. Memory-safe core: zero `unsafe` blocks, enforced at compile time.
2. Privilege-minimised architecture: exactly one binary runs as root,
   and that binary only parses a tiny, strict input.
3. Defence in depth: every destructive operation passes through at least
   two independent guards (path validation + system-disk refusal +
   mount-state refusal + double opt-in).
4. Auditable supply chain: pinned dependencies, license audit, signed
   releases, SBOM, provenance.
5. Transparent threat model (this document) so reviewers can hold us to
   it.

Items still on the work list to close known gaps:

- TOCTOU-safe device handles in the helper (open-then-re-validate).
- `seccomp-bpf` filter restricting the helper's syscall surface after
  parse.
- Cryptographic verification of downloaded ISOs against distro GPG
  signing keys (curated keyring shipped with RaidhOS).
- Pinning Docker base images by digest in `tools/grub/Dockerfile`.
- Signing the generated `BOOTX64.EFI` and providing a Secure Boot path.

See `docs/V0_0_1_STATUS.md` for the current state of each.
