#!/usr/bin/env bash
# Install RaidhOS build prerequisites on macOS via Homebrew.

set -euo pipefail

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew not found. Install it from https://brew.sh and re-run." >&2
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Installing Xcode Command Line Tools..."
  xcode-select --install || true
fi

brew update
brew install --quiet rustup-init pkg-config openssl@3
brew install --quiet --cask --no-quarantine || true

if ! command -v cargo >/dev/null 2>&1; then
  rustup-init -y --default-toolchain stable --profile minimal
fi
