//! Full permutation sweep — Cartesian product of every option over every
//! live output format. Expect failures: some combinations are invalid by
//! design (encoder rejects them), and the goal is to surface which.
//!
//! Continuous ranges (quality 0–100, CRF 0–51, bitrate kbps) are sampled
//! at three points; enums and small integer ranges get full coverage.
//!
//! Each category gets its own `#[ignore]` test because the full sweep is
//! intentionally slow:
//!
//!   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
//!     -- --include-ignored --nocapture
//!
//!   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
//!     image_full -- --ignored --nocapture
//!
//! Outputs land in `test-results/full-sweep/<category>/`; folder is wiped
//! per run. Pass/fail table is printed; missing files = failed cases.
//!
//! Unlike `matrix.rs` (which asserts zero failures), this test is a
//! diagnostic sweep — it always passes at the test-runner level so you
//! can see the full failure list. The exit-code check belongs in `matrix`.

use fade_lib::{
    build_ffmpeg_audio_args, build_ffmpeg_video_args, build_image_magick_args,
    convert::data::{parse_input, write_output},
    ConvertOptions,
};
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Shared helpers ───────────────────────────────────────────────────────────

fn output_root(category: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("full-sweep")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

fn run_cmd(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("spawn `{program}` failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "exit {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("(no stderr)")
                .chars()
                .take(160)
                .collect::<String>()
        ));
    }
    Ok(())
}

struct Case {
    name: String,
    ext: &'static str,
    opts: ConvertOptions,
}

struct Outcome {
    name: String,
    output: PathBuf,
    error: Option<String>,
}

impl Outcome {
    fn passed(&self) -> bool {
        self.error.is_none()
            && self.output.exists()
            && self.output.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }
}

fn report(category: &str, outcomes: Vec<Outcome>) {
    let total = outcomes.len();
    let failed: Vec<&Outcome> = outcomes.iter().filter(|c| !c.passed()).collect();

    println!();
    println!(
        "── {} full sweep ─ {} cases, {} passed, {} failed",
        category,
        total,
        total - failed.len(),
        failed.len()
    );
    for o in &outcomes {
        let status = if o.passed() { "PASS" } else { "FAIL" };
        let size = o.output.metadata().map(|m| m.len()).unwrap_or(0);
        let detail = o.error.as_deref().unwrap_or("");
        println!(
            "  [{}] {:<70} {:>10} bytes  {}",
            status, o.name, size, detail
        );
    }
    println!();
}

