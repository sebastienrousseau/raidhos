# SPDX-License-Identifier: GPL-3.0-only
#
# Binary-only RPM .spec for RaidhOS. Installs the pre-built,
# cosign-signed release tarball directly — no Rust toolchain,
# no Cargo, no compilation. Useful for RHEL / Rocky / Alma /
# CentOS Stream sites that can't enable a COPR repo, and for
# any user who wants a one-shot `dnf install ./raidhos-bin-…rpm`.
#
# Build locally (against a downloaded release tarball):
#   mkdir -p ~/rpmbuild/SOURCES
#   cp raidhos-${VERSION}-x86_64-unknown-linux-gnu.tar.gz \
#      ~/rpmbuild/SOURCES/
#   rpmbuild -bb packaging/fedora/raidhos-bin.spec \
#     --define "version ${VERSION}"
#
# CI builds this in release.yml; the resulting RPM ships as a
# release asset.

%global debug_package %{nil}

Name:           raidhos-bin
Version:        0.0.2
Release:        1%{?dist}
Summary:        Memory-safe multi-ISO USB imager (pre-built, cosign-signed)
License:        GPL-3.0-only
URL:            https://github.com/sebastienrousseau/raidhos

# %ix86 builders should pick the x86_64 tarball; aarch64 builders
# pick aarch64. rpmbuild substitutes %{_arch} at build time.
Source0:        %{url}/releases/download/v%{version}/raidhos-%{version}-%{_arch}-unknown-linux-gnu.tar.gz

# Same runtime deps as the source-built spec — the helper still
# shells to parted / mkfs.* / pkexec.
Requires:       parted
Requires:       dosfstools
Requires:       exfatprogs
Requires:       polkit

# Replaces the source-built package (one of them on disk).
Provides:       raidhos = %{version}-%{release}
Conflicts:      raidhos

# No %build — this spec is a pure repackager.
ExclusiveArch:  x86_64 aarch64

%description
RaidhOS is a memory-safe, audit-friendly multi-ISO USB imager.
Rust core with `forbid(unsafe)`, reproducible build, signed
releases.

This binary RPM installs the cosign-signed release tarball
verbatim. For a source build under your local Rust toolchain,
use the `raidhos` package (Fedora COPR or
`packaging/fedora/raidhos.spec`).

%prep
%setup -q -n raidhos-%{version}

%install
install -Dpm 0755 raidhos-cli         %{buildroot}%{_bindir}/raidhos-cli
install -Dpm 0755 raidhos-priv-helper %{buildroot}%{_libexecdir}/raidhos-priv-helper

install -Dpm 0644 polkit/org.raidhos.priv.policy \
    %{buildroot}%{_datadir}/polkit-1/actions/org.raidhos.priv.policy

install -Dpm 0644 man/raidhos-cli.1   %{buildroot}%{_mandir}/man1/raidhos-cli.1
install -Dpm 0644 completions/raidhos-cli.bash %{buildroot}%{_datadir}/bash-completion/completions/raidhos-cli
install -Dpm 0644 completions/_raidhos-cli     %{buildroot}%{_datadir}/zsh/site-functions/_raidhos-cli
install -Dpm 0644 completions/raidhos-cli.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/raidhos-cli.fish

install -d %{buildroot}%{_datadir}/raidhos/catalog/keys
install -Dpm 0644 catalog/catalog.json %{buildroot}%{_datadir}/raidhos/catalog/catalog.json
for k in catalog/keys/*.asc; do
    install -Dpm 0644 "$k" %{buildroot}%{_datadir}/raidhos/catalog/keys/$(basename "$k")
done

install -Dpm 0644 LICENSE      %{buildroot}%{_licensedir}/%{name}/LICENSE
install -Dpm 0644 README.md    %{buildroot}%{_docdir}/%{name}/README.md
install -Dpm 0644 CHANGELOG.md %{buildroot}%{_docdir}/%{name}/CHANGELOG.md

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/raidhos-cli
%{_libexecdir}/raidhos-priv-helper
%{_datadir}/polkit-1/actions/org.raidhos.priv.policy
%{_mandir}/man1/raidhos-cli.1*
%{_datadir}/bash-completion/completions/raidhos-cli
%{_datadir}/zsh/site-functions/_raidhos-cli
%{_datadir}/fish/vendor_completions.d/raidhos-cli.fish
%dir %{_datadir}/raidhos
%dir %{_datadir}/raidhos/catalog
%dir %{_datadir}/raidhos/catalog/keys
%{_datadir}/raidhos/catalog/catalog.json
%{_datadir}/raidhos/catalog/keys/*.asc

%changelog
* Sun May 18 2026 Sebastien Rousseau <sebastian.rousseau@gmail.com> - 0.0.1-1
- Initial binary-only RPM. Repackages the cosign-signed release
  tarball; same on-disk layout as the source-built package.
