use crate::ConvertOptions;

pub fn build_ffmpeg_video_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
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

    let codec = opts.codec.as_deref().unwrap_or("copy");
    if opts.extract_audio == Some(true) {
        args.push("-vn".to_string());
    } else if opts.remove_audio == Some(true) {
        args.push("-an".to_string());
        if codec == "copy" {
            args.extend(["-vcodec".to_string(), "copy".to_string()]);
        } else {
            args.extend(ffmpeg_video_codec_args(codec));
        }
    } else if codec == "copy" {
        args.extend(["-c".to_string(), "copy".to_string()]);
    } else {
        args.extend(ffmpeg_video_codec_args(codec));
    }

    if let Some(res) = &opts.resolution {
        if res != "original" && opts.extract_audio != Some(true) {
            let scale = resolution_to_scale(res);
            args.extend(["-vf".to_string(), scale]);
        }
    }

    if opts.remove_audio != Some(true) {
        if let Some(br) = opts.bitrate {
            args.extend(["-b:a".to_string(), format!("{}k", br)]);
        }
        if let Some(sr) = opts.sample_rate {
            args.extend(["-ar".to_string(), sr.to_string()]);
        }
    }

    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}

pub fn ffmpeg_video_codec_args(codec: &str) -> Vec<String> {
    match codec {
        "h264" => vec![
            "-vcodec".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
        ],
        "h265" => vec![
            "-vcodec".to_string(),
            "libx265".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
        ],
        "vp9" => vec![
            "-vcodec".to_string(),
            "libvpx-vp9".to_string(),
            "-b:v".to_string(),
            "0".to_string(),
            "-crf".to_string(),
            "33".to_string(),
        ],
        "av1" => vec![
            "-vcodec".to_string(),
            "libaom-av1".to_string(),
            "-crf".to_string(),
            "30".to_string(),
            "-b:v".to_string(),
            "0".to_string(),
        ],
        _ => vec!["-c".to_string(), "copy".to_string()],
    }
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
