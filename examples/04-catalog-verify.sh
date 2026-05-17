#!/usr/bin/env bash
# Example 04 — catalog verification.
#
# Verify a locally-downloaded ISO against the bundled catalog.
# The catalog (`catalog/catalog.json`) names the SHA256SUMS URL,
# the detached signature URL, and the GPG signing-key fingerprint
# for each major distro.
#
# Requires: gpg(1) on PATH.
set -euo pipefail

SLUG=${1:-}
ISO=${2:-}
SUMS=${3:-}
SIG=${4:-}

if [[ -z $SLUG || -z $ISO || -z $SUMS || -z $SIG ]]; then
    echo "usage: $0 <slug> <iso> <sums> <sig>" >&2
    echo "example: $0 ubuntu-24.04-desktop-amd64 \\" >&2
    echo "         ~/Downloads/ubuntu-24.04.3-desktop-amd64.iso \\" >&2
    echo "         ~/Downloads/SHA256SUMS \\" >&2
    echo "         ~/Downloads/SHA256SUMS.gpg" >&2
    exit 2
fi
if ! command -v gpg >/dev/null 2>&1; then
    echo "error: gpg(1) not on PATH" >&2
    exit 1
fi

raidhos-cli catalog list
echo
echo "Verifying $ISO against catalog slug $SLUG …"

raidhos-cli catalog verify \
    --slug "$SLUG" \
    --iso  "$ISO" \
    --sums "$SUMS" \
    --sig  "$SIG"
