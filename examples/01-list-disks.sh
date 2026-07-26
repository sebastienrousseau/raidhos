#!/usr/bin/env bash
# Example 01 — `list-disks`.
#
# Discover physical disks visible to the current user. No
# privileges needed; the tool refuses to list anything mounted at
# /, /boot, or /boot/efi anyway.
set -euo pipefail

if ! command -v raidhos-cli >/dev/null 2>&1; then
    echo "error: raidhos-cli not on PATH; install per docs/INSTALL.md" >&2
    exit 1
fi

# Raw output:
echo "== Raw output =="
raidhos-cli list-disks
echo

# Human-friendly:
echo "== Aligned =="
raidhos-cli list-disks | column -t

# Filter to removable, non-system:
echo
echo "== Removable only =="
raidhos-cli list-disks | awk '/removable=true/ && /system=false/ { print }'
