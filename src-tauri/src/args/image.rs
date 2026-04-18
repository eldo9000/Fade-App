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

    args.extend(format_specific_args(opts));

    args.push(output.to_string());
    args
}

fn sampling_factor(chroma: &str) -> Option<&'static str> {
    match chroma {
        "420" => Some("4:2:0"),
        "422" => Some("4:2:2"),
        "444" => Some("4:4:4"),
        _ => None,
    }
}

fn format_specific_args(opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let fmt = opts.output_format.to_lowercase();

    match fmt.as_str() {
        "jpeg" | "jpg" => {
            if let Some(chroma) = opts.jpeg_chroma.as_deref() {
                if let Some(sf) = sampling_factor(chroma) {
                    args.push("-sampling-factor".to_string());
                    args.push(sf.to_string());
                }
            }
            if opts.jpeg_progressive == Some(true) {
                args.push("-interlace".to_string());
                args.push("Plane".to_string());
            }
        }
        "png" => {
            if let Some(level) = opts.png_compression {
                args.push("-define".to_string());
                args.push(format!("png:compression-level={}", level));
            }
            if let Some(mode) = opts.png_color_mode.as_deref() {
                let ct = match mode {
                    "gray" => Some("0"),
                    "rgb" => Some("2"),
                    "palette" => Some("3"),
                    "graya" => Some("4"),
                    "rgba" => Some("6"),
                    _ => None,
                };
                if let Some(ct) = ct {
                    args.push("-define".to_string());
                    args.push(format!("png:color-type={}", ct));
                }
            }
            if opts.png_interlaced == Some(true) {
                args.push("-interlace".to_string());
                args.push("Plane".to_string());
            }
        }
        "webp" => {
            if opts.webp_lossless == Some(true) {
                args.push("-define".to_string());
                args.push("webp:lossless=true".to_string());
            }
            if let Some(m) = opts.webp_method {
                args.push("-define".to_string());
                args.push(format!("webp:method={}", m));
            }
        }
        "tiff" | "tif" => {
            if let Some(c) = opts.tiff_compression.as_deref() {
                let v = match c {
                    "none" => Some("None"),
                    "lzw" => Some("LZW"),
                    "deflate" => Some("Zip"),
                    "packbits" => Some("RLE"),
                    _ => None,
                };
                if let Some(v) = v {
                    args.push("-compress".to_string());
                    args.push(v.to_string());
                }
            }
            if let Some(d) = opts.tiff_bit_depth {
                args.push("-depth".to_string());
                args.push(d.to_string());
                if d == 32 {
                    args.push("-define".to_string());
                    args.push("quantum:format=floating-point".to_string());
                }
            }
            if let Some(mode) = opts.tiff_color_mode.as_deref() {
                let cs = match mode {
                    "rgb" => Some("RGB"),
                    "cmyk" => Some("CMYK"),
                    "gray" => Some("Gray"),
                    _ => None,
                };
                if let Some(cs) = cs {
                    args.push("-colorspace".to_string());
                    args.push(cs.to_string());
                }
            }
        }
        "bmp" => {
            if let Some(d) = opts.bmp_bit_depth {
                args.push("-depth".to_string());
                args.push(d.to_string());
            }
        }
        "avif" => {
            if let Some(s) = opts.avif_speed {
                args.push("-define".to_string());
                args.push(format!("heic:speed={}", s));
            }
            if let Some(chroma) = opts.avif_chroma.as_deref() {
                if let Some(sf) = sampling_factor(chroma) {
                    args.push("-sampling-factor".to_string());
                    args.push(sf.to_string());
                }
            }
        }
        _ => {}
    }

    args
}
