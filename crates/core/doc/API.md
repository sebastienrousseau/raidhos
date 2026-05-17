<!-- SPDX-License-Identifier: GPL-3.0-only -->

# raidhos-core API reference

Public surface of the `raidhos-core` library. The full
generated rustdoc is the source of truth; this is a hand-curated
quick reference grouped by capability.

For runnable examples, see [`../examples/`](../examples/).

---

## Discovery

### `list_disks() -> Result<Vec<DiskInfo>>`

Enumerate physical disks via the per-OS discovery backend
(`lsblk -J` on Linux, `diskutil list -plist external` on macOS,
`Get-Disk | ConvertTo-Json` on Windows). Returns
`CoreError::Io` if the subprocess fails;
`CoreError::Parse` if its output is malformed.

### `list_partitions(device: String) -> Result<Vec<PartitionInfo>>`

Enumerate partitions on a specific disk. Returns an empty list
on platforms where partition enumeration is not yet wired
(macOS, Windows — Linux only today).

### `scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>>`

Walk the supplied directories one level deep, collecting
`*.iso` files. Used by the UI's "Scan ISOs" action. Returns
sorted by lowercase title.

---

## Validation

### `validate_device_path(device: &str) -> Result<()>`

Per-OS allowlist + metacharacter / path-traversal reject.
Refuses:

- empty paths,
- paths >256 bytes,
- any of `;|&$`, backtick, `<>"'*?()\`, newline, CR, tab,
- `..` (path traversal),
- on Linux, paths that don't start with `/dev/`,
- on macOS, paths that don't match `/dev/disk*`,
- on Windows, paths that don't match `\\.\PhysicalDriveN` (or
  `\\?\PhysicalDriveN`).

Returns `CoreError::Validation` with a human-readable reason.
Defence in depth: `std::process::Command` is argv-based and
doesn't invoke a shell, but rejecting obvious bad input early
prevents accidental misuse by callers building shell snippets.

---

## Install pipeline

### `install(req: InstallRequest, sink: &dyn ProgressSink) -> Result<()>`

End-to-end install. Validates the request, refuses unsafe
targets, then either dry-runs or executes.

Required for the destructive path:

- `req.wipe == true`
- `req.allow_write == true`
- `req.dry_run == false`

Any other combination is rejected before any byte is written.

Progress events are emitted in real time via `sink.emit(...)`.

### `trait ProgressSink { fn emit(&self, event: ProgressEvent); }`

Implement to receive progress events. The CLI prints them; the
UI re-emits them as `raidhos://progress` Tauri events.

---

## Payload integrity

### `verify_payload(payload_dir: &Path) -> Result<(), ManifestError>`

Walk the payload tree, compute a path-prefixed order-stable
SHA-256, and compare against `payload_dir/manifest.json:checksum`.

Returns `Ok(())` on match, `ManifestError::Unpinned` if the
checksum field is empty (advisory during development), and
`ManifestError::Mismatch` if the hashes differ.

### `struct PayloadManifest`

```rust
pub struct PayloadManifest {
    pub name: String,
    pub version: String,
    pub source: String,    // default ""
    pub checksum: String,  // default ""; empty means "unpinned"
    pub notes: String,     // default ""
}
```

Parsed from `manifest.json` via `serde::Deserialize`.

---

## ISO catalog

### `load_catalog() -> Result<Vec<CatalogEntry>>`

Read `catalog/catalog.json` from the working directory or one
of two parent directories. Use `load_catalog_from(path)` for an
explicit path.

### `find_entry<'a>(catalog: &'a [CatalogEntry], slug: &str) -> Option<&'a CatalogEntry>`

Linear scan by `slug`.

### `verify_iso(entry, iso, sums, sig, key_dir) -> Result<VerifiedIso, CatalogError>`

GPG-signed verification of a downloaded ISO:

1. Look up the public key at `key_dir/<entry.gpg_fingerprint>.asc`.
2. Import it into an ephemeral GNUPGHOME (the user's keyring is
   not touched).
3. `gpg --verify <sig> <sums>` — refuse if the signature is
   invalid.
4. Look up `entry.iso_filename` in `<sums>` — refuse if absent.
5. Compute the SHA-256 of the ISO — refuse if it doesn't match.

Returns `VerifiedIso { entry, computed_sha256 }` on success.

### `sha256sums_lookup(body: &str, filename: &str) -> Option<String>`

Parse a single-line lookup from a `SHA256SUMS` body. Public
re-export for callers verifying against an arbitrary signed
manifest.

---

## Error types

### `enum CoreError`

```rust
pub enum CoreError {
    UnsupportedPlatform,
    Io(String),
    Validation(String),
    NotImplemented(String),
    Parse(String),
}
```

`thiserror`-derived. Wire format (`Serialize`/`Deserialize`)
preserved. Display text stable across patch releases so
substring-matching callers don't break.

### `enum ManifestError`

```rust
pub enum ManifestError {
    Manifest(String),
    Io(io::Error),
    Unpinned,
    Mismatch { manifest: String, computed: String },
}
```

### `enum CatalogError`

```rust
pub enum CatalogError {
    Io(String),
    Parse(String),
    GpgMissing,
    SignatureRejected(String),
    MissingFilename(String),
    Sha256Mismatch { expected: String, computed: String },
    KeyMissing(String),
}
```

---

## Data types

| Type | Purpose |
|---|---|
| `DiskInfo` | Physical disk metadata from discovery |
| `PartitionInfo` | Partition metadata |
| `IsoEntry` | ISO discovered by `scan_isos` |
| `InstallRequest` | Caller-supplied input to `install` |
| `ProgressEvent` | One step of pipeline progress |
| `CatalogEntry` | One distro in the bundled catalog |
| `VerifiedIso` | Successful verification outcome |

See the rustdoc (`cargo doc -p raidhos-core --no-deps --open`)
for field-level documentation on each.

---

## Hidden / unstable

### `__fuzz_api`

Re-exports of the hand-rolled parsers (`parse_lsblk_disks`,
`parse_disks_plist`, `parse_get_disk_json`,
`parse_lsblk_partitions`) for `cargo-fuzz`. `#[doc(hidden)]`,
no stability guarantee, may move or rename without notice.
