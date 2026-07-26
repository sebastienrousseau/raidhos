# Security Policy

## Reporting a vulnerability

RaidhOS reads and writes raw block devices, so security issues are taken
seriously. Please **do not** open public GitHub issues for vulnerabilities.

Instead, report privately by either:

- Emailing the maintainer listed in `Cargo.toml` / commits, or
- Using GitHub's "Report a vulnerability" feature on the repository's
  Security tab (private vulnerability reporting).

We aim to acknowledge reports within 7 days and to share a remediation plan
within 30 days.

## Scope

The following surfaces are in scope:

- The `raidhos-core` library (disk discovery, validation, install path).
- The `raidhos-priv-helper` CLI (the only component intended to run with
  elevated privileges).
- The `cli` and `raidhos-ui` (Tauri) front-ends.
- CI workflows and packaging assets shipped from this repository.

Out of scope:

- Vulnerabilities in upstream Ventoy payloads — please report those upstream.
- Issues that require an attacker to already have root / Administrator on the
  user's machine.

## Hardening summary

RaidhOS already includes the following safeguards:

- **Allowlisted device paths.** `validate_device_path` rejects empty paths,
  paths containing shell metacharacters (`;|&$\``, etc.), path-traversal
  segments (`..`), and anything that does not match the platform's expected
  device-id shape (`/dev/...` on Linux, `/dev/diskN` on macOS,
  `\\.\PhysicalDriveN` on Windows).
- **Refuse to operate on system disks.** Disks mounted at `/`, `/boot`, or
  `/boot/efi` (Linux) and internal disks (macOS/Windows) are filtered out
  of the install target list.
- **Refuse mounted devices.** Installs against any disk with a non-empty
  mountpoint list are rejected.
- **Double opt-in for destructive writes.** Both `wipe` and `allow_write`
  must be set; otherwise `install()` aborts before touching the device.
- **No shell invocation in the hot path.** Privileged commands are spawned
  via `std::process::Command` with separated argv; we never build shell
  strings around user input.
- **Polkit policy** (`packaging/linux/org.raidhos.policy`) requires
  `auth_admin_keep` and disables `allow_any` / `allow_inactive`.
- **CSP-aware Tauri config** with a strict default Content-Security-Policy.
- **CI security gates.** Every PR runs `cargo audit`, `cargo deny`, and
  CodeQL on the actions and frontend code.
- **Pinned action versions.** All GitHub Actions are pinned by major
  version; Dependabot keeps them current.

These are real, code-level safeguards. "100% secure" is not a goal we
claim — but we will keep adding controls and welcome reports of any gap.
