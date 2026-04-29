# Known Bug Classes — Fade App

## BC-001: Inverted toggle-state SVG icons
**First observed:** 2026-04-17
**File:** `src/lib/Timeline.svelte`
**Description:** SVG chevron paths assigned to the wrong `{#if}` / `{:else}` branch, causing the icon to show the opposite direction from what the current state requires. When `vizExpanded=true` the chevrons pointed down (suggesting the panel would expand below) instead of up (suggesting it can be collapsed). Swap the `<path d="...">` values between the two branches to fix.
**Pattern:** Any boolean-toggled SVG where paths are authored in `{#if open}` / `{:else}` blocks — verify each branch's visual meaning matches the state label.
**Resolved:** Verified fixed as of current main. `{#if vizExpanded}` correctly renders down-pointing chevrons (collapse) and `{:else}` correctly renders up-pointing chevrons (expand); inline comment at the button confirms the intent.


## BC-002: Audio analysers black / silent before first playback
**First observed:** 2026-04-17
**File:** `src/lib/Timeline.svelte`
**Description:** `_connectSource()` is only called during `togglePlay()` and the media-load effect. When the visualiser panel is expanded before the user has started playback, `_srcConnected` remains `false`, all three draw functions (`_drawOscilloscope`, `_drawSpectrum`, `_drawLissajous`) bail early on the `!_srcConnected` guard, and canvases show only black. Fix: add a reactive `$effect` that calls `_connectSource()` whenever `vizExpanded && mediaReady && audioEl && _audioCtx && !_srcConnected`.
**Pattern:** Any Web Audio graph that guards draw calls on `_srcConnected` (or equivalent) — ensure the source is connected reactively whenever the relevant panel becomes visible, not only on playback start.
**Resolved:** Verified fixed as of current main. A `$effect` at lines 669–673 calls `_connectSource()` when `vizExpanded && mediaReady && audioEl && _audioCtx && !_srcConnected`, exactly as prescribed.


## BC-003: Alembic `as_background_job` silent failure in headless Blender
**First observed:** 2026-04-21
**File:** `scripts/blender_convert.py`
**Description:** `bpy.ops.wm.alembic_import` and `bpy.ops.wm.alembic_export` accept an `as_background_job` parameter that defaults to `True`. In headless (`--background`) mode the async job is dispatched but never executed — the operator returns `{'FINISHED'}` immediately and no file is written. Always pass `as_background_job=False` for both import and export in any headless context.
**Pattern:** Any Blender operator that has an `as_background_job` parameter — verify it is set to `False` when running under `--background`.
**Resolved:** Fixed in commit `9b13f41` (Blender headless backend). `as_background_job=False` present on both `alembic_import` (line 50) and `alembic_export` (line 113).


## BC-004: USD import empty-scene silent success
**First observed:** 2026-04-21
**File:** `scripts/blender_convert.py`
**Description:** `bpy.ops.wm.usd_import` can return `{'FINISHED'}` and exit 0 while having imported nothing into the scene (e.g. unsupported schema, missing textures causing early abort). Without an explicit check the script proceeds to export an empty file. Always verify `len(bpy.data.objects) > 0` after a USD import before proceeding to export.
**Pattern:** Any Blender import operator that can return `FINISHED` without populating the scene — add an object-count check after import before export.
**Resolved:** Fixed in commit `9b13f41` (Blender headless backend). `len(bpy.data.objects) == 0` check present at line 44, raising `RuntimeError` with a clear message before export proceeds.


## BC-005: Encoder-constraint — UI presents invalid encoder-option combinations
**First observed:** 2026-04-24 (recognized as a class)
**Files:**
- `src-tauri/src/args/video.rs` — H.264 profile/pix_fmt auto-promotion (`h264_effective_profile`, lines 257–276)
- `src-tauri/src/args/image.rs` — AVIF libheif speed clamp (line 178)
- `src-tauri/src/convert/video.rs` — DNxHR minimum-resolution guard (lines 29–47)

**Description:** Fade's UI presented encoder-option combinations that the underlying encoder (FFmpeg or ImageMagick/libheif) either silently rejects or produces broken output for. Each instance was fixed independently before the class was recognized. Three confirmed instances:

