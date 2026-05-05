# Fade — Session Status

Last updated: 2026-05-05 (Known Risks: TASK-29 stale ref cleared)

---

## Current Focus

Two full sessions of substantial work landed 2026-04-25.

**Session A — test sweep infrastructure (`46c37db`):** Three new test files added to `src-tauri/tests/`: `matrix.rs` (33-case smoke matrix; pre-release sanity gate), `full_sweep.rs` (~700-case Cartesian diagnostic; surfaces broken combos), `extra_sweep.rs` (cheap-to-test categories: 3D models, subtitle pure-Rust, email, document text). Seven helper functions made `pub` across `email.rs`, `subtitle.rs`, `document.rs` to support direct test calls. All sweep tests subsequently marked `#[ignore]` (manual-only; CI runs `--lib` and `--include-ignored` only on the conversions integration suite).

**Session B — `&Window` decoupling refactor arc (8 tasks, all CI-green):** All 15 conversion modules (`email`, `subtitle`, `document`, `notebook`, `timeline`, `font`, `ebook`, `data`, `tracker`, `model`, `model_blender`, `image`, `audio`, `video`, `archive`) split into a pure `pub fn convert(...)` + thin `pub fn run(...)` wrapper. Established `convert::progress::{ProgressEvent, ProgressFn, noop_progress}` contract for Window-free invocation. Added 6 new `refactored_*_sweep.rs` test files (one per module group), each calling `convert()` directly with `noop_progress()`. All tests `#[ignore]`. 309 tests passing (`--include-ignored`). Conversion pipeline contract documented in `ARCHITECTURE.md`.

CI green on `main`. Arc closed 2026-04-25.

## Next action

2026-04-29: 0.6.6 sweep re-baseline complete. video_full: 1097 total, 1080 passed, 17 failed — all failures confirmed in expected set (12 fixture-shape DNxHR/DNxHD 64×64, 4 env-blocked HAP absent, 1 env-blocked libtheora absent). refactored_av_sweep: 3/3 passed. No regressions. BC-005 fixes confirmed held.

2026-04-29: 0.6.5 image validation pass complete. `image_full` sweep ran clean — 143 cases, 0 failures, all 6 live image output formats verified end-to-end: **jpeg, png, webp, tiff, bmp, avif**. No format-specific clamps needed in `ImageOptions.svelte` (sweep was clean). The 19 todo-flagged formats in the picker (gif, svg, ico, jpegxl, heic, heif, psd, exr, hdr, dds, xcf, raw, cr2, cr3, nef, arw, dng, orf, rw2) remain unimplemented and out of scope for this sprint — re-enabling any of them is a separate decision requiring classifier + encoder path verification.

2026-04-29: DNxHR/DNxHD convert()-only contract documented in `build_ffmpeg_video_args()` doc comment (BC-005 4th instance). INVESTIGATION-LOG DNxHR arg-builder-layer OPEN entry closed (e1d1020). CI-green. No arc in flight. Remaining genuine OPENs in log: HAP encoder absent (env), HAP divisibility unverifiable (env), libtheora absent (env). All code-shaped OPEN entries resolved.

Cleanup sprint complete 2026-04-28 (TASK-22–23, all CI-green). TASK-22: INVESTIGATION-LOG stale OPEN entries marked CONFIRMED (h264 lossless, h265 profile, DNxHD guard). TASK-23: av1_speed remapped from -cpu-used to -preset with 0–10 → 0–13 scaling for libsvtav1; test updated.

BC-005 fix sprint complete 2026-04-28 (TASK-18–21, all CI-green). TASK-18: H.265 codec-aware profile builder — splits h264|h265 branch, adds h265_effective_profile() (27 sweep cases → green). TASK-19: H.264 lossless guard — forces yuv444p/high444 when crf=0 (120 sweep cases → green). TASK-20: DNxHD resolution guard + DNxHR contract annotation. TASK-21: AV1 encoder → libsvtav1 (9 sweep cases → green). Follow-up deferred: av1_speed/-cpu-used → -preset remapping for libsvtav1.

