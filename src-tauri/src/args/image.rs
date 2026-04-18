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
        }
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
        }
        _ => {}
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

    // Strip EXIF / ICC / comments when user opts out of preserve-metadata.
    // Default (None) preserves — matches user expectation for most conversions.
    if opts.preserve_metadata == Some(false) {
        args.push("-strip".to_string());
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConvertOptions;

    fn find_pair(args: &[String], flag: &str, value: &str) -> bool {
        args.windows(2).any(|w| w[0] == flag && w[1] == value)
    }

    fn opts_for(fmt: &str) -> ConvertOptions {
        ConvertOptions {
            output_format: fmt.into(),
            ..Default::default()
        }
    }

    #[test]
    fn image_args_default_has_no_optional_flags() {
        let opts = opts_for("png");
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert_eq!(args.first().unwrap(), "in.png");
        assert_eq!(args.last().unwrap(), "out.png");
        assert!(!args.contains(&"-quality".to_string()));
        assert!(!args.contains(&"-strip".to_string()));
        assert!(!args.contains(&"-auto-orient".to_string()));
        assert!(!args.contains(&"-rotate".to_string()));
        assert!(!args.contains(&"-resize".to_string()));
    }

    #[test]
    fn image_args_quality_propagates() {
        let opts = ConvertOptions {
            quality: Some(85),
            ..opts_for("jpg")
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(find_pair(&args, "-quality", "85"));
    }

    #[test]
    fn image_args_strip_metadata_when_preserve_false() {
        let opts = ConvertOptions {
            preserve_metadata: Some(false),
            ..opts_for("jpg")
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-strip".to_string()));
    }

    #[test]
    fn image_args_no_strip_when_preserve_true() {
        let opts = ConvertOptions {
            preserve_metadata: Some(true),
            ..opts_for("jpg")
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(!args.contains(&"-strip".to_string()));
    }

    #[test]
    fn image_args_no_strip_when_preserve_none() {
        let opts = opts_for("jpg");
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(!args.contains(&"-strip".to_string()));
    }

    #[test]
    fn image_args_webp_lossless_emits_define() {
        let opts = ConvertOptions {
            webp_lossless: Some(true),
            webp_method: Some(4),
            ..opts_for("webp")
        };
        let args = build_image_magick_args("in.png", "out.webp", &opts);
        assert!(find_pair(&args, "-define", "webp:lossless=true"));
        assert!(find_pair(&args, "-define", "webp:method=4"));
    }

    #[test]
    fn image_args_webp_no_flags_without_opts() {
        let opts = opts_for("webp");
        let args = build_image_magick_args("in.png", "out.webp", &opts);
        assert!(!args.iter().any(|a| a == "webp:lossless=true"));
    }

    #[test]
    fn image_args_png_compression_and_color_mode() {
        let opts = ConvertOptions {
            png_compression: Some(9),
            png_color_mode: Some("rgba".into()),
            png_interlaced: Some(true),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(find_pair(&args, "-define", "png:compression-level=9"));
        assert!(find_pair(&args, "-define", "png:color-type=6"));
        assert!(find_pair(&args, "-interlace", "Plane"));
    }

    #[test]
    fn image_args_png_color_mode_unknown_is_ignored() {
        let opts = ConvertOptions {
            png_color_mode: Some("bogus".into()),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(!args.iter().any(|a| a.starts_with("png:color-type=")));
    }

    #[test]
    fn image_args_jpeg_progressive_emits_interlace() {
        let opts = ConvertOptions {
            jpeg_progressive: Some(true),
            jpeg_chroma: Some("420".into()),
            ..opts_for("jpeg")
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(find_pair(&args, "-interlace", "Plane"));
        assert!(find_pair(&args, "-sampling-factor", "4:2:0"));
    }

    #[test]
    fn image_args_tiff_flags_matrix() {
        let cases = [
            ("none", "None"),
            ("lzw", "LZW"),
            ("deflate", "Zip"),
            ("packbits", "RLE"),
        ];
        for (input, expected) in cases {
            let opts = ConvertOptions {
                tiff_compression: Some(input.into()),
                ..opts_for("tiff")
            };
            let args = build_image_magick_args("in.tif", "out.tif", &opts);
            assert!(
                find_pair(&args, "-compress", expected),
                "tiff compression {} -> {}",
                input,
                expected
            );
        }
    }

    #[test]
    fn image_args_tiff_bit_depth_32_adds_floating_point() {
        let opts = ConvertOptions {
            tiff_bit_depth: Some(32),
            ..opts_for("tiff")
        };
        let args = build_image_magick_args("in.tif", "out.tif", &opts);
        assert!(find_pair(&args, "-depth", "32"));
        assert!(find_pair(&args, "-define", "quantum:format=floating-point"));
    }

    #[test]
    fn image_args_avif_speed_and_chroma() {
        let opts = ConvertOptions {
            avif_speed: Some(6),
            avif_chroma: Some("422".into()),
            ..opts_for("avif")
        };
        let args = build_image_magick_args("in.png", "out.avif", &opts);
        assert!(find_pair(&args, "-define", "heic:speed=6"));
        assert!(find_pair(&args, "-sampling-factor", "4:2:2"));
    }

    #[test]
    fn image_args_bmp_bit_depth() {
        let opts = ConvertOptions {
            bmp_bit_depth: Some(24),
            ..opts_for("bmp")
        };
        let args = build_image_magick_args("in.png", "out.bmp", &opts);
        assert!(find_pair(&args, "-depth", "24"));
    }

    #[test]
    fn image_args_resize_percent() {
        let opts = ConvertOptions {
            resize_mode: Some("percent".into()),
            resize_percent: Some(50),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(find_pair(&args, "-resize", "50%"));
    }

    #[test]
    fn image_args_resize_pixels_width_only() {
        let opts = ConvertOptions {
            resize_mode: Some("pixels".into()),
            resize_width: Some(800),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(find_pair(&args, "-resize", "800x"));
    }

    #[test]
    fn image_args_rotation_only_accepts_cardinals() {
        let opts = ConvertOptions {
            rotation: Some(45),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(!args.contains(&"-rotate".to_string()));

        let opts = ConvertOptions {
            rotation: Some(90),
            ..opts_for("png")
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(find_pair(&args, "-rotate", "90"));
    }
}
