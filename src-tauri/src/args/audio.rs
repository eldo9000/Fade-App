use crate::ConvertOptions;

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

    args.push("-vn".to_string());

    if let Some(br) = opts.bitrate {
        args.extend(["-b:a".to_string(), format!("{}k", br)]);
    }
    if let Some(sr) = opts.sample_rate {
        args.extend(["-ar".to_string(), sr.to_string()]);
    }

    // DSP filter chain — order: filters → stereo width → loudnorm → limiter
    let mut filters: Vec<String> = Vec::new();

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
        if m.abs() > 0.01 {
            filters.push(format!("extrastereo=m={m:.3}"));
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
