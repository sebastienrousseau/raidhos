#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Probe the validate_device_path() gate. The CLI exposes the
# gate as a standalone subcommand so wrappers and CI scripts
# can sanity-check user input before constructing an
# InstallRequest.

set -euo pipefail

run() {
    local label="$1" device="$2"
    printf "%-32s " "$label:"
    if raidhos-cli validate-device "$device" >/dev/null 2>&1; then
        echo "accepted"
    else
        echo "refused"
    fi
}

run "valid linux"          "/dev/sdb"
run "valid linux nvme"     "/dev/nvme0n1"
run "valid macOS"          "/dev/disk2"
run "valid windows"        '\\.\PhysicalDrive1'
run "empty"                ""
run "shell metachar (semi)" "/dev/sdb;rm -rf /"
run "shell metachar (pipe)" "/dev/sdb|cat"
run "path traversal"        "/dev/../etc/passwd"
run "missing /dev prefix"   "sdb"
run "overlong"              "/dev/$(printf 'x%.0s' $(seq 1 300))"
