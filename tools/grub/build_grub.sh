#!/bin/sh
set -e

ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
BUILD_DIR="$ROOT_DIR/build/boot/grub"
OUT_EFI="$ROOT_DIR/BOOTX64.EFI"

mkdir -p "$BUILD_DIR"
cp "$ROOT_DIR/tools/grub/grub.cfg" "$BUILD_DIR/grub.cfg"

docker build -t raidhos-grub-compiler "$ROOT_DIR/tools/grub"

docker run --rm -v "$ROOT_DIR":/output raidhos-grub-compiler \
  -d /grub-build/grub-core \
  -O x86_64-efi \
  -o /output/BOOTX64.EFI \
  "boot/grub/grub.cfg=build/boot/grub/grub.cfg" \
  fat part_gpt part_msdos normal search \
  iso9660 loopback configfile test video all_video

echo "Generated $OUT_EFI"
