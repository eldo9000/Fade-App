//! User-saved conversion presets, persisted via `librewin_common::config`.

use librewin_common::config::{read_presets, write_presets, FadePreset};
use tauri::command;

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[command]
pub fn list_presets() -> Vec<FadePreset> {
    read_presets()
}

#[command]
pub fn save_preset(
    name: String,
    media_type: String,
    output_format: String,
    quality: Option<u32>,
    codec: Option<String>,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
) -> Result<FadePreset, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Preset name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("Preset name too long (max 64 chars)".to_string());
    }

    let preset = FadePreset {
        id: uuid_v4(),
        name,
        media_type,
        output_format,
        quality,
        codec,
        bitrate,
        sample_rate,
    };

    let mut presets = read_presets();
    presets.push(preset.clone());
    write_presets(&presets)?;
    Ok(preset)
}

#[command]
pub fn delete_preset(id: String) -> Result<(), String> {
    let mut presets = read_presets();
    let before = presets.len();
    presets.retain(|p| p.id != id);
    if presets.len() == before {
        return Err(format!("Preset not found: {id}"));
    }
    write_presets(&presets)
}
