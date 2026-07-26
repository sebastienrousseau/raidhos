#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# List partitions on a specific disk. Pass the device path as
# the first argument.
#
# Usage: ./02-list-partitions.sh /dev/sdb

set -euo pipefail

device="${1:-/dev/sdb}"

echo "$ sudo raidhos-priv-helper list-partitions $device"
sudo raidhos-priv-helper list-partitions "$device" | jq .
