# RaidhOS user guide

A walkthrough of installing your first multi-ISO USB with RaidhOS.

> **Linux only for v0.0.1.** macOS and Windows can list disks but cannot
> write to them yet. See `docs/V0_0_1_STATUS.md` for the full picture.

## Before you start

You will need:

- A USB stick of **at least 16 GB** (32 GB if you want several large ISOs).
- One or more Linux ISOs already downloaded to your machine.
- About 5 GB of free disk space for the GRUB build artefacts.
- Root or `sudo` access on the host machine.

RaidhOS will refuse to operate on your system disk, on any disk mounted at
`/`, `/boot`, or `/boot/efi`, and on any disk whose partitions are
currently mounted. You should still **back up anything important on the
USB stick** — the install is destructive by design.

## Step 1 — Install the system prerequisites

Pick the bootstrap script for your distro. Each is idempotent and only
installs what is missing.

```bash
./scripts/bootstrap-debian.sh    # Debian, Ubuntu
./scripts/bootstrap-arch.sh      # Arch, Manjaro
./scripts/bootstrap-fedora.sh    # Fedora, RHEL-likes
```

## Step 2 — Build RaidhOS

```bash
# Builds the CLI, the core library, and the privileged helper.
# Skips the Tauri UI so you do not need GTK/WebKit installed yet.
cargo build --release --workspace --exclude raidhos-ui
```

The binaries land in `target/release/`.

## Step 3 — Build the GRUB EFI loader

The first time only, build the reproducible `BOOTX64.EFI`. This requires
Docker.

```bash
./tools/grub/build_grub.sh
```

The resulting `BOOTX64.EFI` appears at the repository root.

## Step 4 — Inspect your candidate disks

```bash
./target/release/raidhos-cli list-disks
```

You will see one line per physical disk: the device path, model, size in
bytes, removable flag, system flag, and a comma-separated list of
mountpoints. Confirm that your USB stick is listed, that `removable=true`,
and that `system=false`.

If your USB is not listed at all, unplug it, wait two seconds, plug it
back in, and re-run.

## Step 5 — Dry-run the install

Always start with a dry-run. It performs every safety check but writes
nothing.

```bash
./target/release/raidhos-cli install --device /dev/sdX --dry-run
```

If the dry-run completes with no error you are good to proceed.

## Step 6 — Real install

```bash
sudo RAIDHOS_PAYLOAD_DIR=/path/to/payload \
  ./target/release/raidhos-priv-helper install \
  --device /dev/sdX \
  --allow-write
```

The helper will:

1. Validate the device path against the per-OS allowlist.
2. Refuse to continue if the disk is mounted or marked as system.
3. Create a GPT partition table.
4. Lay down a small `RAIDHOS_EFI` (FAT32) partition and a `DATA` (exFAT)
   partition that fills the rest of the stick.
5. Copy the payload from `RAIDHOS_PAYLOAD_DIR` into both partitions.
6. Write the generated `grub.cfg`.

A 16 GB stick on USB 3.0 finishes in 2–5 minutes depending on payload
size and your kernel.

## Step 7 — Copy ISOs onto the stick

Once the install finishes, mount the `DATA` partition and drop ISOs into
`boot/isos/`:

```bash
sudo mkdir -p /mnt/raidhos-data
sudo mount /dev/sdX2 /mnt/raidhos-data
sudo cp ~/Downloads/ubuntu-24.04-desktop-amd64.iso \
        /mnt/raidhos-data/boot/isos/
sudo umount /mnt/raidhos-data
```

The CLI also exposes `scan-isos` if you want to inventory ISOs on your
machine before copying.

## Step 8 — Boot from the stick

Reboot, enter your firmware's boot menu (usually F12, F11, F10, or ESC
during POST), and pick the USB device. GRUB will present a menu generated
from the ISOs it finds in `boot/isos/`.

If your machine has **Secure Boot enabled**, boot will fail. See
`docs/SECURE_BOOT.md` for the current state and the workaround.

## Using the Tauri desktop UI

The UI is an alternative front-end for the same operations. Build it
with:

```bash
cargo build --release --workspace
./target/release/raidhos-ui
```

The UI shows disks, lets you pick an install target, asks you to type
`ERASE` to confirm, and runs the privileged helper through `pkexec`.

The UI is still spartan in v0.0.1 — feature-complete with the CLI, but
not polished. Power users will probably stay on the CLI.

## What next

- Edit `~/.config/raidhos/boot.json` to customise the GRUB menu (kernel
  parameters, default entry, etc.).
- Open an issue if a distro's ISO does not boot — boot heuristics are
  still narrow.
- See `docs/TROUBLESHOOTING.md` if anything fails.
