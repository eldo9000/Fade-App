//! Extract: pull individual streams out of a container without re-encoding.

use super::{parse_streams, run_ffmpeg, run_ffprobe, StreamInfo};
use crate::probe_duration;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;
use ts_rs::TS;

/// Return all streams found in `input_path`.
pub fn get_streams(input_path: &str) -> Result<Vec<StreamInfo>, String> {
    let probe = run_ffprobe(input_path)?;
    Ok(parse_streams(&probe))
}

/// One stream target for a multi-stream extract operation.
#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub struct ExtractStreamSpec {
    pub index: u32,
    pub stream_type: String,
    pub output_path: String,
}

fn codec_flags(stream_type: &str) -> [&'static str; 2] {
    match stream_type {
        "video" => ["-c:v", "copy"],
        "audio" => ["-c:a", "copy"],
        "subtitle" => ["-c:s", "copy"],
        _ => ["-c", "copy"],
    }
}

/// Build the ffmpeg arg list for a multi-stream extract.
/// Pure function — testable without spawning a process.
pub(crate) fn build_multi_args(input_path: &str, streams: &[ExtractStreamSpec]) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
    ];
    for spec in streams {
        let [cf, cv] = codec_flags(&spec.stream_type);
        args.extend([
            "-map".to_string(),
            format!("0:{}", spec.index),
            cf.to_string(),
            cv.to_string(),
            spec.output_path.clone(),
        ]);
    }
    args
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    stream_index: u32,
    stream_type: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    // Map selector: e.g. 0:v:0, 0:a:1, 0:s:0
    // stream_index is the absolute stream index in the file.
    let map_sel = format!("0:{}", stream_index);

    let [cf, cv] = codec_flags(stream_type);
    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-map".to_string(),
        map_sel,
        cf.to_string(),
        cv.to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

/// Extract multiple streams in a single ffmpeg decode pass.
pub fn run_multi(
    window: &Window,
    job_id: &str,
    input_path: &str,
    streams: &[ExtractStreamSpec],
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if streams.is_empty() {
        return Err("extract_multi: no streams provided".to_string());
    }
    let duration = probe_duration(input_path);
    let args = build_multi_args(input_path, streams);
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(index: u32, stream_type: &str, output_path: &str) -> ExtractStreamSpec {
        ExtractStreamSpec {
            index,
            stream_type: stream_type.to_string(),
            output_path: output_path.to_string(),
        }
    }

    #[test]
    fn build_multi_args_single_video() {
        let args = build_multi_args("/in.mkv", &[spec(0, "video", "/out.h264")]);
        assert_eq!(
            args,
            ["-y", "-i", "/in.mkv", "-progress", "pipe:1", "-map", "0:0", "-c:v", "copy", "/out.h264"]
        );
    }

    #[test]
    fn build_multi_args_video_and_audio() {
        let args = build_multi_args(
            "/in.mkv",
            &[spec(0, "video", "/v.h264"), spec(1, "audio", "/a.aac")],
        );
        assert_eq!(
            args,
            [
                "-y", "-i", "/in.mkv", "-progress", "pipe:1",
                "-map", "0:0", "-c:v", "copy", "/v.h264",
                "-map", "0:1", "-c:a", "copy", "/a.aac",
            ]
        );
    }

    #[test]
    fn build_multi_args_subtitle() {
        let args = build_multi_args("/in.mkv", &[spec(2, "subtitle", "/sub.srt")]);
        assert_eq!(
            args,
            ["-y", "-i", "/in.mkv", "-progress", "pipe:1", "-map", "0:2", "-c:s", "copy", "/sub.srt"]
        );
    }

    #[test]
    fn build_multi_args_unknown_type_falls_back() {
        let args = build_multi_args("/in.mkv", &[spec(3, "data", "/data.bin")]);
        let map_pos = args.iter().position(|a| a == "-map").unwrap();
        assert_eq!(args[map_pos + 2], "-c");
        assert_eq!(args[map_pos + 3], "copy");
    }

    #[test]
    fn build_multi_args_three_streams() {
        let specs = vec![
            spec(0, "video", "/v.mkv"),
            spec(1, "audio", "/a.mka"),
            spec(2, "subtitle", "/s.srt"),
        ];
        let args = build_multi_args("/in.mkv", &specs);
        // 5 global + 5 per stream × 3
        assert_eq!(args.len(), 5 + 5 * 3);
        assert_eq!(args[5], "-map");
        assert_eq!(args[10], "-map");
        assert_eq!(args[15], "-map");
    }
}
