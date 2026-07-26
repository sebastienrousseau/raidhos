# Contributing to RaidhOS

Thanks for your interest in contributing! This document walks through the
development workflow on macOS, Linux (Debian/Ubuntu, Arch, Fedora) and
Windows.

## Prerequisites

- A stable Rust toolchain (the repo pins to stable via `rust-toolchain.toml`,
  so [rustup](https://rustup.rs) will do the right thing automatically).
- Git.
- (UI only) Tauri's native dependencies — see below.

The bootstrap scripts in `scripts/` install the right system packages for
your OS:

| OS                    | Script                          |
| --------------------- | ------------------------------- |
| Linux (Debian/Ubuntu) | `scripts/bootstrap-debian.sh`   |
| Linux (Arch)          | `scripts/bootstrap-arch.sh`     |
| Linux (Fedora)        | `scripts/bootstrap-fedora.sh`   |
| macOS                 | `scripts/bootstrap-macos.sh`    |
| Windows (PowerShell)  | `scripts/bootstrap-windows.ps1` |

Each script is idempotent and only installs what's missing.

## Build

```bash
# Build the CLI + library + privileged helper (no native UI deps required).
cargo build --workspace --exclude raidhos-ui

# Build everything, including the Tauri UI (requires the OS-specific deps).
cargo build --workspace
```

## Test, lint, format

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude raidhos-ui --all-targets -- -D warnings
cargo test --workspace --exclude raidhos-ui
```

## CI gates — match these before pushing

CI on `feat/v0.0.1` enforces the gates below. Running them
locally before pushing saves a round-trip.

| Gate | Command | Floor |
|---|---|---|
| Format | `cargo fmt --all -- --check` | clean |
| Lint | `cargo clippy --workspace --exclude raidhos-ui --all-targets -- -D warnings` | zero warnings |
| Tests | `cargo test --workspace --exclude raidhos-ui` | all green |
| MSRV | `cargo +1.85 check --workspace --exclude raidhos-ui --all-targets` | clean |
| Coverage (core) | `cargo tarpaulin -p raidhos-core --exclude-files 'crates/{cli,priv-helper,ui-tauri}/**' --exclude-files 'fuzz/**'` | `--fail-under 100` |
| Coverage (cli) | same shape, `-p raidhos-cli --engine llvm --exclude-files 'crates/cli/tests/**'` (plus the other crate excludes) | `--fail-under 100` |
| Coverage (priv-helper) | same shape, `-p raidhos-priv-helper --engine llvm` | `--fail-under 90` (Linux); 100% on macOS dev hosts |
| Supply chain | `cargo deny check` and `cargo audit` | exit 0 with the ignores documented in `deny.toml` |

The 100% coverage gates on `raidhos-core` and `raidhos-cli` are
strict on purpose — any new pub item or branch needs a test, and
binary `main()` should delegate to testable helpers rather than
hand-roll subprocess dispatch. The 90% priv-helper floor reflects
the privileged paths (TOCTOU pin, persistence overlay) that need
real root + block devices; closing that gap is v0.0.2 work.

## Running the CLI

```bash
# List candidate disks (no privileges required for discovery on Linux).
cargo run -p cli -- list-disks

# Dry-run an install — never writes to disk.
cargo run -p cli -- install --device /dev/sdX --dry-run

# Real install (Linux): requires root via the priv-helper.
sudo target/debug/raidhos-priv-helper install --device /dev/sdX --allow-write
```

The CLI accepts the same arguments on all platforms; only the discovery
backend differs (`lsblk` on Linux, `diskutil` on macOS, `Get-Disk` on
Windows).

## Pull-request expectations

- Keep changes focused. Small, reviewable PRs land much faster.
- Run `cargo fmt`, `cargo clippy`, and the tests locally before pushing.
- Update or add tests for any behaviour change. New cross-platform code
  must include unit tests that don't depend on real hardware.
- If a change touches the install / format path on Linux, please describe
  how you verified it on real hardware (or that you didn't, so reviewers
  can).
- Update `docs/` and the README when changing user-visible behaviour.

## Security issues

Please do **not** open public issues for vulnerabilities. See
[SECURITY.md](./SECURITY.md) for the private reporting process.
