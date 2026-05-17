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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!("raidhos-scan-{nanos:x}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(path: &Path, body: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(body).unwrap();
    }

    #[test]
    fn scan_ignores_missing_dir() {
        let out = scan_isos_fs(vec!["/nonexistent/path/that/does/not/exist".into()]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn scan_picks_up_top_level_isos() {
        let root = temp_root();
        write_file(&root.join("alpha.iso"), b"x");
        write_file(&root.join("beta.iso"), b"yy");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        assert_eq!(out.len(), 2);
        // sorted by lowercase title
        assert_eq!(out[0].title, "alpha");
        assert_eq!(out[1].title, "beta");
        assert_eq!(out[0].size_bytes, 1);
        assert_eq!(out[1].size_bytes, 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_descends_one_level() {
        let root = temp_root();
        write_file(&root.join("a.iso"), b"top");
        write_file(&root.join("sub/b.iso"), b"down");
        // nested-too-deep — must NOT be picked up (scanner is one level).
        write_file(&root.join("sub/deep/c.iso"), b"deep");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        let titles: Vec<_> = out.iter().map(|e| e.title.clone()).collect();
        assert!(titles.contains(&"a".to_string()));
        assert!(titles.contains(&"b".to_string()));
        assert!(!titles.contains(&"c".to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_ignores_non_iso_files() {
        let root = temp_root();
        write_file(&root.join("notiso.txt"), b"nope");
        write_file(&root.join("readme"), b"nope");
        write_file(&root.join("real.iso"), b"yes");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "real");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_case_insensitive_extension() {
        let root = temp_root();
        write_file(&root.join("upper.ISO"), b"x");
        write_file(&root.join("mixed.Iso"), b"y");
        write_file(&root.join("lower.iso"), b"z");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        assert_eq!(out.len(), 3);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_handles_multiple_roots() {
        let a = temp_root();
        let b = temp_root();
        write_file(&a.join("first.iso"), b"a");
        write_file(&b.join("second.iso"), b"b");
        let out = scan_isos_fs(vec![a.display().to_string(), b.display().to_string()]).unwrap();
        assert_eq!(out.len(), 2);
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn scan_skips_directories_without_iso() {
        let root = temp_root();
        fs::create_dir_all(root.join("empty-sub")).unwrap();
        write_file(&root.join("alpha.iso"), b"x");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "alpha");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_returns_sorted_results() {
        let root = temp_root();
        write_file(&root.join("Zeta.iso"), b"x");
        write_file(&root.join("alpha.iso"), b"y");
        write_file(&root.join("Mu.iso"), b"z");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        let titles: Vec<_> = out.iter().map(|e| e.title.clone()).collect();
        // case-insensitive sort: alpha (a), Mu (m), Zeta (z)
        assert_eq!(
            titles,
            vec!["alpha".to_string(), "Mu".to_string(), "Zeta".to_string()]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_returns_default_params() {
        let root = temp_root();
        write_file(&root.join("a.iso"), b"x");
        let out = scan_isos_fs(vec![root.display().to_string()]).unwrap();
        assert_eq!(out[0].params, "quiet splash");
        let _ = fs::remove_dir_all(&root);
    }
}
