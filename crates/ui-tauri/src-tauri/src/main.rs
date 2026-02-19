#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use raidhos_core as core;
mod grub;

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

#[derive(Serialize)]
struct PartitionInfo {
    id: String,
    label: String,
    fstype: String,
    mountpoints: Vec<String>,
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
fn list_partitions(device: String) -> Result<Vec<PartitionInfo>, String> {
    let parts = core::list_partitions(device).map_err(|e| e.to_string())?;
    Ok(parts
        .into_iter()
        .map(|p| PartitionInfo {
            id: p.id,
            label: p.label,
            fstype: p.fstype,
            mountpoints: p.mountpoints,
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

#[tauri::command]
fn get_payload_version() -> Result<String, String> {
    let candidates = [
        "payload/manifest.json",
        "../payload/manifest.json",
        "../../payload/manifest.json",
    ];
    for path in candidates {
        if let Ok(body) = std::fs::read(path) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) {
                if let Some(v) = value.get("version").and_then(|v| v.as_str()) {
                    return Ok(v.to_string());
                }
            }
        }
    }
    Ok("unknown".to_string())
}

#[tauri::command]
fn install_elevated(device: String, payload_version: String) -> Result<String, String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let output = std::process::Command::new("pkexec")
        .arg(current_exe)
        .arg("internal-worker")
        .arg("--task")
        .arg("install")
        .arg("--device")
        .arg(device)
        .arg("--payload-version")
        .arg(payload_version)
        .output()
        .map_err(|e| format!("Failed to launch pkexec: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(if err.is_empty() { "Elevation failed or was cancelled by user".to_string() } else { err })
    }
}

fn maybe_run_internal_worker() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("internal-worker") {
        return false;
    }

    let mut task = String::new();
    let mut device = String::new();
    let mut payload_version = String::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--task" => {
                if let Some(v) = args.get(i + 1) { task = v.clone(); }
                i += 2;
            }
            "--device" => {
                if let Some(v) = args.get(i + 1) { device = v.clone(); }
                i += 2;
            }
            "--payload-version" => {
                if let Some(v) = args.get(i + 1) { payload_version = v.clone(); }
                i += 2;
            }
            _ => i += 1,
        }
    }

    if task == "install" && !device.is_empty() {
        let res = run_worker_install(&device, &payload_version);
        match res {
            Ok(msg) => {
                println!("{msg}");
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }

    eprintln!("invalid internal-worker invocation");
    std::process::exit(2);
}

fn run_worker_install(device: &str, payload_version: &str) -> Result<String, String> {
    // attempt to unmount partitions
    if let Ok(parts) = core::list_partitions(device.to_string()) {
        for p in parts {
            for mp in p.mountpoints {
                let _ = std::process::Command::new("umount").arg(&mp).status();
            }
        }
    }
    let _ = std::process::Command::new("wipefs").args(["-a", device]).status();

    struct StdoutSink;
    impl core::ProgressSink for StdoutSink {
        fn emit(&self, event: core::ProgressEvent) {
            let pct = event.percent.map(|p| format!("{p}%")).unwrap_or_default();
            println!("{} {} {}", event.phase, event.message, pct);
        }
    }

    let req = core::InstallRequest {
        device: device.to_string(),
        payload_version: payload_version.to_string(),
        wipe: true,
        dry_run: false,
        allow_write: true,
    };

    core::install(req, &StdoutSink).map_err(|e| e.to_string())?;
    Ok("install complete".to_string())
}

#[tauri::command]
fn write_grub_cfg_to_esp(esp_mount: String, config: BootConfig, data_label: String) -> Result<(), String> {
    let cfg = grub::render_grub_cfg(&config, &data_label);
    let path = std::path::Path::new(&esp_mount)
        .join("EFI")
        .join("BOOT")
        .join("grub.cfg");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, cfg).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn copy_isos_to_data(mount_path: String, sources: Vec<String>) -> Result<Vec<String>, String> {
    let dest_dir = std::path::Path::new(&mount_path).join("boot").join("isos");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let mut copied = Vec::new();
    for src in sources {
        let src_path = std::path::Path::new(&src);
        if !src_path.exists() {
            continue;
        }
        if let Some(name) = src_path.file_name() {
            let dest = dest_dir.join(name);
            std::fs::copy(&src_path, &dest).map_err(|e| e.to_string())?;
            copied.push(dest.display().to_string());
        }
    }
    Ok(copied)
}


fn main() {
    if maybe_run_internal_worker() {
        return;
    }

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_disks,
            install,
            scan_isos,
            save_boot_config,
            write_boot_config_to_device,
            get_payload_version,
            list_partitions,
            write_grub_cfg_to_esp,
            copy_isos_to_data,
            install_elevated
        ])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
