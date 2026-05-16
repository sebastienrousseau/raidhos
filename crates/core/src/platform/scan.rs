//! Filesystem ISO scanner shared by every platform backend. Only walks
//! one directory deep — enough for typical `Downloads/`, `ISO/`,
//! `Media/` layouts.

use crate::{CoreError, IsoEntry, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn scan_isos_fs(dirs: Vec<String>) -> Result<Vec<IsoEntry>> {
    let mut results = Vec::new();
    for dir in dirs {
        let root = PathBuf::from(dir);
        if !root.exists() {
            continue;
        }
        let entries = match fs::read_dir(&root) {
            Ok(it) => it,
            Err(e) => return Err(CoreError::Io(e.to_string())),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                push_iso(&mut results, &path);
            } else if path.is_dir() {
                if let Ok(subs) = fs::read_dir(&path) {
                    for sub in subs.flatten() {
                        let subpath = sub.path();
                        if subpath.is_file() {
                            push_iso(&mut results, &subpath);
                        }
                    }
                }
            }
        }
    }
    results.sort_by_key(|e| e.title.to_lowercase());
    Ok(results)
}

fn push_iso(results: &mut Vec<IsoEntry>, path: &Path) {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("iso") {
            if let Ok(meta) = fs::metadata(path) {
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("ISO")
                    .to_string();
                results.push(IsoEntry {
                    title,
                    path: path.display().to_string(),
                    size_bytes: meta.len(),
                    params: "quiet splash".to_string(),
                });
            }
        }
    }
}
