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

/// Result of a pre-flight ISO verification: did we find a SHA-256
/// companion file, did the recomputed hash match, and (if helpful)
/// the hash itself for display.
#[derive(Serialize)]
struct IsoVerification {
    /// One of `ok` (companion present + matched), `missing` (no
    /// `<iso>.sha256` next to it — can't say either way), or
    /// `mismatch` (companion present but the recomputed hash
    /// differed — treat as corrupted / tampered).
    kind: String,
    /// Human-readable detail surfaced in the UI tooltip.
    message: String,
}

#[derive(Deserialize, Serialize)]
pub struct BootConfig {
    pub entries: Vec<BootEntryConfig>,
    pub default_entry: Option<String>,
    /// Optional GRUB superuser name (Ventoy gap G13). When set,
    /// the renderer emits `set superusers="NAME"` at the top of
    /// grub.cfg. Empty means no password gating.
    #[serde(default)]
    pub grub_superuser: String,
    /// Optional PBKDF2 hash for the superuser. **Must** be the
    /// output of `grub-mkpasswd-pbkdf2`, starting with
    /// `grub.pbkdf2.sha512.…`. The renderer emits
    /// `password_pbkdf2 NAME HASH`. Plaintext passwords are
    /// rejected by `is_grub_pbkdf2_hash()`.
    #[serde(default)]
    pub grub_password_pbkdf2: String,
    /// If `true`, the renderer groups entries by their `class`
    /// field into GRUB `submenu` blocks instead of a flat list.
    /// Entries without a class fall through to the top level so
    /// power-user entries stay one keypress away. Closes Ventoy
    /// gap G20 (ListView ↔ TreeView toggle).
    #[serde(default)]
    pub tree_view: bool,
    /// If `true`, the renderer appends an F2-hotkeyed "Browse
    /// local disks" menu entry that walks (hd*,*) and chains
    /// into any discovered `/boot/grub/grub.cfg`. Lets users
    /// boot ISOs that live on an internal drive rather than on
    /// the USB. Closes Ventoy gap G22.
    #[serde(default)]
    pub enable_disk_browser: bool,
}

#[derive(Deserialize, Serialize)]
pub struct BootEntryConfig {
    pub title: String,
    pub path: String,
    pub params: String,
    pub initrd: String,
    pub kargs: String,
    /// Optional short tag used to group entries (Ventoy's
    /// `menu_class`). When set, the renderer emits `--class TAG`
    /// on the `menuentry` so a GRUB theme can style entries by
    /// class. Sanitised before output. Closes Ventoy gap G11.
    #[serde(default)]
    pub class: String,
    /// Optional one-line help text shown beside the entry
    /// (Ventoy's `menu_tip`). Rendered as a GRUB comment above
    /// the `menuentry` block. Sanitised before output.
    #[serde(default)]
    pub tip: String,
    /// If `true`, the renderer skips this entry entirely
    /// (Ventoy's `image_blacklist`). Lets users keep an ISO on
    /// the USB but hide it from the menu. Closes Ventoy gap G17.
    #[serde(default)]
    pub hidden: bool,
    /// Optional path to a persistence-backend file on the DATA
    /// partition (Ventoy's `persistence` plugin per-ISO map).
    /// When set, the renderer appends `persistent
    /// persistent-path=<value>` to the live-kernel command line.
    /// Closes Ventoy gap G18. Pair with the per-distro label table
    /// in `raidhos_core::expected_persistence_label`.
    #[serde(default)]
    pub persistence_backend: String,
    /// Optional auto-install descriptor (Ventoy gap G12).
    /// When `kind` is anything other than `none` and `path` is
    /// non-empty, the renderer appends the right per-distro
    /// kargs to the linux line — kickstart for Fedora/RHEL,
    /// preseed for Debian, autoinstall for Ubuntu subiquity,
    /// autoyast for SUSE, or cloud-init nocloud for distros
    /// that read from a `user-data` directory. See
    /// `AutoinstallKind` for the per-kind karg shape.
    #[serde(default)]
    pub autoinstall: AutoinstallConfig,
    /// Optional override path for the boot configuration
    /// (Ventoy gap G16, partial). When set, the renderer
    /// emits `configfile ($root)<path>` *first* on the ISO
    /// branch; if the file exists at boot, GRUB chains into
    /// it instead of searching inside the ISO for a
    /// `casper/vmlinuz` / `live/vmlinuz` / embedded grub.cfg.
    /// Falls back to the auto-detect logic when the file is
    /// missing, so the field is safe to leave set even when
    /// the override hasn't been written yet.
    ///
    /// This does **not** replicate Ventoy's sed-style
    /// substitution inside the ISO's own grub.cfg — that
    /// requires an init shim that v0.0.1 doesn't ship.
    #[serde(default)]
    pub conf_replace_path: String,
}

