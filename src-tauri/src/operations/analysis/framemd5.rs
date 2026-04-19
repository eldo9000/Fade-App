//! Per-frame MD5 hashes via `-f framemd5`.
//!
//! If `diff_path` is provided, both files are hashed and the first mismatching
//! line index is returned along with both hash lists (capped).

use serde::Serialize;
use std::process::Command;

#[derive(Serialize, Clone)]
pub struct FrameHash {
    pub idx: usize,
    pub hash: String,
}

#[derive(Serialize, Clone)]
pub struct FrameMd5Result {
    pub hashes: Vec<FrameHash>,
    pub first_divergence: Option<usize>,
}

fn hash_file(input_path: &str, stream: &str) -> Result<Vec<String>, String> {
    let map_arg = match stream {
        "audio" => vec!["-map".to_string(), "0:a:0".to_string()],
        "video" => vec!["-map".to_string(), "0:v:0".to_string()],
        _ => vec![],
    };
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostats".to_string(),
        "-i".to_string(),
        input_path.to_string(),
    ];
    args.extend(map_arg);
    args.extend(["-f".to_string(), "framemd5".to_string(), "-".to_string()]);

    let out = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(if stderr.trim().is_empty() {
            "framemd5 failed".to_string()
        } else {
            stderr.to_string()
        });
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    // framemd5 lines:  0,         0,         0,        1,    6220800, <md5>
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next().map(|s| s.trim().to_string()))
        .filter(|h| h.len() == 32)
        .collect())
}

#[tauri::command]
pub fn analyze_framemd5(
    input_path: String,
    stream: String, // "video" | "audio" | "both"
    diff_path: Option<String>,
) -> Result<FrameMd5Result, String> {
    // `both` isn't directly supported by -f framemd5 in a single pass without
    // losing which hash belongs to which stream; hash video then audio and
    // concat. Keeps the return shape flat.
    let hashes_a = if stream == "both" {
        let mut v = hash_file(&input_path, "video")?;
        v.extend(hash_file(&input_path, "audio")?);
        v
    } else {
        hash_file(&input_path, &stream)?
    };

    if let Some(diff) = diff_path.as_deref() {
        let hashes_b = if stream == "both" {
            let mut v = hash_file(diff, "video")?;
            v.extend(hash_file(diff, "audio")?);
            v
        } else {
            hash_file(diff, &stream)?
        };
        let first_divergence = hashes_a
            .iter()
            .zip(hashes_b.iter())
            .position(|(a, b)| a != b)
            .or_else(|| {
                if hashes_a.len() != hashes_b.len() {
                    Some(hashes_a.len().min(hashes_b.len()))
                } else {
                    None
                }
            });
        // Cap display to first 256 hashes per side.
        let cap = 256;
        let hashes: Vec<FrameHash> = hashes_a
            .into_iter()
            .enumerate()
            .take(cap)
            .map(|(idx, hash)| FrameHash { idx, hash })
            .collect();
        Ok(FrameMd5Result {
            hashes,
            first_divergence,
        })
    } else {
        let cap = 256;
        let hashes: Vec<FrameHash> = hashes_a
            .into_iter()
            .enumerate()
            .take(cap)
            .map(|(idx, hash)| FrameHash { idx, hash })
            .collect();
        Ok(FrameMd5Result {
            hashes,
            first_divergence: None,
        })
    }
}
