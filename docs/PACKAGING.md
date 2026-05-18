<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Packaging

For distros and channel maintainers packaging RaidhOS. The
repository ships seed packaging files; this document explains
how to use them and what RaidhOS expects from its environment.

---

## Contents

- [Pre-packaged channels](#pre-packaged-channels)
- [What RaidhOS needs at runtime](#what-raidhos-needs-at-runtime)
- [Filesystem layout](#filesystem-layout)
- [polkit policy](#polkit-policy)
- [Debian / Ubuntu (`.deb`)](#debian--ubuntu-deb)
- [Arch / Manjaro (AUR)](#arch--manjaro-aur)
- [Fedora / RHEL (`.rpm`)](#fedora--rhel-rpm)
- [Homebrew](#homebrew)
- [winget](#winget)
- [AppImage](#appimage)
- [Container](#container)
- [Conda-forge / nix / scoop](#conda-forge--nix--scoop)
- [Third-party licences](#third-party-licences)

---

## Pre-packaged channels

| Channel | Maintainer | File in repo |
|---|---|---|
| Debian / Ubuntu `.deb` | upstream | [`packaging/debian/`](../packaging/debian/) |
| Homebrew (personal tap) | upstream | [`packaging/homebrew/raidhos.rb`](../packaging/homebrew/raidhos.rb) |
| winget | upstream | [`packaging/winget/sebastienrousseau.RaidhOS.yaml`](../packaging/winget/sebastienrousseau.RaidhOS.yaml) |
| AppImage | upstream | [`packaging/appimage/build-appimage.sh`](../packaging/appimage/build-appimage.sh) |
| AUR | community | not yet shipped |
| Fedora `.rpm` | community | not yet shipped |
| nixpkgs | community | not yet shipped |
| conda-forge | community | not yet shipped |
| scoop | community | not yet shipped |

If you maintain a channel that isn't listed here, please open a
PR adding the seed files. The maintainer ships the in-repo
seed; channel maintainers handle the per-channel release
mechanics.

---

## What RaidhOS needs at runtime

### Always

- Permission to read `/proc/mounts` and `/sys/block` (Linux)
  or the equivalent on other OSes — for non-elevated
  discovery.
- The privileged helper has read+write on the target block
  device when elevated.

### For destructive installs (the helper, elevated)

| OS | Runtime deps |
|---|---|
| Linux | `parted`, `dosfstools` (`mkfs.vfat`), `exfatprogs` (`mkfs.exfat`), `util-linux` (`mount`, `umount`, `wipefs`), `coreutils` (`cp`), polkit + `pkexec` |
| macOS | `diskutil` (system), `bless` (system), `cp` (system) — no third-party install needed |
| Windows | PowerShell 5+ (preinstalled), `robocopy` (preinstalled), `bcdboot` (preinstalled) |

### For the UI

- Tauri 2 runtime dependencies — see
  [`TAURI_NOTES.md`](TAURI_NOTES.md).

### For optional features

- `docker` if you want to (re-)build `BOOTX64.EFI` locally.
- `gpg(1)` if you want to use `raidhos-cli catalog verify`.

---

## Filesystem layout

The recommended FHS layout on Linux:

```text
/usr/bin/raidhos-cli                                  # CLI (world-executable)
/usr/lib/raidhos/raidhos-priv-helper                  # helper (world-executable, only ran via pkexec)
/usr/share/polkit-1/actions/org.raidhos.policy        # polkit rule
/usr/share/man/man1/raidhos-cli.1.gz                  # man page
/usr/share/bash-completion/completions/raidhos-cli    # bash completion
/usr/share/zsh/site-functions/_raidhos-cli            # zsh completion
/usr/share/fish/vendor_completions.d/raidhos-cli.fish # fish completion
/usr/share/doc/raidhos/LICENSE
/usr/share/doc/raidhos/SECURITY.md
/usr/share/doc/raidhos/CHANGELOG.md
/usr/share/raidhos/catalog/catalog.json               # bundled catalog
/usr/share/raidhos/catalog/keys/<fp>.asc              # bundled GPG keys
```

The helper goes in `/usr/lib/raidhos/`, not `/usr/bin/`,
because users should never run it directly without elevation.
The polkit policy references the helper by absolute path.

---

## polkit policy

[`packaging/linux/org.raidhos.policy`](../packaging/linux/org.raidhos.policy):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "...PolicyKit Policy Configuration 1.0//EN" "...">
<policyconfig>
  <action id="org.raidhos.pkexec">
    <description>RaidhOS requires permission to format removable media</description>
    <message>Authentication is required to format and install RaidhOS to a USB device.</message>
    <defaults>
      <allow_any>no</allow_any>
      <allow_inactive>no</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
  </action>
</policyconfig>
```

Distros that prefer stricter defaults can change
`auth_admin_keep` to `auth_admin` (re-prompt every time).
Document the override in your packaging notes.

---

## Debian / Ubuntu (`.deb`)

Files: [`packaging/debian/`](../packaging/debian/).

```bash
sudo apt-get install -y dh-make dpkg-dev
dpkg-buildpackage -us -uc -b
sudo dpkg -i ../raidhos_0.0.1-1_amd64.deb
```

Update the SHA-256 in the Homebrew formula and the winget
manifest after publishing the release tarball.

---

## Arch / Manjaro (AUR)

Two PKGBUILDs are recommended (mirroring the
`raidhos-bin` / `raidhos` split that other Rust projects use):

| Package | What it is |
|---|---|
| `raidhos-bin` | Pre-built binary from GitHub Releases (faster install). |
| `raidhos` | Builds from source via `cargo`. |

Both should verify the cosign signature in the
`prepare()` step.

Seed PKGBUILD (not in repo today — please contribute):

```bash
# Maintainer: <you>
pkgname=raidhos-bin
pkgver=0.0.1
pkgrel=1
arch=('x86_64' 'aarch64')
url='https://github.com/sebastienrousseau/raidhos'
license=('GPL3')
depends=('parted' 'dosfstools' 'exfatprogs' 'polkit')
source=("https://github.com/sebastienrousseau/raidhos/releases/download/v${pkgver}/raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz"
        "https://github.com/sebastienrousseau/raidhos/releases/download/v${pkgver}/raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz.sig"
        "https://github.com/sebastienrousseau/raidhos/releases/download/v${pkgver}/raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz.pem")
sha256sums=('SKIP' 'SKIP' 'SKIP')   # use cosign verification in prepare()
prepare() {
    cosign verify-blob \
        --certificate "raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz.pem" \
        --signature   "raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz.sig" \
        --certificate-identity-regexp "https://github.com/sebastienrousseau/raidhos/.+" \
        --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
        "raidhos-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz"
}
package() {
    cd "${srcdir}/raidhos-v${pkgver}-x86_64-unknown-linux-gnu"
    install -Dm755 raidhos-cli "${pkgdir}/usr/bin/raidhos-cli"
    install -Dm755 raidhos-priv-helper "${pkgdir}/usr/lib/raidhos/raidhos-priv-helper"
    install -Dm644 "${srcdir}/raidhos/packaging/linux/org.raidhos.policy" \
        "${pkgdir}/usr/share/polkit-1/actions/org.raidhos.policy"
}
```

---

## Fedora / RHEL (`.rpm`)

`rpmbuild` spec sketch (not yet in repo):

```spec
Name:           raidhos
Version:        0.0.1
Release:        1%{?dist}
Summary:        Memory-safe multi-ISO USB imager
License:        GPLv3
URL:            https://github.com/sebastienrousseau/raidhos
Source0:        %{url}/archive/v%{version}/%{name}-v%{version}.tar.gz
BuildRequires:  rust >= 1.85, cargo
Requires:       parted, dosfstools, exfatprogs, polkit

%description
RaidhOS turns a USB stick into a multi-ISO boot drive. Rust core,
forbid(unsafe), reproducible build, signed releases.

%build
cargo build --release --locked --workspace --exclude raidhos-ui

%install
install -D -m 0755 target/release/raidhos-cli         %{buildroot}/usr/bin/raidhos-cli
install -D -m 0755 target/release/raidhos-priv-helper %{buildroot}/usr/lib/raidhos/raidhos-priv-helper
install -D -m 0644 packaging/linux/org.raidhos.policy %{buildroot}/usr/share/polkit-1/actions/org.raidhos.policy

%files
/usr/bin/raidhos-cli
/usr/lib/raidhos/raidhos-priv-helper
/usr/share/polkit-1/actions/org.raidhos.policy
%license LICENSE
%doc SECURITY.md CHANGELOG.md
```

---

## Homebrew

[`packaging/homebrew/raidhos.rb`](../packaging/homebrew/raidhos.rb).

After a release:

1. Compute the per-target SHA-256 (already in `SHA256SUMS`).
2. Replace the `REPLACE_WITH_RELEASE_SHA256` placeholders in
   `raidhos.rb` with the real values for each `on_macos` /
   `on_linux` × `on_arm` / `on_intel` permutation.
3. Push to your tap repo (e.g.
   `https://github.com/sebastienrousseau/homebrew-tap`).
4. Test: `brew install --build-from-source raidhos`.

The formula caveats mention the v0.0.1 platform reality
(macOS install path code-complete, hardware validation
pending).

---

## winget

[`packaging/winget/sebastienrousseau.RaidhOS.yaml`](../packaging/winget/sebastienrousseau.RaidhOS.yaml).

Submit to
[`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs)
under
`manifests/s/sebastienrousseau/RaidhOS/<version>/`.
Update SHA-256s from `SHA256SUMS`. PR review by Microsoft
typically takes a few hours.

---

## AppImage

[`packaging/appimage/build-appimage.sh`](../packaging/appimage/build-appimage.sh).

```bash
cargo build --release --workspace --exclude raidhos-ui
packaging/appimage/build-appimage.sh
# Produces build/RaidhOS-x86_64.AppImage
```

`appimagetool` is required —
<https://github.com/AppImage/AppImageKit/releases>.

The AppImage bundles the CLI + helper. It does **not** bundle
`parted`, `mkfs.vfat`, `mkfs.exfat` — those are expected on the
host (almost always present on modern Linux installs). Document
this prominently in your AppImage release.

---

## Container

```dockerfile
# Minimal CLI container. Does not include the privileged helper.
FROM debian:bookworm-slim@sha256:b6507e340c43553136f5078daa1bd49ce8b975e6e1f1d0a2b8e5b06a4d3d7c20
RUN apt-get update && apt-get install -y \
    parted dosfstools exfatprogs && \
    rm -rf /var/lib/apt/lists/*
COPY raidhos-cli /usr/bin/raidhos-cli
ENTRYPOINT ["/usr/bin/raidhos-cli"]
```

Publish to `ghcr.io/sebastienrousseau/raidhos-cli:<version>`.
The container is suitable for `list-disks`, `catalog`, and
dry-run flows. **Real installs need the privileged helper on a
host**, not a container.

---

## Conda-forge / nix / scoop

Not yet shipped. PRs welcome — the seed files for AUR / RPM
above are a good template.

For nix specifically, the project is `cargo`-shaped, so
`rustPlatform.buildRustPackage` + the GPL-3 licence should be
straightforward. Pin the toolchain via `rust-bin` to match
`rust-toolchain.toml`.

---

## Third-party licences

| Component | Licence | Notes |
|---|---|---|
| `raidhos-core`, `raidhos-cli`, `raidhos-priv-helper`, `raidhos-ui` | GPL-3.0-only | This project. |
| `parted` (runtime dep on Linux) | GPL-3.0-or-later | Not bundled; expected on host. |
| `grub` 2.12 (built from source for `BOOTX64.EFI`) | GPL-3.0-or-later | Bundled output, source pinned. |
| Ventoy payload (bundled separately) | GPL-3.0+ | Payload version pinned in `payload/manifest.json`. |
| Tauri 2 | MIT / Apache-2.0 | UI runtime. |
| All Cargo dependencies | listed in CycloneDX SBOM per release | Allowlist enforced by `deny.toml`. |

If you redistribute a bundle (AppImage, container) that ships
GPL-3 components together with non-GPL components, you must
comply with the GPL-3 source-availability requirements.
