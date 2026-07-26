<!-- SPDX-License-Identifier: GPL-3.0-only -->

## Summary

<!-- 1-3 sentences. What does this PR change and why? -->

## Type of change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (CLI / library / config / on-disk format)
- [ ] Documentation only
- [ ] CI / build / packaging only
- [ ] Refactor / internal change with no user-visible diff

## Crates touched

- [ ] `raidhos-core`
- [ ] `raidhos-cli`
- [ ] `raidhos-priv-helper`
- [ ] `raidhos-ui`
- [ ] Workspace files (Cargo.toml, deny.toml, CI workflows, packaging)

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --exclude raidhos-ui --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace --exclude raidhos-ui --all-targets` passes
- [ ] `cargo test -p raidhos-core --doc` passes
- [ ] New tests cover the changed paths (or the change is doc-only)
- [ ] Updated [`CHANGELOG.md`](../CHANGELOG.md) under `Unreleased`
- [ ] Updated per-crate README / doc/ if the public surface changed
- [ ] No new `unsafe` introduced (workspace forbids it)

## Safety review (required for any destructive code path)

- [ ] No new privileged-helper subcommand without the
      `--allow-write` + `wipe` double-gate.
- [ ] Validation runs in `raidhos-core::validate_*` *before* opening
      any device.
- [ ] Any new subprocess uses `Runtime::run_output` (not raw
      `Command::new`).
- [ ] If touching the polkit policy: the rule still requires
      interactive authentication.

## How to test manually

<!-- Commands the reviewer can paste. Always start with `--dry-run`
     or `validate-device` so reviewers don't accidentally flash. -->

```bash
```

## Related

<!-- Closes #N / refs #N / discussion link / threat-model section. -->
