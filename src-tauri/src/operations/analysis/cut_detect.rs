//! Shot/cut detection via the `scdet` (FFmpeg ≥4.4) or classic `scene` filter.
//!
//! Both filters print `pts_time` / score lines to stderr. We parse them and
//! apply a post-filter `min_shot_s` to drop adjacent cuts that are closer
//! than the minimum shot length.

use super::run_ffmpeg_capture;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct CutPoint {
    pub time: f64,
    pub score: f64,
}

#[tauri::command]
pub fn analyze_cut_detect(
    input_path: String,
    algo: String,     // "scdet" | "scene"
    threshold: f64,   // scdet: 5-15 · scene: 0.2-0.5
    min_shot_s: f64,  // post-filter
) -> Result<Vec<CutPoint>, String> {
    let args = if algo == "scene" {
        // select='gt(scene,T)' + showinfo to get pts_time.
        let filter = format!(
            "select='gt(scene\\,{:.3})',showinfo",
            threshold
        );
        vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            input_path,
            "-vf".to_string(),
            filter,
            "-an".to_string(),
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ]
    } else {
        // scdet — emits lines like:
        //   [scdet @ 0x...] lavfi.scd.score: 12.345, lavfi.scd.time: 4.004
        vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            input_path,
            "-vf".to_string(),
            format!("scdet=threshold={:.3}", threshold),
            "-an".to_string(),
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ]
    };
    let stderr = run_ffmpeg_capture(&args)?;

    let mut cuts: Vec<CutPoint> = Vec::new();
    if algo == "scene" {
        // showinfo lines: "Parsed_showinfo_... n:X pts_time:Y ..."
        // scene filter score is in the same line as "scene_score=".
        let mut pending_score: Option<f64> = None;
        for line in stderr.lines() {
            if let Some(idx) = line.find("scene_score:") {
                pending_score = line[idx + 12..]
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok());
            }
            if let Some(idx) = line.find("pts_time:") {
                let time = line[idx + 9..]
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok());
                if let Some(t) = time {
                    cuts.push(CutPoint {
                        time: t,
                        score: pending_score.unwrap_or(0.0),
                    });
                    pending_score = None;
                }
            }
        }
    } else {
        // scdet: look for "lavfi.scd.time" + "lavfi.scd.score" on same line
        for line in stderr.lines() {
            if !line.contains("lavfi.scd.time") {
                continue;
            }
            let score = line
                .split("lavfi.scd.score:")
                .nth(1)
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let time = line
                .split("lavfi.scd.time:")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.trim_end_matches(',').parse::<f64>().ok());
            if let Some(t) = time {
                cuts.push(CutPoint { time: t, score });
            }
        }
    }

    // Post-filter min shot length.
    if min_shot_s > 0.0 {
        let mut filtered: Vec<CutPoint> = Vec::with_capacity(cuts.len());
        let mut last_t = f64::NEG_INFINITY;
        for c in cuts {
            if c.time - last_t >= min_shot_s {
                last_t = c.time;
                filtered.push(c);
            }
        }
        cuts = filtered;
    }

    Ok(cuts)
}
