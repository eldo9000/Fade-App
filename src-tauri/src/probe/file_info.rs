use crate::{classify_ext, FileInfo};
use std::path::Path;
use std::process::Command;
use tauri::command;

/// Return file info (duration, dimensions, codec, media type, size).
#[command]
pub async fn get_file_info(path: String) -> Result<FileInfo, String> {
    tokio::task::spawn_blocking(move || -> Result<FileInfo, String> {
        let p = Path::new(&path);
        if !p.exists() {
            return Err(format!("File not found: {path}"));
        }
        let file_size = p.metadata().map(|m| m.len()).unwrap_or(0);
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let mtype = classify_ext(&ext);

        // Data, document, and archive files don't need ffprobe/identify
        if mtype == "data" || mtype == "document" || mtype == "archive" {
            return Ok(FileInfo {
                duration_secs: None,
                width: None,
                height: None,
                codec: None,
                format: Some(ext.to_string()),
                file_size,
                media_type: mtype.to_string(),
            });
        }

        if mtype == "image" {
            let out = Command::new("identify")
                .args(["-format", "%wx%h\n", &path])
                .output()
                .map_err(|e| e.to_string())?;
            let s = String::from_utf8_lossy(&out.stdout);
            let dims: Vec<&str> = s.trim().splitn(2, 'x').collect();
            let width = dims.first().and_then(|v| v.parse().ok());
            let height = dims.get(1).and_then(|v| v.parse().ok());
            return Ok(FileInfo {
                duration_secs: None,
                width,
                height,
                codec: None,
                format: Some(ext.to_string()),
                file_size,
                media_type: "image".to_string(),
            });
        }

        let out = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &path,
            ])
            .output()
            .map_err(|e| e.to_string())?;

        let json: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;

        let duration_secs = json["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok());
        let format = json["format"]["format_name"]
            .as_str()
            .map(|s| s.split(',').next().unwrap_or(s).to_string());

        let mut width = None;
        let mut height = None;
        let mut codec = None;

        if let Some(streams) = json["streams"].as_array() {
            for stream in streams {
                let ct = stream["codec_type"].as_str().unwrap_or("");
                if ct == "video" {
                    width = stream["width"].as_u64().map(|v| v as u32);
                    height = stream["height"].as_u64().map(|v| v as u32);
                    codec = stream["codec_name"].as_str().map(|s| s.to_string());
                    break;
                }
                if ct == "audio" && codec.is_none() {
                    codec = stream["codec_name"].as_str().map(|s| s.to_string());
                }
            }
        }

        Ok(FileInfo {
            duration_secs,
            width,
            height,
            codec,
            format,
            file_size,
            media_type: mtype.to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
