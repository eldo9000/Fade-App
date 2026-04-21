//! Filesystem-related Tauri commands that don't fit a larger module.

use tauri::command;

/// Max directories to descend into below the root in recursive mode.
/// Chosen to cover realistic media-project trees (season/episode/takes/etc.)
/// without letting a `/` or `$HOME` walk enumerate the whole machine.
const SCAN_MAX_DEPTH: usize = 8;

/// Cap on total files returned by a single `scan_dir` call. A multi-hundred-
/// MB JSON payload through IPC is never a feature; drop-target UX works fine
/// at this size.
const SCAN_MAX_ENTRIES: usize = 10_000;

/// Return true if a file or directory exists at `path`.
#[command]
pub fn file_exists(path: String) -> Result<bool, String> {
    Ok(std::path::Path::new(&path).exists())
}

/// List files in a directory. When `recursive` is true, descends into all
/// subdirectories (dotfiles and dot-dirs skipped). Returns full paths, sorted.
///
/// Depth capped at `SCAN_MAX_DEPTH` directories below the root; total results
/// capped at `SCAN_MAX_ENTRIES`. Past either ceiling the walk stops early and
/// returns whatever was collected so far — callers see a partial list rather
/// than a hang or an IPC payload blowout.
#[command]
pub fn scan_dir(path: String, recursive: Option<bool>) -> Result<Vec<String>, String> {
    let recurse = recursive.unwrap_or(false);
    let mut files: Vec<String> = Vec::new();
    let root = std::path::PathBuf::from(&path);
    // Return [] for missing or non-directory paths so callers can use this
    // to probe "is this a dir?" without a separate command round-trip.
    if !root.is_dir() {
        return Ok(files);
    }
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root, 0)];
    'outer: while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            if p.is_file() {
                if let Some(s) = p.to_str() {
                    files.push(s.to_owned());
                    if files.len() >= SCAN_MAX_ENTRIES {
                        break 'outer;
                    }
                }
            } else if recurse && p.is_dir() && depth < SCAN_MAX_DEPTH {
                stack.push((p, depth + 1));
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "fade-fs-test-{}-{}-{}",
            tag,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn file_exists_true_for_real_file() {
        let dir = unique_tmp("exists");
        let f = dir.join("a.txt");
        fs::write(&f, b"hi").unwrap();
        assert!(file_exists(f.to_string_lossy().to_string()).unwrap());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_exists_true_for_directory() {
        let dir = unique_tmp("exists-dir");
        assert!(file_exists(dir.to_string_lossy().to_string()).unwrap());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_exists_false_for_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "fade-fs-missing-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        assert!(!file_exists(missing.to_string_lossy().to_string()).unwrap());
    }

    #[test]
    fn scan_dir_returns_sorted_full_paths_excluding_dotfiles() {
        let dir = unique_tmp("scan");
        fs::write(dir.join("b.txt"), b"").unwrap();
        fs::write(dir.join("a.txt"), b"").unwrap();
        fs::write(dir.join(".hidden"), b"").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), None).unwrap();
        // Only the two non-hidden files, sorted, absolute paths.
        assert_eq!(files.len(), 2, "files were: {files:?}");
        assert!(files[0].ends_with("a.txt"));
        assert!(files[1].ends_with("b.txt"));
        for f in &files {
            assert!(std::path::Path::new(f).is_absolute());
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_dir_skips_subdirectories() {
        let dir = unique_tmp("scan-sub");
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("inside.txt"), b"").unwrap();
        fs::write(dir.join("top.txt"), b"").unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), None).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("top.txt"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_dir_returns_ok_for_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "fade-fs-nope-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let result = scan_dir(missing.to_string_lossy().to_string(), None);
        // Non-existent path → Ok([]) — callers distinguish "empty" from "error"
        // by the Ok/Err variant; a missing path is not an error, just empty.
        assert_eq!(result, Ok(vec![]));
    }

    #[test]
    fn scan_dir_recursive_descends_into_subdirectories() {
        let dir = unique_tmp("scan-rec");
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("inside.txt"), b"").unwrap();
        fs::write(dir.join("top.txt"), b"").unwrap();
        fs::create_dir(dir.join(".hidden")).unwrap();
        fs::write(dir.join(".hidden").join("skip.txt"), b"").unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), Some(true)).unwrap();
        assert_eq!(files.len(), 2, "files were: {files:?}");
        assert!(files.iter().any(|f| f.ends_with("inside.txt")));
        assert!(files.iter().any(|f| f.ends_with("top.txt")));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_dir_recursive_stops_at_max_depth() {
        // Build a chain root/d1/d2/.../dN each containing one file. Depth cap
        // is SCAN_MAX_DEPTH — descending deeper must not find `beyond.txt`.
        let dir = unique_tmp("scan-depth");
        let mut cur = dir.clone();
        for i in 0..=SCAN_MAX_DEPTH + 2 {
            fs::write(cur.join(format!("f{i}.txt")), b"").unwrap();
            let next = cur.join(format!("d{i}"));
            fs::create_dir(&next).unwrap();
            cur = next;
        }
        fs::write(cur.join("beyond.txt"), b"").unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), Some(true)).unwrap();
        // Depth 0 sees f0.txt; depths 1..=SCAN_MAX_DEPTH contribute fN.txt.
        // Beyond SCAN_MAX_DEPTH the walk stops, so `beyond.txt` and deeper
        // `fN.txt` sitting in dN-directories past the cap are excluded.
        assert!(files.iter().any(|f| f.ends_with("f0.txt")));
        assert!(
            !files.iter().any(|f| f.ends_with("beyond.txt")),
            "depth cap failed — returned: {files:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_dir_caps_total_entries() {
        let dir = unique_tmp("scan-cap");
        let n = SCAN_MAX_ENTRIES + 50;
        for i in 0..n {
            fs::write(dir.join(format!("f{i:05}.txt")), b"").unwrap();
        }
        let files = scan_dir(dir.to_string_lossy().to_string(), Some(false)).unwrap();
        assert!(
            files.len() <= SCAN_MAX_ENTRIES,
            "entry cap exceeded: got {}",
            files.len()
        );
        assert_eq!(files.len(), SCAN_MAX_ENTRIES);
        fs::remove_dir_all(&dir).ok();
    }
}
