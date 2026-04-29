2026-04-16 | CONFIRMED | output-format-driven right panel — dispatch 1 green, commit 1ea385a
2026-04-17 | CONFIRMED | viz chevrons inverted + no _connectSource on expand — dispatch 1 green
2026-04-20 | CONFIRMED | 10 mutex .lock().unwrap() sites → .expect() — dispatch 1 green, commit c547715
2026-04-20 | CONFIRMED | extract UpdateManager, PresetManager, CropEditor from App.svelte — dispatch 1 green, commits 5ca633a 0a593a0 a13e6c1
2026-04-20 | CONFIRMED | extract QueueManager from App.svelte — dispatch 1 green, commit d0dedaf
2026-04-20 | CONFIRMED | extract ChromaKeyPanel, AnalysisTools, OperationsPanel from App.svelte — dispatch 1 green, commits 6322110 6359cc4 0b1c7c5

2026-04-25 | OPEN | dispatch 1/3 — encoder-constraint class (3 confirmed instances) undocumented in KNOWN-BUG-CLASSES.md; BC-005 entry needed

2026-04-25 | CONFIRMED | encoder-constraint class (3 confirmed instances) documented as BC-005 in KNOWN-BUG-CLASSES.md — dispatch 1 green, commit 5c1d58d

2026-04-28 | OPEN | HAP encoder absent from Homebrew FFmpeg 8.1 build — all 15 hap_full sweep cases fail with "Encoder not found"; --enable-hap not in Homebrew configure flags; HAP codec path in args/video.rs is unreachable on standard macOS dev installs
2026-04-28 | OPEN | HAP resolution divisibility constraint (multiple-of-4) unverifiable until encoder is available — sweep cases at 1023×769 included but cannot distinguish constraint rejection from encoder-absent rejection on this build
2026-04-28 | OPEN | h264 lossless (crf=0) requires high444 profile — full_sweep video_full surfaces 120 failures: baseline/main/high all reject lossless ("baseline profile doesn't support lossless"). Only yuv444p cases pass because h264_effective_profile() auto-promotes to high444. No guard for crf=0 with non-yuv444p in src/args/video.rs. BC-005 instance.
2026-04-28 | CONFIRMED | h264 lossless (crf=0) — forced yuv444p + high444 profile in args/video.rs when crf=0; commit ea93db0 (TASK-19)
2026-04-28 | OPEN | h265 (libx265) rejects h264 profile names emitted by shared arg path — full_sweep video_full surfaces all 27 h265 cases failing with "unknown profile <high>". src/args/video.rs:297 collapses h264 and h265 into one branch and unconditionally calls h264_effective_profile(), which returns "high"/"high422"/"high444" — invalid for libx265 (expects "main"/"main10"/"main444-8"/etc.). BC-005 instance; needs codec-aware profile builder.
2026-04-28 | CONFIRMED | h265 profile mismatch — added h265_effective_profile(), split h264|h265 branch in args/video.rs; commit fa74e91 (TASK-18)
2026-04-28 | OPEN | DNxHD fixed-bitrate-resolution coupling unguarded — full_sweep video_full surfaces all 7 dnxhd_br cases failing on the 64×64 fixture. dnxhd encoder validates (bitrate, resolution, fps, pix_fmt) tuples; no pre-flight check in src/args/video.rs or src/convert/video.rs. BC-005 instance analogous to existing DNxHR resolution guard.
2026-04-28 | CONFIRMED | DNxHD resolution guard added in convert/video.rs for explicit-resolution path (commit 7ee89bf, TASK-20); 64×64 fixture bypass remains (guard requires opts.resolution to be set)
2026-04-28 | CONFIRMED | DNxHR resolution guard bypassed at arg-builder layer — convert()-only contract documented in build_ffmpeg_video_args() doc comment; BC-005 4th instance added; commit e1d1020
2026-04-28 | OPEN | libaom-av1 absent from Homebrew FFmpeg 8.1 — all 9 av1_* full_sweep cases fail with "Encoder not found". Build ships libsvtav1 only; src/args/video.rs hardcodes libaom-av1 for codec="av1". Arg builder should detect and prefer libsvtav1 when libaom-av1 is unavailable, or document the requirement.
2026-04-28 | CONFIRMED | libaom-av1 absent — switched default AV1 encoder to libsvtav1 in args/video.rs
2026-04-28 | OPEN | libtheora absent from Homebrew FFmpeg 8.1 — theora_default full_sweep case fails with "Encoder not found"; same encoder-absent class as HAP/AV1. Theora codec path in src/args/video.rs unreachable on standard macOS dev installs.
2026-04-29 | CONFIRMED | dispatch 1/3 — DNxHR/DNxHD resolution guards in convert() only; convert()-only contract documented in build_ffmpeg_video_args() doc comment + BC-005 4th instance added; cargo check pass
2026-04-29 | CONFIRMED | image_full sweep clean (143 cases, 6 live formats: jpeg, png, webp, tiff, bmp, avif) — gating run for 0.6.5 image validation
2026-04-29 | CONFIRMED | 0.6.6 sweep re-baseline — video_full: 1097 total, 1080 passed, 17 failed (12 fixture-shape: DNxHR/DNxHD 64×64 no-resolution; 4 env-blocked: HAP encoder absent; 1 env-blocked: libtheora absent; none new); refactored_av_sweep: 3 total, 3 passed, 0 failed. BC-005 fixes confirmed held — no regressions.
