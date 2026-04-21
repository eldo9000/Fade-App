use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn find_blender() -> Option<PathBuf> {
    // 1. PATH lookup via `which` / `where`
    #[cfg(windows)]
    let which_cmd = "where";
    #[cfg(not(windows))]
    let which_cmd = "which";

    if let Ok(out) = std::process::Command::new(which_cmd)
        .arg("blender")
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let first = s.lines().next().unwrap_or("").trim().to_string();
            if !first.is_empty() {
                return Some(PathBuf::from(first));
            }
        }
    }

    // 2. macOS hardcoded paths
    #[cfg(target_os = "macos")]
    {
        let system = PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender");
        if system.exists() {
            return Some(system);
        }

        // ~/Applications/Blender.app/Contents/MacOS/Blender
        if let Some(home) = std::env::var_os("HOME") {
            let user = PathBuf::from(home).join("Applications/Blender.app/Contents/MacOS/Blender");
            if user.exists() {
                return Some(user);
            }
        }
    }

    // 3. Windows: scan Program Files for any Blender Foundation\Blender*\blender.exe
    #[cfg(windows)]
    {
        let base = PathBuf::from(r"C:\Program Files\Blender Foundation");
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("blender.exe");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // 4. Linux known paths
    #[cfg(target_os = "linux")]
    {
        for p in &["/usr/bin/blender", "/usr/local/bin/blender"] {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return Some(pb);
            }
        }
    }

    None
}

pub fn blender_not_found_msg() -> String {
    "blender not found\n\nInstall with:\n  macOS:   download from https://blender.org or: brew install --cask blender\n  Linux:   apt install blender  (or equivalent)\n  Windows: download from https://blender.org".to_string()
}

pub fn needs_blender(input_ext: &str, output_ext: &str) -> bool {
    const BLENDER_FORMATS: &[&str] = &["usd", "usdc", "usda", "usdz", "abc", "blend"];
    let i = input_ext.to_ascii_lowercase();
    let o = output_ext.to_ascii_lowercase();
    BLENDER_FORMATS.contains(&i.as_str()) || BLENDER_FORMATS.contains(&o.as_str())
}

pub fn build_blender_args(
    _blender_bin: &Path,
    script_path: &Path,
    input: &str,
    output: &str,
    is_blend_input: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();

    if is_blend_input {
        args.push(OsString::from(input));
    }

    args.push(OsString::from("--background"));
    args.push(OsString::from("--python"));
    args.push(script_path.as_os_str().to_owned());
    args.push(OsString::from("--python-exit-code"));
    args.push(OsString::from("1"));
    args.push(OsString::from("--"));
    args.push(OsString::from("--input"));
    args.push(OsString::from(input));
    args.push(OsString::from("--output"));
    args.push(OsString::from(output));

    args
}
