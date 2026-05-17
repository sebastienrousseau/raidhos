#!/usr/bin/env bash
# Example 02 — dry-run install.
#
# Exercises every safety check (device-path allowlist, system-disk
# refusal, mount-state refusal, opt-in flags) without writing
# anything. Pick a USB stick first with `01-list-disks.sh`.
set -euo pipefail

DEVICE=${1:-}
if [[ -z $DEVICE ]]; then
    echo "usage: $0 <DEVICE>            # e.g. $0 /dev/sdb" >&2
    exit 2
fi

if ! command -v raidhos-cli >/dev/null 2>&1; then
    echo "error: raidhos-cli not on PATH" >&2
    exit 1
fi

echo "Dry-running install against $DEVICE …"
echo

# --dry-run is the default; we set it explicitly so the example is
# instructive.
raidhos-cli install \
    --device "$DEVICE" \
    --dry-run=true \
    --wipe=true \
    --allow-write=false