/// Per-entry auto-install descriptor (Ventoy gap G12). Kept
/// distro-aware so the user describes intent (`kind: kickstart`,
/// `path: /ks/centos.ks`) and the renderer figures out the karg
/// shape — `inst.ks=hd:LABEL=<data>:/ks/centos.ks` for Fedora,
/// `auto=install preseed/file=…` for Debian, and so on.
#[derive(Deserialize, Serialize, Default, Clone)]
pub struct AutoinstallConfig {
    /// Auto-install mechanism. `None` (the default) means no
    /// kargs are added; this is back-compat-safe for older
    /// configs that don't carry the field.
    #[serde(default)]
    pub kind: AutoinstallKind,
    /// Path on the DATA partition (or URL for cloud-init
    /// network sources). Sanitised before emission.
    #[serde(default)]
    pub path: String,
}

/// Auto-install mechanism family. Each variant maps to a
/// per-distro karg pattern in `autoinstall_kargs()`.
#[derive(Deserialize, Serialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum AutoinstallKind {
    /// No auto-install. Default — emits no extra kargs.
    #[default]
    None,
    /// Fedora / RHEL / Rocky / Alma / openSUSE Leap installers.
    /// Karg: `inst.ks=hd:LABEL=<DATA>:<path>`.
    Kickstart,
    /// Debian / Ubuntu (live-installer) preseed.
    /// Karg: `auto=install preseed/file=<path>`.
    Preseed,
    /// Ubuntu subiquity (24.04+ desktop / server).
    /// Karg: `autoinstall ds=nocloud;s=<path>`.
    Autoinstall,
    /// openSUSE / SLE AutoYaST.
    /// Karg: `autoyast=<path>`.
    Autoyast,
    /// cloud-init NoCloud datasource (generic).
    /// Karg: `ds=nocloud;s=<path>`.
    CloudInit,
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
        bios_compat: false,
        data_filesystem: Default::default(),
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

/// Open the native file picker filtered to `.iso` files and
/// return the picked paths. We call the dialog plugin from Rust
/// because the JS-side global (`window.__TAURI__.dialog.open`)
/// isn't reliably exposed under `withGlobalTauri: true` —
/// invoking the picker via `invoke('open_iso_picker')` works
/// universally. Returns an empty vec when the user cancels.
#[tauri::command]
async fn open_iso_picker(app: tauri::AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    // `blocking_pick_files` blocks the current thread; that's
    // safe here because the command is `async`, so Tauri runs it
    // on a worker thread rather than the main event loop.
    let picked = app
        .dialog()
        .file()
        .add_filter("ISO images", &["iso"])
        .blocking_pick_files()
        .unwrap_or_default();
    picked
        .into_iter()
        .filter_map(|fp| match fp {
            tauri_plugin_dialog::FilePath::Path(pb) => Some(pb.display().to_string()),
            tauri_plugin_dialog::FilePath::Url(_) => None,
        })
        .collect()
}

/// Host metadata + sensible default ISO scan locations. The
/// frontend uses these to render OS-appropriate help text (e.g.
/// "drop ISOs in ~/Downloads" on macOS rather than the Linux
/// `/media`, `/mnt`, `/home` defaults).
#[derive(Serialize)]
struct HostInfo {
    /// Lowercased OS family: `linux`, `macos`, `windows`, or
    /// `unknown`. Used by the frontend to pick copy strings.
    os: String,
    /// Suggested directories to scan for ISOs. macOS gets the
    /// home `Downloads` / `Desktop` / `Documents`; Windows gets
    /// the user-profile equivalents; Linux keeps the original
    /// `/media`, `/mnt`, `/home` set.
    suggested_scan_dirs: Vec<String>,
}

