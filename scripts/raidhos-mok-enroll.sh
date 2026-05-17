#!/usr/bin/env bash
# Helper for end users to enrol the RaidhOS Machine Owner Key (MOK)
# into their firmware so Secure Boot will accept a RaidhOS-signed
# BOOTX64.EFI. Runs once per machine.
#
# Usage:
#   sudo ./scripts/raidhos-mok-enroll.sh /path/to/MOK.crt
#
# After enrolment, reboot. The shim's MokManager will prompt to confirm
# the enrolment (this is intentional — it ensures a user is physically
# present at the keyboard, not a remote attacker).
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <MOK.crt>" >&2
    exit 64
fi

CERT=$1

if [[ $EUID -ne 0 ]]; then
    echo "error: must be run as root" >&2
    exit 1
fi

if ! command -v mokutil >/dev/null 2>&1; then
    echo "error: mokutil not found. Install on Debian/Ubuntu:" >&2
    echo "  sudo apt install mokutil" >&2
    echo "or on Fedora:" >&2
    echo "  sudo dnf install mokutil" >&2
    exit 1
fi

if [[ ! -s $CERT ]]; then
    echo "error: certificate $CERT missing" >&2
    exit 1
fi

# mokutil --import sets the password the user must type during
# MokManager at next boot. Prompt explicitly so the user is aware.
echo "You will set a one-time password now. At next reboot the firmware"
echo "MokManager screen will ask you to type it back to confirm the"
echo "enrolment. This is a physical-presence check — do not store the"
echo "password anywhere; type it from memory after reboot."
mokutil --import "$CERT"

echo
echo "Enrolment queued. Reboot now to complete it."
