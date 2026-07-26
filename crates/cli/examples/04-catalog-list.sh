#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Print the bundled distro catalog. Each entry has a slug
# (machine-friendly identifier) and metadata used by
# `catalog verify`.

set -euo pipefail

echo "$ raidhos-cli catalog list"
raidhos-cli catalog list

echo
echo "JSON form:"
raidhos-cli catalog list --json
