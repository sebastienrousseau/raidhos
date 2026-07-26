#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# ╔════════════════════════════════════════════════════════════╗
# ║  DESTRUCTIVE — this script ACTUALLY FLASHES the device.    ║
# ║  Read the source before running. There is no undo.         ║
# ╚════════════════════════════════════════════════════════════╝
#
# Usage: ./04-install-real.sh /dev/sdb [persistence-mb]

set -euo pipefail

device="${1:-}"
persist="${2:-0}"
payload="${RAIDHOS_PAYLOAD_DIR:-/srv/raidhos/payload}"

if [ -z "$device" ]; then
    echo "usage: $0 <device> [persistence-mb]" >&2
    exit 2
fi

if [ ! -d "$payload" ]; then
    echo "RAIDHOS_PAYLOAD_DIR=$payload does not exist." >&2
    exit 2
fi

echo "About to wipe and re-partition $device."
echo "Payload: $payload"
echo "Persistence: ${persist} MiB"
printf "Type 'WIPE %s' to continue: " "$device"
read -r answer
if [ "$answer" != "WIPE $device" ]; then
    echo "Aborted."
    exit 1
fi

echo "$ sudo RAIDHOS_PAYLOAD_DIR=$payload raidhos-priv-helper install --device $device --allow-write --persistence-mb $persist"
sudo RAIDHOS_PAYLOAD_DIR="$payload" \
    raidhos-priv-helper install \
        --device "$device" \
        --allow-write \
        --persistence-mb "$persist" | jq .
