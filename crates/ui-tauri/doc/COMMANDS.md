<!-- SPDX-License-Identifier: GPL-3.0-only -->

# `raidhos-ui` Tauri commands

Every `#[tauri::command]` exposed by the backend. The frontend
calls them via:

```javascript
const result = await window.__TAURI__.core.invoke('<name>', { ...args });
```

Argument names use **snake_case** at the Rust side and
**camelCase** at the JS side (Tauri converts automatically).

---

## Discovery

### `list_disks() -> Vec<DiskInfo>`

Enumerate physical disks. Returns the same `DiskInfo` shape as
the CLI's `list-disks --json`.

```javascript
const disks = await invoke('list_disks');
// [{ id, model, size_bytes, removable, mountpoints, is_system }, …]
```

### `list_partitions(device: String) -> Vec<PartitionInfo>`

List partitions on a specific disk. Empty list on platforms
without per-partition enumeration wired yet.

```javascript
await invoke('list_partitions', { device: '/dev/sdb' });
```

### `scan_isos(dirs: Vec<String>) -> Vec<IsoEntry>`

Walk the supplied directories for `*.iso` files (one level
deep).

```javascript
await invoke('scan_isos', { dirs: ['/home/me/Downloads', '/media'] });
```

---

## Install pipeline

### `install(args: InstallRequest) -> ()`

Run the install pipeline. Emits `raidhos://progress` events as
it works. Dry-runs unless `args.allow_write === true` *and*
`args.wipe === true`.

```javascript
await invoke('install', {
  args: {
    device: '/dev/sdb',
    payload_version: '0.1.0',
    wipe: true,
    dry_run: true,
    allow_write: false,
  },
});
```

### `install_elevated(args: InstallRequest) -> ()`

Spawn `raidhos-priv-helper install` via the OS's elevation
primitive (pkexec / osascript / UAC). Returns when the helper
finishes; intermediate `raidhos://progress` events are
forwarded from the helper's stderr.

```javascript
await invoke('install_elevated', { args: req });
```

---

## Config persistence

### `save_boot_config(config: BootConfig) -> ()`

Persist a `boot.json` to the user's
`directories::ProjectDirs::config_dir()` location.

### `load_boot_config() -> Option<BootConfig>`

Read it back. `None` if the file doesn't exist yet.

### `write_boot_config_to_device(mount_path: String, config: BootConfig) -> ()`

Write the same `boot.json` into a mounted DATA partition. Used
to update an existing USB without re-flashing.

### `write_grub_cfg_to_esp(mount_path: String, config: BootConfig) -> ()`

Render `boot.json` into a `grub.cfg` and write it to
`<mount>/grub/grub.cfg`. The mount path must already be
writable.

### `copy_isos_to_data(mount_path: String, sources: Vec<String>) -> ()`

Copy each path in `sources` into
`<mount>/boot/isos/<basename>`.

---

## Metadata

### `get_payload_version() -> String`

Read `payload/manifest.json:version`. Used by the UI to
display the bundled payload's version.

---

## Events

| Event | Payload | Emitted by |
|---|---|---|
| `raidhos://progress` | `{ phase, message, percent }` | `install`, `install_elevated` |
| `tauri://drag-drop` | `{ paths: [string] }` | webview drag-drop |

Subscribe with:

```javascript
const unlisten = await window.__TAURI__.event.listen('raidhos://progress',
  (event) => { console.log(event.payload); });
// Call unlisten() when the listener is no longer needed.
```

---

## See also

- [`USAGE.md`](USAGE.md) — task-oriented cookbook.
- [`../README.md`](../README.md) — UI reference.
- [`../../core/doc/API.md`](../../core/doc/API.md) — per-symbol
  reference for the library powering each command.
