<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Arch / CachyOS / Manjaro / EndeavourOS packaging

This directory contains two PKGBUILDs published to the AUR:

| Package | Builds from | Use case |
|---|---|---|
| [`PKGBUILD`](PKGBUILD) | Source tarball at `v$pkgver` | Standard install. Builds with the local rustup toolchain. |
| [`PKGBUILD.bin`](PKGBUILD.bin) | Pre-built signed release tarball | Fast install. Skips the Rust toolchain dependency. |

Both produce the same on-disk layout:

```text
/usr/bin/raidhos-cli
/usr/libexec/raidhos-priv-helper
/usr/share/polkit-1/actions/org.raidhos.priv.policy
/usr/share/man/man1/raidhos-cli.1
/usr/share/bash-completion/completions/raidhos-cli
/usr/share/zsh/site-functions/_raidhos-cli
/usr/share/fish/vendor_completions.d/raidhos-cli.fish
/usr/share/raidhos/catalog/catalog.json
/usr/share/raidhos/catalog/keys/*.asc
/usr/share/licenses/raidhos/LICENSE
```

## Install for end users

Three paths, in order of "least new machinery":

```bash
# 1. Straight pacman against the release .pkg.tar.zst.
#    No AUR helper, no makepkg, nothing to clone.
curl -LO https://github.com/sebastienrousseau/raidhos/releases/download/v0.0.1/raidhos-bin-0.0.1-1-x86_64.pkg.tar.zst
sudo pacman -U raidhos-bin-0.0.1-1-x86_64.pkg.tar.zst
```

```bash
# 2. AUR — same package, fetched and built by your AUR helper.
yay -S raidhos-bin     # pre-built binary, cosign-verified
yay -S raidhos         # build from source under your rustup
# Or paru, pikaur, pacstall — pick your favourite helper.
```

```bash
# 3. makepkg from this repo (developers / packagers).
makepkg -si -p PKGBUILD.bin     # prebuilt binary
# or
makepkg -si                     # builds from source
```

**CachyOS** users get the same binary as Arch — the package is
built with `-march=x86-64` (not `-march=x86-64-v3`), so it runs
on every supported CPU. If you want a v3 / v4-optimised local
build, use the AUR `raidhos` source package; rustup will pick
up your `RUSTFLAGS`.

The `.pkg.tar.zst` is published as a release asset by
[`arch-pkg` in `.github/workflows/release.yml`](../../.github/workflows/release.yml).
It's built inside an `archlinux:base-devel` container against
the same `raidhos-${VERSION}-x86_64-unknown-linux-gnu.tar.gz`
that ships as the standard Linux tarball.

## Verify the binary release first

```bash
cosign verify-blob \
    --certificate-identity-regexp 'https://github.com/sebastienrousseau/raidhos/.*' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com \
    --signature  raidhos-0.0.1-x86_64-unknown-linux-gnu.tar.gz.sig \
    --certificate raidhos-0.0.1-x86_64-unknown-linux-gnu.tar.gz.pem \
    raidhos-0.0.1-x86_64-unknown-linux-gnu.tar.gz
```

The signing identity for v0.0.1 is the release workflow at
`https://github.com/sebastienrousseau/raidhos/.github/workflows/release.yml@refs/tags/v0.0.1`.

## Maintainer notes

1. After tagging `vX.Y.Z`:
   ```bash
   pkgver=X.Y.Z
   cd packaging/arch
   updpkgsums                  # Refresh sha256sums (overwrites SKIP)
   makepkg --printsrcinfo > .SRCINFO
   ```
2. Push to the AUR remote:
   ```bash
   git clone ssh://aur@aur.archlinux.org/raidhos.git aur-raidhos
   cp PKGBUILD .SRCINFO aur-raidhos/
   cd aur-raidhos && git commit -am "upgpkg: raidhos X.Y.Z-1" && git push
   ```
3. Repeat for `raidhos-bin`.

## What's *not* in these packages

- The `raidhos-ui` Tauri 2 desktop app. Tauri requires bundler
  resources (`tauri.conf.json` icons, capabilities, etc.) that
  don't fit a generic Linux package without per-distro shims. The
  GUI ships as an AppImage (`packaging/appimage/`) and a Flatpak
  (`packaging/flatpak/`) until we add a dedicated `raidhos-ui-bin`
  AUR entry.
- The GRUB EFI bundle. Built reproducibly via the Docker pipeline
  in `tools/grub/`; vendored at install time by the release
  tarball, not by Arch's GRUB package (which lacks our specific
  module set).
