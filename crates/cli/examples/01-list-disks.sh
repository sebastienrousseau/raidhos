#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Enumerate physical disks visible to the current user, then
# pretty-print them as a table. The raw column-separated output
# is also written to disks.txt so downstream scripts can pipe it.

set -euo pipefail

echo "$ raidhos-cli list-disks"
raidhos-cli list-disks | tee disks.txt

echo
echo "Pretty-printed:"
raidhos-cli list-disks | column -t

echo
echo "JSON form:"
raidhos-cli list-disks --json
