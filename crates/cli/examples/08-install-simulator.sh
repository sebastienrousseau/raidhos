#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Non-destructive install preview against a sparse file.
#
# Same install pipeline as a real USB stick; the destruction
# happens to a file the user owns. After this script finishes
# you have a GPT-formatted disk image at /tmp/raidhos-sim.img
# that you can inspect, mount via losetup -fP, or throw away.
#
# This is the safe answer to "I want to see what it does before
# I trust it with /dev/sdX".

set -euo pipefail

target="${1:-/tmp/raidhos-sim.img}"
size_mb="${2:-1024}"

echo "$ raidhos-cli install --simulator $target --simulator-size-mb $size_mb --allow-write"
raidhos-cli install \
    --simulator "$target" \
    --simulator-size-mb "$size_mb" \
    --allow-write

echo
echo "Done. The image is at $target."
echo
echo "Inspect:"
echo "  parted -m $target print"
echo
echo "Attach as a loop device (Linux):"
echo "  sudo losetup -fP $target"
echo "  ls /dev/loop*p*"
echo "  sudo umount /dev/loopXpY    # when finished"
echo "  sudo losetup -d /dev/loopX"
echo
echo "Or attach on macOS:"
echo "  hdiutil attach -nomount $target"
echo
echo "Or just delete it:"
echo "  rm $target"