fn make_png(path: &Path) -> Result<(), String> {
    run_cmd(
        "magick",
        &[
            "-size",
            "32x32",
            "xc:blue",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

fn make_wav(path: &Path) -> Result<(), String> {
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-ac",
            "2",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

fn make_mp4(path: &Path) -> Result<(), String> {
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:size=64x64:rate=30",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

fn run_image_cases(dir: &Path, fixture: &Path, cases: Vec<Case>) -> Vec<Outcome> {
    cases
        .into_iter()
        .map(|c| {
            let out = dir.join(format!("{}.{}", c.name, c.ext));
            let args = build_image_magick_args(
                fixture.to_str().expect("fixture path"),
                out.to_str().expect("out path"),
                &c.opts,
            );
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let err = run_cmd("magick", &arg_refs).err();
            Outcome {
                name: c.name,
                output: out,
                error: err,
            }
        })
        .collect()
}

fn run_audio_cases(dir: &Path, fixture: &Path, cases: Vec<Case>) -> Vec<Outcome> {
    cases
        .into_iter()
        .map(|c| {
            let out = dir.join(format!("{}.{}", c.name, c.ext));
            let args = build_ffmpeg_audio_args(
                fixture.to_str().expect("fixture path"),
                out.to_str().expect("out path"),
                &c.opts,
            );
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let err = run_cmd("ffmpeg", &arg_refs).err();
            Outcome {
                name: c.name,
                output: out,
                error: err,
            }
        })
        .collect()
}

fn run_video_cases(dir: &Path, fixture: &Path, cases: Vec<Case>) -> Vec<Outcome> {
    cases
        .into_iter()
        .map(|c| {
            let out = dir.join(format!("{}.{}", c.name, c.ext));
            let args = build_ffmpeg_video_args(
                fixture.to_str().expect("fixture path"),
                out.to_str().expect("out path"),
                &c.opts,
            );
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let err = run_cmd("ffmpeg", &arg_refs).err();
            Outcome {
                name: c.name,
                output: out,
                error: err,
            }
        })
        .collect()
}

// ── Image cases ──────────────────────────────────────────────────────────────

fn jpeg_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for q in [10u32, 50, 90] {
        for chroma in ["420", "422", "444"] {
            for prog in [false, true] {
                v.push(Case {
                    name: format!("jpeg_q{q}_chroma{chroma}_prog{prog}"),
                    ext: "jpg",
                    opts: ConvertOptions {
                        output_format: "jpeg".into(),
                        quality: Some(q),
                        jpeg_chroma: Some(chroma.into()),
                        jpeg_progressive: Some(prog),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn png_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for compression in [0u32, 3, 6, 9] {
        for color_mode in ["rgb", "rgba", "gray", "graya", "palette"] {
            for interlaced in [false, true] {
                v.push(Case {
                    name: format!("png_c{compression}_{color_mode}_i{interlaced}"),
                    ext: "png",
                    opts: ConvertOptions {
                        output_format: "png".into(),
                        png_compression: Some(compression),
                        png_color_mode: Some(color_mode.into()),
                        png_interlaced: Some(interlaced),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn webp_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for lossless in [false, true] {
        for method in [0u32, 3, 6] {
            for q in [10u32, 50, 90] {
                v.push(Case {
                    name: format!("webp_lossless{lossless}_m{method}_q{q}"),
                    ext: "webp",
                    opts: ConvertOptions {
                        output_format: "webp".into(),
                        webp_lossless: Some(lossless),
                        webp_method: Some(method),
                        quality: Some(q),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn tiff_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for compression in ["none", "lzw", "deflate", "packbits"] {
        for bit_depth in [8u32, 16, 32] {
            for color_mode in ["rgb", "cmyk", "gray"] {
                v.push(Case {
                    name: format!("tiff_{compression}_b{bit_depth}_{color_mode}"),
                    ext: "tiff",
                    opts: ConvertOptions {
                        output_format: "tiff".into(),
                        tiff_compression: Some(compression.into()),
                        tiff_bit_depth: Some(bit_depth),
                        tiff_color_mode: Some(color_mode.into()),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn bmp_cases() -> Vec<Case> {
    [8u32, 16, 24, 32]
        .into_iter()
        .map(|b| Case {
            name: format!("bmp_b{b}"),
            ext: "bmp",
            opts: ConvertOptions {
                output_format: "bmp".into(),
                bmp_bit_depth: Some(b),
                ..Default::default()
            },
        })
        .collect()
}

fn avif_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for speed in [0u32, 5, 10] {
        for chroma in ["420", "422", "444"] {
            for q in [10u32, 50, 90] {
                v.push(Case {
                    name: format!("avif_s{speed}_c{chroma}_q{q}"),
                    ext: "avif",
                    opts: ConvertOptions {
                        output_format: "avif".into(),
                        avif_speed: Some(speed),
                        avif_chroma: Some(chroma.into()),
                        quality: Some(q),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn gif_image_cases() -> Vec<Case> {
    // Static GIF output from a raster input (ImageMagick native, no ffmpeg).
    [
        Case {
            name: "gif_default".into(),
            ext: "gif",
            opts: ConvertOptions {
                output_format: "gif".into(),
                ..Default::default()
            },
        },
        Case {
            name: "gif_quality50".into(),
            ext: "gif",
            opts: ConvertOptions {
                output_format: "gif".into(),
                quality: Some(50),
                ..Default::default()
            },
        },
    ]
    .into()
}

fn ico_cases() -> Vec<Case> {
    vec![Case {
        name: "ico_default".into(),
        ext: "ico",
        opts: ConvertOptions {
            output_format: "ico".into(),
            ..Default::default()
        },
    }]
}

fn psd_cases() -> Vec<Case> {
    vec![Case {
        name: "psd_default".into(),
        ext: "psd",
        opts: ConvertOptions {
            output_format: "psd".into(),
            ..Default::default()
        },
    }]
}

fn hdr_cases() -> Vec<Case> {
    vec![Case {
        name: "hdr_default".into(),
        ext: "hdr",
        opts: ConvertOptions {
            output_format: "hdr".into(),
            ..Default::default()
        },
    }]
}

fn dds_cases() -> Vec<Case> {
    vec![Case {
        name: "dds_default".into(),
        ext: "dds",
        opts: ConvertOptions {
            output_format: "dds".into(),
            ..Default::default()
        },
    }]
}

fn heic_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for q in [50u32, 80, 95] {
        v.push(Case {
            name: format!("heic_q{q}"),
            ext: "heic",
            opts: ConvertOptions {
                output_format: "heic".into(),
                quality: Some(q),
                ..Default::default()
            },
        });
    }
    v
}

fn heif_cases() -> Vec<Case> {
    vec![Case {
        name: "heif_default".into(),
        ext: "heif",
        opts: ConvertOptions {
            output_format: "heif".into(),
            ..Default::default()
        },
    }]
}

fn svg_output_cases() -> Vec<Case> {
    // SVG output: ImageMagick can produce SVG from raster (limited fidelity).
    vec![Case {
        name: "svg_default".into(),
        ext: "svg",
        opts: ConvertOptions {
            output_format: "svg".into(),
            ..Default::default()
        },
    }]
}

/// SVG → raster: input is an SVG file; outputs are PNG and JPEG.
/// The fixture helper creates a minimal SVG.
fn run_svg_input_cases(dir: &Path) -> Vec<Outcome> {
    let svg_fixture = dir.join("_fixture.svg");
    // Minimal valid SVG for testing.
    std::fs::write(
        &svg_fixture,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="blue"/></svg>"#,
    )
    .expect("write svg fixture");

    let cases = vec![
        Case {
            name: "svg_to_png".into(),
            ext: "png",
            opts: ConvertOptions {
                output_format: "png".into(),
                ..Default::default()
            },
        },
        Case {
            name: "svg_to_jpeg".into(),
            ext: "jpg",
            opts: ConvertOptions {
                output_format: "jpeg".into(),
                ..Default::default()
            },
        },
    ];
    run_image_cases(dir, &svg_fixture, cases)
}

#[test]
#[ignore]
fn image_full() {
    let dir = output_root("image");
    let fixture = dir.join("_fixture.png");
    make_png(&fixture).expect("fixture");

    let mut cases = Vec::new();
    cases.extend(jpeg_cases());
    cases.extend(png_cases());
    cases.extend(webp_cases());
    cases.extend(tiff_cases());
    cases.extend(bmp_cases());
    cases.extend(avif_cases());
    cases.extend(gif_image_cases());
    cases.extend(ico_cases());
    cases.extend(psd_cases());
    cases.extend(hdr_cases());
    cases.extend(dds_cases());
    cases.extend(heic_cases());
    cases.extend(heif_cases());
    cases.extend(svg_output_cases());

    let mut outcomes = run_image_cases(&dir, &fixture, cases);

    // SVG input → raster: separate fixture required.
    outcomes.extend(run_svg_input_cases(&dir));

    report("image", outcomes);
}

// ── Audio cases ──────────────────────────────────────────────────────────────

fn mp3_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for channels in ["mono", "stereo", "joint"] {
        for br in [64u32, 128, 320] {
            v.push(Case {
                name: format!("mp3_cbr{br}_{channels}"),
                ext: "mp3",
                opts: ConvertOptions {
                    output_format: "mp3".into(),
                    mp3_bitrate_mode: Some("cbr".into()),
                    bitrate: Some(br),
                    channels: Some(channels.into()),
                    ..Default::default()
                },
            });
        }
        for q in [0u32, 4, 9] {
            v.push(Case {
                name: format!("mp3_vbrq{q}_{channels}"),
                ext: "mp3",
                opts: ConvertOptions {
                    output_format: "mp3".into(),
                    mp3_bitrate_mode: Some("vbr".into()),
                    mp3_vbr_quality: Some(q),
                    channels: Some(channels.into()),
                    ..Default::default()
                },
            });
        }
    }
    v
}

fn wav_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for bd in [16u32, 24, 32] {
        for sr in [22050u32, 44100, 48000, 96000] {
            for channels in ["mono", "stereo"] {
                v.push(Case {
                    name: format!("wav_b{bd}_sr{sr}_{channels}"),
                    ext: "wav",
                    opts: ConvertOptions {
                        output_format: "wav".into(),
                        bit_depth: Some(bd),
                        sample_rate: Some(sr),
                        channels: Some(channels.into()),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn flac_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for compression in 0u32..=8 {
        for bd in [16u32, 24] {
            v.push(Case {
                name: format!("flac_c{compression}_b{bd}"),
                ext: "flac",
                opts: ConvertOptions {
                    output_format: "flac".into(),
                    flac_compression: Some(compression),
                    bit_depth: Some(bd),
                    ..Default::default()
                },
            });
        }
    }
    v
}

fn ogg_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for q in [-1i32, 0, 5, 10] {
        v.push(Case {
            name: format!("ogg_vbrq{q}"),
            ext: "ogg",
            opts: ConvertOptions {
                output_format: "ogg".into(),
                ogg_bitrate_mode: Some("vbr".into()),
                ogg_vbr_quality: Some(q),
                ..Default::default()
            },
        });
    }
    for mode in ["cbr", "abr"] {
        for br in [64u32, 128, 320] {
            v.push(Case {
                name: format!("ogg_{mode}{br}"),
                ext: "ogg",
                opts: ConvertOptions {
                    output_format: "ogg".into(),
                    ogg_bitrate_mode: Some(mode.into()),
                    bitrate: Some(br),
                    ..Default::default()
                },
            });
        }
    }
    v
}

fn aac_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for profile in ["lc", "he", "hev2"] {
        for br in [64u32, 128, 256] {
            v.push(Case {
                name: format!("aac_{profile}_{br}"),
                ext: "aac",
                opts: ConvertOptions {
                    output_format: "aac".into(),
                    aac_profile: Some(profile.into()),
                    bitrate: Some(br),
                    ..Default::default()
                },
            });
        }
    }
    v
}

fn opus_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for app in ["audio", "voip", "lowdelay"] {
        for vbr in [false, true] {
            for br in [32u32, 96, 256] {
                v.push(Case {
                    name: format!("opus_{app}_vbr{vbr}_{br}"),
                    ext: "opus",
                    opts: ConvertOptions {
                        output_format: "opus".into(),
                        opus_application: Some(app.into()),
                        opus_vbr: Some(vbr),
                        bitrate: Some(br),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn m4a_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for sub in ["aac", "alac"] {
        for bd in [16u32, 24, 32] {
            for br in [128u32, 256] {
                v.push(Case {
                    name: format!("m4a_{sub}_b{bd}_{br}"),
                    ext: "m4a",
                    opts: ConvertOptions {
                        output_format: "m4a".into(),
                        m4a_subcodec: Some(sub.into()),
                        bit_depth: Some(bd),
                        bitrate: Some(br),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

#[test]
#[ignore]
fn audio_full() {
    let dir = output_root("audio");
    let fixture = dir.join("_fixture.wav");
    make_wav(&fixture).expect("fixture");

    let mut cases = Vec::new();
    cases.extend(mp3_cases());
    cases.extend(wav_cases());
    cases.extend(flac_cases());
    cases.extend(ogg_cases());
    cases.extend(aac_cases());
    cases.extend(opus_cases());
    cases.extend(m4a_cases());

    let outcomes = run_audio_cases(&dir, &fixture, cases);
    report("audio", outcomes);
}

// ── Data cases ───────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn data_full() {
    let dir = output_root("data");
    let fixture = dir.join("_fixture.csv");
    std::fs::write(&fixture, "name,value,note\nalpha,1,first\nbeta,2,second\n")
        .expect("write fixture");
    let raw = std::fs::read_to_string(&fixture).expect("read fixture");
    let value = parse_input("csv", &raw).expect("parse csv");

    let formats = ["json", "yaml", "xml", "tsv", "csv"];
    let pretty_opts = [false, true];
    let delims: &[(char, u8)] = &[(',', b','), (';', b';'), ('|', b'|'), ('\t', b'\t')];

    let mut outcomes = Vec::new();
    for fmt in formats {
        for pretty in pretty_opts {
            for (sym, byte) in delims {
                // delimiter only meaningful for csv/tsv output; include all combos anyway
                let dlabel = match sym {
                    ',' => "comma",
                    ';' => "semi",
                    '|' => "pipe",
                    '\t' => "tab",
                    _ => "?",
                };
                let name = format!("csv_to_{fmt}_pretty{pretty}_{dlabel}");
                let out = dir.join(format!("{name}.{fmt}"));
                let result = write_output(fmt, &value, pretty, *byte)
                    .and_then(|s| std::fs::write(&out, s).map_err(|e| e.to_string()));
                outcomes.push(Outcome {
                    name,
                    output: out,
                    error: result.err(),
                });
            }
        }
    }

    report("data", outcomes);
}

// ── Video cases ──────────────────────────────────────────────────────────────

fn h264_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for crf in [0u32, 18, 28, 40, 51] {
        for preset in ["ultrafast", "fast", "medium", "slow", "veryslow"] {
            for profile in ["baseline", "main", "high"] {
                for pix_fmt in ["yuv420p", "yuv422p", "yuv444p"] {
                    for tune in ["none", "film", "animation", "grain"] {
                        v.push(Case {
                            name: format!("h264_crf{crf}_{preset}_{profile}_{pix_fmt}_{tune}"),
                            ext: "mp4",
                            opts: ConvertOptions {
                                output_format: "mp4".into(),
                                codec: Some("h264".into()),
                                crf: Some(crf),
                                preset: Some(preset.into()),
                                h264_profile: Some(profile.into()),
                                pix_fmt: Some(pix_fmt.into()),
                                tune: Some(tune.into()),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }
    }
    v
}

fn h265_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for crf in [18u32, 28, 40] {
        for preset in ["ultrafast", "medium", "veryslow"] {
            for pix_fmt in ["yuv420p", "yuv422p", "yuv444p"] {
                v.push(Case {
                    name: format!("h265_crf{crf}_{preset}_{pix_fmt}"),
                    ext: "mp4",
                    opts: ConvertOptions {
                        output_format: "mp4".into(),
                        codec: Some("h265".into()),
                        crf: Some(crf),
                        preset: Some(preset.into()),
                        pix_fmt: Some(pix_fmt.into()),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn av1_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for crf in [25u32, 35, 50] {
        for speed in [0u32, 5, 10] {
            v.push(Case {
                name: format!("av1_crf{crf}_s{speed}"),
                ext: "mp4",
                opts: ConvertOptions {
                    output_format: "mp4".into(),
                    codec: Some("av1".into()),
                    crf: Some(crf),
                    av1_speed: Some(speed),
                    ..Default::default()
                },
            });
        }
    }
    v
}

fn vp9_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for mode in ["crf", "cbr", "cvbr"] {
        for crf in [25u32, 40] {
            for speed in [0u32, 3, 5] {
                v.push(Case {
                    name: format!("vp9_{mode}_crf{crf}_s{speed}"),
                    ext: "webm",
                    opts: ConvertOptions {
                        output_format: "webm".into(),
                        codec: Some("vp9".into()),
                        webm_bitrate_mode: Some(mode.into()),
                        crf: Some(crf),
                        webm_video_bitrate: Some(1000),
                        vp9_speed: Some(speed),
                        ..Default::default()
                    },
                });
            }
        }
    }
    v
}

fn prores_cases() -> Vec<Case> {
    (0u32..=5)
        .map(|p| Case {
            name: format!("prores_p{p}"),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("prores".into()),
                prores_profile: Some(p),
                ..Default::default()
            },
        })
        .collect()
}

fn dnxhr_cases() -> Vec<Case> {
    ["dnxhr_lb", "dnxhr_sq", "dnxhr_hq", "dnxhr_hqx", "dnxhr_444"]
        .into_iter()
        .map(|p| Case {
            name: format!("dnxhr_{p}"),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("dnxhr".into()),
                dnxhr_profile: Some(p.into()),
                ..Default::default()
            },
        })
        .collect()
}

fn dnxhd_cases() -> Vec<Case> {
    // DNxHD is a fixed-bitrate codec: specific bitrates are only valid at exact
    // resolution × frame-rate combinations. The sweep intentionally passes every
    // documented bitrate through without guards — failures surface BC-005 violations.
    [36u32, 115, 120, 145, 175, 185, 220]
        .into_iter()
        .map(|br| Case {
            name: format!("dnxhd_br{br}"),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("dnxhd".into()),
                dnxhd_bitrate: Some(br),
                ..Default::default()
            },
        })
        .collect()
}

fn cineform_cases() -> Vec<Case> {
    // cfhd quality scale: 0 = best (lossless), 12 = worst.
    // ConvertOptions has no cineform_quality field — the arg builder hardcodes -q:v 3.
    // A single default case confirms the codec path works end-to-end.
    vec![Case {
        name: "cineform_default".into(),
        ext: "mov",
        opts: ConvertOptions {
            output_format: "mov".into(),
            codec: Some("cineform".into()),
            ..Default::default()
        },
    }]
}

fn hap_cases() -> Vec<Case> {
    // HAP is a GPU-decompressed texture codec with three sub-formats:
    //   hap       — RGB, S3TC DXT1
    //   hap_q     — RGB + extra quality layer
    //   hap_alpha — RGBA, S3TC DXT5
    //
    // Known encoder constraints to surface (BC-005 pattern):
    //   · Resolution must be a multiple of 4 for all sub-formats.
    //   · hap_alpha requires an alpha channel (rgba pixel format);
    //     testing on a non-alpha source confirms the arg-builder path even
    //     though actual alpha preservation is not verified.
    //   · Canonical container is MOV; HAP-in-MP4 may be rejected by FFmpeg
    //     depending on build flags — include MP4 to surface that constraint.
    //
    // Resolutions tested:
    //   1920×1080 — full HD, multiple of 4 (canonical, expected pass)
    //   1280×720  — 720p, multiple of 4 (expected pass)
    //     640×480 — SD, multiple of 4 (expected pass)
    //   1023×769  — NOT a multiple of 4; surfaces divisibility constraint
    let mut v = Vec::new();
    for fmt in ["hap", "hap_q", "hap_alpha"] {
        for (res, res_label) in [
            ("1920x1080", "1920x1080"),
            ("1280x720", "1280x720"),
            ("640x480", "640x480"),
            ("1023x769", "1023x769_off"),
        ] {
            // MOV — canonical container for HAP
            v.push(Case {
                name: format!("hap_{fmt}_mov_{res_label}"),
                ext: "mov",
                opts: ConvertOptions {
                    output_format: "mov".into(),
                    codec: Some("hap".into()),
                    hap_format: Some(fmt.into()),
                    resolution: Some(res.into()),
                    ..Default::default()
                },
            });
        }
        // MP4 — non-canonical container; may fail depending on FFmpeg build
        v.push(Case {
            name: format!("hap_{fmt}_mp4_1920x1080"),
            ext: "mp4",
            opts: ConvertOptions {
                output_format: "mp4".into(),
                codec: Some("hap".into()),
                hap_format: Some(fmt.into()),
                resolution: Some("1920x1080".into()),
                ..Default::default()
            },
        });
    }
    v
}

#[test]
#[ignore]
fn hap_full() {
    // Sweep HAP sub-formats across resolutions and containers to surface
    // encoder constraints (BC-005). Run with:
    //   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
    //     -- --include-ignored hap_full --nocapture
    let dir = output_root("hap");
    let fixture = dir.join("_fixture.mp4");
    make_mp4(&fixture).expect("fixture");

    let outcomes = run_video_cases(&dir, &fixture, hap_cases());
    report("hap", outcomes);
}

fn other_video_cases() -> Vec<Case> {
    let mut v = vec![
        Case {
            name: "ffv1_default".into(),
            ext: "mkv",
            opts: ConvertOptions {
                output_format: "mkv".into(),
                codec: Some("ffv1".into()),
                ..Default::default()
            },
        },
        Case {
            name: "mjpeg_default".into(),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("mjpeg".into()),
                ..Default::default()
            },
        },
        Case {
            name: "rawvideo_default".into(),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("rawvideo".into()),
                ..Default::default()
            },
        },
        Case {
            name: "qtrle_default".into(),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("qtrle".into()),
                ..Default::default()
            },
        },
    ];

    for fmt in ["hap", "hap_alpha", "hap_q", "hap_q_alpha"] {
        v.push(Case {
            name: format!("hap_{fmt}"),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("hap".into()),
                hap_format: Some(fmt.into()),
                ..Default::default()
            },
        });
    }

    for std in ["ntsc", "pal"] {
        v.push(Case {
            name: format!("dv_{std}"),
            ext: "mov",
            opts: ConvertOptions {
                output_format: "mov".into(),
                codec: Some("dvvideo".into()),
                dv_standard: Some(std.into()),
                ..Default::default()
            },
        });
    }

    for codec in ["mpeg1video", "mpeg2video"] {
        v.push(Case {
            name: format!("{codec}_default"),
            ext: "mpg",
            opts: ConvertOptions {
                output_format: "mpg".into(),
                codec: Some(codec.into()),
                ..Default::default()
            },
        });
    }

    v.push(Case {
        name: "theora_default".into(),
        ext: "ogv",
        opts: ConvertOptions {
            output_format: "ogv".into(),
            codec: Some("theora".into()),
            ..Default::default()
        },
    });

    for br in [1000u32, 4000, 8000] {
        v.push(Case {
            name: format!("xvid_br{br}"),
            ext: "avi",
            opts: ConvertOptions {
                output_format: "avi".into(),
                codec: Some("mpeg4".into()),
                avi_video_bitrate: Some(br),
                ..Default::default()
            },
        });
    }

    v
}

fn gif_cases() -> Vec<Case> {
    let mut v = Vec::new();
    for width in ["original", "320", "640"] {
        for fps in ["original", "10"] {
            for palette in [64u32, 256] {
                for dither in ["none", "bayer", "floyd"] {
                    for loop_mode in ["infinite", "once", "none"] {
                        v.push(Case {
                            name: format!("gif_w{width}_f{fps}_p{palette}_{dither}_{loop_mode}"),
                            ext: "gif",
                            opts: ConvertOptions {
                                output_format: "gif".into(),
                                gif_width: Some(width.into()),
                                gif_fps: Some(fps.into()),
                                gif_palette_size: Some(palette),
                                gif_dither: Some(dither.into()),
                                gif_loop: Some(loop_mode.into()),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }
    }
    v
}

#[test]
#[ignore]
fn video_full() {
    let dir = output_root("video");
    let fixture = dir.join("_fixture.mp4");
    make_mp4(&fixture).expect("fixture");

    let mut cases = Vec::new();
    cases.extend(h264_cases());
    cases.extend(h265_cases());
    cases.extend(av1_cases());
    cases.extend(vp9_cases());
    cases.extend(prores_cases());
    cases.extend(dnxhr_cases());
    cases.extend(dnxhd_cases());
    cases.extend(cineform_cases());
    cases.extend(other_video_cases());
    cases.extend(gif_cases());

    let outcomes = run_video_cases(&dir, &fixture, cases);
    report("video", outcomes);
}
