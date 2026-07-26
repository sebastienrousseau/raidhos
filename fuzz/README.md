# RaidhOS fuzz harness

Fuzz targets for the trust-boundary parsers in `raidhos-core`.

## Local run

```bash
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run validate_device_path
```

Each target runs until you press `Ctrl-C` or a crash is found. Crashes
land under `artifacts/<target>/`; corpus seeds under `corpus/<target>/`.

## Targets

| Target                  | Function under test                                     | Notes |
| ----------------------- | ------------------------------------------------------- | ----- |
| `validate_device_path`  | `raidhos_core::validate_device_path`                    | Always reachable. Goal: never panic. |
| `parse_lsblk_disks`     | Linux `lsblk` JSON parser                               | Placeholder on non-Linux hosts. |
| `parse_disks_plist`     | macOS `diskutil` plist walker                           | Placeholder on non-macOS hosts. |
| `parse_get_disk_json`   | Windows `Get-Disk` JSON parser                          | Placeholder on non-Windows hosts. |

The platform-specific parsers are kept fuzz-targetable through
`pub(crate)` exposure inside each `platform/{linux,macos,windows}.rs`.
We expect to expose them through a `doc(hidden)` helper in a follow-up
so the fuzz targets can exercise them on any host.

## CI

`.github/workflows/fuzz.yml` runs a 60-second smoke burst per target on
push. Long-running campaigns happen on developer machines or via
OSS-Fuzz integration (planned).
