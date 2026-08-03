#!/usr/bin/env bash
# Install a checksum-verified rustup-init release on supported Linux hosts.

set -euo pipefail

readonly RUSTUP_VERSION="1.28.2"

case "$(uname -m)" in
  x86_64)
    readonly RUSTUP_TARGET="x86_64-unknown-linux-gnu"
    readonly RUSTUP_SHA256="20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c"
    ;;
  aarch64 | arm64)
    readonly RUSTUP_TARGET="aarch64-unknown-linux-gnu"
    readonly RUSTUP_SHA256="e3853c5a252fca15252d07cb23a1bdd9377a8c6f3efa01531109281ae47f841c"
    ;;
  *)
    echo "Unsupported rustup architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

rustup_init="$(mktemp)"
trap 'rm -f "$rustup_init"' EXIT

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${RUSTUP_TARGET}/rustup-init" \
  --output "$rustup_init"
printf '%s  %s\n' "$RUSTUP_SHA256" "$rustup_init" | sha256sum --check --status
chmod 0700 "$rustup_init"
"$rustup_init" -y --default-toolchain stable --profile minimal
