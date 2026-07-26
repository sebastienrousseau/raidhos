<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-ui` examples

The UI is a Tauri 2 desktop app with a static HTML/CSS/JS
frontend. Each example here is a small standalone HTML file
that demonstrates one Tauri IPC command or one UX pattern.

To run an example: start the Tauri dev shell and point the
webview at the example file.

```bash
# From the repo root:
cargo tauri dev --config crates/ui-tauri/src-tauri/tauri.conf.json
# Then in DevTools console:
window.location.href = '/examples/01-list-disks.html';
```

Or copy the example into `frontend/` temporarily and refresh.

| File | Tauri command(s) | What it demonstrates |
|---|---|---|
| [`01-list-disks.html`](01-list-disks.html) | `list_disks` | Discover disks and render a sortable table. |
| [`02-scan-isos.html`](02-scan-isos.html) | `scan_isos`, `tauri://drag-drop` | Drag-and-drop ISOs onto the window. |
| [`03-progress-events.html`](03-progress-events.html) | `install`, `raidhos://progress` | Subscribe to the progress event stream. |
| [`04-install-dry-run.html`](04-install-dry-run.html) | `install` (dry-run) | Build an `InstallRequest` and run it without writes. |
| [`05-save-boot-config.html`](05-save-boot-config.html) | `save_boot_config` | Persist + reload `boot.json` to `ProjectDirs`. |
| [`06-write-grub-cfg.html`](06-write-grub-cfg.html) | `write_grub_cfg_to_esp` | Render and write a `grub.cfg` to a mounted ESP. |

## Conventions

- One HTML file per example. No frameworks, no bundlers — plain
  HTML + plain JS, same as the production frontend.
- The `<script>` block sits in `<head>` and uses `defer` so it
  runs after the DOM is ready without inline event handlers
  (the strict CSP forbids inline handlers).
- All examples set the CSP `<meta>` tag matching production.
- Examples never call destructive commands (`install_elevated`,
  `copy_isos_to_data`) without explicit confirmation.

## See also

- [`../README.md`](../README.md) — the UI reference.
- [`../doc/USAGE.md`](../doc/USAGE.md) — task-oriented cookbook.
- [`../doc/COMMANDS.md`](../doc/COMMANDS.md) — per-command reference.
- [`../frontend/`](../frontend/) — the production frontend.
