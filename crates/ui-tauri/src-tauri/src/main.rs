#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use raidhos_core as core;
mod grub;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewWindow as Window};

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

#[derive(Deserialize, Serialize)]
pub struct BootConfig {
    pub entries: Vec<BootEntryConfig>,
    pub default_entry: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct BootEntryConfig {
    pub title: String,
    pub path: String,
    pub params: String,
    pub initrd: String,
    pub kargs: String,
}

/// Tauri event channel name. Frontend subscribes via
/// `window.__TAURI__.event.listen("raidhos://progress", ...)`.
const PROGRESS_EVENT: &str = "raidhos://progress";

/// Push events from the install pipeline straight to the frontend
/// via Tauri's event bus. Replaces the `Mutex<Vec<…>>` polling
/// pattern.
struct WindowSink {
    window: Window,
}

impl core::ProgressSink for WindowSink {
    fn emit(&self, event: core::ProgressEvent) {
        let payload = ProgressEvent {
            phase: event.phase,
            message: event.message,
            percent: event.percent,
        };
        // Log but never panic — a failed event must not abort the
        // install. The frontend will fall back to the return value.
        let _ = self.window.emit(PROGRESS_EVENT, payload);
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
fn install(window: Window, args: InstallArgs) -> Result<(), String> {
    let sink = WindowSink {
        window: window.clone(),
    };
    let req = core::InstallRequest {
        device: args.device,
        payload_version: args.payload_version,
        wipe: args.wipe,
        dry_run: args.dry_run,
        allow_write: args.allow_write,
        // The GUI runs the install via the privileged helper; it
        // doesn't expose simulator mode. End-users who want
        // simulator preview should use `raidhos-cli install
        // --simulator …` from a terminal.
        simulator: false,
    };
    core::install(req, &sink).map_err(|e| e.to_string())?;
    Ok(())
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

fn project_config_dir() -> Result<std::path::PathBuf, String> {
    let dirs = directories::ProjectDirs::from("org", "raidhos", "raidhos")
        .ok_or_else(|| "no project config directory available on this platform".to_string())?;
    Ok(dirs.config_dir().to_path_buf())
}

#[tauri::command]
fn save_boot_config(config: BootConfig) -> Result<(), String> {
    let dir = project_config_dir()?;
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

/// Locate the privileged helper binary next to this executable.
///
/// In dev (`cargo run`), it lives at `target/debug/raidhos-priv-helper`
/// alongside `raidhos-ui`. In packaged builds (deb/Homebrew/AppImage)
/// it lives in the same install prefix as the UI binary.
fn locate_priv_helper() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no parent dir for current_exe".to_string())?;
    let candidates: &[&str] = if cfg!(windows) {
        &["raidhos-priv-helper.exe"]
    } else {
        &["raidhos-priv-helper"]
    };
    for name in candidates {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "raidhos-priv-helper not found next to {}",
        exe.display()
    ))
}

#[tauri::command]
fn install_elevated(
    window: Window,
    device: String,
    payload_version: String,
) -> Result<String, String> {
    let helper = locate_priv_helper()?;

    // Emit a "starting elevation" event so the UI can show a spinner.
    let _ = window.emit(
        PROGRESS_EVENT,
        ProgressEvent {
            phase: "elevate".to_string(),
            message: "Requesting administrator privileges".to_string(),
            percent: Some(1),
        },
    );

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = std::process::Command::new("pkexec");
        c.arg(&helper);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        // osascript with "do shell script with administrator privileges"
        // returns the command's stdout. Quote the helper path defensively.
        let helper_str = helper.to_string_lossy().to_string();
        let script = format!(
            r#"do shell script "{} install --device {} --payload-version {} --allow-write" with administrator privileges"#,
            helper_str,
            shell_quote_for_osascript(&device),
            shell_quote_for_osascript(&payload_version),
        );
        let mut c = std::process::Command::new("osascript");
        c.arg("-e").arg(script);
        c
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        // PowerShell Start-Process -Verb RunAs triggers UAC.
        let script = format!(
            "Start-Process -FilePath '{}' -ArgumentList 'install','--device','{}','--payload-version','{}','--allow-write' -Verb RunAs -Wait -PassThru",
            helper.to_string_lossy(),
            device.replace('\'', "''"),
            payload_version.replace('\'', "''"),
        );
        let mut c = std::process::Command::new("powershell");
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ]);
        c
    };

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new(&helper);

