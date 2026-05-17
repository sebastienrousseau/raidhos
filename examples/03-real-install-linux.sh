#!/usr/bin/env bash
# Example 03 — real install on Linux.
#
# Runs the destructive install pipeline against a USB stick.
# Requires:
#   - raidhos-priv-helper on PATH (installed via package manager
#     or `cargo install --git ... raidhos-priv-helper`)
#   - $RAIDHOS_PAYLOAD_DIR set to a directory with esp/ + data/
#   - parted, mkfs.vfat, mkfs.exfat on PATH
#   - root / pkexec
set -euo pipefail

DEVICE=${1:-}
if [[ -z $DEVICE ]]; then
    echo "usage: $0 <DEVICE>            # e.g. $0 /dev/sdb" >&2
    exit 2
fi
if [[ -z ${RAIDHOS_PAYLOAD_DIR:-} ]]; then
    echo "error: RAIDHOS_PAYLOAD_DIR not set" >&2
    exit 2
fi

for tool in raidhos-priv-helper parted mkfs.vfat; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: $tool not on PATH" >&2
        exit 1
    fi
done
if ! command -v mkfs.exfat >/dev/null 2>&1 && \
   ! command -v mkexfatfs >/dev/null 2>&1; then
    echo "error: no exFAT formatter; install exfatprogs" >&2
    exit 1
fi

# Pre-flight: dry-run first to catch validation errors before we
# escalate. The dry-run is non-destructive.
echo "Pre-flight dry-run on $DEVICE …"
raidhos-cli install --device "$DEVICE" --dry-run

# Destructive step. Two opt-ins: --wipe (default true) and
# --allow-write (off by default; required).
echo
echo "Running real install on $DEVICE …"
echo "RAIDHOS_PAYLOAD_DIR=$RAIDHOS_PAYLOAD_DIR"
echo

sudo -E raidhos-priv-helper install \
    --device "$DEVICE" \
    --allow-write
