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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn uuid_v4_is_36_chars_with_hyphens() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn uuid_v4_is_unique_across_many_calls() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            assert!(seen.insert(uuid_v4()), "uuid collision");
        }
    }

    #[test]
    fn save_preset_rejects_empty_name() {
        let err = save_preset(
            "".to_string(),
            "image".to_string(),
            "jpeg".to_string(),
            Some(80),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("empty"),
            "expected empty-name error, got {err}"
        );
    }

    #[test]
    fn save_preset_rejects_whitespace_only_name() {
        let err = save_preset(
            "   \t  ".to_string(),
            "image".to_string(),
            "jpeg".to_string(),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn save_preset_rejects_overlong_name() {
        let long = "x".repeat(65);
        let err = save_preset(
            long,
            "image".to_string(),
            "jpeg".to_string(),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("too long"),
            "expected too-long error, got {err}"
        );
    }

    #[test]
    fn fade_preset_round_trips_json() {
        let preset = FadePreset {
            id: uuid_v4(),
            name: "Test".to_string(),
            media_type: "video".to_string(),
            output_format: "mp4".to_string(),
            quality: None,
            codec: Some("h264".to_string()),
            bitrate: Some(5000),
            sample_rate: None,
        };
        let json = serde_json::to_string(&preset).expect("serialize");
        let back: FadePreset = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, preset.id);
        assert_eq!(back.name, preset.name);
        assert_eq!(back.media_type, preset.media_type);
        assert_eq!(back.codec.as_deref(), Some("h264"));
        assert_eq!(back.bitrate, Some(5000));
        assert_eq!(back.quality, None);
    }
}
