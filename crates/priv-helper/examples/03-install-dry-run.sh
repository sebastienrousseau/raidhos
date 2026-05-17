#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Dry-run the install pipeline. Every safety check runs (mount
# check, system disk check, payload presence, partition layout
# planning) — but no bytes are ever written.
#
# Usage: ./03-install-dry-run.sh /dev/sdb

set -euo pipefail

device="${1:-/dev/sdb}"
payload="${RAIDHOS_PAYLOAD_DIR:-/srv/raidhos/payload}"

if [ ! -d "$payload" ]; then
    echo "RAIDHOS_PAYLOAD_DIR=$payload does not exist." >&2
    echo "Stage a payload tree first (see docs/PAYLOAD.md)." >&2
    exit 2
fi

echo "$ sudo RAIDHOS_PAYLOAD_DIR=$payload raidhos-priv-helper install --device $device --dry-run"
sudo RAIDHOS_PAYLOAD_DIR="$payload" \
    raidhos-priv-helper install --device "$device" --dry-run | jq .