    // For Linux, we still need to append the install args after pkexec
    // gets the helper path.
    #[cfg(target_os = "linux")]
    {
        cmd.arg("install")
            .arg("--device")
            .arg(&device)
            .arg("--payload-version")
            .arg(&payload_version)
            .arg("--allow-write");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to launch elevation: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(if err.is_empty() {
            "Elevation failed or was cancelled by user".to_string()
        } else {
            err
        })
    }
}

#[cfg(target_os = "macos")]
fn shell_quote_for_osascript(s: &str) -> String {
    // osascript "do shell script" double-quotes its argument; we need
    // to escape any embedded `"` and `\`.
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[tauri::command]
fn write_grub_cfg_to_esp(
    esp_mount: String,
    config: BootConfig,
    data_label: String,
) -> Result<(), String> {
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
            std::fs::copy(src_path, &dest).map_err(|e| e.to_string())?;
            copied.push(dest.display().to_string());
        }
    }
    Ok(copied)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_double_quote() {
        let q = shell_quote_for_osascript(r#"hello"world"#);
        assert_eq!(q, r#"hello\"world"#);
    }

    #[test]
    fn shell_quote_escapes_backslash() {
        let q = shell_quote_for_osascript(r"path\with\slash");
        assert_eq!(q, r"path\\with\\slash");
    }

    #[test]
    fn shell_quote_escapes_backslash_before_quote() {
        // Order matters — backslashes must escape first so a literal
        // `\` immediately followed by `"` becomes `\\` then `\"`.
        let q = shell_quote_for_osascript(r#"a\"b"#);
        assert_eq!(q, r#"a\\\"b"#);
    }

    #[test]
    fn shell_quote_passes_through_plain_text() {
        let q = shell_quote_for_osascript("just text 123");
        assert_eq!(q, "just text 123");
    }

    #[test]
    fn shell_quote_handles_empty_string() {
        assert_eq!(shell_quote_for_osascript(""), "");
    }

    #[test]
    fn project_config_dir_returns_some_path_on_supported_host() {
        // directories::ProjectDirs::from returns Some on Linux / macOS /
        // Windows. We don't pin the actual path because it varies by
        // host; we just verify the function succeeds and yields
        // something that looks like a config directory.
        let p = project_config_dir();
        assert!(p.is_ok(), "got {p:?}");
        let pb = p.unwrap();
        let s = pb.to_string_lossy();
        // It either ends with our org/app segment, or it lives somewhere
        // sensible under the user's home / appdata / xdg-config.
        assert!(
            s.contains("raidhos") || !pb.as_os_str().is_empty(),
            "unexpected config dir: {s}",
        );
    }

    #[test]
    fn copy_isos_to_data_copies_existing_iso_and_skips_missing() {
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-ui-copy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let mount = scratch.join("mount");
        let real_iso = scratch.join("ubuntu.iso");
        std::fs::write(&real_iso, b"fake-iso-bytes").unwrap();

        let copied = copy_isos_to_data(
            mount.display().to_string(),
            vec![
                real_iso.display().to_string(),
                "/no/such/missing.iso".to_string(),
            ],
        )
        .expect("copy");

        assert_eq!(copied.len(), 1);
        assert!(copied[0].ends_with("ubuntu.iso"));
        assert!(mount.join("boot").join("isos").join("ubuntu.iso").exists());
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn write_grub_cfg_to_esp_writes_file_under_efi_boot() {
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-ui-grubcfg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).unwrap();

        let cfg = BootConfig {
            entries: vec![BootEntryConfig {
                title: "ubuntu".into(),
                path: "boot/isos/ubuntu.iso".into(),
                params: "quiet splash".into(),
                initrd: String::new(),
                kargs: String::new(),
            }],
            default_entry: Some("ubuntu".into()),
        };
        let res = write_grub_cfg_to_esp(scratch.display().to_string(), cfg, "DATA".into());
        assert!(res.is_ok(), "got {res:?}");
        let path = scratch.join("EFI").join("BOOT").join("grub.cfg");
        assert!(path.exists(), "missing grub.cfg at {}", path.display());

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("ubuntu"));
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
