#!/usr/bin/env bash
# Example 05 — install with persistence (Linux only).
#
# Adds an `ext4` persistence overlay file on the DATA partition.
# Live-build derivatives (Tails, Debian Live, Ubuntu live) pick it
# up automatically and write user data to it across reboots.
set -euo pipefail

DEVICE=${1:-}
SIZE_MB=${2:-4096}

if [[ -z $DEVICE ]]; then
    echo "usage: $0 <DEVICE> [SIZE_MB]" >&2
    echo "       $0 /dev/sdb 4096" >&2
    exit 2
fi
if [[ -z ${RAIDHOS_PAYLOAD_DIR:-} ]]; then
    echo "error: RAIDHOS_PAYLOAD_DIR not set" >&2
    exit 2
fi

echo "Installing on $DEVICE with ${SIZE_MB} MiB persistence …"

sudo -E raidhos-priv-helper install \
    --device "$DEVICE" \
    --allow-write \
    --persistence-mb "$SIZE_MB"

echo
echo "Persistence overlay should now be at:"
echo "  /mnt/raidhos-data-persist/persistence (ext4-formatted)"
echo "  /mnt/raidhos-data-persist/persistence.conf ('/ union')"
