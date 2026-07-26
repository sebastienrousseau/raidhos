#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Walk one or more directories for *.iso files (one level deep).
# Each found ISO appears with title, byte size, and the params
# string that would be passed to its GRUB menu entry.

set -euo pipefail

dirs="${*:-$HOME/Downloads /media}"

echo "$ raidhos-cli scan-isos --dirs $(echo "$dirs" | tr ' ' ',')"
raidhos-cli scan-isos --dirs "$(echo "$dirs" | tr ' ' ',')"
