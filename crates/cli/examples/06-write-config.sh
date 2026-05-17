#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Round-trip a saved boot.json into a mounted DATA partition.
# The mount must already exist (the install pipeline creates
# it; this command updates an existing image).
#
# Usage:
#   ./06-write-config.sh <mount-path> <config-path>

set -euo pipefail

mount="${1:-/mnt/raidhos-data}"
config="${2:-$HOME/.config/raidhos/boot.json}"

if [ ! -d "$mount" ]; then
    echo "Mount path $mount does not exist. Mount the DATA partition first." >&2
    exit 2
fi
if [ ! -f "$config" ]; then
    echo "No boot.json at $config. Generate one with the UI's Save action." >&2
    exit 2
fi

echo "$ raidhos-cli write-config --mount-path $mount --config-path $config"
raidhos-cli write-config --mount-path "$mount" --config-path "$config"
