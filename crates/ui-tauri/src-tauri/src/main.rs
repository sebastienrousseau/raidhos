#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
fn list_disks(_state: State<'_, AppState>) -> Vec<DiskInfo> {
    // Placeholder wiring. Will call raidhos-core once disk discovery is implemented.
    vec![DiskInfo {
        id: String::from("/dev/sdx"),
        model: String::from("USB Device"),
        size_bytes: 64 * 1024 * 1024 * 1024,
        removable: true,
    }]
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_disks])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
