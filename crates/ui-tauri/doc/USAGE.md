<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-ui usage

Tauri 2 desktop app. This document is for **users** of the UI;
build-time notes are in
[`docs/TAURI_NOTES.md`](../../../docs/TAURI_NOTES.md).

---

## Launch

```bash
# Linux: as installed (package or build)
raidhos-ui

# macOS: open the .app bundle once installed via brew
open -a RaidhOS

# Windows: Start Menu shortcut
```

The window opens to the dashboard:

- **Header** — Save Config / Refresh / List Disks toolbar.
- **Boot Entries** — list of ISOs discovered on the host.
- **Install** — disk picker + confirmation + progress log.
- **Boot Config** — per-entry kernel params and default entry.

Toggle the "Guided mode" checkbox in the top nav for the
two-step wizard.

---

## Discover disks

The UI auto-runs `list_disks` on startup. To refresh, click
**Refresh** (re-runs `list_disks`) or **List Disks** (re-runs
discovery + ISO scan).

System / boot disks are **hidden by default** in guided mode and
**dimmed** in dashboard mode. There is no way to install onto
the system disk from the UI.

---

## Pick a USB

Click a disk card. The card highlights and the **Install**
section unlocks. The selected device path appears under
"Selected:".

To confirm:

1. Type the device path into the "Type the device path to
   confirm" input (e.g. `/dev/sdb`).
2. Tick **I understand this will erase the selected device.**
3. Tick **Enable actual write (not a dry-run).**
4. Type **`ERASE`** into the final confirmation box.

The **Install** button stays disabled until all four conditions
are met.

---

## Drag-and-drop ISOs

After a successful install, drag `.iso` files from your file
manager into the RaidhOS window. The drop banner appears at
the bottom and the files are copied into `boot/isos/` on the
DATA partition.

Drop targets:

- Tauri 2: `tauri://drag-drop` event
- Tauri 1 fallback: `tauri://file-drop` event

The frontend listens to both for compatibility.

---

## Boot config

Each ISO becomes a GRUB menu entry. Per-entry, you can edit:

- **Params** — kernel parameters (default: `quiet splash`).
- **Initrd** — explicit initrd path (optional).
- **Kernel args** — additional kernel command-line args.

Use the **↑** / **↓** buttons to reorder. The **Default entry**
radio sets the default boot.

The boot config is saved to:

- Linux: `~/.config/raidhos/boot.json`
- macOS: `~/Library/Application Support/raidhos/boot.json`
- Windows: `%APPDATA%\raidhos\boot.json`

via the `directories` crate.

---

## Push events

The install pipeline emits typed `raidhos://progress` events:

```javascript
window.__TAURI__.event.listen('raidhos://progress', (event) => {
    console.log(event.payload.phase,
                event.payload.message,
                event.payload.percent);
});
```

The UI renders them in the **Progress** card; the events are
also queued for the wizard's final summary.

---

## Accessibility

The UI honours:

- `@media (prefers-color-scheme: light)` — system light theme.
- `@media (prefers-contrast: more)` — high-contrast theme.
- `@media (prefers-reduced-motion: reduce)` — no animations.

Every interactive element has an `aria-label` or descriptive
content; the install progress is a `role="log"` `aria-live`
region; the reset confirmation is a `role="dialog"`
`aria-modal` element.

Keyboard:

- <kbd>Tab</kbd> / <kbd>Shift+Tab</kbd> — focus order is the
  semantic order.
- <kbd>Enter</kbd> on a disk card selects it.
- <kbd>↑</kbd> / <kbd>↓</kbd> on a boot-entry list reorders.
- `:focus-visible` ring on every focusable.

---

## See also

- [`raidhos-cli` USAGE](../../cli/doc/USAGE.md) — the headless
  equivalent of the UI's operations.
- [`docs/USER_GUIDE.md`](../../../docs/USER_GUIDE.md) —
  end-to-end first-install walkthrough.
- [`docs/TAURI_NOTES.md`](../../../docs/TAURI_NOTES.md) —
  per-OS Tauri prerequisites and the capabilities model.
