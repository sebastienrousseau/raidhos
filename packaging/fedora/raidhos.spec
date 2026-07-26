# SPDX-License-Identifier: GPL-3.0-only
#
# RPM .spec for RaidhOS. Covers Fedora, RHEL 9+, CentOS Stream 9+,
# Rocky Linux 9+, AlmaLinux 9+, and openSUSE Tumbleweed / Leap.
#
# Published via Fedora COPR:
#   https://copr.fedorainfracloud.org/coprs/sebastienrousseau/raidhos
#
# Build locally:
#   rpmbuild -ba packaging/fedora/raidhos.spec \
#     --define "_sourcedir $PWD"
#
# Vendoring policy: this spec builds from a release tarball
# (Source0) rather than `cargo build` against crates.io. RHEL
# variants don't have a recent enough rust by default; we
# rust-toolchain.toml into the build root and let `cargo` resolve
# offline against the vendored Cargo.lock.

Name:           raidhos
Version:        0.0.1
Release:        1%{?dist}
Summary:        Memory-safe multi-ISO USB imager
License:        GPL-3.0-only
URL:            https://github.com/sebastienrousseau/raidhos
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

# Build tooling
BuildRequires:  cargo                  >= 1.85
BuildRequires:  rust                   >= 1.85
BuildRequires:  pkgconf-pkg-config
BuildRequires:  gcc

# Runtime deps the install pipeline shells out to
Requires:       parted
Requires:       dosfstools
Requires:       exfatprogs
Requires:       polkit
Recommends:     gnupg2
Recommends:     systemd-boot-unsigned

# Architectures we ship release builds for
ExclusiveArch:  x86_64 aarch64

%description
RaidhOS turns a USB stick into a multi-ISO boot drive in a few clicks.

The Rust core enforces `forbid(unsafe)` workspace-wide, the privileged
helper is hardened with a 64 KiB argv length cap plus a seccomp-bpf
denylist (ptrace / bpf / kexec / kernel-module load), every destructive
operation requires both `--wipe` and `--allow-write`, and every release
ships with a cosign keyless signature and a SLSA L3 build attestation.

This package ships:
  * /usr/bin/raidhos-cli            — developer-facing command line
  * /usr/libexec/raidhos-priv-helper — the privileged binary launched
                                      via pkexec for destructive ops
  * polkit policy and shell completions

The Tauri 2 desktop GUI ships separately as a Flatpak or AppImage
(see `https://github.com/sebastienrousseau/raidhos#install`).

%prep
%autosetup -n %{name}-%{version}

%build
# Build the CLI + privileged helper. Exclude the Tauri UI; the GUI
# needs webkit2gtk/libsoup which are heavier deps better expressed
# in the Flatpak manifest.
export CARGO_NET_OFFLINE=true
%cargo_build --release --locked --workspace --exclude raidhos-ui

%install
install -Dpm 0755 target/release/raidhos-cli         %{buildroot}%{_bindir}/raidhos-cli
install -Dpm 0755 target/release/raidhos-priv-helper %{buildroot}%{_libexecdir}/raidhos-priv-helper

install -Dpm 0644 packaging/linux/org.raidhos.policy \
    %{buildroot}%{_datadir}/polkit-1/actions/org.raidhos.priv.policy

# Man page + shell completions emitted by build.rs at compile time.
out_dir=$(find target/release/build/raidhos-cli-*/out/dist -maxdepth 1 -mindepth 1 2>/dev/null | head -n1)
if [ -n "$out_dir" ]; then
    install -Dpm 0644 "$out_dir/raidhos-cli.1"           %{buildroot}%{_mandir}/man1/raidhos-cli.1
    install -Dpm 0644 "$out_dir/raidhos-cli.bash"         %{buildroot}%{_datadir}/bash-completion/completions/raidhos-cli
    install -Dpm 0644 "$out_dir/_raidhos-cli"             %{buildroot}%{_datadir}/zsh/site-functions/_raidhos-cli
    install -Dpm 0644 "$out_dir/raidhos-cli.fish"          %{buildroot}%{_datadir}/fish/vendor_completions.d/raidhos-cli.fish
fi

# Catalog (GPG keyring + catalog.json) under /usr/share/raidhos so
# `raidhos-cli catalog list` finds it without environment overrides.
install -d %{buildroot}%{_datadir}/raidhos/catalog/keys
install -Dpm 0644 catalog/catalog.json %{buildroot}%{_datadir}/raidhos/catalog/catalog.json
for k in catalog/keys/*.asc; do
    install -Dpm 0644 "$k" %{buildroot}%{_datadir}/raidhos/catalog/keys/$(basename "$k")
done

%check
%cargo_test --release --locked --workspace --exclude raidhos-ui

%files
%license LICENSE
%doc README.md CHANGELOG.md SECURITY.md
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
* Sat May 17 2026 Sebastien Rousseau <sebastian.rousseau@gmail.com> - 0.0.1-1
- Initial Fedora / RHEL package. Ships CLI + privileged helper +
  polkit policy + catalog keyring.
