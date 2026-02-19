#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use raidhos_core as core;
use serde::Serialize;
use tauri::State;

#[derive(Clone, Default)]
struct AppState;

#[derive(Serialize)]
struct DiskInfo {
    id: String,
    model: String,
    size_bytes: u64,
    removable: bool,
}

#[tauri::command]
fn list_disks(_state: State<'_, AppState>) -> Result<Vec<DiskInfo>, String> {
    let disks = core::list_disks().map_err(|e| e.to_string())?;
    Ok(disks
        .into_iter()
        .map(|d| DiskInfo {
            id: d.id,
            model: d.model,
            size_bytes: d.size_bytes,
            removable: d.removable,
        })
        .collect())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_disks])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
