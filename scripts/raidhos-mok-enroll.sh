#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# End-user helper: enrol a RaidhOS MOK certificate so the signed
# BOOTX64.EFI boots under Secure Boot. Run this **on the target
# machine** (not inside a RaidhOS chroot) with the RaidhOS USB
# attached or with the .cer file present locally.
#
# Usage:
#   sudo scripts/raidhos-mok-enroll.sh [--cer PATH]
#
# Default `--cer` lookup, in order:
#   1. /run/media/$USER/RAIDHOS_EFI/RaidhOS-MOK.cer
#   2. /media/$USER/RAIDHOS_EFI/RaidhOS-MOK.cer
#   3. ./RaidhOS-MOK.cer
#
# After import, reboot and follow the MokManager prompts:
#   1. "Press any key to perform MOK management"
#   2. "Enroll MOK"
#   3. "View key 0"  → confirm CN=RaidhOS Secure Boot
#   4. "Continue"  → "Yes"
#   5. Enter the one-time password the script printed
#   6. "Reboot"
#
# This is a one-time setup per machine. The enrolled key persists
# in the firmware until explicitly removed via `mokutil --delete`.
#
# Closes Ventoy gap G3 (Secure Boot signed shim/GRUB) on the
# end-user side; the signing side lives in
# `tools/secure-boot/{generate-mok,sign-bootx64}.sh`.

set -euo pipefail

CER=""
while [ $# -gt 0 ]; do
    case "$1" in
        --cer) CER="$2"; shift 2 ;;
        -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [ "$(id -u)" -ne 0 ]; then
    echo "error: this script must run as root (sudo)." >&2
    exit 1
fi

if ! command -v mokutil >/dev/null 2>&1; then
    echo "error: mokutil(1) not found. Install the shim-signed package:" >&2
    echo "  Debian/Ubuntu : sudo apt-get install mokutil" >&2
    echo "  Fedora        : sudo dnf install mokutil" >&2
    echo "  Arch          : sudo pacman -S mokutil" >&2
    exit 1
fi

if [ -z "$CER" ]; then
    user="${SUDO_USER:-$USER}"
    for cand in \
        "/run/media/$user/RAIDHOS_EFI/RaidhOS-MOK.cer" \
        "/media/$user/RAIDHOS_EFI/RaidhOS-MOK.cer" \
        "./RaidhOS-MOK.cer"; do
        if [ -r "$cand" ]; then
            CER="$cand"
            break
        fi
    done
fi

if [ -z "$CER" ] || [ ! -r "$CER" ]; then
    echo "error: RaidhOS-MOK.cer not found." >&2
    echo "       Pass --cer PATH explicitly or copy the cert to the working directory." >&2
    exit 1
fi

echo "Found certificate: $CER"
if command -v openssl >/dev/null 2>&1; then
    openssl x509 -inform DER -in "$CER" -noout -subject -fingerprint -dates -sha256 \
        2>/dev/null || true
fi
echo

echo "Submitting to mokutil. You'll be asked for a one-time password —"
echo "you'll need to type it in the firmware MokManager on the next boot."
echo "This is a physical-presence check — do not store the password anywhere."
echo
mokutil --import "$CER"

echo
echo "Enrolment request queued. Reboot, follow the MokManager prompts,"
echo "and the next boot of BOOTX64.EFI will pass Secure Boot verification."
echo
echo "If you change your mind: 'sudo mokutil --delete RaidhOS-MOK.cer'"
