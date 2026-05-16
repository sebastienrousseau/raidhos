# FAQ

### Why does RaidhOS exist when Ventoy already does this?

Ventoy is excellent and battle-tested, but its core is C with bespoke
boot-time code. RaidhOS rebuilds the same idea on a memory-safe core
(`unsafe_code = "forbid"` workspace-wide), with a reproducible build
pipeline, an explicit threat model, and a small attack surface — about
2,000 lines of Rust for the core library. The aim is "the USB imager
you can audit", not "the USB imager with the most features."

### Is RaidhOS production-ready?

For Linux power users: yes, with the caveat that v0.0.1 is the first
tagged release and you should still keep a backup of anything important
on the target USB. For macOS and Windows users: not yet — discovery
works but the install pipeline is stubbed.

### What licence is RaidhOS under?

GPL-3.0-only. See `LICENSE`.

### Does it support Secure Boot?

Not yet. See `docs/SECURE_BOOT.md` for current state and roadmap.

### Does it support persistence?

Not yet. Persistence images are a v1 roadmap item.

### Does it support BIOS / legacy boot?

Not yet. `BOOTX64.EFI` is UEFI-only. A `--legacy` flag is planned.

### What ISOs are supported?

Anything with a `casper/` (Debian/Ubuntu live) or `live/` (Debian Live)
layout works out of the box. ISOs that ship their own `boot/grub/grub.cfg`
on the ISO root are also picked up via `configfile`. Other layouts will
print "No known kernel path found in ISO." — open an issue with the ISO
name and we will add the path.

### Why does it refuse to operate on my disk?

A device is refused if:

- it is mounted at `/`, `/boot`, or `/boot/efi`,
- the platform-discovery code flags it as internal/system,
- any of its partitions is mounted,
- the device path is empty, longer than 256 chars, contains shell
  metacharacters, or contains `..`,
- the device path does not match the per-OS shape (`/dev/...` on Linux,
  `/dev/diskN` on macOS, `\\.\PhysicalDriveN` on Windows).

You can read the exact rules in `crates/core/src/lib.rs:validate_device_path`.

### Why two opt-ins (`wipe` and `allow_write`)?

Defence in depth. `wipe` says "yes, I expect this to be destructive."
`allow_write` says "yes, actually do it now, this is not a dry-run."
Both are required before any byte is written. The CLI defaults dry-run
to true and `allow_write` to false.

### Is there telemetry?

No. RaidhOS does not phone home. The only network traffic the binary
makes is whatever you ask it to (e.g. downloading an ISO yourself
outside the tool).

### How do I report a security issue?

Please do not open a public issue. See `SECURITY.md` for the private
reporting path.

### Will it run on Apple Silicon?

Yes for discovery. The install pipeline is Linux-only in v0.0.1.

### Will it ever run on FreeBSD / OpenBSD / NetBSD?

Eventually, but not in v0.0.1. The Linux/macOS/Windows platforms come
first.

### How does RaidhOS compare to balenaEtcher / Rufus / UNetbootin?

| Capability                | RaidhOS     | Ventoy | Etcher | Rufus | UNetbootin |
| ------------------------- | ----------- | ------ | ------ | ----- | ---------- |
| Multi-ISO on one stick    | yes (Linux) | yes    | no     | no    | no         |
| Memory-safe core          | yes (Rust)  | no     | mixed  | no    | no         |
| Reproducible build        | yes (GRUB)  | no     | no     | no    | no         |
| Cross-platform install    | partial     | yes    | yes    | Win   | yes        |
| Secure Boot               | not yet     | yes    | yes    | yes   | no         |
| Persistence               | not yet     | yes    | no     | yes   | no         |
| Free / open source        | GPL-3.0     | GPL-3  | Apache | GPL-3 | GPL-2      |

RaidhOS wins on auditability and supply-chain story. It loses on
breadth-of-features and platform parity. That gap is what v0.0.1 is the
start of closing.

### Where do I send feedback?

GitHub issues for bugs and feature requests. Security issues via the
process in `SECURITY.md`.
