<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Tauri 2 build notes

The `raidhos-ui` crate is a Tauri 2 application. Tauri 2 uses
the host's WebView (WebKit2GTK on Linux, WKWebView on macOS,
WebView2 on Windows), so each platform needs the right native
development packages.

The bootstrap scripts under [`scripts/`](../scripts/) install
everything in one command. This document records the exact
package list per OS so you can audit what gets pulled in.

---

## Contents

- [Linux (Debian / Ubuntu)](#linux-debian--ubuntu)
- [Linux (Arch / Manjaro)](#linux-arch--manjaro)
- [Linux (Fedora / RHEL)](#linux-fedora--rhel)
- [macOS](#macos)
- [Windows](#windows)
- [Capabilities-based permission model](#capabilities-based-permission-model)
- [CSP](#csp)
- [Plugins](#plugins)

---

## Linux (Debian / Ubuntu)

```bash
sudo apt-get update
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libsoup-3.0-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    build-essential \
    curl \
    file \
    wget
```

Tauri 2 dropped `libwebkit2gtk-4.0-dev` in favour of `-4.1-dev`
mid-2024 — make sure you have the 4.1 series.

Bootstrap script:
[`scripts/bootstrap-debian.sh`](../scripts/bootstrap-debian.sh).

---

## Linux (Arch / Manjaro)

```bash
sudo pacman -Syu --noconfirm --needed \
    base-devel \
    curl \
    wget \
    file \
    openssl \
    libsoup3 \
    webkit2gtk-4.1 \
    libappindicator-gtk3 \
    librsvg
```

Bootstrap script:
[`scripts/bootstrap-arch.sh`](../scripts/bootstrap-arch.sh).

---

## Linux (Fedora / RHEL)

```bash
sudo dnf install -y \
    webkit2gtk4.1-devel \
    openssl-devel \
    curl \
    wget \
    file \
    libappindicator-gtk3 \
    librsvg2-devel
sudo dnf group install -y "C Development Tools and Libraries"
```

Bootstrap script:
[`scripts/bootstrap-fedora.sh`](../scripts/bootstrap-fedora.sh).

---

## macOS

```bash
xcode-select --install
brew install rustup-init
rustup-init -y --default-toolchain stable
```

macOS uses WKWebView from the system — no separate WebKit
install. Apple Silicon builds work natively.

Bootstrap script:
[`scripts/bootstrap-macos.sh`](../scripts/bootstrap-macos.sh).

---

## Windows

```powershell
# Install the MSVC build tools (or Visual Studio Community).
winget install -e --id Microsoft.VisualStudio.2022.BuildTools `
    --override "--add Microsoft.VisualStudio.Workload.VCTools --quiet --norestart"

# WebView2 runtime (already on Windows 11; install it explicitly on 10).
winget install -e --id Microsoft.EdgeWebView2

# Rust toolchain.
winget install -e --id Rustlang.Rustup
```

Bootstrap script:
[`scripts/bootstrap-windows.ps1`](../scripts/bootstrap-windows.ps1).

---

## Capabilities-based permission model

Tauri 2 replaced the v1 allowlist with a per-window
capabilities file. RaidhOS declares one in
[`crates/ui-tauri/src-tauri/capabilities/default.json`](../crates/ui-tauri/src-tauri/capabilities/default.json):

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:window:default",
    "core:webview:default",
    "core:app:default",
    "core:path:default",
    "dialog:default",
    "fs:default",
    "shell:default"
  ]
}
```

This is intentionally broad while the UI stabilises. We'll
trim to the minimum required set in v0.0.2 once the IPC surface
is locked in.

---

## CSP

The Content-Security-Policy declared in
[`crates/ui-tauri/src-tauri/tauri.conf.json`](../crates/ui-tauri/src-tauri/tauri.conf.json)
is:

```text
default-src 'self';
img-src 'self' data:;
style-src 'self';
script-src 'self';
connect-src 'self' ipc: tauri:;
frame-ancestors 'none';
base-uri 'self';
form-action 'none';
object-src 'none'
```

There is no `'unsafe-inline'` for scripts or styles. That
means:

- Inline `<style>` blocks: **disallowed**. All styles live in
  [`frontend/styles.css`](../crates/ui-tauri/frontend/styles.css).
- Inline `<script>` blocks: **disallowed**. All scripts live in
  [`frontend/app.js`](../crates/ui-tauri/frontend/app.js).
- Inline `style="..."` attributes: **disallowed**. We use utility
  classes instead (`actions--mt-2`, `actions--mb-3` in
  styles.css).
- Inline event handlers (`onclick=`, `onerror=`): **disallowed**.
  All event handling is `addEventListener` in app.js.

If you add a third-party CDN font / image / API, you must
either bundle it as a local asset or extend the CSP — and the
extension goes through code review.

---

## Plugins

| Plugin | Why |
|---|---|
| `tauri-plugin-dialog` | Native file pickers (currently unused but staged for the catalog-download flow). |
| `tauri-plugin-fs` | Capability-scoped file access from the frontend. |
| `tauri-plugin-shell` | Spawn external commands from the frontend (only used for elevation today). |

All three are added in
[`crates/ui-tauri/src-tauri/Cargo.toml`](../crates/ui-tauri/src-tauri/Cargo.toml)
and initialised in `main.rs`.

---

## Common build issues

### `error: package 'tauri 2.X' cannot be built because it requires rustc 1.78 or newer`

Update your toolchain: `rustup update stable`.

### `failed to read icon /path/to/icon.png`

The repo ships a 67-byte placeholder PNG at
`crates/ui-tauri/src-tauri/icons/icon.png`. Replace with real
artwork before release. The placeholder is enough for
`cargo build` to succeed.

### Linux Wayland: black window

Some WebKit2GTK versions have DMABUF rendering bugs on Wayland.
Workaround:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 raidhos-ui
```

### macOS: `Could not load WebView2 runtime`

That's a Windows-only error message that shouldn't appear on
macOS. If it does, you're running a Windows build under
Rosetta. Use the macOS build instead.

### Windows: `error 0x800704C7` from WebView2

The user cancelled an install prompt. Re-run as the user that
will use RaidhOS (or per-machine: install WebView2 runtime).

---

## See also

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — where the UI fits in
  the workspace.
- [`HARDENING.md`](HARDENING.md) — Tauri / UI controls section.
- [Tauri 2 docs](https://tauri.app/) — upstream.
