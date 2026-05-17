<!-- SPDX-License-Identifier: GPL-3.0-only -->

<p align="center">
  <img src="https://raw.githubusercontent.com/sebastienrousseau/raidhos/main/docs/img/raidhos-logo.svg" alt="RaidhOS logo" width="128" />
</p>

<h1 align="center">raidhos-ui</h1>

<p align="center">
  Tauri 2 desktop application. Static HTML/CSS/JS frontend
  served under a strict CSP. Spawns
  <code>raidhos-priv-helper</code> for the destructive install
  step via the OS's elevation primitive (<code>pkexec</code> /
  <code>osascript</code> / UAC).
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/raidhos/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/raidhos/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/raidhos-ui"><img src="https://img.shields.io/crates/v/raidhos-ui.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Crates.io" /></a>
  <a href="https://docs.rs/raidhos-ui"><img src="https://img.shields.io/badge/docs.rs-raidhos--ui-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs.rs" /></a>
  <a href="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos"><img src="https://api.securityscorecards.dev/projects/github.com/sebastienrousseau/raidhos/badge" alt="OpenSSF Scorecard" /></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg?style=for-the-badge" alt="License" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — bundled with the desktop app
- [Quick Start](#quick-start) — discover, pick, flash, recover

**App reference**

- [Layout](#layout) — frontend + backend
- [Tauri commands](#tauri-commands) — IPC surface
- [Push events](#push-events) — `raidhos://progress`
- [Frontend examples](#frontend-examples) — runnable JS snippets
- [Three-step wizard](#three-step-wizard) — beginner UX
- [Accessibility](#accessibility) — WCAG 2.2 AA pass

**Operational**

- [CSP](#csp) — strict, no `unsafe-inline`
- [Capabilities](#capabilities) — Tauri 2 permission model
- [Testing](#testing) — unit, integration
- [Documentation](#documentation) — all reference docs
- [License](#license)

---

## Install

The UI is bundled with the platform installer:

| Channel | What you get |
|---|---|
| Homebrew | `brew install --cask raidhos` → mounts a `.dmg`, drag the app into `/Applications` |
| winget | `winget install sebastienrousseau.RaidhOS` |
| Debian | `sudo dpkg -i raidhos_0.0.1_amd64.deb` |
| AppImage | `chmod +x raidhos-x86_64.AppImage && ./raidhos-x86_64.AppImage` |
| Source | see [Build](#build) |

The installer always installs `raidhos-cli` and
`raidhos-priv-helper` alongside the GUI. On Linux it also
installs the polkit policy that the UI uses for elevation.

### Build

```bash
# Install OS-specific Tauri 2 prerequisites first — see
# docs/TAURI_NOTES.md.
cargo build --release -p raidhos-ui
./target/release/raidhos-ui
```

The binary lands at `target/release/raidhos-ui`. MSRV: 1.78.

No npm, no Webpack, no Vite. The Tauri build serves
[`frontend/`](frontend/) directly.

---

## Quick Start

1. Launch the app. The dashboard shows three panes:
   *Discovery → Picker → Confirmation*.
2. Plug in a USB stick (or drop ISO files onto the window).
3. Pick the entries you want, pick the device, type `ERASE`
   in the confirmation pane, click *Flash*.
4. The OS asks you to authenticate (pkexec / osascript / UAC).
5. Progress events stream into the right pane in real time.

The *Three-step wizard* (top nav toggle) collapses this into
two clicks for users who don't need the dashboard.

---

## Layout

```text
crates/ui-tauri/
├── src-tauri/                    Rust backend
│   ├── Cargo.toml                Tauri 2 + plugins
│   ├── tauri.conf.json           v2 schema, strict CSP
│   ├── capabilities/default.json Per-window capability set
│   ├── icons/icon.png            Placeholder; replace before release
│   └── src/
│       ├── main.rs               Tauri commands + elevation wrapper
│       └── grub.rs               grub.cfg generator + sanitiser
└── frontend/                     Static webview content
    ├── index.html                One file, no inline script/style
    ├── styles.css                Light theme + a11y + drop banner
    └── app.js                    Plain JS, no bundler
```

---

## Tauri commands

Each `#[tauri::command]` is the surface the frontend can call
via `window.__TAURI__.core.invoke('name', args)`:

| Command | Purpose |
|---|---|
| `list_disks` | Enumerate physical disks. |
| `list_partitions` | Enumerate partitions on a specific disk. |
| `scan_isos` | Walk directories for `.iso` files. |
| `install` | Run the install pipeline; emits `raidhos://progress`. |
| `install_elevated` | Spawn `raidhos-priv-helper` via OS elevation. |
| `save_boot_config` | Persist `boot.json` to `directories::ProjectDirs`. |
| `write_boot_config_to_device` | Write `boot.json` into a mounted partition. |
| `write_grub_cfg_to_esp` | Generate + write `grub.cfg` to a mounted ESP. |
| `copy_isos_to_data` | Copy `.iso` files into `boot/isos/` on DATA. |
| `get_payload_version` | Read `payload/manifest.json:version`. |

---

## Push events

The install pipeline emits typed `raidhos://progress` events.
Frontend subscribes:

```javascript
window.__TAURI__.event.listen('raidhos://progress', (event) => {
  console.log(event.payload.phase, event.payload.message, event.payload.percent);
});
```

No `Mutex<Vec<…>>` polling. Source: `src/main.rs:WindowSink`.

---

## Frontend examples

Runnable JS snippets live in [`examples/`](examples/). Each one
is a small standalone HTML/JS file you can `cargo tauri dev` and
poke at:

| File | Demonstrates |
|---|---|
| [`01-list-disks.html`](examples/01-list-disks.html) | Call `list_disks` and render a table. |
| [`02-scan-isos.html`](examples/02-scan-isos.html) | Drag-and-drop ISOs onto the window. |
| [`03-progress-events.html`](examples/03-progress-events.html) | Subscribe to `raidhos://progress`. |
| [`04-install-dry-run.html`](examples/04-install-dry-run.html) | Build an `InstallRequest` and dry-run. |
| [`05-save-boot-config.html`](examples/05-save-boot-config.html) | Persist + reload `boot.json`. |
| [`06-write-grub-cfg.html`](examples/06-write-grub-cfg.html) | Render and write a `grub.cfg`. |

Inline reference snippets:

```javascript
// List disks.
const disks = await window.__TAURI__.core.invoke('list_disks');

// Dry-run install.
await window.__TAURI__.core.invoke('install', {
  args: {
    device: '/dev/sdb',
    payload_version: '0.1.0',
    wipe: true,
    dry_run: true,
    allow_write: false,
  },
});

// Drag-and-drop ISOs (paths come from Tauri's file-drop event).
window.__TAURI__.event.listen('tauri://drag-drop', async (event) => {
  const paths = event.payload.paths.filter((p) => p.endsWith('.iso'));
  await window.__TAURI__.core.invoke('copy_isos_to_data', {
    mountPath: '/Volumes/DATA',
    sources: paths,
  });
});
```

Example index: [`examples/README.md`](examples/README.md).

---

## Three-step wizard

Toggleable in the top nav. Two panes:

1. **Pick ISO entries** — the scanned ISOs become menu entries.
2. **Pick USB & confirm** — type the device path + `ERASE`.

Defaults off so power users see the full dashboard.

---

## Accessibility

WCAG 2.2 AA pass. Source:
[`frontend/index.html`](frontend/index.html) and
[`frontend/styles.css`](frontend/styles.css).

- Skip-to-main link
- `<main>` landmark
- `role="dialog"` / `aria-modal` on the reset modal
- `role="toolbar"`, `role="log"` on progress
- `aria-label`s and `aria-live` regions throughout
- Keyboard-only flow
- `prefers-color-scheme` light theme
- `prefers-contrast` high-contrast theme
- `prefers-reduced-motion` honoured
- `:focus-visible` ring on every focusable

---

## CSP

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

No `'unsafe-inline'` for either scripts or styles. Inline
`<style>` blocks, inline `<script>` blocks, `style="..."`
attributes, and `onclick="..."` event-handler attributes are
all **disallowed**.

Source:
[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).

---

## Capabilities

Tauri 2's capabilities model replaces Tauri 1's allowlist:

```json
{
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

Source:
[`src-tauri/capabilities/default.json`](src-tauri/capabilities/default.json).
The set is intentionally broad while the UI stabilises; v0.0.2
trims to the minimum required per-command.

---

## Testing

```bash
# Tauri backend unit tests
cargo test -p raidhos-ui --all-targets

# Frontend smoke (Playwright)
make ui-test
```

The Tauri command surface is covered by the backend unit tests
in `src-tauri/src/main.rs`'s `#[cfg(test)]` module. The
`grub.rs` generator and sanitiser have their own dedicated test
module.

---

## Documentation

| Doc | What's in it |
|---|---|
| [`doc/USAGE.md`](doc/USAGE.md) | Cookbook: drag-drop, wizard, progress, etc. |
| [`doc/COMMANDS.md`](doc/COMMANDS.md) | Per-Tauri-command reference |
| [`doc/ACCESSIBILITY.md`](doc/ACCESSIBILITY.md) | WCAG 2.2 AA conformance map |
| [`../../docs/TAURI_NOTES.md`](../../docs/TAURI_NOTES.md) | Per-OS build prerequisites + plugin model |
| [`../../docs/HARDENING.md`](../../docs/HARDENING.md) | Tauri / UI controls section |
| [`../../docs/USER_GUIDE.md`](../../docs/USER_GUIDE.md) | Drag-and-drop, wizard, push events |
| [`../core/README.md`](../core/README.md) | Library behind the IPC commands |
| [`../priv-helper/README.md`](../priv-helper/README.md) | Elevated install path |

---

## License

`raidhos-ui` is licensed under the **GPL-3.0-only**. See
[`LICENSE`](../../LICENSE) for the full text.
