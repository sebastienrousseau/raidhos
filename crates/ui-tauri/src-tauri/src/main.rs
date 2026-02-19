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
fn write_grub_cfg_to_esp(esp_mount: String, config: BootConfig, data_label: String) -> Result<(), String> {
    let cfg = render_grub_cfg(&config, &data_label);
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

fn render_grub_cfg(config: &BootConfig, data_label: &str) -> String {
    let mut out = String::new();
    out.push_str("set timeout=5\n");
    if let Some(default) = &config.default_entry {
        out.push_str(&format!("set default=\"{}\"\n", sanitize(default)));
    }
    out.push_str("insmod part_gpt\n");
    out.push_str("insmod fat\n");
    out.push_str("insmod exfat\n");
    out.push_str("insmod iso9660\n");
    out.push_str("insmod loopback\n");
    out.push_str("insmod search\n");
    out.push_str(&format!(
        "search --no-floppy --label {} --set=root\n",
        sanitize(data_label)
    ));
    out.push_str("set isopath=/boot/isos\n");
    out.push_str("export root\n");
    out.push_str("export isopath\n");

    for entry in &config.entries {
        let title = sanitize(&entry.title);
        let path = sanitize(&entry.path);
        let params = sanitize(&entry.params);
        let initrd = sanitize(&entry.initrd);
        let kargs = sanitize(&entry.kargs);

        out.push_str(&format!("menuentry \"{}\" {{\n", title));
        out.push_str(&format!("  set isofile=\"($root){}\"\n", path_prefix(&path)));
        out.push_str("  loopback loop $isofile\n");
        out.push_str("  if [ -f (loop)/boot/grub/grub.cfg ]; then\n");
        out.push_str("    configfile (loop)/boot/grub/grub.cfg\n");
        out.push_str("  elif [ -f (loop)/casper/vmlinuz ]; then\n");
        out.push_str(&format!(
            "    linux (loop)/casper/vmlinuz {} {} iso-scan/filename=$isofile\n",
            params, kargs
        ));
        if !initrd.is_empty() {
            out.push_str(&format!("    initrd {}\n", initrd));
        } else {
            out.push_str("    initrd (loop)/casper/initrd\n");
        }
        out.push_str("  elif [ -f (loop)/live/vmlinuz ]; then\n");
        out.push_str(&format!(
            "    linux (loop)/live/vmlinuz {} {} boot=live findiso=$isofile\n",
            params, kargs
        ));
        if !initrd.is_empty() {
            out.push_str(&format!("    initrd {}\n", initrd));
        } else {
            out.push_str("    initrd (loop)/live/initrd.img\n");
        }
        out.push_str("  else\n");
        out.push_str("    echo \"No known kernel path found in ISO.\"\n");
        out.push_str("  fi\n");
        out.push_str("}\n");
    }
    out
}

fn sanitize(input: &str) -> String {
    input.replace('"', "").replace('\n', " ").trim().to_string()
}

fn path_prefix(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_disks, install, scan_isos, save_boot_config, write_boot_config_to_device, get_payload_version, list_partitions, write_grub_cfg_to_esp, copy_isos_to_data])
        .run(tauri::generate_context!())
        .expect("error while running RaidhOS");
}
