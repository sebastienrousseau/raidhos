<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-ui

Tauri 2 desktop application. Static HTML/CSS/JS frontend served
under a strict CSP. Spawns `raidhos-priv-helper` for the
destructive install step via the OS's elevation primitive
(`pkexec` / `osascript` / UAC).

---

## Build

```bash
# Install OS-specific Tauri 2 prerequisites first — see
# docs/TAURI_NOTES.md.
cargo build --release -p raidhos-ui
```

The binary lands at `target/release/raidhos-ui`.

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

No npm, no Webpack, no Vite. The Tauri build serves
[`frontend/`](frontend/) directly.

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

## Frontend examples

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

## Three-step wizard

Toggleable in the top nav. Two panes:

1. **Pick ISO entries** — the scanned ISOs become menu entries.
2. **Pick USB & confirm** — type the device path + `ERASE`.

Defaults off so power users see the full dashboard.

---

## See also

- [`docs/TAURI_NOTES.md`](../../docs/TAURI_NOTES.md) — per-OS
  build prerequisites + plugin model.
- [`docs/HARDENING.md`](../../docs/HARDENING.md) — Tauri / UI
  controls section.
- [`docs/USER_GUIDE.md`](../../docs/USER_GUIDE.md) —
  drag-and-drop, wizard, push events from the user's side.
