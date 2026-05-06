//! probe_subtitles — list subtitle streams in a media container.

use crate::operations::{parse_streams, run_ffprobe};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SubStream {
    pub index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

#[tauri::command]
pub async fn probe_subtitles(input_path: String) -> Result<Vec<SubStream>, String> {
    tokio::task::spawn_blocking(move || -> Result<Vec<SubStream>, String> {
        crate::validate_input_path(&input_path)?;
        let json = run_ffprobe(&input_path)?;
        let streams = parse_streams(&json);
        Ok(streams
            .into_iter()
            .filter(|s| s.stream_type == "subtitle")
            .map(|s| SubStream {
                index: s.index,
                codec: s.codec,
                language: s.language,
                title: s.title,
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
