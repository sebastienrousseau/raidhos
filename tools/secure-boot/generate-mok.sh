#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Generate a RaidhOS MOK (Machine Owner Key) keypair for signing
# the bundled GRUB EFI binary so it boots under Secure Boot after
# the user enrols the public key via mokutil + MokManager.
#
# Output:
#   <outdir>/RaidhOS-MOK.key   2048-bit RSA private key (PEM, no
#                              passphrase by default — pass --passphrase
#                              to encrypt).
#   <outdir>/RaidhOS-MOK.crt   X.509 self-signed cert (PEM).
#   <outdir>/RaidhOS-MOK.cer   Same cert in DER (this is the file
#                              `mokutil --import` consumes).
#
# Usage:
#   tools/secure-boot/generate-mok.sh [--outdir DIR] [--cn STRING]
#                                     [--passphrase PASS] [--days N]
#
# Threat model:
#   The private key gates Secure Boot for this RaidhOS install.
#   Store it offline (USB stick in a safe, GitHub Actions OIDC
#   key-management, an HSM, …) and never commit it to the repo.
#   The repo's `.gitignore` should already cover `*.key` and
#   `secure-boot-out/`. This script refuses to write a private key
#   inside a tracked git path unless --force is passed.
#
# Closes Ventoy gap G3 (Secure Boot with a user-enrolled key path).

set -euo pipefail

OUTDIR="${PWD}/secure-boot-out"
CN="RaidhOS Secure Boot"
DAYS=3650
PASSPHRASE=""
FORCE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --outdir)
            OUTDIR="$2"; shift 2 ;;
        --cn)
            CN="$2"; shift 2 ;;
        --passphrase)
            PASSPHRASE="$2"; shift 2 ;;
        --days)
            DAYS="$2"; shift 2 ;;
        --force)
            FORCE=1; shift ;;
        -h|--help)
            sed -n '2,30p' "$0"; exit 0 ;;
        *)
            echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

if ! command -v openssl >/dev/null 2>&1; then
    echo "error: openssl(1) is required and not on PATH" >&2
    exit 1
fi

# Refuse to drop a private key inside a tracked git working tree
# unless --force is given. Reduces accidental commits.
if [ "$FORCE" = "0" ] && git -C "$OUTDIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "error: $OUTDIR is inside a git working tree." >&2
    echo "       Re-run with --force if you really want this, but" >&2
    echo "       update .gitignore first to keep .key files out of the index." >&2
    exit 1
fi

mkdir -p "$OUTDIR"
chmod 700 "$OUTDIR"

KEY="$OUTDIR/RaidhOS-MOK.key"
CRT="$OUTDIR/RaidhOS-MOK.crt"
CER="$OUTDIR/RaidhOS-MOK.cer"

if [ -e "$KEY" ] || [ -e "$CRT" ] || [ -e "$CER" ]; then
    echo "error: output files already exist under $OUTDIR; refusing to overwrite." >&2
    exit 1
fi

echo "Generating RSA-2048 keypair in $OUTDIR/"
if [ -n "$PASSPHRASE" ]; then
    openssl req -newkey rsa:2048 -nodes -keyout "$KEY" \
        -new -x509 -sha256 -days "$DAYS" -subj "/CN=$CN/" -out "$CRT"
    # Encrypt the key in-place if a passphrase was supplied.
    openssl rsa -in "$KEY" -aes256 -passout "pass:$PASSPHRASE" -out "$KEY.enc"
    mv "$KEY.enc" "$KEY"
else
    openssl req -newkey rsa:2048 -nodes -keyout "$KEY" \
        -new -x509 -sha256 -days "$DAYS" -subj "/CN=$CN/" -out "$CRT"
fi
openssl x509 -in "$CRT" -outform DER -out "$CER"

chmod 600 "$KEY"
chmod 644 "$CRT" "$CER"

echo
echo "Done."
echo "  Private key : $KEY"
echo "  Certificate : $CRT"
echo "  DER cert    : $CER  ← pass this to 'mokutil --import' on the target host"
echo
echo "Next:"
echo "  1. Sign the GRUB binary:"
echo "       tools/secure-boot/sign-bootx64.sh \\"
echo "           --key  $KEY \\"
echo "           --cert $CRT \\"
echo "           --bin  payload/esp/EFI/BOOT/BOOTX64.EFI"
echo "  2. Distribute $CER alongside the RaidhOS release."
echo "  3. End user enrols on first boot:"
echo "       sudo mokutil --import RaidhOS-MOK.cer"
echo "     and confirms via MokManager after reboot (physical-presence prompt)."
