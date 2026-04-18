# Fade Architecture

Reference layout for Libre apps. See also: `~/Downloads/Github/Business-OS/standards/ENGINEERING.md` § Module extraction protocol.

## Backend — `src-tauri/src/`

```
lib.rs              ~850 LOC   Entry point (run()), AppState, ConvertOptions,
                               FileInfo, shared helpers (probe_duration, truncate_stderr,
                               parse_out_time_ms, classify_ext, build_output_path,
                               validate_suffix), convert_file command orchestration,
                               cancel_job, check_tools, tests.

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
└── archive.rs          7z extract + repack (parse_7z_percent)

probe/              read-only metadata/visualization commands
├── file_info.rs        get_file_info (ffprobe / identify)
├── waveform.rs         get_waveform + zcr_to_hue (frequency-coloured)
├── spectrogram.rs      get_spectrogram (base64 PNG)
└── filmstrip.rs        get_filmstrip (streaming event-per-frame)

preview/            pre-conversion diff previews
├── video_diff.rs       preview_diff (blend=difference + amplified lutyuv)
└── image_quality.rs    preview_image_quality (magick composite Difference)

presets.rs          list/save/delete_preset (persisted via librewin_common)
theme.rs            get_theme, get_accent (thin wrappers)
fs_commands.rs      scan_dir
main.rs             6 LOC — calls fade_lib::run()
```

## Frontend — `src/`

```
App.svelte          ~1960 LOC — top-level: queue state, 3-column template,
                    load pipeline, diff/crop UI, event listeners. Still too
                    large; next split: PreviewPane + OptionsPanel components.

main.js             mount target
app.css             global styles

lib/
├── Queue.svelte        left column — queue list
├── Timeline.svelte     centre column — audio/video scrubber, waveform, spectrogram
├── ImageOptions.svelte right column — image settings
├── VideoOptions.svelte right column — video settings
├── AudioOptions.svelte right column — audio settings + DSP
├── DataOptions.svelte  right column — data conversions (wraps FormatPicker)
├── FormatPicker.svelte shared format chooser (data, document, archive)
├── utils.js            mediaTypeFor, validateOptions
└── stores/
    ├── zoom.svelte.js      zoom level + hotkeys + localStorage
    └── settings.svelte.js  persisted app settings
```

## Cross-cutting contracts

- **Job lifecycle**: `convert_file` spawns a thread per job. Emits `job-progress` / `job-done` / `job-error` / `job-cancelled` events keyed by `job_id`. Cancellation goes through `AppState.cancellations[job_id]: Arc<AtomicBool>` + `AppState.processes[job_id]: Child`.
- **Frontend invocation**: Every `#[command]` fn is re-exported at the crate root so `generate_handler!` finds it by bare name. Call from JS via `invoke('command_name', { args })`.
- **Module helpers**: `pub(crate)` for cross-module visibility; `pub` only for the crate root re-exports and the Tauri command surface.

## Known debt

Tracked in `KNOWN-BUG-CLASSES.md`. Notable structural items:
- App.svelte template still renders 3 columns inline (~830 LOC of markup).
- Presets CRUD and loadPipeline are entangled with option objects / DOM refs; extract after column split.
- ~41 unique clippy-pedantic warnings untouched since the pre-extraction pass.
- Frontend test coverage is thin (2 files).
