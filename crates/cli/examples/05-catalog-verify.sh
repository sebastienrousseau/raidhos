#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Verify a downloaded ISO against the catalog. Requires gpg(1)
# on PATH. The GPG fingerprint and SHA256SUMS layout are pinned
# per-entry in catalog/catalog.json.
#
# Usage:
#   ./05-catalog-verify.sh <slug> <iso> <SHA256SUMS> <SHA256SUMS.gpg>
#
# Defaults assume an Ubuntu 24.04 download in ~/Downloads.

set -euo pipefail

slug="${1:-ubuntu-24.04-desktop-amd64}"
iso="${2:-$HOME/Downloads/ubuntu-24.04.3-desktop-amd64.iso}"
sums="${3:-$HOME/Downloads/SHA256SUMS}"
sig="${4:-$HOME/Downloads/SHA256SUMS.gpg}"

if [ ! -f "$iso" ]; then
    echo "ISO not found at $iso — download it first." >&2
    exit 2
fi
if ! command -v gpg >/dev/null; then
    echo "gpg(1) is required and not on PATH." >&2
    exit 2
fi

echo "$ raidhos-cli catalog verify --slug $slug --iso $iso --sums $sums --sig $sig"
raidhos-cli catalog verify \
    --slug "$slug" \
    --iso  "$iso" \
    --sums "$sums" \
    --sig  "$sig"
