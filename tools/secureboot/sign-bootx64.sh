#!/usr/bin/env bash
# Sign BOOTX64.EFI with a user-supplied RaidhOS signing key, producing
# a `BOOTX64.signed.efi` next to the input.
#
# Requires sbsigntool (`sbsign`) and a PEM/DER keypair. Generate one with:
#   openssl req -newkey rsa:2048 -nodes -keyout MOK.key -new -x509 \
#     -sha256 -days 3650 -subj "/CN=RaidhOS MOK/" -out MOK.crt
#
# Usage:
#   tools/secureboot/sign-bootx64.sh <input.efi> <MOK.crt> <MOK.key> [out]
set -euo pipefail

if [[ $# -lt 3 ]]; then
    echo "usage: $0 <input.efi> <MOK.crt> <MOK.key> [out.efi]" >&2
    exit 64
fi

INPUT=$1
CERT=$2
KEY=$3
OUT=${4:-${INPUT%.efi}.signed.efi}

if ! command -v sbsign >/dev/null 2>&1; then
    echo "error: sbsign not found. Install sbsigntool / sbsigntools." >&2
    exit 1
fi

if [[ ! -s $INPUT ]]; then
    echo "error: $INPUT missing or empty" >&2
    exit 1
fi
if [[ ! -s $CERT ]]; then
    echo "error: certificate $CERT missing" >&2
    exit 1
fi
if [[ ! -s $KEY ]]; then
    echo "error: key $KEY missing" >&2
    exit 1
fi

sbsign --key "$KEY" --cert "$CERT" --output "$OUT" "$INPUT"
echo "signed: $OUT"

if command -v sbverify >/dev/null 2>&1; then
    sbverify --cert "$CERT" "$OUT"
fi
