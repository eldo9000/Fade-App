use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Parse `blender --version` stdout and return `(major, minor)` from the first line.
/// Returns `None` if the line doesn't start with `"Blender "` or the version can't be parsed.
pub fn parse_blender_version(output: &str) -> Option<(u32, u32)> {
    let first = output.lines().next()?.trim();
    let rest = first.strip_prefix("Blender ")?;
    // rest is like "4.1.0 (hash abc)" or "3.6.0 LTS" or "2.93.0"
    let version_str = rest.split_whitespace().next()?;
    let mut parts = version_str.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Run `blender --version` and return an error if the version is < 3.0 or can't be determined.
pub fn check_blender_version(bin: &Path) -> Result<(), String> {
    let out = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to run Blender to check version: {e}"))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    match parse_blender_version(&stdout) {
        None => Err(
            "Blender version could not be determined. Fade requires Blender 3.0 or later. \
             Download from https://blender.org"
                .to_string(),
        ),
        Some((major, minor)) if major < 3 => Err(format!(
            "Blender {major}.{minor} is not supported. Fade requires Blender 3.0 or later. \
             Download from https://blender.org"
        )),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_blender_does_not_panic() {
        // Result is environment-dependent; just confirm no panic and
        // that any returned path is non-empty.
        let result = find_blender();
        if let Some(path) = result {
            assert!(
                !path.as_os_str().is_empty(),
                "Blender path should be non-empty"
            );
        }
        // None is valid — Blender may not be installed in CI
    }

    #[test]
    fn parse_blender_version_modern() {
        assert_eq!(
            parse_blender_version("Blender 4.1.0 (hash abc)"),
            Some((4, 1))
        );
    }

    #[test]
    fn parse_blender_version_lts() {
        assert_eq!(parse_blender_version("Blender 3.6.0 LTS"), Some((3, 6)));
    }

    #[test]
    fn parse_blender_version_old() {
        assert_eq!(parse_blender_version("Blender 2.93.0"), Some((2, 93)));
    }

    #[test]
    fn parse_blender_version_empty() {
        assert_eq!(parse_blender_version(""), None);
    }

    #[test]
    fn parse_blender_version_garbage() {
        assert_eq!(parse_blender_version("not blender output"), None);
    }

    #[test]
    fn check_blender_version_environment_conditional() {
        // Skip if Blender is not installed — same pattern as find_blender_does_not_panic.
        let Some(bin) = find_blender() else { return };
        // Just verify the function runs without panicking; result is environment-dependent.
        let _ = check_blender_version(&bin);
    }
}
