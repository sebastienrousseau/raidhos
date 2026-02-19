# GRUB EFI Build (Docker)

This builds a standalone `BOOTX64.EFI` with an embedded minimal `grub.cfg` that
chainloads the real config from the ESP at `/EFI/BOOT/grub.cfg`.

## Build

```bash
./tools/grub/build_grub.sh
```

## Output

- `BOOTX64.EFI` in the repo root.
- Copy to `ESP/EFI/BOOT/BOOTX64.EFI`.

## Notes

- Requires Docker.
- GRUB version: 2.12.
