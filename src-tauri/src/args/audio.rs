use crate::ConvertOptions;

/// Maps the OGG quality slider (-1..=10) to an Opus target bitrate in kbps.
fn ogg_quality_to_opus_kbps(q: i32) -> u32 {
    match q {
        i32::MIN..=-1 => 32,
        0 => 48,
        1 => 64,
        2 => 80,
        3 => 96,
        4 => 112,
        5 => 128,
        6 => 160,
        7 => 192,
        8 => 224,
        9 => 256,
        _ => 320,
    }
}

/// Build codec-specific args (encoder, bitrate/quality, sample_fmt, channels,
/// and any format-specific extras) for the given `opts`.
///
/// Returns `(codec_args, suppress_base_bitrate)`. When `suppress_base_bitrate`
/// is true the caller must NOT emit the base `-b:a <bitrate>k` flag (lossless
/// or VBR-driven formats handle bitrate themselves, or it doesn't apply).
fn build_codec_args(opts: &ConvertOptions) -> (Vec<String>, bool) {
    let fmt = opts.output_format.to_lowercase();
    let mut args: Vec<String> = Vec::new();
    let mut suppress_base_bitrate = false;

    // ── encoder + bitrate/quality dispatch ──
    match fmt.as_str() {
        "mp3" => {
            args.extend(["-c:a".to_string(), "libmp3lame".to_string()]);
            if opts.mp3_bitrate_mode.as_deref() == Some("vbr") {
                suppress_base_bitrate = true;
                let q = opts.mp3_vbr_quality.unwrap_or(2);
                args.extend(["-q:a".to_string(), q.to_string()]);
            }
        }
        "flac" => {
            args.extend(["-c:a".to_string(), "flac".to_string()]);
            suppress_base_bitrate = true; // lossless
            if let Some(level) = opts.flac_compression {
                args.extend(["-compression_level".to_string(), level.to_string()]);
            }
        }
        "ogg" => {
            // libvorbis is not available in standard homebrew FFmpeg builds.
            // libopus in an OGG container produces valid .ogg files and is
            // universally supported. Quality slider (-1..10) maps to kbps.
            args.extend(["-c:a".to_string(), "libopus".to_string()]);
            match opts.ogg_bitrate_mode.as_deref() {
                Some("vbr") | None => {
                    suppress_base_bitrate = true;
                    args.extend(["-vbr".to_string(), "on".to_string()]);
                    let q = opts.ogg_vbr_quality.unwrap_or(5);
                    let br = ogg_quality_to_opus_kbps(q);
                    args.extend(["-b:a".to_string(), format!("{}k", br)]);
                }
                Some("cbr") => {
                    args.extend(["-vbr".to_string(), "off".to_string()]);
                    // base -b:a emitted by caller
                }
                Some("abr") => {
                    args.extend(["-vbr".to_string(), "constrained".to_string()]);
                    // base -b:a emitted by caller
                }
                _ => {}
            }
        }
        "aac" => {
            // Native FFmpeg `aac` encoder only supports LC (aac_low), which is
            // also the default — no -profile:a flag needed. HE/HEv2 would require
            // libfdk_aac (non-redistributable) and silently fail otherwise.
            args.extend(["-c:a".to_string(), "aac".to_string()]);
        }
        "opus" => {
            args.extend(["-c:a".to_string(), "libopus".to_string()]);
            if let Some(app) = opts.opus_application.as_deref() {
                if matches!(app, "audio" | "voip" | "lowdelay") {
                    args.extend(["-application".to_string(), app.to_string()]);
                }
            }
            if let Some(vbr) = opts.opus_vbr {
                args.extend([
                    "-vbr".to_string(),
                    if vbr { "on" } else { "off" }.to_string(),
                ]);
            }
        }
        "m4a" => match opts.m4a_subcodec.as_deref() {
            Some("alac") => {
                args.extend(["-c:a".to_string(), "alac".to_string()]);
                suppress_base_bitrate = true;
            }
            _ => {
                args.extend(["-c:a".to_string(), "aac".to_string()]);
            }
        },
        "wma" => match opts.wma_mode.as_deref() {
            Some("pro") => {
                args.extend(["-c:a".to_string(), "wmapro".to_string()]);
            }
            Some("lossless") => {
                args.extend(["-c:a".to_string(), "wmalossless".to_string()]);
                suppress_base_bitrate = true;
            }
            _ => {
                args.extend(["-c:a".to_string(), "wmav2".to_string()]);
            }
        },
        "aiff" => {
            suppress_base_bitrate = true;
            match opts.bit_depth {
                Some(24) => args.extend(["-c:a".to_string(), "pcm_s24be".to_string()]),
                Some(32) => args.extend(["-c:a".to_string(), "pcm_f32be".to_string()]),
                _ => {}
            }
        }
        "alac" => {
            args.extend(["-c:a".to_string(), "alac".to_string()]);
            suppress_base_bitrate = true;
        }
        "ac3" => {
            args.extend(["-c:a".to_string(), "ac3".to_string()]);
            if let Some(br) = opts.ac3_bitrate {
                suppress_base_bitrate = true;
                args.extend(["-b:a".to_string(), format!("{}k", br)]);
            }
        }
        "dts" => {
            args.extend(["-c:a".to_string(), "dca".to_string()]);
            args.extend(["-strict".to_string(), "-2".to_string()]);
            if let Some(br) = opts.dts_bitrate {
                suppress_base_bitrate = true;
                args.extend(["-b:a".to_string(), format!("{}k", br)]);
            }
        }
        "wav" => {
            suppress_base_bitrate = true;
            match opts.bit_depth {
                Some(24) => args.extend(["-c:a".to_string(), "pcm_s24le".to_string()]),
                Some(32) => args.extend(["-c:a".to_string(), "pcm_f32le".to_string()]),
                _ => {}
            }
        }
        _ => {
            // unknown audio format — let ffmpeg's container defaults handle it
        }
    }

    // ── bit_depth → -sample_fmt (format-gated) ──
    if let Some(depth) = opts.bit_depth {
        let sample_fmt = match fmt.as_str() {
            "wav" | "aiff" => match depth {
                16 => Some("s16"),
                // 24 and 32 handled by explicit codec in build_codec_args
                _ => None,
            },
            "flac" => match depth {
                16 => Some("s16"),
                24 | 32 => Some("s32"),
                _ => None,
            },
            "alac" => match depth {
                // ALAC encoder only supports s16p and s32p. 24-bit content
                // is stored as s32p (upper 24 bits carry the audio).
                16 => Some("s16p"),
                24 | 32 => Some("s32p"),
                _ => None,
            },
            "m4a" if opts.m4a_subcodec.as_deref() == Some("alac") => match depth {
                16 => Some("s16p"),
                24 | 32 => Some("s32p"),
                _ => None,
            },
            _ => None,
        };
        if let Some(sf) = sample_fmt {
            args.extend(["-sample_fmt".to_string(), sf.to_string()]);
        }
    }

    // ── channels → -ac (+ mp3 -joint_stereo) ──
    let mut channels_set = false;
    if let Some(ch) = opts.channels.as_deref() {
        match ch {
            "mono" => {
                args.extend(["-ac".to_string(), "1".to_string()]);
                channels_set = true;
            }
            "stereo" => {
                args.extend(["-ac".to_string(), "2".to_string()]);
                channels_set = true;
            }
            "joint" => {
                args.extend(["-ac".to_string(), "2".to_string()]);
                channels_set = true;
                if fmt == "mp3" {
                    args.extend(["-joint_stereo".to_string(), "1".to_string()]);
                }
            }
            "5.1" => {
                args.extend(["-ac".to_string(), "6".to_string()]);
                channels_set = true;
            }
            _ => {} // "source" or unknown → omit
        }
    }
    // libopus refuses to open with unknown/exotic channel layouts. Force stereo
    // when the user picked "source" so opus/ogg never fails on weird inputs.
    if !channels_set && matches!(fmt.as_str(), "opus" | "ogg") {
        args.extend(["-ac".to_string(), "2".to_string()]);
    }

    (args, suppress_base_bitrate)
}

