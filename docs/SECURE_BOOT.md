# Secure Boot

## Short answer

**RaidhOS v0.0.1 does not support Secure Boot.** If your machine has
Secure Boot enabled (the default on most laptops shipped since 2020),
the USB stick will fail to boot with a "security policy" / "Verifying
shim SBAT" / "Selected boot image did not authenticate" error.

To boot a RaidhOS stick today, disable Secure Boot in firmware. Re-enable
it after you are done.

## Why it fails

The `BOOTX64.EFI` shipped by RaidhOS is produced by
`grub-mkstandalone` (`tools/grub/build_grub.sh`). The output is
**unsigned**. Firmware with Secure Boot on will only execute binaries
signed by a key in the platform key database — which means, in practice,
Microsoft's UEFI CA or a key the user has enrolled. The default RaidhOS
binary is neither.

This is not a bug, it is a missing feature. Closing it is a roadmap
item.

## The path to signed boot

There are three credible options, in increasing order of effort and
quality:

### Option 1 — Use the upstream signed shim (recommended)

The `shim-signed` package, maintained by the Debian and Ubuntu teams,
contains a small first-stage bootloader signed by Microsoft. It will
load a second-stage bootloader (typically `grubx64.efi`) verified
against a key the user has either pre-enrolled via MOK (Machine Owner
Key) or distributed in the shim's own database.

The plan:

1. Generate a RaidhOS signing keypair (cert + private key kept in HSM
   or in GitHub OIDC-bound signing).
2. Ship the public cert alongside binaries and in a small enrolment
   helper.
3. Embed the cert into a copy of `shim` at build time, or distribute it
   for `mokutil --import`.
4. Sign every released `BOOTX64.EFI` with the RaidhOS key.
5. First boot enters `MokManager`; user enrols the key once; subsequent
   boots are silent.

Trade-off: users have to enrol the key once. Acceptable. Industry
standard (this is exactly the path NixOS, Pop!_OS, and most third-party
boot images use).

### Option 2 — Bundle a known-good signed shim from another project

For example, bundle Ventoy's published, Microsoft-CA-signed shim
unchanged. Faster to ship, but it ties RaidhOS to an external project's
release cadence and key-rotation policy. Reasonable bridge, not a
destination.

### Option 3 — Pursue Microsoft UEFI CA signing for the RaidhOS shim itself

Microsoft will sign third-party shims that meet specific requirements
(SBAT entries, no-revoke-list, etc.). Process takes weeks to months. Pay
off is highest: zero-touch Secure Boot for users.

We will land Option 1 first, evaluate Option 3 in parallel.

## Roadmap

- **v0.0.2**: document Option 1 in detail; provide a manual signing
  script that takes a user-supplied key.
- **v0.0.3**: publish a RaidhOS signing key; ship a signed
  `BOOTX64.EFI`; ship a MOK-enrolment helper.
- **v1.0**: pursue Microsoft UEFI CA signing.

## Verifying the unsigned binary

In the meantime, the integrity of the unsigned `BOOTX64.EFI` is
recoverable. Releases include:

- A SHA-256 over the binary.
- A cosign keyless signature (via GitHub OIDC) over the SHA-256.
- A SLSA L3 provenance attestation showing the GitHub Actions run that
  produced the artefact.

So while you cannot verify under Secure Boot, you can verify out-of-band
that the file you have is the file we built. That is not a substitute
for signed boot, but it is a meaningful integrity guarantee.

## Why not "just" use grub2-signed?

`grub2-signed` exists in Debian and Ubuntu repos, but it is signed
against a specific bootloader configuration and revocation timeline that
those distributions own. Embedding it in a third-party project tends to
break across kernel / GRUB updates and SBAT revisions. Option 1 above
gives us full control without locking us to any distro's release.

## Disabling Secure Boot — step by step

If you need to boot a RaidhOS stick today:

1. Save your work and reboot.
2. Enter firmware setup (usually Del, F2, F10, or F12 immediately after
   power-on — vendor-specific).
3. Find **Secure Boot** under a `Security`, `Boot`, or `Authentication`
   menu.
4. Set it to **Disabled** (some firmwares offer **Setup Mode** instead;
   both work for our purposes).
5. Save and exit.
6. Boot the USB from the firmware boot menu.
7. **Re-enable Secure Boot** once you are done — it protects you from a
   real class of attacks.

If your firmware refuses to let you disable Secure Boot (some
enterprise-locked laptops): RaidhOS does not currently have a workaround
for v0.0.1.
