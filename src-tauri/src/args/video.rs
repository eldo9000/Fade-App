use crate::ConvertOptions;

/// Build the full ffmpeg argument list for a video conversion. GIF output
/// uses a completely different pipeline (palettegen/paletteuse) and is
/// dispatched to `build_gif_args`.
pub fn build_ffmpeg_video_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    if opts.output_format == "gif" && opts.extract_audio != Some(true) {
        return build_gif_args(input, output, opts);
    }

    let mut args: Vec<String> = vec!["-y".to_string()];

    // ── Input / seek ──
    if let Some(ss) = opts.trim_start {
        args.extend(["-ss".to_string(), ss.to_string()]);
    }

    args.extend(["-i".to_string(), input.to_string()]);

    if let Some(t) = opts.trim_end {
        let end = if let Some(ss) = opts.trim_start {
            t - ss
        } else {
            t
        };
        args.extend(["-t".to_string(), end.to_string()]);
    }

    // Strip container + stream metadata when user opts out. Default preserves.
    if opts.preserve_metadata == Some(false) {
        args.extend(["-map_metadata".to_string(), "-1".to_string()]);
    }

    // ── Encoder selection ──
    let codec = opts.codec.as_deref().unwrap_or("copy");
    let is_copy = codec == "copy";

    if opts.extract_audio == Some(true) {
        args.push("-vn".to_string());
    } else if opts.remove_audio == Some(true) {
        args.push("-an".to_string());
        if is_copy {
            args.extend(["-vcodec".to_string(), "copy".to_string()]);
        } else {
            args.extend(ffmpeg_video_codec_args(codec));
        }
    } else if is_copy {
        args.extend(["-c".to_string(), "copy".to_string()]);
    } else {
        args.extend(ffmpeg_video_codec_args(codec));
    }

    // ── Codec-level quality/preset/profile/tune/pix_fmt ──
    if !is_copy && opts.extract_audio != Some(true) {
        args.extend(codec_quality_args(codec, opts));
    }

    // ── Output-format-specific video bitrate overrides ──
    // (AVI forces CBR-ish video bitrate; WebM has its own bitrate mode.)
    let mut format_override_applied = false;
    if !is_copy && opts.extract_audio != Some(true) {
        if opts.output_format == "avi" {
            if let Some(vbr) = opts.avi_video_bitrate {
                args.extend(["-b:v".to_string(), format!("{}k", vbr)]);
                format_override_applied = true;
            }
        } else if opts.output_format == "webm" {
            let mode = opts.webm_bitrate_mode.as_deref().unwrap_or("crf");
            let crf = opts.crf.unwrap_or(33);
            match mode {
                "cbr" => {
                    // Reinterpret `bitrate` (audio field) as target kbps for CBR video.
                    // TODO(spec): consider a dedicated `webm_video_bitrate` field.
                    let br = opts.bitrate.unwrap_or(2000);
                    args.extend([
                        "-b:v".to_string(), format!("{}k", br),
                        "-minrate".to_string(), format!("{}k", br),
                        "-maxrate".to_string(), format!("{}k", br),
                    ]);
                    format_override_applied = true;
                },
                "cvbr" => {
                    let br = opts.bitrate.unwrap_or(2000);
                    let maxr = (br as f64 * 1.5).round() as u32;
                    args.extend([
                        "-b:v".to_string(), format!("{}k", br),
                        "-maxrate".to_string(), format!("{}k", maxr),
                    ]);
                    format_override_applied = true;
                },
                _ => {
                    // "crf" — rely on -crf (already emitted above) + -b:v 0
                    args.extend(["-b:v".to_string(), "0".to_string(), "-crf".to_string(), crf.to_string()]);
                    format_override_applied = true;
                },
            }
        }
    }
    let _ = format_override_applied;

    // ── Frame rate ──
    if opts.extract_audio != Some(true) {
        if let Some(fr) = opts.frame_rate.as_deref() {
            if fr != "original" {
                args.extend(["-r".to_string(), fr.to_string()]);
            }
        }
    }

    // ── Video filter (-vf): scale + optional subtitle burn ──
    if opts.extract_audio != Some(true) {
        let mut filters: Vec<String> = Vec::new();
        if let Some(res) = &opts.resolution {
            if res != "original" {
                filters.push(resolution_to_scale(res));
            }
        }
        if opts.output_format == "mkv" && opts.mkv_subtitle.as_deref() == Some("burn") {
            filters.push(format!("subtitles={}", input));
        }
        if !filters.is_empty() {
            args.extend(["-vf".to_string(), filters.join(",")]);
        }
    }

    // ── MKV subtitle handling (non-burn) ──
    if opts.output_format == "mkv" {
        match opts.mkv_subtitle.as_deref() {
            Some("none") => args.push("-sn".to_string()),
            Some("copy") => args.extend(["-c:s".to_string(), "copy".to_string()]),
            _ => {},
        }
    }

    // ── Audio flags ──
    if opts.remove_audio != Some(true) {
        if let Some(br) = opts.bitrate {
            // Skip if we consumed bitrate for webm CBR/CVBR video above
            let consumed_for_video = opts.output_format == "webm"
                && matches!(opts.webm_bitrate_mode.as_deref(), Some("cbr") | Some("cvbr"));
            if !consumed_for_video {
                args.extend(["-b:a".to_string(), format!("{}k", br)]);
            }
        }
        if let Some(sr) = opts.sample_rate {
            args.extend(["-ar".to_string(), sr.to_string()]);
        }
    }

    // ── Progress & output ──
    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}