Sweep sprint complete 2026-04-28 (TASK-15–17, all CI-green). TASK-15: full_sweep.rs run — 292 image/audio/data cases pass, 173 video failures across 6 new BC-005 constraint classes logged to INVESTIGATION-LOG.md (h264-lossless-profile, h265-profile-name-mismatch, DNxHD-bitrate-resolution-coupling, DNxHR-arg-builder-guard-bypass, libaom-av1-absent, libtheora-absent). TASK-16: extra_sweep.rs — 17/17 pass, no new findings. TASK-17: mkdtemp 0700 sandbox applied to 4 remaining medium-priority temp-file sites (subtitle.rs, tracker.rs, vmaf.rs, operations/mod.rs). All temp-file hardening now complete. No arc in flight.

Security + quality sprint complete 2026-04-28 (TASK-1–14, all CI-green). Closed: zip-slip containment, input validation, subtitle filter escaping, ts-rs codegen, archive portability/progress, SQL validator, merge race fix, fs_commands hardening, bindable defaults audit, image stderr drain, HAP sweep coverage, renderer-facing temp sandbox.

Previous: Sprint complete 2026-04-25 (threads 1–3, all CI-green): stale Known Risks race entry removed (`71f5d93`); DNxHD bitrate + CineForm sweep cases added to `full_sweep.rs` (`1dbf064`); `window_progress_emitter_batched` helper added, `video.rs`/`audio.rs` run() wrappers migrated (`af0c7b6`, `263d7d2`).

## Audit outcome summary

**33 findings closed across 18 batches (B1–B18).** Key structural wins:

- `JobOutcome` typed enum replaced string sentinels `"CANCELLED"` / `"__DONE__"` (B11)
- ts-rs codegen: 12 TypeScript types generated at build time from Rust structs (B17)
- Streaming waveform RMS: O(file) → O(n) memory for `get_waveform` (B12)
- `run_ffmpeg` consolidated from 3 diverged copies to 1 canonical with rate-limiter (B8)
- `createLimiter` batch concurrency semaphore: 100 unbounded ffmpegs → clamped to `hardwareConcurrency` (B10)
- validate_output_name umbrella covering all 29 `OperationPayload` variants (B15)
- parking_lot::Mutex across 32 files, return-shape drift normalized (B18)

## Known Risks

- **`$bindable` chain verified correct** — all mutation paths use `$bindable()` + `bind:` explicitly.
- **Blender backend path resolution — HARDENED.** `locate_script()` checks 4 candidate paths (CWD, exe-adjacent, macOS bundle `../Resources/`, Linux FHS `../share/fade/`). `find_blender()` does 5-stage fallback (PATH → macOS hardcoded → Windows Program Files scan → Linux hardcoded). Both emit diagnostic errors listing paths tried. Remaining gap: no Blender version check (deferred). (TASK-29 unit tests landed 555e602.)
**full_sweep.rs diagnostic findings (2026-04-25):**
- **H.264 profile/pix_fmt impossible combos — CLOSED (TASK-1 + TASK-2 this arc).** `full_sweep` surfaced 660 failing H.264 combos; fixed by arg-builder auto-promotion (`723cbff`) + UI disable of unreachable profile buttons (`50c89cb`).
- **Missing `1px.jpg` fixture — CLOSED (TASK-4 this arc).** Pre-existing ignored test in `lib.rs` referenced a non-existent fixture; fixture restored (`8b61613`).
- **AVIF speed cap — CLOSED (`457d22c`).** Clamped to 9 in arg builder + UI slider capped at 9.
- **DNxHR minimum resolution — CLOSED (`0d1c045`).** Guard in `convert/video.rs` returns clear error for sub-1280×720 resolution.
- **7zz tar.gz/tar.xz repack — CLOSED (`8e8298e`).** `repack_tar_compressed` two-step function added to `convert/archive.rs`.

## Mode

Active development. Three sprints closed 2026-04-28 (21 tasks total). 156 sweep failures resolved (27 h265 + 120 h264-lossless + 9 av1). Remaining open sweep findings: DNxHD/DNxHR 64×64 fixture (resolution not set, guard bypass), h265-lossless (deferred), libtheora/HAP absent (env). No arc in flight.
