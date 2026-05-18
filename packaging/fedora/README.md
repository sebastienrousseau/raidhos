<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Fedora / RHEL / Rocky / Alma / openSUSE packaging

This directory contains two RPM `.spec` files:

| Spec | Builds from | Use case |
|---|---|---|
| [`raidhos.spec`](raidhos.spec) | Source tarball at `v$VERSION` | Fedora COPR — source-built under the COPR runners' Rust. Works on Fedora N / N-1. |
| [`raidhos-bin.spec`](raidhos-bin.spec) | Pre-built signed release tarball | Released as `.rpm` asset on every tag. No Rust toolchain, no COPR enable, works on RHEL / Rocky / Alma / CentOS Stream 9+ as well as Fedora. |

Both produce the same on-disk layout (see [File layout produced](#file-layout-produced) below).

## Install paths

| Distro | Recommended | How to install |
|---|---|---|
| Fedora ≥ 41 | COPR (current release) or release `.rpm` | `sudo dnf copr enable sebastienrousseau/raidhos && sudo dnf install raidhos`  *or*  `sudo dnf install ./raidhos-bin-${VERSION}-1.x86_64.rpm` |
| RHEL 9 / Rocky 9 / Alma 9 | release `.rpm` | `curl -LO …/raidhos-bin-${VERSION}-1.x86_64.rpm && sudo dnf install ./raidhos-bin-…rpm` |
| CentOS Stream 9 | release `.rpm` | same as RHEL |
| openSUSE Tumbleweed | manual | `sudo zypper ar https://copr.fedorainfracloud.org/coprs/sebastienrousseau/raidhos/repo/openSUSE-Tumbleweed/raidhos.repo` |

The Tauri 2 GUI is **not** in this RPM (webkit2gtk + libsoup3
deps make the package too heavy for COPR's free build allowance).
For the GUI, install the Flatpak from
[`packaging/flatpak/`](../flatpak/) which is the upstream-blessed
path for desktop apps on RPM-family distros.

## Build locally

```bash
# Pull the release tarball
spectool -g packaging/fedora/raidhos.spec --define "_sourcedir $PWD"

# Build srpm + rpm in one go
rpmbuild -ba packaging/fedora/raidhos.spec --define "_sourcedir $PWD"
```

The resulting RPMs land in `~/rpmbuild/RPMS/$(uname -m)/`.

## Verify the cosign signature before install

Same identity / issuer as the AUR binary package:

```bash
cosign verify-blob \
    --certificate-identity-regexp 'https://github.com/sebastienrousseau/raidhos/.*' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com \
    --signature  raidhos-0.0.1.rpm.sig \
    --certificate raidhos-0.0.1.rpm.pem \
    raidhos-0.0.1.rpm
```

## Maintainer flow

1. Tag `vX.Y.Z` upstream.
2. `sed -i "s/^Version:.*/Version:        X.Y.Z/" raidhos.spec`
3. Update `%changelog`.
4. `copr-cli build sebastienrousseau/raidhos packaging/fedora/raidhos.spec`
5. Wait for the COPR runners (~6-10 minutes per arch).
6. `dnf upgrade raidhos` on a test box to confirm.

## File layout produced

```text
/usr/bin/raidhos-cli
/usr/libexec/raidhos-priv-helper
/usr/share/polkit-1/actions/org.raidhos.priv.policy
/usr/share/man/man1/raidhos-cli.1.gz
/usr/share/bash-completion/completions/raidhos-cli
/usr/share/zsh/site-functions/_raidhos-cli
/usr/share/fish/vendor_completions.d/raidhos-cli.fish
/usr/share/raidhos/catalog/catalog.json
/usr/share/raidhos/catalog/keys/*.asc
/usr/share/licenses/raidhos/LICENSE
/usr/share/doc/raidhos/README.md
/usr/share/doc/raidhos/CHANGELOG.md
/usr/share/doc/raidhos/SECURITY.md
```

Matches the AUR / Debian layout exactly, so cross-distro CI tests
can assume the same paths.
