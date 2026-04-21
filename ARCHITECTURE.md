# Fade Architecture

Reference layout for Libre apps. See also: `~/Downloads/Github/Business-OS/standards/ENGINEERING.md` § Module extraction protocol.

## Backend — `src-tauri/src/`

```
lib.rs              ~3300 LOC  Entry point (run()), AppState, ConvertOptions,
                               FileInfo, shared helpers (probe_duration, truncate_stderr,
                               parse_out_time_ms, classify_ext, build_output_path,
                               validate_output_name, validate_output_dir), convert_file
                               command orchestration, cancel_job, check_tools, tests.

args/               pure arg builders — no I/O, trivially testable
├── image.rs            build_image_magick_args
├── video.rs            build_ffmpeg_video_args, ffmpeg_video_codec_args, resolution_to_scale
└── audio.rs            build_ffmpeg_audio_args (incl. DSP filter chain)

convert/            per-media-type conversion pipelines (spawn + progress)
├── image.rs            ImageMagick driver
├── video.rs            ffmpeg video driver
├── audio.rs            ffmpeg audio driver
├── data.rs             pure-Rust JSON/CSV/YAML/TOML/XML (parse_input + write_output)
├── document.rs         Markdown/HTML/text (strip_md, html_to_text, html_to_md)
├── archive.rs          7z extract + repack (parse_7z_percent)
├── subtitle.rs         ffmpeg subtitle convert
├── font.rs             fontforge/ffmpeg font convert
├── ebook.rs            Calibre ebook convert
├── model.rs            Assimp 3D model convert
├── notebook.rs         Jupyter notebook convert
├── timeline.rs         DaVinci/Premiere timeline convert
└── tracker.rs          OpenMPT tracker convert

operations/         single-purpose non-converting filters (run via run_operation)
├── mod.rs              run_operation dispatch, run_ffmpeg canonical, JobOutcome, RateLimiter
├── rate_limiter.rs     RateLimiter { min_interval, min_delta } for progress throttle
├── merge.rs            video/audio merge (concat demuxer)
├── cut.rs              trim by time range
├── rewrap.rs           container rewrap (stream copy)
├── extract.rs          stream extract (single + multi via ExtractMulti)
├── replace_audio.rs    swap audio track
├── remove_audio.rs     strip audio
├── remove_video.rs     strip video
├── conform.rs          framerate/SAR conform
├── loop_op.rs          loop N times
├── split.rs            split at keyframes/duration
├── frame_ops.rs        frame-level operations (rotate, flip, etc.)
├── metadata_strip.rs   strip metadata tags
├── silence_remove.rs   remove silence segments
├── chroma_key.rs       chroma-key compositing
├── video_filters.rs    general video filter chain
├── audio_filters.rs    general audio filter chain
├── audio_offset.rs     audio delay offset
├── analysis/           analysis-only commands (read-only, return JSON)
│   ├── mod.rs
│   ├── audio_norm.rs   EBU/Peak/ReplayGain loudness normalise
│   ├── loudness.rs     loudness measurement
│   ├── cut_detect.rs   scene/cut detection
│   ├── black_detect.rs black-frame detection
│   ├── vmaf.rs         VMAF quality scoring
│   └── framemd5.rs     per-frame MD5 checksums
└── subtitling/         subtitle-specific operations
    ├── mod.rs
    ├── diff.rs         subtitle diff
    ├── lint.rs         subtitle lint
    └── probe.rs        subtitle stream probe

probe/              read-only metadata/visualization commands
├── file_info.rs        get_file_info (ffprobe / identify)
├── waveform.rs         get_waveform + zcr_to_hue (streaming RMS, frequency-coloured)
├── spectrogram.rs      get_spectrogram (base64 PNG)
└── filmstrip.rs        get_filmstrip (streaming event-per-frame, cancellable)

preview/            pre-conversion diff previews
├── video_diff.rs       preview_diff (blend=difference + amplified lutyuv)
└── image_quality.rs    preview_image_quality (magick composite Difference)

presets.rs          list/save/delete_preset (persisted JSON, fs2 exclusive lock)
theme.rs            get_theme, get_accent (thin wrappers)
fs_commands.rs      scan_dir, file_exists
main.rs             6 LOC — calls fade_lib::run()
```

## Frontend — `src/`

```
App.svelte          ~3100 LOC — top-level: queue state, 3-column template,
                    load pipeline, diff/crop UI, event listeners.

main.js             mount target
app.css             global styles

lib/
├── Queue.svelte            left column — queue list
├── QueueManager.svelte     queue add/remove/reorder + drag-and-drop
├── Timeline.svelte         centre column — audio/video scrubber, waveform, spectrogram, filmstrip
├── AnalysisTools.svelte    analysis panel — loudness, VMAF, cut detect, black detect, framemd5
├── OperationsPanel.svelte  right column — single-purpose operations (merge, cut, rewrap, etc.)
├── ImageOptions.svelte     right column — image settings
├── VideoOptions.svelte     right column — video settings
├── AudioOptions.svelte     right column — audio settings + DSP
├── DataOptions.svelte      right column — data conversions (wraps FormatPicker)
├── FormatPicker.svelte     shared format chooser (data, document, archive)
├── ArchiveOptions.svelte   archive-specific options
├── ChromaKeyPanel.svelte   chroma key preview + controls
├── CropEditor.svelte       interactive crop region editor
├── SilencePad.svelte       silence-pad controls
├── CodecWarning.svelte     codec compatibility warning badge
├── PresetManager.svelte    preset save/load/delete UI
├── UpdateManager.svelte    in-app update prompt + progress
├── concurrency.js          createLimiter(n) — dependency-free batch semaphore
├── itemStatus.js           applyProgressIfActive() — terminal-state guard for job events
├── utils.js                mediaTypeFor, validateOptions
├── types/
│   └── generated/          ts-rs codegen output — ConvertOptions.ts, OperationPayload.ts,
│                           JobProgress.ts, JobDone.ts, JobError.ts, JobCancelled.ts, etc.
└── stores/
    ├── zoom.svelte.js      zoom level + hotkeys + localStorage
    └── settings.svelte.js  persisted app settings
```

## Cross-cutting contracts

- **Job lifecycle**: `convert_file` spawns a thread per job. Emits `job-progress` / `job-done` / `job-error` / `job-cancelled` events keyed by `job_id`. Cancellation goes through `AppState.cancellations[job_id]: Arc<AtomicBool>` + `AppState.processes[job_id]: Child`.
- **Frontend invocation**: Every `#[command]` fn is re-exported at the crate root so `generate_handler!` finds it by bare name. Call from JS via `invoke('command_name', { args })`.
- **Module helpers**: `pub(crate)` for cross-module visibility; `pub` only for the crate root re-exports and the Tauri command surface.
- **IPC types**: ts-rs 10 generates `.ts` definitions at build time to `src/lib/types/generated/`. Rust field renames become tsc errors.
- **Mutex**: all `Arc<Mutex<T>>` use `parking_lot::Mutex` — no poison, `.lock()` returns guard directly.

## Known debt

Tracked in `KNOWN-BUG-CLASSES.md`. Notable structural items:
- App.svelte template still renders 3 columns inline (~830 LOC of markup).
- Presets CRUD and loadPipeline are entangled with option objects / DOM refs; extract after column split.
- ~41 unique clippy-pedantic warnings untouched since the pre-extraction pass.
- Frontend test coverage is thin (2 files).
