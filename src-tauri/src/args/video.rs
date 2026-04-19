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
                    // Prefer dedicated webm_video_bitrate; fall back to shared
                    // `bitrate` (legacy behavior) so older FE payloads still work.
                    let br = opts.webm_video_bitrate.or(opts.bitrate).unwrap_or(2000);
                    args.extend([
                        "-b:v".to_string(),
                        format!("{}k", br),
                        "-minrate".to_string(),
                        format!("{}k", br),
                        "-maxrate".to_string(),
                        format!("{}k", br),
                    ]);
                    format_override_applied = true;
                }
                "cvbr" => {
                    let br = opts.webm_video_bitrate.or(opts.bitrate).unwrap_or(2000);
                    let maxr = (br as f64 * 1.5).round() as u32;
                    args.extend([
                        "-b:v".to_string(),
                        format!("{}k", br),
                        "-maxrate".to_string(),
                        format!("{}k", maxr),
                    ]);
                    format_override_applied = true;
                }
                _ => {
                    // "crf" — rely on -crf (already emitted above) + -b:v 0
                    args.extend([
                        "-b:v".to_string(),
                        "0".to_string(),
                        "-crf".to_string(),
                        crf.to_string(),
                    ]);
                    format_override_applied = true;
                }
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
            _ => {}
        }
    }

    // ── Audio flags ──
    if opts.remove_audio != Some(true) {
        if let Some(br) = opts.bitrate {
            // Skip if we consumed `bitrate` as video kbps for webm CBR/CVBR.
            // When webm_video_bitrate is supplied the audio `bitrate` stays
            // available, so only suppress audio when we fell back to it.
            let consumed_for_video = opts.output_format == "webm"
                && matches!(
                    opts.webm_bitrate_mode.as_deref(),
                    Some("cbr") | Some("cvbr")
                )
                && opts.webm_video_bitrate.is_none();
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
        "prores" => vec!["-vcodec".to_string(), "prores_ks".to_string()],
        "theora" => vec!["-vcodec".to_string(), "libtheora".to_string()],
        "mpeg2video" => vec!["-vcodec".to_string(), "mpeg2video".to_string()],
        "mjpeg" => vec!["-vcodec".to_string(), "mjpeg".to_string()],
        "mpeg4" => vec!["-vcodec".to_string(), "mpeg4".to_string()],
        "mpeg1video" => vec!["-vcodec".to_string(), "mpeg1video".to_string()],
        "ffv1" => vec!["-vcodec".to_string(), "ffv1".to_string()],
        "qtrle" => vec!["-vcodec".to_string(), "qtrle".to_string()],
        "dnxhd" => vec!["-vcodec".to_string(), "dnxhd".to_string()],
        "dnxhr" => vec!["-vcodec".to_string(), "dnxhd".to_string()], // DNxHR uses the dnxhd encoder with an hr_* profile
        "cineform" => vec!["-vcodec".to_string(), "cfhd".to_string()],
        "hap" => vec!["-vcodec".to_string(), "hap".to_string()],
        "rawvideo" => vec!["-vcodec".to_string(), "rawvideo".to_string()],
        "dvvideo" => vec!["-vcodec".to_string(), "dvvideo".to_string()],
        "xdcam422" => vec!["-vcodec".to_string(), "mpeg2video".to_string()],
        "xdcam35" => vec!["-vcodec".to_string(), "mpeg2video".to_string()],
        "wmv2" => vec!["-vcodec".to_string(), "wmv2".to_string()],
        "rv20" => vec!["-vcodec".to_string(), "rv20".to_string()],
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
        }
        "vp9" => {
            // WebM override path handles -b:v/-crf when output_format=="webm";
            // for non-webm mp4/mkv containers with vp9, emit defaults here.
            if opts.output_format != "webm" {
                let crf = opts.crf.unwrap_or(33);
                out.extend([
                    "-crf".to_string(),
                    crf.to_string(),
                    "-b:v".to_string(),
                    "0".to_string(),
                ]);
            }
            if let Some(s) = opts.vp9_speed {
                out.extend([
                    "-deadline".to_string(),
                    "good".to_string(),
                    "-cpu-used".to_string(),
                    s.to_string(),
                ]);
            }
            if let Some(pf) = opts.pix_fmt.as_deref() {
                out.extend(["-pix_fmt".to_string(), pf.to_string()]);
            }
        }
        "av1" => {
            let crf = opts.crf.unwrap_or(30);
            out.extend([
                "-crf".to_string(),
                crf.to_string(),
                "-b:v".to_string(),
                "0".to_string(),
            ]);
            if let Some(s) = opts.av1_speed {
                out.extend(["-cpu-used".to_string(), s.to_string()]);
            }
            if let Some(pf) = opts.pix_fmt.as_deref() {
                out.extend(["-pix_fmt".to_string(), pf.to_string()]);
            }
        }
        "prores" => {
            // Default to HQ (profile 3) if not overridden by pix_fmt/user opts.
            out.extend(["-profile:v".to_string(), "3".to_string()]);
        }
        "dnxhd" => {
            // Fixed bitrate required — must match source resolution × fps exactly.
            let br = opts.dnxhd_bitrate.unwrap_or(185);
            out.extend(["-b:v".to_string(), format!("{}M", br)]);
        }
        "dnxhr" => {
            let profile = opts.dnxhr_profile.as_deref().unwrap_or("dnxhr_sq");
            out.extend(["-profile:v".to_string(), profile.to_string()]);
        }
        "cineform" => {
            // cfhd quality scale: 0 = best (lossless), 12 = worst. 3 = standard.
            out.extend(["-q:v".to_string(), "3".to_string()]);
        }
        "hap" => {
            // Sub-format selects the texture compression variant stored in the MOV.
            let fmt = opts.hap_format.as_deref().unwrap_or("hap");
            if fmt != "hap" {
                out.extend(["-format".to_string(), fmt.to_string()]);
            }
        }
        "rawvideo" => {
            let pix = opts.pix_fmt.as_deref().unwrap_or("yuv422p");
            out.extend(["-pix_fmt".to_string(), pix.to_string()]);
        }
        "dvvideo" => {
            // DV requires exact resolution and frame rate; scale is forced here.
            match opts.dv_standard.as_deref().unwrap_or("ntsc") {
                "pal" => out.extend([
                    "-s".to_string(),
                    "720x576".to_string(),
                    "-pix_fmt".to_string(),
                    "yuv420p".to_string(),
                    "-r".to_string(),
                    "25".to_string(),
                ]),
                _ => out.extend([
                    "-s".to_string(),
                    "720x480".to_string(),
                    "-pix_fmt".to_string(),
                    "yuv411p".to_string(),
                    "-r".to_string(),
                    "30000/1001".to_string(),
                ]),
            }
        }
        "xdcam422" => {
            // XDCAM HD422: 50 Mbps CBR, 4:2:2 chroma, MPEG-2 422P profile.
            out.extend([
                "-b:v".to_string(),
                "50M".to_string(),
                "-minrate".to_string(),
                "50M".to_string(),
                "-maxrate".to_string(),
                "50M".to_string(),
                "-profile:v".to_string(),
                "0".to_string(),
                "-pix_fmt".to_string(),
                "yuv422p".to_string(),
            ]);
        }
        "xdcam35" => {
            // XDCAM HD: 35 Mbps VBR max, 4:2:0 chroma, MPEG-2 Main profile.
            out.extend([
                "-b:v".to_string(),
                "35M".to_string(),
                "-maxrate".to_string(),
                "35M".to_string(),
                "-profile:v".to_string(),
                "4".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
            ]);
        }
        _ => {}
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
        }
        "1280x720" => {
            "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        }
        "854x480" => {
            "scale=854:480:force_original_aspect_ratio=decrease,pad=854:480:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        }
        "1440x1080" => {
            "scale=1440:1080:force_original_aspect_ratio=decrease,pad=1440:1080:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        }
        other => format!("scale={}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConvertOptions;

    fn find_pair(args: &[String], flag: &str, value: &str) -> bool {
        args.windows(2).any(|w| w[0] == flag && w[1] == value)
    }

    fn webm_vp9(mode: &str) -> ConvertOptions {
        ConvertOptions {
            output_format: "webm".into(),
            codec: Some("vp9".into()),
            webm_bitrate_mode: Some(mode.into()),
            ..Default::default()
        }
    }

    // ── webm CBR matrix ──────────────────────────────────────────────────────
    #[test]
    fn video_args_webm_cbr_with_webm_video_bitrate() {
        let opts = ConvertOptions {
            webm_video_bitrate: Some(3000),
            ..webm_vp9("cbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "3000k"));
        assert!(find_pair(&args, "-minrate", "3000k"));
        assert!(find_pair(&args, "-maxrate", "3000k"));
    }

    #[test]
    fn video_args_webm_cbr_falls_back_to_bitrate() {
        let opts = ConvertOptions {
            bitrate: Some(1800),
            ..webm_vp9("cbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "1800k"));
        // audio bitrate should be suppressed because bitrate was consumed.
        assert!(!find_pair(&args, "-b:a", "1800k"));
    }

    #[test]
    fn video_args_webm_cbr_both_prefers_webm_video_bitrate_and_keeps_audio() {
        let opts = ConvertOptions {
            webm_video_bitrate: Some(5000),
            bitrate: Some(128),
            ..webm_vp9("cbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "5000k"));
        assert!(find_pair(&args, "-b:a", "128k"));
    }

    #[test]
    fn video_args_webm_cbr_default_bitrate_when_none() {
        let opts = webm_vp9("cbr");
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "2000k"));
        assert!(find_pair(&args, "-minrate", "2000k"));
        assert!(find_pair(&args, "-maxrate", "2000k"));
    }

    // ── webm CVBR matrix ─────────────────────────────────────────────────────
    #[test]
    fn video_args_webm_cvbr_with_webm_video_bitrate() {
        let opts = ConvertOptions {
            webm_video_bitrate: Some(4000),
            ..webm_vp9("cvbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "4000k"));
        assert!(find_pair(&args, "-maxrate", "6000k"));
    }

    #[test]
    fn video_args_webm_cvbr_falls_back_to_bitrate() {
        let opts = ConvertOptions {
            bitrate: Some(1000),
            ..webm_vp9("cvbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "1000k"));
        assert!(find_pair(&args, "-maxrate", "1500k"));
        assert!(!find_pair(&args, "-b:a", "1000k"));
    }

    #[test]
    fn video_args_webm_cvbr_default_bitrate_when_none() {
        let opts = webm_vp9("cvbr");
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "2000k"));
        assert!(find_pair(&args, "-maxrate", "3000k"));
    }

    // ── webm CRF (VBR) mode ─────────────────────────────────────────────────
    #[test]
    fn video_args_webm_crf_uses_b_v_zero_and_default_crf() {
        let opts = webm_vp9("crf");
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "0"));
        assert!(find_pair(&args, "-crf", "33"));
    }

    #[test]
    fn video_args_webm_crf_honors_explicit_crf() {
        let opts = ConvertOptions {
            crf: Some(24),
            ..webm_vp9("crf")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-crf", "24"));
    }

    // ── Audio suppression guard ─────────────────────────────────────────────
    #[test]
    fn video_args_webm_cbr_suppresses_audio_bitrate_when_consumed_for_video() {
        // No webm_video_bitrate + bitrate set → bitrate consumed as video kbps,
        // must NOT appear as -b:a.
        let opts = ConvertOptions {
            bitrate: Some(2500),
            ..webm_vp9("cbr")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(!args.windows(2).any(|w| w[0] == "-b:a"));
    }

    #[test]
    fn video_args_webm_crf_does_not_suppress_audio_bitrate() {
        // CRF mode does not consume `bitrate` — audio flag must survive.
        let opts = ConvertOptions {
            bitrate: Some(192),
            ..webm_vp9("crf")
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:a", "192k"));
    }

    // ── Non-webm output formats ─────────────────────────────────────────────
    #[test]
    fn video_args_mp4_h264_emits_codec_and_preset_defaults() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            crf: Some(20),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mov", "out.mp4", &opts);
        assert!(find_pair(&args, "-vcodec", "libx264"));
        assert!(find_pair(&args, "-crf", "20"));
        assert!(find_pair(&args, "-preset", "medium"));
    }

    #[test]
    fn video_args_mp4_h265_uses_libx265() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h265".into()),
            preset: Some("slow".into()),
            h264_profile: Some("main".into()),
            tune: Some("film".into()),
            pix_fmt: Some("yuv420p".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mov", "out.mp4", &opts);
        assert!(find_pair(&args, "-vcodec", "libx265"));
        assert!(find_pair(&args, "-preset", "slow"));
        assert!(find_pair(&args, "-profile:v", "main"));
        assert!(find_pair(&args, "-tune", "film"));
        assert!(find_pair(&args, "-pix_fmt", "yuv420p"));
    }

    #[test]
    fn video_args_mp4_tune_none_is_skipped() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            tune: Some("none".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mov", "out.mp4", &opts);
        assert!(!args.contains(&"-tune".to_string()));
    }

    #[test]
    fn video_args_mov_copy_codec_uses_c_copy() {
        let opts = ConvertOptions {
            output_format: "mov".into(),
            codec: Some("copy".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mov", "out.mov", &opts);
        assert!(find_pair(&args, "-c", "copy"));
        assert!(!args.contains(&"-crf".to_string()));
    }

    #[test]
    fn video_args_avi_video_bitrate_override() {
        let opts = ConvertOptions {
            output_format: "avi".into(),
            codec: Some("h264".into()),
            avi_video_bitrate: Some(4500),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.avi", &opts);
        assert!(find_pair(&args, "-b:v", "4500k"));
    }

    #[test]
    fn video_args_mp4_vp9_non_webm_path_emits_b_v_zero() {
        // vp9 inside an mp4/mkv container takes the non-webm default path.
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("vp9".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-crf", "33"));
        assert!(find_pair(&args, "-b:v", "0"));
    }

    #[test]
    fn video_args_av1_default_crf_is_30() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("av1".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-crf", "30"));
        assert!(find_pair(&args, "-b:v", "0"));
    }

    #[test]
    fn video_args_remove_audio_emits_an() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            remove_audio: Some(true),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(args.contains(&"-an".to_string()));
        assert!(find_pair(&args, "-vcodec", "libx264"));
    }

    #[test]
    fn video_args_extract_audio_emits_vn_and_skips_video_flags() {
        let opts = ConvertOptions {
            output_format: "mp3".into(),
            codec: Some("h264".into()),
            extract_audio: Some(true),
            bitrate: Some(192),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp3", &opts);
        assert!(args.contains(&"-vn".to_string()));
        assert!(!args.contains(&"-crf".to_string()));
        assert!(find_pair(&args, "-b:a", "192k"));
    }

    #[test]
    fn video_args_preserve_metadata_false_strips() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            preserve_metadata: Some(false),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-map_metadata", "-1"));
    }

    #[test]
    fn video_args_mkv_subtitle_burn_adds_filter() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            mkv_subtitle: Some("burn".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        let vf_idx = args.iter().position(|a| a == "-vf").expect("has -vf");
        assert!(args[vf_idx + 1].contains("subtitles=in.mkv"));
    }

    #[test]
    fn video_args_mkv_subtitle_none_emits_sn() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            mkv_subtitle: Some("none".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        assert!(args.contains(&"-sn".to_string()));
    }

    #[test]
    fn video_args_resolution_1080p_adds_scale_pad() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            resolution: Some("1920x1080".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        let vf_idx = args.iter().position(|a| a == "-vf").expect("has -vf");
        assert!(args[vf_idx + 1].contains("scale=1920:1080"));
    }

    #[test]
    fn video_args_gif_dispatches_to_gif_pipeline() {
        // output_format=gif without extract_audio should route to build_gif_args,
        // which emits palettegen/paletteuse and -an but no codec flags.
        let opts = ConvertOptions {
            output_format: "gif".into(),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.gif", &opts);
        let vf_idx = args.iter().position(|a| a == "-vf").expect("has -vf");
        assert!(args[vf_idx + 1].contains("palettegen"));
        assert!(args[vf_idx + 1].contains("paletteuse"));
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-vcodec".to_string()));
    }
}