pub fn build_ffmpeg_audio_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
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

    // Strip ID3/Vorbis tags + cover art when user opts out. Default preserves.
    if opts.preserve_metadata == Some(false) {
        args.extend(["-map_metadata".to_string(), "-1".to_string()]);
    }

    args.push("-vn".to_string());

    let (codec_args, suppress_base_bitrate) = build_codec_args(opts);
    args.extend(codec_args);

    if !suppress_base_bitrate {
        if let Some(br) = opts.bitrate {
            args.extend(["-b:a".to_string(), format!("{}k", br)]);
        }
    }
    // libopus only accepts 8000/12000/16000/24000/48000 Hz. Always force 48000
    // for both opus and ogg (ogg now uses libopus) so 44.1/96/192 kHz inputs
    // don't produce "sample rate not supported" errors.
    let is_opus = matches!(opts.output_format.to_lowercase().as_str(), "opus" | "ogg");
    let sample_rate = if is_opus {
        Some(48000u32)
    } else {
        opts.sample_rate
    };
    if let Some(sr) = sample_rate {
        args.extend(["-ar".to_string(), sr.to_string()]);
    }

    // DSP filter chain — order: pad_front → declick_in → filters → declick_out → limiter → pad_end
    // De-click uses a 5 ms quarter-sine (qsin) ramp: starts/ends at exact zero amplitude,
    // preventing the waveform discontinuity click at trim cut points. Applied after pad_front
    // so it operates on actual audio content, not the leading silence.
    let mut filters: Vec<String> = Vec::new();

    let pad_front_secs = opts.pad_front.filter(|&s| s > 0.0).unwrap_or(0.0);
    if pad_front_secs > 0.0 {
        let ms = (pad_front_secs * 1000.0).round() as u64;
        filters.push(format!("adelay={ms}:all=1"));
    }

    // De-click in: only needed when audio is cut mid-stream at the start.
    if opts.trim_start.is_some() {
        filters.push(format!(
            "afade=t=in:st={:.6}:d=0.005:curve=qsin",
            pad_front_secs
        ));
    }

    if let Some(freq) = opts.dsp_highpass_freq {
        if freq > 0.0 {
            filters.push(format!("highpass=f={freq:.1}:p=2"));
        }
    }
    if let Some(freq) = opts.dsp_lowpass_freq {
        if freq > 0.0 {
            filters.push(format!("lowpass=f={freq:.1}:p=2"));
        }
    }
    if let Some(width_pct) = opts.dsp_stereo_width {
        // width_pct: −100 (mono) … 0 (no change) … +100 (wide)
        // extrastereo expects a multiplier: m = 1 + pct/100
        let m = 1.0 + width_pct / 100.0;
        if (m - 1.0).abs() > 0.01 {
            // aformat ensures stereo before extrastereo — mono input (L=R) passes through
            // unchanged since extrastereo's side signal (L-R) is zero.
            filters.push(format!(
                "aformat=channel_layouts=stereo,extrastereo=m={m:.3}"
            ));
        }
    }
    if opts.normalize_loudness == Some(true) {
        let lufs = opts.normalize_lufs.unwrap_or(-16.0);
        let tp = opts.normalize_true_peak.unwrap_or(-1.0);
        filters.push(format!("loudnorm=I={lufs:.1}:TP={tp:.1}:LRA=11"));
    }
    if let Some(db) = opts.dsp_limiter_db {
        let linear = 10.0_f64.powf(db / 20.0);
        filters.push(format!("alimiter=limit={linear:.6}:attack=5:release=50"));
    }

    // De-click out: only needed when audio is cut mid-stream at the end.
    // Placed before pad_end so the fade targets the audio content, not trailing silence.
    if let Some(te) = opts.trim_end {
        let content_dur = te - opts.trim_start.unwrap_or(0.0);
        let fade_st = pad_front_secs + content_dur - 0.005;
        if fade_st > 0.0 {
            filters.push(format!("afade=t=out:st={fade_st:.6}:d=0.005:curve=qsin"));
        }
    }

    if let Some(secs) = opts.pad_end {
        if secs > 0.0 {
            filters.push(format!("apad=pad_dur={secs:.3}"));
        }
    }

    if !filters.is_empty() {
        args.extend(["-af".to_string(), filters.join(",")]);
    }

    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}
