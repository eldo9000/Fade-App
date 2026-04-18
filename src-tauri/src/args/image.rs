use crate::ConvertOptions;

pub fn build_image_magick_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = vec![input.to_string()];

    if opts.auto_rotate == Some(true) {
        args.push("-auto-orient".to_string());
    }

    if let (Some(cw), Some(ch)) = (opts.crop_width, opts.crop_height) {
        if cw > 0 && ch > 0 {
            let cx = opts.crop_x.unwrap_or(0);
            let cy = opts.crop_y.unwrap_or(0);
            args.push("-crop".to_string());
            args.push(format!("{}x{}+{}+{}", cw, ch, cx, cy));
            args.push("+repage".to_string());
        }
    }

    match opts.resize_mode.as_deref() {
        Some("percent") => {
            let pct = opts.resize_percent.unwrap_or(100);
            args.push("-resize".to_string());
            args.push(format!("{}%", pct));
        },
        Some("pixels") => {
            let w = opts.resize_width.unwrap_or(0);
            let h = opts.resize_height.unwrap_or(0);
            if w > 0 && h > 0 {
                args.push("-resize".to_string());
                args.push(format!("{}x{}", w, h));
            } else if w > 0 {
                args.push("-resize".to_string());
                args.push(format!("{}x", w));
            } else if h > 0 {
                args.push("-resize".to_string());
                args.push(format!("x{}", h));
            }
        },
        _ => {},
    }

    if let Some(deg) = opts.rotation {
        if deg == 90 || deg == 180 || deg == 270 {
            args.push("-rotate".to_string());
            args.push(deg.to_string());
        }
    }

    if opts.flip_v == Some(true) {
        args.push("-flip".to_string());
    }
    if opts.flip_h == Some(true) {
        args.push("-flop".to_string());
    }

    if let Some(q) = opts.quality {
        args.push("-quality".to_string());
        args.push(q.to_string());
    }

    args.push("-strip".to_string());
    args.push(output.to_string());
    args
}