#[tauri::command]
fn get_host_info() -> HostInfo {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
    .to_string();

    let home = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok());

    let dirs = match os.as_str() {
        "macos" => home
            .as_ref()
            .map(|h| {
                vec![
                    format!("{h}/Downloads"),
                    format!("{h}/Desktop"),
                    format!("{h}/Documents"),
                ]
            })
            .unwrap_or_else(|| vec!["~/Downloads".into()]),
        "windows" => home
            .as_ref()
            .map(|h| {
                vec![
                    format!("{h}\\Downloads"),
                    format!("{h}\\Desktop"),
                    format!("{h}\\Documents"),
                ]
            })
            .unwrap_or_else(|| vec!["%USERPROFILE%\\Downloads".into()]),
        _ => vec!["/media".into(), "/mnt".into(), "/home".into()],
    };

    HostInfo {
        os,
        suggested_scan_dirs: dirs,
    }
}

/// Pre-flight check: look for a `<iso>.sha256` companion file
/// alongside the ISO and verify the recomputed SHA-256 matches.
/// Never errors out — returns a structured `IsoVerification` so the
/// frontend can show a green/amber/red badge per entry. A `missing`
/// result is the common case (most ISOs don't ship companion files
/// in place) and should be shown neutrally, not as a failure.
#[tauri::command]
fn verify_iso(path: String) -> IsoVerification {
    let p = std::path::Path::new(&path);
    match core::verify_iso_companion_sha256(p) {
        Ok(hash) => IsoVerification {
            kind: "ok".to_string(),
            message: format!("SHA-256 verified ({}…)", &hash[..16.min(hash.len())]),
        },
        Err(core::CatalogError::Sha256Mismatch { expected, computed }) => IsoVerification {
            kind: "mismatch".to_string(),
            message: format!(
                "Hash mismatch — expected {}… got {}…",
                &expected[..16.min(expected.len())],
                &computed[..16.min(computed.len())]
            ),
        },
        Err(e) => {
            // Most "errors" here are just "no .sha256 companion file
            // present". Surface them as `missing`, not failures, so
            // the UX stays calm.
            let msg = e.to_string();
            if msg.contains("read companion") {
                IsoVerification {
                    kind: "missing".to_string(),
                    message: "No SHA-256 companion file alongside the ISO".to_string(),
                }
            } else {
                IsoVerification {
                    kind: "missing".to_string(),
                    message: msg,
                }
            }
        }
    }
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
            verify_iso,
            get_host_info,
            open_iso_picker,
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
    fn get_host_info_returns_os_and_at_least_one_scan_dir() {
        let info = get_host_info();
        // The cfg switch covers Linux / macOS / Windows; the test
        // host has to be one of these three to run the workspace.
        assert!(
            ["linux", "macos", "windows"].contains(&info.os.as_str()),
            "unexpected os: {}",
            info.os
        );
        assert!(!info.suggested_scan_dirs.is_empty());
        // The macOS / Windows variants must not leak the Linux
        // defaults — that's the whole reason this command exists.
        if info.os == "macos" {
            assert!(
                info.suggested_scan_dirs
                    .iter()
                    .any(|d| d.contains("Downloads")),
                "macos dirs should include Downloads, got {:?}",
                info.suggested_scan_dirs
            );
            assert!(
                !info.suggested_scan_dirs.iter().any(|d| d == "/media"),
                "macos dirs should not include /media"
            );
        }
    }

    #[test]
    fn verify_iso_reports_missing_when_no_companion_file() {
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-ui-verify-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let iso = scratch.join("ubuntu.iso");
        std::fs::write(&iso, b"fake-iso-bytes").unwrap();

        let v = verify_iso(iso.display().to_string());
        assert_eq!(v.kind, "missing");
        assert!(
            v.message.contains("No SHA-256 companion") || v.message.contains("read companion"),
            "unexpected message: {}",
            v.message
        );

        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn verify_iso_reports_ok_when_companion_matches() {
        use sha2::{Digest, Sha256};
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-ui-verify-ok-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let iso = scratch.join("ubuntu.iso");
        let body = b"fake-iso-bytes";
        std::fs::write(&iso, body).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(body);
        let hex = format!("{:x}", hasher.finalize());
        std::fs::write(scratch.join("ubuntu.iso.sha256"), &hex).unwrap();

        let v = verify_iso(iso.display().to_string());
        assert_eq!(v.kind, "ok", "message was: {}", v.message);
        assert!(v.message.starts_with("SHA-256 verified"));

        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn verify_iso_reports_mismatch_when_companion_is_wrong() {
        let scratch = std::env::temp_dir().join(format!(
            "raidhos-ui-verify-mismatch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let iso = scratch.join("ubuntu.iso");
        std::fs::write(&iso, b"fake-iso-bytes").unwrap();
        // 64 hex chars but deliberately not the real hash.
        std::fs::write(scratch.join("ubuntu.iso.sha256"), "0".repeat(64)).unwrap();

        let v = verify_iso(iso.display().to_string());
        assert_eq!(v.kind, "mismatch");
        assert!(v.message.contains("mismatch"), "msg: {}", v.message);

        std::fs::remove_dir_all(&scratch).ok();
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
                class: String::new(),
                tip: String::new(),
                hidden: false,
                persistence_backend: String::new(),
                autoinstall: Default::default(),
                conf_replace_path: String::new(),
            }],
            default_entry: Some("ubuntu".into()),
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let res = write_grub_cfg_to_esp(scratch.display().to_string(), cfg, "DATA".into());
        assert!(res.is_ok(), "got {res:?}");
        let path = scratch.join("EFI").join("BOOT").join("grub.cfg");
        assert!(path.exists(), "missing grub.cfg at {}", path.display());

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("ubuntu"));
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// Round-trip the JSON shape that the Tauri frontend sends to
    /// `save_boot_config` (Ventoy gap G23). Any drift between
    /// `crates/ui-tauri/frontend/app.js` and the Rust struct field
    /// names will fail this — caught at unit-test time rather than
    /// silently dropping fields in production.
    #[test]
    fn boot_config_round_trips_frontend_json_shape() {
        let frontend_payload = serde_json::json!({
            "default_entry": "/u.iso",
            "entries": [
                {
                    "title": "Ubuntu 24.04",
                    "path": "/u.iso",
                    "params": "quiet splash",
                    "initrd": "",
                    "kargs": "",
                    "class": "linux",
                    "tip": "LTS desktop",
                    "hidden": false,
                    "persistence_backend": "/persistence/ubuntu.dat"
                }
            ],
            "tree_view": true,
            "enable_disk_browser": true,
            "grub_superuser": "admin",
            "grub_password_pbkdf2": "grub.pbkdf2.sha512.10000.deadbeef.cafef00d"
        });
        let cfg: BootConfig = serde_json::from_value(frontend_payload).expect("deserialise");
        assert_eq!(cfg.default_entry.as_deref(), Some("/u.iso"));
        assert_eq!(cfg.entries.len(), 1);
        assert_eq!(cfg.entries[0].class, "linux");
        assert_eq!(cfg.entries[0].tip, "LTS desktop");
        assert!(!cfg.entries[0].hidden);
        assert_eq!(
            cfg.entries[0].persistence_backend,
            "/persistence/ubuntu.dat"
        );
        assert!(cfg.tree_view);
        assert!(cfg.enable_disk_browser);
        assert_eq!(cfg.grub_superuser, "admin");
        assert_eq!(
            cfg.grub_password_pbkdf2,
            "grub.pbkdf2.sha512.10000.deadbeef.cafef00d"
        );
    }

    /// Pin the end-to-end example in `docs/BOOT_CONFIG.md` against
    /// the renderer so a documentation drift fails CI. The example
    /// exercises every v0.0.1 Ventoy-gap closure (G6 / G7 / G11 /
    /// G13 / G17 / G18 / G20 / G22).
    #[test]
    fn docs_boot_config_example_renders_every_feature() {
        let example = serde_json::json!({
            "default_entry": "/boot/isos/ubuntu-24.04.iso",
            "entries": [
                {
                    "title": "Ubuntu 24.04 LTS Desktop",
                    "path": "/boot/isos/ubuntu-24.04.iso",
                    "params": "quiet splash",
                    "initrd": "", "kargs": "",
                    "class": "linux", "tip": "LTS",
                    "hidden": false,
                    "persistence_backend": "/persistence/ubuntu.dat"
                },
                {
                    "title": "Fedora", "path": "/boot/isos/fedora-41.iso",
                    "params": "", "initrd": "", "kargs": "",
                    "class": "linux", "tip": "", "hidden": false,
                    "persistence_backend": ""
                },
                {
                    "title": "Windows 10", "path": "/boot/isos/windows10.iso",
                    "params": "", "initrd": "", "kargs": "",
                    "class": "windows", "tip": "", "hidden": false,
                    "persistence_backend": ""
                },
                {
                    "title": "Memtest", "path": "/boot/efi/memtest86plus.efi",
                    "params": "", "initrd": "", "kargs": "",
                    "class": "", "tip": "", "hidden": false,
                    "persistence_backend": ""
                },
                {
                    "title": "OpenWrt", "path": "/boot/imgs/openwrt-x86_64.img",
                    "params": "", "initrd": "", "kargs": "",
                    "class": "", "tip": "", "hidden": false,
                    "persistence_backend": ""
                },
                {
                    "title": "ARCHIVED CentOS 7", "path": "/boot/isos/centos-7.iso",
                    "params": "", "initrd": "", "kargs": "",
                    "class": "linux", "tip": "", "hidden": true,
                    "persistence_backend": ""
                }
            ],
            "tree_view": true,
            "enable_disk_browser": true,
            "grub_superuser": "admin",
            "grub_password_pbkdf2": "grub.pbkdf2.sha512.10000.deadbeef.cafef00d"
        });
        let cfg: BootConfig = serde_json::from_value(example).expect("deserialise example");
        let out = grub::render_grub_cfg(&cfg, "RAIDHOS_DATA");

        // G13 password gate — both lines emitted because hash is valid.
        assert!(
            out.contains("set superusers=\"admin\""),
            "G13 superuser missing"
        );
        assert!(
            out.contains("password_pbkdf2 admin grub.pbkdf2.sha512."),
            "G13 hash missing"
        );

        // G20 TreeView — submenu blocks for `linux` and `windows`.
        assert!(
            out.contains("submenu \"linux\" {"),
            "G20 linux submenu missing"
        );
        assert!(
            out.contains("submenu \"windows\" {"),
            "G20 windows submenu missing"
        );

        // G7 .efi chainload + G6 .img loopback chainload — both top-level
        // (no class), so they appear outside any submenu.
        assert!(
            out.contains("chainloader \"($root)/boot/efi/memtest86plus.efi\""),
            "G7 missing"
        );
        assert!(
            out.contains("loopback loop $imgfile"),
            "G6 loopback missing"
        );
        assert!(out.contains("chainloader (loop)"), "G6 chainload missing");

        // G18 per-ISO persistence kargs on Ubuntu's linux line.
        assert!(
            out.contains("persistent persistent-path=/persistence/ubuntu.dat"),
            "G18 persistence missing",
        );

        // G17 hidden entry — CentOS must not appear anywhere.
        assert!(!out.contains("CentOS"), "G17 hidden entry leaked");

        // G22 disk browser — F2-hotkeyed menuentry at the end.
        assert!(out.contains("--hotkey=f2"), "G22 disk browser missing");

        // Brace balance preserved across every branch.
        assert_eq!(
            out.matches('{').count(),
            out.matches('}').count(),
            "brace mismatch"
        );
    }

    /// A boot.json written by an older client (pre-v0.0.1 gap
    /// closures) must still deserialise without error. The new
    /// fields all use `#[serde(default)]`.
    #[test]
    fn boot_config_legacy_payload_back_compat() {
        let legacy = serde_json::json!({
            "default_entry": null,
            "entries": [
                {"title": "X", "path": "/x.iso", "params": "", "initrd": "", "kargs": ""}
            ]
        });
        let cfg: BootConfig = serde_json::from_value(legacy).expect("deserialise");
        assert_eq!(cfg.entries.len(), 1);
        assert_eq!(cfg.entries[0].class, "");
        assert!(!cfg.entries[0].hidden);
        assert!(!cfg.tree_view);
        assert!(!cfg.enable_disk_browser);
        assert_eq!(cfg.grub_superuser, "");
    }
}