/// Encoder selection. Left un-parameterized for backwards compatibility —
/// call sites that want CRF/preset/etc. should additionally extend with
/// `codec_quality_args(codec, opts)`.
pub fn ffmpeg_video_codec_args(codec: &str) -> Vec<String> {
    match codec {
        "h264" => vec!["-vcodec".to_string(), "libx264".to_string()],
        "h265" => vec!["-vcodec".to_string(), "libx265".to_string()],
        "vp9" => vec!["-vcodec".to_string(), "libvpx-vp9".to_string()],
        "av1" => vec!["-vcodec".to_string(), "libaom-av1".to_string()],
        _ => vec!["-c".to_string(), "copy".to_string()],
    }
}

/// Emit codec-specific quality / preset / profile / tune / pix_fmt flags
/// based on user options. Skips everything for `copy`.
fn codec_quality_args(codec: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    match codec {
        "h264" | "h265" => {
            if let Some(crf) = opts.crf {
                out.extend(["-crf".to_string(), crf.to_string()]);
            }
            let preset = opts.preset.as_deref().unwrap_or("medium");
            out.extend(["-preset".to_string(), preset.to_string()]);
            if let Some(p) = opts.h264_profile.as_deref() {
                out.extend(["-profile:v".to_string(), p.to_string()]);
            }
            if let Some(t) = opts.tune.as_deref() {
                if t != "none" {
                    out.extend(["-tune".to_string(), t.to_string()]);
                }
            }
            if let Some(pf) = opts.pix_fmt.as_deref() {
                out.extend(["-pix_fmt".to_string(), pf.to_string()]);
            }
        },
        "vp9" => {
            // WebM override path handles -b:v/-crf when output_format=="webm";
            // for non-webm mp4/mkv containers with vp9, emit defaults here.
            if opts.output_format != "webm" {
                let crf = opts.crf.unwrap_or(33);
                out.extend([
                    "-crf".to_string(), crf.to_string(),
                    "-b:v".to_string(), "0".to_string(),
                ]);
            }
            if let Some(s) = opts.vp9_speed {
                out.extend([
                    "-deadline".to_string(), "good".to_string(),
                    "-cpu-used".to_string(), s.to_string(),
                ]);
            }
            if let Some(pf) = opts.pix_fmt.as_deref() {
                out.extend(["-pix_fmt".to_string(), pf.to_string()]);
            }
        },
        "av1" => {
            let crf = opts.crf.unwrap_or(30);
            out.extend([
                "-crf".to_string(), crf.to_string(),
                "-b:v".to_string(), "0".to_string(),
            ]);
            if let Some(s) = opts.av1_speed {
                out.extend(["-cpu-used".to_string(), s.to_string()]);
            }
            if let Some(pf) = opts.pix_fmt.as_deref() {
                out.extend(["-pix_fmt".to_string(), pf.to_string()]);
            }
        },
        _ => {},
    }
    out
}

/// GIF pipeline: two-pass palettegen/paletteuse filtergraph. Completely
/// different from the standard video encoder path — no codec, preset, CRF,
/// or audio flags apply.
fn build_gif_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["-y".to_string()];

    if let Some(ss) = opts.trim_start {
        args.extend(["-ss".to_string(), ss.to_string()]);
    }

    args.extend(["-i".to_string(), input.to_string()]);

    if let Some(t) = opts.trim_end {
        let end = if let Some(ss) = opts.trim_start {
            t - ss
        } else {
            t
        };
        args.extend(["-t".to_string(), end.to_string()]);
    }

    if opts.preserve_metadata == Some(false) {
        args.extend(["-map_metadata".to_string(), "-1".to_string()]);
    }

    // Build the filtergraph.
    let mut pre_split: Vec<String> = Vec::new();
    let fps = opts.gif_fps.as_deref().unwrap_or("10");
    if fps != "original" {
        pre_split.push(format!("fps={}", fps));
    }
    let width = opts.gif_width.as_deref().unwrap_or("480");
    if width != "original" {
        pre_split.push(format!("scale={}:-1:flags=lanczos", width));
    }
    pre_split.push("split[s0][s1]".to_string());

    let palette_size = opts.gif_palette_size.unwrap_or(256);
    let dither = match opts.gif_dither.as_deref().unwrap_or("floyd") {
        "none" => "none",
        "bayer" => "bayer",
        _ => "floyd_steinberg",
    };

    let filter = format!(
        "{};[s0]palettegen=max_colors={}[p];[s1][p]paletteuse=dither={}",
        pre_split.join(","),
        palette_size,
        dither
    );

    args.extend(["-vf".to_string(), filter]);

    let loop_val = match opts.gif_loop.as_deref().unwrap_or("infinite") {
        "once" => "1",
        "none" => "-1",
        _ => "0",
    };
    args.extend(["-loop".to_string(), loop_val.to_string()]);

    // No audio in GIF.
    args.push("-an".to_string());

    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}

pub fn resolution_to_scale(res: &str) -> String {
    match res {
        "1920x1080" => {
            "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        "1280x720" => {
            "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        "854x480" => {
            "scale=854:480:force_original_aspect_ratio=decrease,pad=854:480:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        other => format!("scale={}", other),
    }
}
