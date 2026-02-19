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

#[derive(Serialize)]
struct IsoEntry {
    title: String,
    path: String,
    size_bytes: u64,
    params: String,
}

#[derive(Deserialize)]
struct BootConfig {
    entries: Vec<BootEntryConfig>,
    default_entry: Option<String>,
}

#[derive(Deserialize)]
struct BootEntryConfig {
    title: String,
    path: String,
    params: String,
    initrd: String,
    kargs: String,
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
    allow_write: bool,
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
        allow_write: args.allow_write,
    };

    core::install(req, &sink).map_err(|e| e.to_string())?;

    let guard = state.last_events.lock().expect("lock events");
    Ok(guard.clone())
}

#[tauri::command]
fn scan_isos(dirs: Vec<String>) -> Result<Vec<IsoEntry>, String> {
    let entries = core::scan_isos(dirs).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| IsoEntry {
            title: e.title,
            path: e.path,
            size_bytes: e.size_bytes,
            params: e.params,
        })
        .collect())
}

#[tauri::command]
fn save_boot_config(config: BootConfig) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let dir = std::path::Path::new(&home).join(".config").join("raidhos");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("boot.json");
    let body = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn write_boot_config_to_device(mount_path: String, config: BootConfig) -> Result<(), String> {
    let dir = std::path::Path::new(&mount_path).join("raidhos");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("boot.json");
    let body = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_disks, install, scan_isos, save_boot_config, write_boot_config_to_device])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
