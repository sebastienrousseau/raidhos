<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Design notes

Decisions that didn't fit neatly into
[`ARCHITECTURE.md`](ARCHITECTURE.md) or
[`THREAT_MODEL.md`](THREAT_MODEL.md) but that future
contributors will want to know about.

Each note states **the decision**, **the reasoning**, and
**what we'd revisit if circumstances change**.

---

## Contents

- [Workspace shape](#workspace-shape)
- [Trust boundary in the library, not the CLI](#trust-boundary-in-the-library-not-the-cli)
- [JSON-over-stdout for the helper, not D-Bus or a socket](#json-over-stdout-for-the-helper-not-d-bus-or-a-socket)
- [Hand-rolled parsers, not crates](#hand-rolled-parsers-not-crates)
- [Two opt-ins, not one](#two-opt-ins-not-one)
- [`thiserror`, not `anyhow`](#thiserror-not-anyhow)
- [`clap` everywhere, not custom argv](#clap-everywhere-not-custom-argv)
- [Seccomp denylist, not allowlist](#seccomp-denylist-not-allowlist)
- [TOCTOU defence in helper, not in core](#toctou-defence-in-helper-not-in-core)
- [Static frontend, not a JS framework](#static-frontend-not-a-js-framework)
- [Push events, not polling](#push-events-not-polling)
- [`gpg(1)` shell-out, not a pure-Rust OpenPGP library](#gpg1-shell-out-not-a-pure-rust-openpgp-library)
- [GPL-3.0-only, not Apache/MIT](#gpl-30-only-not-apachemit)
- [Tauri 2, not Tauri 1 or Electron](#tauri-2-not-tauri-1-or-electron)
- [`forbid(unsafe)`, not `deny(unsafe)`](#forbidunsafe-not-denyunsafe)
- [SemVer at 0.x means *anything* can break](#semver-at-0x-means-anything-can-break)

---

## Workspace shape

**Decision.** Four crates: `raidhos-core` (library),
`raidhos-cli`, `raidhos-priv-helper`, `raidhos-ui`. Plus a
static `frontend/` directory inside `raidhos-ui`.

**Reasoning.**

- Library separation gives the audit story a focal point —
  the trust boundary is one crate.
- CLI / helper / UI separation means each binary's
  dependencies are tiny. The UI doesn't drag Tauri into the
  helper's audit surface; the helper doesn't pull `clap_complete`
  into the UI.
- Frontend as static files means there's no JS build system
  to audit. Tauri 2 serves the files directly.

**Revisit if.** The UI surface grows by an order of magnitude
and the static-file approach becomes unworkable. Then we'd
reach for Svelte/Solid (still no Webpack) — see "Static
frontend, not a JS framework" below.

---

## Trust boundary in the library, not the CLI

**Decision.** All validation, all platform-specific behaviour,
and the install orchestrator live in `raidhos-core`. The CLI
and the UI are *thin* wrappers; the helper is *also* a thin
wrapper.

**Reasoning.** Three different binaries call the same code
paths. Putting validation in any one binary would leave the
other two unprotected. Centralising in the library means
auditors review one place.

**Revisit if.** Any binary acquires logic that the library
genuinely shouldn't see (e.g. UI-specific clipboard handling).
That stays in the binary; only the validation gates have to be
in the library.

---

## JSON-over-stdout for the helper, not D-Bus or a socket

**Decision.** `raidhos-priv-helper` reads `argv`, writes one
JSON object to stdout, exits. No daemon, no socket, no IPC
framework.

**Reasoning.**

- A daemon means a long-lived privileged process, lifecycle
  management, authentication on the wire, and ongoing exposure
  to client requests.
- Fork-exec-with-pkexec means the OS handles authentication
  via the platform's identity stack (polkit / Authorization
  Services / UAC). One-shot stdout is well-understood by every
  language.
- Auditors can read the helper end-to-end in 200 lines.

**Trade-off.** Concurrent installs are impossible without two
elevation prompts. That's a feature in this context.

**Revisit if.** We ever need to expose an event stream
(progress, large logs) that doesn't fit one terminal frame.
Tauri's IPC channel already covers UI progress; the CLI is
fine with line-oriented output.

---

## Hand-rolled parsers, not crates

**Decision.** The `lsblk` JSON, `diskutil` XML plist, and
`Get-Disk` JSON parsers are hand-rolled in
[`crates/core/src/parsers.rs`](../crates/core/src/parsers.rs).

**Reasoning.**

- Each parser only cares about a handful of fields; the bulk
  of the upstream schema is irrelevant.
- A general XML library on macOS would add ~10 K SLOC of
  unrelated parser logic to the audit surface.
- Hand-rolled is fuzz-friendly; the parsers are pure-functions
  over `&[u8]`, ideal `cargo-fuzz` shape.

**Revisit if.** Microsoft or Apple ever change their output
format in a way that breaks the tolerant text walkers (e.g.
plist 2.0). At that point we either still hand-roll, or pick a
narrowly-scoped crate.

---

## Two opt-ins, not one

**Decision.** Both `wipe == true` and `allow_write == true`
must be true for the install to write. CLI defaults
`--wipe=true` and `--allow-write=false`, so a careless
re-invocation can't go destructive.

**Reasoning.**

- One flag is too easy to muscle-memory. Two means the user has
  to *think* between dry-run and real.
- The flags are also semantically distinct: `wipe` says "I
  expect destruction"; `allow_write` says "do it now, this is
  not a rehearsal."

**Trade-off.** Slight scripting overhead.

**Revisit if.** Telemetry (which we don't collect) shows users
routinely setting both in a single command — at which point we
might add a `--yes-really` aggregate. But the friction is the
point.

---

## `thiserror`, not `anyhow`

**Decision.** `CoreError` is a typed enum via `thiserror`.

**Reasoning.**

- The CLI / UI / helper all match on error *kinds*
  (`Validation`, `NotImplemented`, etc.) to decide their exit
  code and UX. `anyhow` flattens that to a stringly-typed blob.
- Library code should expose typed errors; `anyhow` is for
  application code that doesn't need to differentiate.

**Trade-off.** A bit more boilerplate when adding variants.

---

## `clap` everywhere, not custom argv

**Decision.** Both `raidhos-cli` and `raidhos-priv-helper`
parse argv via `clap` derive.

**Reasoning.**

- `clap` rejects unknown flags loudly (security-relevant for
  the helper).
- `clap`-derived structs are self-documenting and produce
  great man pages + completions for free.
- The helper accepting hand-rolled flags is what produced the
  `internal-worker` confusion in early v0.0.1; removing that
  was a strict simplification.

---

## Seccomp denylist, not allowlist

**Decision.** The helper installs a small seccomp **denylist**
of dangerous syscalls (`ptrace`, `bpf`, `kexec_*`, …), not an
allowlist of permitted syscalls.

**Reasoning.**

- An allowlist would have to cover every syscall every child
  process (`parted`, `mkfs.vfat`, `mount`, …) uses. That's a
  large surface that varies by libc version, glibc vs musl,
  and minor distro differences.
- The denylist catches the dangerous primitives unambiguously
  and avoids breaking legitimate work.

**Trade-off.** A novel escalation primitive added to the
kernel that isn't on the denylist would slip through. Mitigation:
the denylist is small enough to audit and update.

**Revisit if.** The helper is restructured into a tightly-bounded
worker process where an allowlist is tractable. Track in
v0.1.0.

---

## TOCTOU defence in helper, not in core

**Decision.** The `OpenOptions::open` + `fstat` + hold-the-fd
pattern lives in
[`crates/priv-helper/src/toctou.rs`](../crates/priv-helper/src/toctou.rs),
not in `raidhos-core`.

**Reasoning.** Holding an `fd` for the install lifetime is a
*process* property, not a *library* property. A library
function that opens an fd and returns can't keep it alive past
the caller's scope. The helper, being the one process that
spans the install, is the right home.

**Trade-off.** The CLI's `install` subcommand doesn't get
TOCTOU protection unless it routes through the helper. By
v0.0.2 we want all destructive paths to do so.

---

## Static frontend, not a JS framework

**Decision.** Plain HTML/CSS/JS under `frontend/`. No npm, no
bundler, no Svelte / Solid / React.

**Reasoning.**

- The audit surface gains nothing from JSX or reactivity. The
  install pipeline is intrinsically procedural.
- Tauri 2 serves the static files directly.
- No `package-lock.json` to babysit; no npm supply-chain risk.

**Revisit if.** The UI gains state complexity that vanilla JS
can't handle cleanly — at which point Solid / Lit are likely
candidates because both can run framework-free without
bundling.

---

## Push events, not polling

**Decision.** Progress is emitted via Tauri's event system
(`raidhos://progress`). The frontend subscribes; the backend
emits.

**Reasoning.**

- Polling a `Mutex<Vec<…>>` from JS introduces shared mutable
  state and a polling interval.
- Push events are typed, push-once, never lost.

**Trade-off.** Tauri's event system is opaque to non-Tauri
callers. The CLI prints progress to stdout instead.

---

## `gpg(1)` shell-out, not a pure-Rust OpenPGP library

**Decision.** `verify_iso` invokes the `gpg` binary against an
ephemeral GNUPGHOME, not `sequoia-openpgp` or `rpgp`.

**Reasoning.**

- `gpg(1)` is the de facto standard, battle-tested, present on
  most user systems and well-known to security reviewers.
- Pure-Rust OpenPGP stacks are mature enough but add 10–30
  thousand SLOC of cryptographic code to our audit surface.
- The ephemeral GNUPGHOME means we never touch the user's
  real keyring.

**Trade-off.** Requires `gpg` to be installed. We surface a
clear error if it isn't.

**Revisit if.** A pure-Rust OpenPGP stack stabilises and gains
the same maturity reputation as `rustls` vs `openssl`. Looking
at you, `sequoia-openpgp`.

---

## GPL-3.0-only, not Apache/MIT

**Decision.** GPL-3.0-only.

**Reasoning.**

- RaidhOS links against `parted` (GPL-3) and `grub` (GPL-3) at
  runtime, and bundles GPL-3 Ventoy payload bytes. Anything
  more permissive would create derivative-work confusion.
- GPL-3.0-only (not `-or-later`) keeps the licence pinned to a
  specific revision — important for a security tool where the
  patent terms matter.

**Revisit if.** Upstream relicensing changes the inputs (this
is extremely unlikely for `parted` / `grub`).

---

## Tauri 2, not Tauri 1 or Electron

**Decision.** Tauri 2 (post-2024-stable).

**Reasoning.**

- Smaller binary than Electron (<10 MiB vs >100 MiB).
- Native OS WebView, not bundled Chromium.
- Tauri 2's capabilities model is a genuine security upgrade
  over Tauri 1's allowlist.

**Trade-off.** Tauri 2 reached stable mid-2024; some plugins
were still maturing in 2025. We've absorbed that maturity gap
by v0.0.1.

---

## `forbid(unsafe)`, not `deny(unsafe)`

**Decision.** Workspace-wide `unsafe_code = "forbid"`.

**Reasoning.** `deny` can be overridden with
`#![allow(unsafe_code)]`. `forbid` cannot. A single line in a
PR that crosses the line would have to be visible and
intentional — and we'd reject it.

---

## SemVer at 0.x means *anything* can break

**Decision.** While the project is `0.x.y`, every release may
contain breaking changes — in argv, in JSON shapes, in `boot.json`,
in `manifest.json`. We will signpost in the changelog, but
downstream packagers should treat `0.x` as pre-1.0 even though
each release is signed and reproducible.

**Reasoning.** The library API surface is still moving. Locking
it in too early gives us bad ergonomics for years.

**Revisit at v1.0.** At which point breaking changes become
proper major bumps and we maintain at least one stable
predecessor minor.
