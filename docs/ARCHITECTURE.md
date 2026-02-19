# Architecture Overview

RaidhOS is a cross-platform USB installer with a Rust core and a polished Tauri UI.

## Components

- `crates/core` (raidhos-core)
  - Device discovery, safety checks, and installer orchestration.
- `crates/cli`
  - Developer-facing CLI wrapper around `raidhos-core`.
- `crates/priv-helper`
  - Minimal privileged helper for disk operations per OS.
- `crates/ui-tauri`
  - Tauri-based desktop UI (frontend + Rust commands).
- `payload/`
  - Ventoy payload version and metadata.

## Data Flow

1. UI requests disk list from core.
2. User selects target and confirms destructive action.
3. UI invokes privileged helper for partition + format + payload install.
4. Progress and logs stream back to UI.

## Safety Principles

- Block system disks by default.
- Require explicit device selection.
- Double confirmation for destructive writes.
- Clear logs and undo guidance.
