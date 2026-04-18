//! Filesystem-related Tauri commands that don't fit a larger module.

use tauri::command;

/// Return true if a file or directory exists at `path`.
#[command]
pub fn file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

/// List all files (non-recursive) in a directory. Returns full paths, sorted.
/// Falls back to the current working directory if the given path fails to open.
#[command]
pub fn scan_dir(path: String) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            if p.is_file() && !name_str.starts_with('.') {
                p.to_str().map(str::to_owned)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}
