#!/usr/bin/env bash
# Install RaidhOS build prerequisites on Fedora.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  SUDO=sudo
else
  SUDO=""
fi

PACKAGES=(
  "@development-tools"
  pkgconf-pkg-config
  curl
  ca-certificates
  git
  openssl-devel
  webkit2gtk4.1-devel
  gtk3-devel
  librsvg2-devel
  libappindicator-gtk3-devel
  parted
  dosfstools
  exfatprogs
  polkit
)

echo "Installing/updating: ${PACKAGES[*]}"
$SUDO dnf install -y "${PACKAGES[@]}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Installing rustup (stable)..."
  "$(dirname "${BASH_SOURCE[0]}")/install-rustup.sh"
fi
