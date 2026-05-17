#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Demonstrate the helper's JSON output shape. The contract is:
# exactly one JSON object on stdout, regardless of success or
# failure. Progress messages go to stderr.

set -euo pipefail

echo "=== Success path (list-disks) ============================"
echo "$ raidhos-priv-helper list-disks"
raidhos-priv-helper list-disks | jq '{ok, data: (.data | length), error}'

echo
echo "=== Validation refusal (bad device path) ================="
echo "$ raidhos-priv-helper install --device 'rm -rf /'"
raidhos-priv-helper install --device 'rm -rf /' || rc=$?
echo "exit code: ${rc:-0}"

echo
echo "=== Argv parse error (unknown subcommand) ================"
echo "$ raidhos-priv-helper definitely-not-a-real-subcommand"
raidhos-priv-helper definitely-not-a-real-subcommand || rc=$?
echo "exit code: ${rc:-0}"
