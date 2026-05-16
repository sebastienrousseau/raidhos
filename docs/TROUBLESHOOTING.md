# Troubleshooting

If you hit something not covered here, please open an issue with the
`raidhos-cli list-disks` output and the exact command you ran.

## Install fails immediately with "device not found"

The device path you passed (`--device /dev/sdX`) is not in the list of
discovered disks. Causes:

- The USB has been unplugged or remounted; re-run `list-disks`.
- You typed the wrong letter (`/dev/sdb` vs `/dev/sdc`).
- Your USB is masked behind a hub that does not report on hotplug; try a
  direct port.

## "refusing to operate on system disk"

The disk you targeted is either mounted at `/`, `/boot`, or `/boot/efi`,
or is flagged as internal by the platform discovery code. This is by
design. Confirm the USB device path with `lsblk` or `diskutil list`.

If you believe the detection is wrong (e.g. an external SSD reported as
internal on macOS), open an issue with the full discovery output.

## "device has mounted partitions; unmount first"

Auto-mounting is in your way:

```bash
sudo udisksctl unmount -b /dev/sdX1
sudo udisksctl unmount -b /dev/sdX2
```

Or, more bluntly:

```bash
sudo umount /dev/sdX?*
```

Then re-run the install.

## pkexec opens, you authenticate, the install still fails

The privileged helper prints its error on stderr; the UI surfaces only
the JSON `error` field. Run the helper directly to see the full output:

```bash
sudo RAIDHOS_PAYLOAD_DIR=/path/to/payload \
  ./target/release/raidhos-priv-helper install \
  --device /dev/sdX \
  --allow-write
```

Common causes:

- `RAIDHOS_PAYLOAD_DIR` not exported across the `sudo` boundary (use
  `sudo -E` or set the variable inline as above).
- `mkfs.exfat` or `mkexfatfs` missing; install `exfatprogs` (Debian,
  Ubuntu, Fedora) or `exfat-utils` (Arch).
- The polkit policy is not installed; copy
  `packaging/linux/org.raidhos.policy` into `/usr/share/polkit-1/actions/`
  (or invoke the helper with `sudo` instead of `pkexec`).

## "exFAT formatter not found"

Install one of the exFAT toolchains:

| Distro              | Package                   |
| ------------------- | ------------------------- |
| Debian, Ubuntu      | `exfatprogs`              |
| Arch                | `exfatprogs`              |
| Fedora              | `exfatprogs`              |
| Older (deprecated)  | `exfat-utils`             |

## The Tauri UI does not start

```text
error while running RaidhOS
```

- Confirm the OS-specific Tauri deps are installed (run the bootstrap
  script for your distro again).
- On Wayland sessions, run with `WEBKIT_DISABLE_DMABUF_RENDERER=1
  ./target/release/raidhos-ui` if you see a black window.
- Run with `RUST_LOG=trace` for verbose output.

## "RAIDHOS_PAYLOAD_DIR is not set"

The privileged helper requires this environment variable to know where
to copy ESP and DATA contents from. Set it on the same command line:

```bash
sudo RAIDHOS_PAYLOAD_DIR=/srv/raidhos/payload-1.1.10 \
  ./target/release/raidhos-priv-helper install ...
```

Either `sudo -E` or inline assignment will preserve the variable; a
plain `sudo` will strip it.

## My system has Secure Boot and the stick will not boot

This is expected. See `docs/SECURE_BOOT.md`. Workaround for now:

1. Reboot and enter UEFI/firmware setup.
2. Disable Secure Boot (or set it to Setup Mode).
3. Save and exit.
4. Boot the USB from the firmware menu (F12/F11/F10/ESC).

Secure Boot support is on the roadmap.

## A specific distro ISO will not boot

The GRUB config in `crates/ui-tauri/src-tauri/src/grub.rs` recognises
`casper/` (Debian/Ubuntu live) and `live/` (Debian Live). Other layouts
fall through to `"No known kernel path found in ISO."`. Open an issue
with the ISO name and the output of:

```bash
isoinfo -i your.iso -l | head -30
```

We will add the layout in the next release.

## Coverage gate failure on PR (CI)

`cargo tarpaulin -p raidhos-core --fail-under 95` is strict by design.
If you added code without tests, the gate will block the PR. Add unit
tests for the new code in `crates/core/src/lib.rs` (or its
`platform/{linux,macos,windows}.rs` submodules) and re-push.

## I want to revert a USB stick to a normal data partition

```bash
sudo wipefs -a /dev/sdX
sudo parted /dev/sdX -s mklabel msdos
sudo parted /dev/sdX -s mkpart primary fat32 1MiB 100%
sudo mkfs.vfat -F 32 /dev/sdX1
```
