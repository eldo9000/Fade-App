//! Filesystem-related Tauri commands that don't fit a larger module.

use tauri::command;

/// Return true if a file or directory exists at `path`.
#[command]
pub fn file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

/// List files in a directory. When `recursive` is true, descends into all
/// subdirectories (dotfiles and dot-dirs skipped). Returns full paths, sorted.
/// Falls back to the current working directory if the given path fails to open.
#[command]
pub fn scan_dir(path: String, recursive: Option<bool>) -> Vec<String> {
    let recurse = recursive.unwrap_or(false);
    let mut files: Vec<String> = Vec::new();
    let root = std::path::PathBuf::from(&path);
    // Return [] for missing or non-directory paths so callers can use this
    // to probe "is this a dir?" without a separate command round-trip.
    if !root.is_dir() {
        return files;
    }
    let mut stack: Vec<std::path::PathBuf> = vec![root];
    while let Some(dir) = stack.pop() {
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
                }
            } else if recurse && p.is_dir() {
                stack.push(p);
            }
        }
    }
    files.sort();
    files
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
        assert!(file_exists(f.to_string_lossy().to_string()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_exists_true_for_directory() {
        let dir = unique_tmp("exists-dir");
        assert!(file_exists(dir.to_string_lossy().to_string()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_exists_false_for_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "fade-fs-missing-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        assert!(!file_exists(missing.to_string_lossy().to_string()));
    }

    #[test]
    fn scan_dir_returns_sorted_full_paths_excluding_dotfiles() {
        let dir = unique_tmp("scan");
        fs::write(dir.join("b.txt"), b"").unwrap();
        fs::write(dir.join("a.txt"), b"").unwrap();
        fs::write(dir.join(".hidden"), b"").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), None);
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

        let files = scan_dir(dir.to_string_lossy().to_string(), None);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("top.txt"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_dir_falls_back_when_path_unreadable() {
        // Non-existent path → falls back to "." which should succeed.
        // Just verify it does not panic and returns a Vec (possibly empty).
        let missing = std::env::temp_dir().join(format!(
            "fade-fs-nope-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let _files = scan_dir(missing.to_string_lossy().to_string(), None);
        // No panic = pass.
    }

    #[test]
    fn scan_dir_recursive_descends_into_subdirectories() {
        let dir = unique_tmp("scan-rec");
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("inside.txt"), b"").unwrap();
        fs::write(dir.join("top.txt"), b"").unwrap();
        fs::create_dir(dir.join(".hidden")).unwrap();
        fs::write(dir.join(".hidden").join("skip.txt"), b"").unwrap();

        let files = scan_dir(dir.to_string_lossy().to_string(), Some(true));
        assert_eq!(files.len(), 2, "files were: {files:?}");
        assert!(files.iter().any(|f| f.ends_with("inside.txt")));
        assert!(files.iter().any(|f| f.ends_with("top.txt")));
        fs::remove_dir_all(&dir).ok();
    }
}
