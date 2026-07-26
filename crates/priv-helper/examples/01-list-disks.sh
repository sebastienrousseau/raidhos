#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Inventory disks visible to root, via the privileged helper.
# Output is a single JSON object on stdout; jq pretty-prints it
# for human reading.

set -euo pipefail

echo "$ sudo raidhos-priv-helper list-disks"
sudo raidhos-priv-helper list-disks | jq .
