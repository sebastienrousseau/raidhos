# Payload Layout

RaidhOS expects a payload directory with two subfolders:

```
RAIDHOS_PAYLOAD_DIR/
  esp/   # EFI partition contents
  data/  # Data partition contents
```

## Copy behavior

- `esp/` is copied into the EFI partition root.
- `data/` is copied into the data partition root.
- The copy is recursive and preserves permissions.

## Required files

The payload must contain the bootloader and runtime assets required to read
`raidhos/boot.json` from the data partition. The bootloader logic is expected
to read and render entries from that file.

## Environment variable

Set the payload directory before installing:

```
export RAIDHOS_PAYLOAD_DIR=/path/to/payload
```

## Errors

If the payload directory is missing, or does not contain `esp/` and `data/`,
installation will fail with a validation error.
