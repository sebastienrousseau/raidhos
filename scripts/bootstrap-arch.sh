#!/usr/bin/env bash
# Install RaidhOS build prerequisites on Arch Linux.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  SUDO=sudo
else
  SUDO=""
fi

PACKAGES=(
  base-devel
  pkgconf
  curl
  ca-certificates
  git
  openssl
  webkit2gtk-4.1
  gtk3
  librsvg
  libayatana-appindicator
  parted
  dosfstools
  exfatprogs
  polkit
)

missing=()
for pkg in "${PACKAGES[@]}"; do
  if ! pacman -Qi "$pkg" >/dev/null 2>&1; then
    missing+=("$pkg")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Installing: ${missing[*]}"
  $SUDO pacman -Sy --noconfirm --needed "${missing[@]}"
else
  echo "All Arch prerequisites are already present."
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing rustup (stable)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
fi
