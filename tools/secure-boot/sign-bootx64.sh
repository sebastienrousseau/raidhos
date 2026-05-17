#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Sign a UEFI binary (typically `payload/esp/EFI/BOOT/BOOTX64.EFI`)
# with a RaidhOS MOK key produced by `generate-mok.sh`. The signed
# binary boots under Secure Boot once the user has enrolled the
# matching DER certificate via mokutil.
#
# Usage:
#   tools/secure-boot/sign-bootx64.sh \
#       --key  secure-boot-out/RaidhOS-MOK.key \
#       --cert secure-boot-out/RaidhOS-MOK.crt \
#       --bin  payload/esp/EFI/BOOT/BOOTX64.EFI
#
# Optional:
#   --out  PATH      Write the signed binary to PATH instead of
#                    overwriting --bin (default: in-place).
#
# Requires `sbsign(1)` from sbsigntools. On Debian/Ubuntu:
#   sudo apt-get install sbsigntool
# On Fedora:
#   sudo dnf install sbsigntools
# On Arch:
#   sudo pacman -S sbsigntools
#
# Closes Ventoy gap G3 (Secure Boot signed shim/GRUB).

set -euo pipefail

KEY=""
CERT=""
BIN=""
OUT=""

while [ $# -gt 0 ]; do
    case "$1" in
        --key)  KEY="$2";  shift 2 ;;
        --cert) CERT="$2"; shift 2 ;;
        --bin)  BIN="$2";  shift 2 ;;
        --out)  OUT="$2";  shift 2 ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$KEY" ] || [ -z "$CERT" ] || [ -z "$BIN" ]; then
    echo "usage: $0 --key KEY --cert CRT --bin BOOTX64.EFI [--out PATH]" >&2
    exit 2
fi

if ! command -v sbsign >/dev/null 2>&1; then
    echo "error: sbsign(1) not found. Install sbsigntools." >&2
    exit 1
fi

for f in "$KEY" "$CERT" "$BIN"; do
    [ -r "$f" ] || { echo "error: $f not readable" >&2; exit 1; }
done

DEST="${OUT:-$BIN}"
TMP=$(mktemp -t raidhos-sign-XXXXXX.efi)
trap 'rm -f "$TMP"' EXIT

echo "Signing $BIN with $CERT → $DEST"
sbsign --key "$KEY" --cert "$CERT" --output "$TMP" "$BIN"

# sbverify confirms the signature attached cleanly. Non-fatal if
# sbverify is missing.
if command -v sbverify >/dev/null 2>&1; then
    sbverify --cert "$CERT" "$TMP"
fi

mv "$TMP" "$DEST"
echo "Signed: $DEST"
