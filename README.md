# RaidhOS

> The USB imager you can audit. Memory-safe core, reproducible build,
> explicit threat model.

[![CI](https://github.com/sebastienrousseau/raidhos/actions/workflows/ci.yml/badge.svg)](https://github.com/sebastienrousseau/raidhos/actions/workflows/ci.yml)
[![Audit](https://github.com/sebastienrousseau/raidhos/actions/workflows/audit.yml/badge.svg)](https://github.com/sebastienrousseau/raidhos/actions/workflows/audit.yml)
[![CodeQL](https://github.com/sebastienrousseau/raidhos/actions/workflows/codeql.yml/badge.svg)](https://github.com/sebastienrousseau/raidhos/actions/workflows/codeql.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](./LICENSE)
[![Rust 1.78+](https://img.shields.io/badge/rust-1.78%2B-blue.svg)](./rust-toolchain.toml)
[![Coverage gate](https://img.shields.io/badge/coverage-%E2%89%A595%25-brightgreen.svg)](./.github/workflows/ci.yml)
[![Unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](./Cargo.toml)

RaidhOS turns a USB stick into a multi-ISO boot drive in a few clicks.
A single Rust core powers a CLI and a Tauri desktop UI. The same
discovery, validation, and install pipeline runs on Linux, macOS, and
Windows — though for v0.0.1 only the **Linux install path is wired
end-to-end**. macOS and Windows can list and validate, not yet write.
See [`docs/V0_0_1_STATUS.md`](./docs/V0_0_1_STATUS.md) for the full
breakdown.

> **Screenshots:** the UI lives at `crates/ui-tauri/frontend/`. Real
> screenshots will land in `docs/screenshots/` once the v0.0.1 UI pass
> is in.

## Why another USB imager?

| | Ventoy | balenaEtcher | Rufus | **RaidhOS** |
|---|---|---|---|---|
| Memory-safe core | C | C++/JS | C | **Rust, `forbid(unsafe)`** |
| Multi-ISO on one stick | yes | no | no | **yes** |
| Reproducible build | no | no | no | **yes (Docker GRUB)** |
| Coverage gate | no | no | no | **≥95% on core** |
| Signed releases (cosign) | no | partial | no | **yes (planned for v0.0.1)** |
| Published threat model | no | no | no | **yes** |
| Cross-platform install | yes | yes | Win only | **Linux only in v0.0.1** |
| Secure Boot | yes | yes | yes | **not yet** |
| Persistence | yes | no | partial | **not yet** |

RaidhOS is positioned as *the* USB imager security engineers and
distros can audit. We are not yet broader than Ventoy. We are aiming
to be cleaner.

## Safety model

- Refuses any disk mounted at `/`, `/boot`, or `/boot/efi`, plus any
  disk flagged internal/system by the OS.
- Validates every device path against a per-OS allowlist before any
  subprocess sees it.
- Requires **both** `wipe=true` and `allow_write=true` before writing.
- No shell invocation in the privileged path —
  `std::process::Command` with separated argv only.
- Workspace-wide `unsafe_code = "forbid"`. Zero `unsafe` blocks.
- Polkit policy with `auth_admin_keep`, `allow_any=no`,
  `allow_inactive=no`.
- Strict CSP in the Tauri webview (`'self'`-only for scripts and
  styles; no `'unsafe-inline'`).

Full details: [`SECURITY.md`](./SECURITY.md),
[`docs/THREAT_MODEL.md`](./docs/THREAT_MODEL.md),
[`docs/HARDENING.md`](./docs/HARDENING.md).

## Quick start

```bash
# 1. Install OS prerequisites.
./scripts/bootstrap-debian.sh    # or: bootstrap-arch.sh / -fedora.sh /
                                  #     -macos.sh / -windows.ps1

# 2. Build the CLI, the core library, and the privileged helper.
#    The Tauri UI is excluded; install its native deps separately if
#    you want it.
cargo build --release --workspace --exclude raidhos-ui

# 3. Inspect candidate disks (no privileges needed).
./target/release/raidhos-cli list-disks

# 4. Dry-run an install — never writes to disk.
./target/release/raidhos-cli install --device /dev/sdX --dry-run
```

A real install on Linux:

```bash
# Build the reproducible BOOTX64.EFI (Docker required, one-time).
./tools/grub/build_grub.sh

# Point RAIDHOS_PAYLOAD_DIR at a directory containing esp/ and data/
# subdirectories, then invoke the privileged helper.
sudo RAIDHOS_PAYLOAD_DIR=/path/to/payload \
  ./target/release/raidhos-priv-helper install \
  --device /dev/sdX --allow-write
```

The privileged helper is the **only** RaidhOS binary that needs to run
as root or Administrator. The UI invokes it via `pkexec` on Linux. On
macOS and Windows the install path is `NotImplemented` until v0.0.2.

Full walkthrough: [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md).
Stuck? [`docs/TROUBLESHOOTING.md`](./docs/TROUBLESHOOTING.md).

## Platform support

| Platform                | Discovery | Install | UI build  |
| ----------------------- | --------- | ------- | --------- |
| Linux (Debian/Ubuntu)   | ✅ `lsblk` | ✅       | ✅        |
| Linux (Arch)            | ✅ `lsblk` | ✅       | ✅        |
| Linux (Fedora)          | ✅ `lsblk` | ✅       | ✅        |
| macOS (Intel / Apple)   | ✅ `diskutil` | 🚧    | ✅        |
| Windows                 | ✅ `Get-Disk` | 🚧    | ✅        |

🚧 = install pipeline returns `NotImplemented`. Discovery and the
safety checks are live everywhere.

## Workspace layout

```text
crates/
├── core/          # raidhos-core: discovery, validation, install,
│                  #  payload integrity, error type
│   ├── src/
│   │   ├── lib.rs
│   │   ├── payload.rs
│   │   └── platform/{linux,macos,windows,unsupported}.rs
├── cli/           # raidhos-cli: developer-facing CLI
├── priv-helper/   # raidhos-priv-helper: privileged JSON-over-stdout
│                  #  helper (the only binary intended to run as root)
└── ui-tauri/      # Tauri desktop UI (static frontend + Rust commands)
fuzz/              # cargo-fuzz scaffold for trust-boundary parsers
docs/              # Architecture, payload schema, user guide,
│                  #  troubleshooting, threat model, hardening, FAQ
packaging/linux/   # polkit policy
scripts/           # Per-OS bootstrap scripts
tools/grub/        # Reproducible Docker build for the BOOTX64.EFI
.github/workflows/ # CI, release, CodeQL, audit, fuzz smoke
```

## Documentation

- [User guide](./docs/USER_GUIDE.md)
- [Troubleshooting](./docs/TROUBLESHOOTING.md)
- [FAQ](./docs/FAQ.md)
- [Architecture](./docs/ARCHITECTURE.md)
- [Payload layout](./docs/PAYLOAD.md)
- [Roadmap](./docs/ROADMAP.md)
- [Threat model](./docs/THREAT_MODEL.md)
- [Hardening summary](./docs/HARDENING.md)
- [Secure Boot status](./docs/SECURE_BOOT.md)
- [v0.0.1 status report](./docs/V0_0_1_STATUS.md)
- [Boot config schema (JSON)](./docs/BOOT_CONFIG_SCHEMA.json)
- [Contributing](./CONTRIBUTING.md)
- [Security policy](./SECURITY.md)
- [Changelog](./CHANGELOG.md)

## Releases

Tagged releases (`v0.0.1` onwards) ship:

- Pre-built binaries for `x86_64`/`aarch64` × Linux/macOS/Windows.
- A `BOOTX64.EFI` produced by the reproducible Docker GRUB build.
- SHA-256 checksums and a single `SHA256SUMS` manifest.
- cosign keyless signatures (`.sig` + `.pem`) for every artefact.
- SLSA L3 build provenance attestations.
- A CycloneDX SBOM.

The release workflow lives at `.github/workflows/release.yml`.
Verification instructions: [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md).

## License

RaidhOS is distributed under the GNU GPL v3.0. See
[LICENSE](./LICENSE).