1. **H.264 profile/pix_fmt impossible combos** — `yuv422p` and `yuv444p` pixel formats are incompatible with `baseline`, `main`, and `high` H.264 profiles; only `high422`/`high444` support them. Fade's UI allowed selecting any profile regardless of pix_fmt. Fix: `h264_effective_profile()` in `src-tauri/src/args/video.rs` auto-promotes the emitted ffmpeg profile arg when pix_fmt forces a higher-chroma profile, and the UI disables unreachable profile buttons. Commits: `723cbff` (arg fix), `50c89cb` (UI disable).
   > Authoritative source — `src-tauri/src/args/video.rs:266-276`:
   > ```rust
   > fn h264_effective_profile<'a>(profile: Option<&str>, pix_fmt: Option<&str>) -> &'a str {
   >     match pix_fmt {
   >         Some("yuv422p") => "high422",
   >         Some("yuv444p") => "high444",
   >         _ => match profile {
   >             Some("baseline") => "baseline",
   >             Some("main") => "main",
   >             _ => "high",
   >         },
   >     }
   > }
   > ```

2. **AVIF libheif speed cap** — ImageMagick's `heic:speed` define is backed by libheif, which accepts values 0–9 only; values above 9 are silently clamped or produce encoder errors. Fade's UI allowed speed values up to 10. Fix: `.min(9)` clamp in `src-tauri/src/args/image.rs`. Commit: `457d22c`.
   > Authoritative source — `src-tauri/src/args/image.rs:175-179`:
   > ```rust
   > "avif" => {
   >     if let Some(s) = opts.avif_speed {
   >         args.push("-define".to_string());
   >         args.push(format!("heic:speed={}", s.min(9)));
   >     }
   > ```

3. **DNxHR minimum-resolution guard** — The FFmpeg `dnxhd` encoder (which handles DNxHR profiles) requires a minimum output resolution of 1280×720; sub-HD resolutions produce an encoder error. Fade's UI allowed setting arbitrary resolutions with DNxHR selected. Fix: pre-flight resolution check in `src-tauri/src/convert/video.rs` returns a clear error before spawning FFmpeg. Commit: `0d1c045`.
   > Authoritative source — `src-tauri/src/convert/video.rs:29-43`:
   > ```rust
   > // ── DNxHR minimum-resolution guard ──────────────────────────────────
   > // The dnxhd encoder (which handles DNxHR) requires at least 1280×720.
   > // If the caller has explicitly set a resolution we can check it now and
   > // return a clear error before spawning ffmpeg.
   > if opts.codec.as_deref() == Some("dnxhr") {
   >     if let Some(res) = &opts.resolution {
   >         if let Some((w_str, h_str)) = res.split_once('x') {
   >             if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
   >                 if w < 1280 || h < 720 {
   >                     return ConvertResult::Error(
   >                         "DNxHR requires a minimum output resolution of 1280×720. \
   >                          Set a higher resolution or leave unscaled."
   >                             .to_string(),
   >                     );
   >                 }
   >             }
   >         }
   >     }
   > }
   > ```

4. **DNxHR/DNxHD resolution guard — convert()-only contract** — The 1280×720 minimum-resolution guards for `dnxhr` and `dnxhd` live in `convert::video::convert()`, not in `build_ffmpeg_video_args()`. Because `build_ffmpeg_video_args()` returns `Vec<String>` it cannot express an error. Any direct caller of the arg builder (unit tests, future pipeline stages) that passes `opts.codec = Some("dnxhr"|"dnxhd")` with a sub-minimum resolution bypasses the pre-flight check and produces a valid-looking argument vector that FFmpeg rejects at runtime. Fix: documented as a contract comment on `build_ffmpeg_video_args()` so callers know to route through `convert()`. See BC-005 pattern note below.
   > Authoritative source — `src-tauri/src/args/video.rs` (doc comment on `build_ffmpeg_video_args()`):
   > ```
   > // DNxHR / DNxHD resolution contract (BC-005)
   > // This function returns Vec<String> and cannot express an error.
   > // It contains no minimum-resolution guard for DNxHR or DNxHD.
   > // Those guards live exclusively in convert::video::convert().
   > // Callers that invoke build_ffmpeg_video_args() directly must not
   > // pass opts.codec = Some("dnxhr"|"dnxhd") with opts.resolution
   > // below 1280×720.
   > ```

**Pattern:** When the UI exposes encoder options (codec, pixel format, profile, speed, resolution) as independent controls, any combination that the underlying encoder rejects will only surface as a cryptic CLI error at runtime. Before adding a new encoder-option control, explicitly document which combinations are invalid and add a pre-flight guard in the arg builder (clamp, auto-promote, or early error). Do not rely on the encoder to produce a useful error message.

**Guard placement corollary:** When the arg builder cannot express an error (returns `Vec<String>`), the guard must live in the caller (e.g., `convert()`). Document this split explicitly as a contract comment on the arg builder so callers know they must not bypass it.

**Resolved:** All four instances documented. `723cbff`/`50c89cb` (H.264), `457d22c` (AVIF), `0d1c045` (DNxHR guard), convert()-only contract documented 2026-04-29 (DNxHR/DNxHD arg-builder bypass). No open instances known as of 2026-04-29.
