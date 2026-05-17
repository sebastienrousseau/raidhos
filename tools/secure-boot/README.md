<!-- SPDX-License-Identifier: GPL-3.0-only -->

# RaidhOS Secure Boot signing

This directory holds the **signing-side** of RaidhOS's Secure
Boot story. The complementary end-user helper that enrols the
key on the target machine lives at
[`scripts/raidhos-mok-enroll.sh`](../../scripts/raidhos-mok-enroll.sh).

| Script | Purpose | Run on |
|---|---|---|
| [`generate-mok.sh`](generate-mok.sh) | Generate a RaidhOS-MOK keypair (RSA-2048, X.509 + DER). | The release-signing host. **Private key stays here.** |
| [`sign-bootx64.sh`](sign-bootx64.sh) | Sign `BOOTX64.EFI` with the MOK key (uses `sbsign`). | Same release-signing host. |
| [`../../scripts/raidhos-mok-enroll.sh`](../../scripts/raidhos-mok-enroll.sh) | `mokutil --import` the DER cert. | The end user's machine, once per install. |

## Quick start (release maintainer)

```bash
# 1. Generate the keypair (one-time, ever — re-use across releases).
./tools/secure-boot/generate-mok.sh \
    --outdir ~/.raidhos/secure-boot \
    --cn "RaidhOS Secure Boot v1" \
    --passphrase "use-a-real-passphrase-here"

# 2. Sign the GRUB EFI binary produced by tools/grub/build_grub.sh.
./tools/secure-boot/sign-bootx64.sh \
    --key  ~/.raidhos/secure-boot/RaidhOS-MOK.key \
    --cert ~/.raidhos/secure-boot/RaidhOS-MOK.crt \
    --bin  payload/esp/EFI/BOOT/BOOTX64.EFI

# 3. Ship the .cer alongside the release tarball / on the USB
#    (typically copied into the ESP partition).
cp ~/.raidhos/secure-boot/RaidhOS-MOK.cer payload/esp/RaidhOS-MOK.cer
```

## Quick start (end user)

```bash
# After flashing the USB with RaidhOS, on the target machine:
sudo scripts/raidhos-mok-enroll.sh
# … or, if the USB isn't mounted at a standard path:
sudo scripts/raidhos-mok-enroll.sh --cer /path/to/RaidhOS-MOK.cer

# Reboot → MokManager prompts → enrol the key (physical presence).
# Now BOOTX64.EFI verifies under Secure Boot.
```

## Why MOK, not Microsoft-signed shim?

Ventoy ships a Microsoft-CA-signed shim that loads its own
signing key — this works without per-user enrolment but means
the shim is trusted by every Secure-Boot machine on Earth, so a
compromise of Ventoy's signing flow is a major incident.

RaidhOS chooses the **opposite** trade-off: nothing is
pre-trusted by firmware; users explicitly enrol the RaidhOS
public key on each machine via MokManager (with a
physical-presence prompt). The setup is one extra step on first
install; the security model is dramatically narrower.

A future v0.0.3 may add a Microsoft-shim-review path
(`shim --vendor-cert` + Microsoft attestation, ~6 months
turnaround) for users who can't go through MokManager (server
fleets, kiosks). Tracked as Ventoy gap G3 → v0.0.3 in
[`docs/VENTOY_COMPARISON.md`](../../docs/VENTOY_COMPARISON.md).

## Threat model

The MOK private key is a **single point of compromise** for
Secure Boot on every machine that's enrolled it. Treat it like
a code-signing certificate:

- Store the `.key` file offline (encrypted USB, HSM, GitHub
  Actions OIDC + KMS).
- The `.key` must **never** appear in this repository. The
  `generate-mok.sh` script refuses to write a private key
  inside a tracked git worktree.
- Pass `--passphrase` to encrypt the key at rest.
- Rotate the key on suspected compromise. Users will need to
  re-enrol the new public cert via MokManager.

For the workspace-level threat model see
[`docs/THREAT_MODEL.md`](../../docs/THREAT_MODEL.md).

## See also

- [`docs/SECURE_BOOT.md`](../../docs/SECURE_BOOT.md) — the full
  Secure Boot architecture and rationale.
- [`docs/VENTOY_COMPARISON.md`](../../docs/VENTOY_COMPARISON.md)
  — G3 (Secure Boot) gap status and roadmap mapping.
- [`docs/HARDENING.md`](../../docs/HARDENING.md) — every
  shipping security control with `file:line` citations.
