<!-- SPDX-License-Identifier: GPL-3.0-only -->

# FAQ

Frequently-asked questions, grouped. For *errors*, see
[`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).

---

## Contents

- [Project](#project)
- [Compared to Ventoy / Etcher / Rufus](#compared-to-ventoy--etcher--rufus)
- [Capabilities](#capabilities)
- [Security & supply chain](#security--supply-chain)
- [Platform support](#platform-support)
- [Operational](#operational)
- [Contributing](#contributing)

---

## Project

### Why does RaidhOS exist when Ventoy already does this?

Ventoy is excellent and battle-tested, but its core is C with
bespoke boot-time code. RaidhOS rebuilds the same idea on a
memory-safe core (`unsafe_code = "forbid"` workspace-wide),
with a reproducible build pipeline, an explicit threat model,
and a small attack surface — about 2,000 lines of Rust for the
core library. The aim is **"the USB imager you can audit"**,
not "the USB imager with the most features."

### Is RaidhOS production-ready?

- **Linux power users**: yes, with the caveat that v0.0.1 is
  the first tagged release. Keep a backup of anything on the
  target USB before flashing.
- **macOS / Windows users**: the install path is code-complete
  but **not yet hardware-validated**. Discovery and dry-run
  work; we'll declare these platforms stable in v0.0.2.

### What licence is RaidhOS under?

GPL-3.0-only. RaidhOS links against `parted` and `grub` which
are GPL-3 at runtime, and the bundled Ventoy payload is also
GPL-3. See [`LICENSE`](../LICENSE).

### Who maintains it?

Sebastien Rousseau is the current maintainer. Decision-making
process: [`GOVERNANCE.md`](GOVERNANCE.md).

### Is there a roadmap?

[`ROADMAP.md`](ROADMAP.md). Current state per release:
[`V0_0_1_STATUS.md`](V0_0_1_STATUS.md).

---

## Compared to Ventoy / Etcher / Rufus

| Capability | RaidhOS | Ventoy | Etcher | Rufus | UNetbootin |
|---|:---:|:---:|:---:|:---:|:---:|
| Multi-ISO on one stick | yes (Linux today) | yes | no | no | no |
| Memory-safe core | **yes (Rust, `forbid(unsafe)`)** | no (C) | mixed (JS/C++) | no (C) | no |
| Reproducible build | **yes (Docker-pinned GRUB)** | no | no | no | no |
| Published threat model | **yes** | no | no | no | no |
| Coverage gate ≥95% on core | **yes** | no | no | no | no |
| Fuzzing in CI | **yes (4 targets)** | no | no | no | no |
| Pre-built signed binaries | yes (cosign + SLSA) | yes | yes | yes | yes |
| Cross-platform install | partial | yes | yes | Windows | yes |
| Secure Boot out of box | not yet (v0.0.3 target) | yes | yes | yes | no |
| Persistence | yes (Linux) | yes | no | partial | no |
| Drag-and-drop UI | yes | yes | yes | yes | no |
| ISO catalog with GPG verification | **yes (bundled)** | partial | no | no | no |

RaidhOS wins on **auditability**, **supply chain**, and
**reproducibility**. It loses on **platform parity** (catching
up in v0.0.2) and **breadth of features**. That gap is what
v0.0.1 is the start of closing.

### Will you re-implement Ventoy's full feature set?

Probably not — many Ventoy features (Windows-installer support,
some niche bootloader chains) are valuable but not on the
critical path for our threat model. We will add features that
*don't* dilute the audit story.

---

## Capabilities

### What ISOs are supported?

Anything with a `casper/` (Debian/Ubuntu live) or `live/`
(Debian Live) layout works out of the box. ISOs that ship
their own `boot/grub/grub.cfg` on the ISO root are picked up
via `configfile`. Other layouts fall through to "No known
kernel path found in ISO." — open an issue with the ISO name
and we'll add the path.

The bundled catalog
([`catalog/catalog.json`](../catalog/catalog.json)) names
Debian 12, Ubuntu 24.04, Fedora 41, Arch current, and Tails
6.x. Adding a distro is one PR.

### Does it support persistence?

Yes (Linux) via `--persistence-mb N`. The helper creates an
`ext4` overlay on the DATA partition with a tiny
`persistence.conf` of `/ union`. Distros that recognise the
`persistence` label (most live-build derivatives) will pick it
up automatically. See
[`USER_GUIDE.md#persistence`](USER_GUIDE.md#persistence).

### Does it support BIOS / legacy boot?

Not yet. `BOOTX64.EFI` is UEFI-only. A `--legacy` flag is
planned for v0.0.2.

### Does it support Secure Boot?

Not yet. Workaround: disable Secure Boot in firmware. v0.0.3
will ship a signed `BOOTX64.EFI` plus a MOK enrolment helper.
[`SECURE_BOOT.md`](SECURE_BOOT.md).

### Why two opt-ins (`wipe` and `allow_write`)?

Defence in depth. `wipe` says "yes, I expect this to be
destructive." `allow_write` says "yes, actually do it now, this
isn't a dry-run." Both required. CLI defaults to
`--wipe=true --allow-write=false --dry-run=true` so a careless
re-invocation can't go destructive.

### Can I script it?

Yes. `raidhos-cli` is fully scriptable. The helper emits JSON
on stdout so you don't need to scrape human text. Exit codes:
`0` for success, `1` for runtime failures (validation, I/O),
`2` for argv / clap parse errors. Examples:
[`examples/`](../examples/).

---

## Security & supply chain

### Why does it refuse to operate on my disk?

A device is refused if:

- it is mounted at `/`, `/boot`, or `/boot/efi`,
- the platform-discovery code flags it as internal/system,
- any of its partitions is mounted,
- the device path is empty, longer than 256 chars, contains
  shell metacharacters, or contains `..`,
- the device path does not match the per-OS shape (`/dev/...`
  on Linux, `/dev/diskN` on macOS, `\\.\PhysicalDriveN` on
  Windows).

Exact rules:
[`validate_device_path`](../crates/core/src/lib.rs).

### How is the supply chain protected?

| Layer | Control |
|---|---|
| Dependency advisories | `cargo deny check` (weekly cron + every PR) |
| Licence allowlist | 12 permissive licences in `deny.toml` |
| Dependency audit | `cargo audit --deny warnings` (weekly cron) |
| Reproducible build (Rust) | `cargo build --release --locked` + pinned toolchain + single codegen unit |
| Reproducible build (GRUB) | Docker base pinned by digest |
| Release signing | cosign keyless via OIDC |
| Release provenance | SLSA L3 attestation |
| SBOM | CycloneDX per release |
| Static analysis | CodeQL (Actions + JavaScript) |
| Project hygiene | OpenSSF Scorecards |

Full mapping: [`HARDENING.md`](HARDENING.md).

### Is there telemetry?

No. RaidhOS does not phone home. The only network traffic the
binary makes is whatever **you** ask it to (e.g. fetching an
ISO outside the tool, or invoking `gpg` against a network
keyserver — which it doesn't do during verify).

### How do I report a security issue?

Please don't open a public issue. See
[`SECURITY.md`](../SECURITY.md) for the private reporting path
and our 7-day acknowledgement, 30-day fix SLA.

### What's the threat model?

[`THREAT_MODEL.md`](THREAT_MODEL.md) names every attacker and
which controls address each. Short version: we defend local
unprivileged users, local users invoking RaidhOS, ISO authors,
payload-mirror operators, supply-chain attackers, subprocess
output attackers, network MITM, webview content, and TOCTOU
racers. We do **not** defend pre-existing root, hardware
attackers, side channels, or compromised firmware.

---

## Platform support

### Will it run on Apple Silicon?

Yes for discovery, validation, dry-run, and (once hardware
validated in v0.0.2) install. The Tauri 2 UI runs natively on
both arm64 and x86_64 macOS.

### Will it run on Windows on ARM (aarch64)?

Yes. The release matrix produces `aarch64-pc-windows-msvc`
artefacts. Discovery via `Get-Disk` is identical to x64.

### Will it ever run on FreeBSD / OpenBSD / NetBSD?

Eventually, but not in v0.0.1 or v0.0.2. Linux / macOS /
Windows come first; BSD comes after the platform parity is
locked in.

### Will it ever run on Android / iOS?

No. RaidhOS needs raw block-device access and elevation, both
of which are blocked by those OSes' sandbox models.

### What is the minimum supported Rust version?

1.78. Pinned in [`rust-toolchain.toml`](../rust-toolchain.toml)
and asserted in `Cargo.toml`'s `rust-version`. CI verifies this
across Ubuntu, macOS, and Windows.

### What is the minimum supported kernel version (Linux)?

5.10 or later for seccomp-bpf, but the helper does not require
seccomp — it warns and continues if the kernel rejects the
filter.

---

## Operational

### Where do I send feedback?

GitHub issues for bugs and feature requests:
<https://github.com/sebastienrousseau/raidhos/issues>. Security
issues via the process in [`SECURITY.md`](../SECURITY.md).

### How are releases cut?

See [`RELEASE.md`](RELEASE.md). TL;DR: a tag push triggers a
GitHub Actions workflow that builds the matrix, signs with
cosign, attests SLSA L3, generates an SBOM, and uploads to
GitHub Releases.

### How do I verify a release?

[`VERIFY.md`](VERIFY.md). One paragraph: download the artefact
plus its `.sig`, `.pem`, `.sha256` files; run `sha256sum -c`
and `cosign verify-blob` with the project's
certificate-identity regex.

### How can I package RaidhOS for my distro?

[`PACKAGING.md`](PACKAGING.md). The repo ships a Debian
`packaging/debian/`, a Homebrew formula
`packaging/homebrew/raidhos.rb`, a winget manifest
`packaging/winget/`, and an AppImage build script
`packaging/appimage/build-appimage.sh` to seed your work.

### Is there a CHANGELOG?

Yes — [`CHANGELOG.md`](../CHANGELOG.md), Keep-a-Changelog
format.

---

## Contributing

### What is the contribution process?

[`CONTRIBUTING.md`](../CONTRIBUTING.md). TL;DR: fork, make the
change with tests, `cargo fmt && cargo clippy && cargo test`,
PR. We squash-merge by default. New cross-platform code must
include unit tests that don't depend on real hardware.

### What is the code of conduct?

[Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

### Will you accept large patches?

Yes, in line with the project's stated direction. For anything
non-trivial, open an issue first so we can agree the design.

### Does the project sign tags and commits?

Yes — every release tag is GPG/SSH-signed by the maintainer
key, in addition to the per-artefact cosign signatures.
