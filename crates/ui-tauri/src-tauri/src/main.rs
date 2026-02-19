#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use raidhos_core as core;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Clone, Default)]
struct AppState {
    last_events: Mutex<Vec<ProgressEvent>>,
}

#[derive(Serialize)]
struct DiskInfo {
    id: String,
    model: String,
    size_bytes: u64,
    removable: bool,
    mountpoints: Vec<String>,
    is_system: bool,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    phase: String,
    message: String,
    percent: Option<u8>,
}

struct VecSink<'a> {
    events: &'a Mutex<Vec<ProgressEvent>>,
}

impl<'a> core::ProgressSink for VecSink<'a> {
    fn emit(&self, event: core::ProgressEvent) {
        let mut guard = self.events.lock().expect("lock events");
        guard.push(ProgressEvent {
            phase: event.phase,
            message: event.message,
            percent: event.percent,
        });
    }
}

#[derive(Deserialize)]
struct InstallArgs {
    device: String,
    payload_version: String,
    wipe: bool,
    dry_run: bool,
}

#[tauri::command]
fn list_disks() -> Result<Vec<DiskInfo>, String> {
    let disks = core::list_disks().map_err(|e| e.to_string())?;
    Ok(disks
        .into_iter()
        .map(|d| DiskInfo {
            id: d.id,
            model: d.model,
            size_bytes: d.size_bytes,
            removable: d.removable,
            mountpoints: d.mountpoints,
            is_system: d.is_system,
        })
        .collect())
}

#[tauri::command]
fn install(args: InstallArgs, state: State<'_, AppState>) -> Result<Vec<ProgressEvent>, String> {
    {
        let mut guard = state.last_events.lock().expect("lock events");
        guard.clear();
    }

    let sink = VecSink {
        events: &state.last_events,
    };

    let req = core::InstallRequest {
        device: args.device,
        payload_version: args.payload_version,
        wipe: args.wipe,
        dry_run: args.dry_run,
    };

    core::install(req, &sink).map_err(|e| e.to_string())?;

    let guard = state.last_events.lock().expect("lock events");
    Ok(guard.clone())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_disks, install])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
