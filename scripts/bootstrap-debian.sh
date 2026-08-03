#!/usr/bin/env bash
# Install RaidhOS build prerequisites on Debian / Ubuntu.
# Idempotent: only installs packages that are missing.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  SUDO=sudo
else
  SUDO=""
fi

PACKAGES=(
  build-essential
  pkg-config
  curl
  ca-certificates
  git
  libssl-dev
  libwebkit2gtk-4.1-dev
  libgtk-3-dev
  librsvg2-dev
  libayatana-appindicator3-dev
  parted
  dosfstools
  exfatprogs
  policykit-1
)

missing=()
for pkg in "${PACKAGES[@]}"; do
  if ! dpkg -s "$pkg" >/dev/null 2>&1; then
    missing+=("$pkg")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Installing: ${missing[*]}"
  $SUDO apt-get update
  $SUDO apt-get install -y --no-install-recommends "${missing[@]}"
else
  echo "All Debian/Ubuntu prerequisites are already present."
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing rustup (stable)..."
  "$(dirname "${BASH_SOURCE[0]}")/install-rustup.sh"
fi
