#!/usr/bin/env bash
# Example 06 — enrol a RaidhOS MOK key for Secure Boot.
#
# Run **once per machine**. After enrolment and reboot, Secure
# Boot will accept a RaidhOS-signed BOOTX64.EFI.
#
# This example wraps scripts/raidhos-mok-enroll.sh — see
# docs/SECURE_BOOT.md for the full path to signed boot.
set -euo pipefail

CERT=${1:-}
if [[ -z $CERT ]]; then
    echo "usage: $0 <RaidhOS-MOK.crt>" >&2
    exit 2
fi
if [[ ! -s $CERT ]]; then
    echo "error: certificate $CERT missing or empty" >&2
    exit 1
fi
if ! command -v mokutil >/dev/null 2>&1; then
    echo "error: mokutil not on PATH; install it from your distro" >&2
    exit 1
fi

# Locate the enrolment helper. Prefer the package-installed copy
# if present.
HELPER=""
for cand in \
    /usr/share/raidhos/scripts/raidhos-mok-enroll.sh \
    /usr/local/share/raidhos/scripts/raidhos-mok-enroll.sh \
    "$(dirname "$0")/../scripts/raidhos-mok-enroll.sh"; do
    if [[ -x $cand ]]; then
        HELPER=$cand
        break
    fi
done
if [[ -z $HELPER ]]; then
    echo "error: raidhos-mok-enroll.sh not found" >&2
    exit 1
fi

echo "Enrolling $CERT via $HELPER …"
echo "You will be prompted for a one-time password."
echo "Reboot after this command completes; MokManager will ask you"
echo "to type the password back to confirm."

sudo "$HELPER" "$CERT"
