# Sprint H — Advanced Chroma Key, Archive Expansion, Video Inserts

**Goal:** Implement Neural Matte (RVM), CorridorKey managed install, ISO/DMG archive support, and Video Inserts.

---

## TASK-H1: Neural Matte (RVM) chroma key

**Scope:** AI-based background removal without green screen using Robust Video Matting.

**What to do:**
- Tool: `rvm` Python package (MIT) or standalone RVM inference script
- Managed install: check for RVM; offer pip install of `robust-video-matting`
- Input: any video file (no green screen required)
- Output: video with alpha channel (MOV QTRLE or WebM VP9 with alpha)
- Progress: per-frame progress
- Wire Tauri command `run_neural_matte`
- Note: GPU acceleration via CUDA/MPS optional but not required; CPU fallback must work

**Done when:** RVM removes background from test video clip. CI green (gated on RVM presence).

---

## TASK-H2: CorridorKey managed install

**Scope:** CorridorKey — advanced green screen with hair and motion-blur support.

**What to do:**
- CorridorKey is a standalone binary (not pip-installable) — check their distribution model
- Implement managed download + install (same pattern as any other managed tool)
- Version check on launch; re-download if stale
- Wire Tauri command that invokes CorridorKey CLI with input video + green screen bounds
- UI: color picker for key color, tolerance slider, hair detail toggle

**Done when:** CorridorKey processes a green-screen clip with managed install flow. CI green.

---

## TASK-H3: ISO and DMG archive extraction

**Scope:** Enable ISO and DMG as live input formats (currently `building: true`).

**What to do:**
- **ISO**: 7z can extract ISO (`7z x file.iso`). Set `iso` to `live: true`, wire through `convert/archive.rs`
- **DMG**: macOS: `hdiutil attach -nomount <file.dmg>` then `cp -r /Volumes/<name>/* <output>` then `hdiutil detach`. Linux: `7z x` handles some DMG types. Set `dmg` to `live: true` with macOS note.
- Add extraction sweep cases for ISO and DMG

**Done when:** ISO extracts via 7z. DMG extracts on macOS or emits platform error. CI green.

---

## TASK-H4: Archive creation for CBR/CBZ

**Scope:** Enable CBR and CBZ as live output formats (currently only extracted, not created).

**What to do:**
- CBZ = ZIP containing image files. CBR = RAR (rarely needed for creation). Focus on CBZ.
- In `convert/archive.rs`: add `create_cbz(input_dir, output_path)` using zip
- Input: folder of images → CBZ; or any archive → CBZ (re-pack)
- Set `cbz` to `live: true` as output
- CBR creation requires `rar` binary (proprietary) — offer CBZ as alternative; emit note in UI
- Add sweep case

**Done when:** Folder of PNGs packages to CBZ correctly. CI green.

---

## TASK-H5: Video Inserts

**Scope:** Implement the scaffolded Video Inserts operation.

**What to do:**
- Clarify definition: "video inserts" = replace a segment of a video with footage from another file, keeping original audio sync
- FFmpeg approach: complex filter — `[0:v]trim=0:<start>,setpts=PTS-STARTPTS[a]; [1:v]trim=0:<dur>,setpts=PTS-STARTPTS[b]; [0:v]trim=<end>,setpts=PTS-STARTPTS[c]; [a][b][c]concat=n=3[out]`
- UI: timeline with in/out markers for replacement region; second file picker for insert clip
- Implement `run_video_insert()` in `operations/video_inserts.rs`
- Handle audio: keep original (drop insert audio) or mix both — offer toggle
- Add basic test

**Done when:** A segment of a test video is replaced with insert clip, output plays correctly. CI green.

---

## TASK-H6: Subtitling — generate and edit operations

**Scope:** Complete the subtitling pipeline. Probe and lint exist; generation and editing ops are scaffolded.

**What to do:**
- **Generate**: wire Whisper (from G2) as the source; output SRT embedded or burned
- **Burn**: `ffmpeg -vf subtitles=<srt>` — wire `run_burn_subtitles()` in operations
- **Soft embed**: `-c:s mov_text` (MP4) or `-c:s ass` (MKV) — wire `run_embed_subtitles()`
- **Edit**: basic timing shift (`-itsoffset` for container subs or SRT line adjustment in Rust)
- Connect to subtitling page which currently shows analyze-only options

**Done when:** Burn subtitles and soft embed both produce correct output. CI green.
