#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Dry-run the install pipeline. Every safety check runs; no
# bytes are written. Pass the device as the first argument
# (defaults to a sentinel that always fails on Linux).

set -euo pipefail

device="${1:-/dev/sdX}"

echo "$ raidhos-cli install --device $device --dry-run"
# The CLI defaults to --dry-run when --allow-write is not set,
# but we pass it explicitly for clarity.
raidhos-cli install --device "$device" --dry-run --wipe || rc=$?
rc=${rc:-0}

case "$rc" in
    0) echo "Dry-run completed successfully." ;;
    1) echo "Dry-run refused by a safety check (this is expected for /dev/sdX)." ;;
    *) echo "Unexpected exit code $rc" >&2; exit "$rc" ;;
esac
