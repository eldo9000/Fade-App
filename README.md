<img src="Fade-icon-dark.png" width="80" alt="Fade">

# Fade

The multimedia utility knife. Nothing creative. Just tools.

For when you need to do that one thing... to that one file... quickly.

![Fade screenshot](Screenshot.2.jpg)

---

You're in the middle of something. A client needs the file in a different format. The audio needs trimming, the clip needs converting before it's all usable.

You don't need to open a full editor. You don't need to look something up. You just need it done.

Fade handles it. Drop the file, do the task, move on.

---

## Media conversion

**Audio** — MP3, WAV, FLAC, OGG, AAC, Opus, M4A, WMA, AIFF, ALAC, AC3, DTS, Vorbis, Dolby Digital+, Dolby TrueHD, MIDI, MOD, XM, IT, SF2

**Video** — MP4, MOV, WebM, MKV, AVI, GIF, M4V, FLV, MPG, OGV, TS, 3GP, DivX, RMVB, ASF, WMV

**Codec presets** — H.264, H.265/HEVC, AV1, VP9, Apple ProRes, DNxHD, DNxHR, CineForm, QT Animation, Uncompressed, FFV1, XDCAM HD422/HD35, AVC-Intra, XAVC, XAVC Long GOP, HAP, Theora, MPEG-2, MJPEG, Xvid, DV, MPEG-1

**Image** — JPEG, PNG, WebP, TIFF, BMP, AVIF, GIF, SVG, ICO, JPEG XL, HEIC, HEIF, PSD, EXR, HDR, DDS, XCF, RAW, CR2, CR3, NEF, ARW, DNG, ORF, RW2

**3D** — OBJ, GLTF, GLB, STL, PLY, COLLADA, 3DS, X3D, FBX (ASCII), USD, USDZ, Alembic, Blender, STEP, IGES

---

## Intact video operations

No re-encode. Fast, lossless, container-level.

- **Cut / Extract** — trim to a single range with zero quality loss
- **Replace Audio** — swap an audio track into an existing video
- **Rewrap** — repackage into MP4, MKV, MOV, or WebM without touching streams
- **Merge** — concatenate multiple clips in order
- **Extract** — pull video, audio, subtitle, or all streams out
- **Subtitling** — soft-embed or burn subtitles (shared page with Analysis / AI)

---

## Processing operations

Re-encode. Filter chains, resolution / fps changes, timeline mutation.

- **Conform** — resolution, fps, color space, codec, bitrate, audio options in one page
- **Silence Remover** — detect and strip silence with threshold + minimum duration + pad
- **Video Inserts** *(scaffolded)*

---

## Analysis

Reports, no output file.

- **Loudness & True Peak** — EBU R128 via `loudnorm` JSON report
- **Audio Normalize** — EBU R128 two-pass, Peak, or ReplayGain tag-only
- **Cut Detection** — `scdet` or scene-select with post-filter on minimum shot length
- **Black Detection** — `blackdetect` ranges
- **VMAF** — perceptual quality score against a reference (HD / 4K / phone models)
- **FrameMD5** — per-frame hashing for video / audio / both, with diff mode
- **Subtitling (analyze)** — probe tracks, lint CPS / duration / line length, diff two subtitle files

---

## AI tools *(scaffolded)*

- Audio Separation
- Transcription
- Translation
- Colorize
- Background Remover
- Subtitling (generate)

## Chroma key *(scaffolded)*

- FFmpeg chromakey / colorkey
- Neural matte (RVM) — bundled, no green screen needed
- CorridorKey — managed install for hair / motion-blur on green screen

## Burn & rip *(scaffolded)*

- DVD / Blu-ray authoring
- DVD rip
- Web Video preset

---

## Files

**Data** — JSON, CSV, TSV, XML, YAML, SQLite, Parquet, Jupyter

**Document** — HTML, PDF, TXT, MD

**Office** — PPTX, PPT, DOCX, DOC, XLSX, XLS, ODT, ODP, ODS, RTF, Keynote, Pages, Numbers

**Ebook** — EPUB, MOBI, AZW3, FB2, LIT

**Subtitle** — SRT, VTT, ASS, SSA, SBV, TTML

**Timeline** — EDL, FCPXML, Premiere XML, OTIO, AAF (via OpenTimelineIO)

**Archive** — ZIP, TAR, GZ, 7z, ISO, DMG, CBR, CBZ, RAR

**Font** — TTF, OTF, WOFF, WOFF2

**Email** — MSG, EML, MBOX

---

## Workflow

- Visual waveform scrubber with in/out trim handles
- Batch queue — drop a folder, process everything at once
- Output control — destination, suffix, or convert in place
- Proxy folder for heavy source files
- Compact / expanded queue views
- Mouse back-button navigation in operation pages
- Incompatible output formats grayed out based on what's selected
- Runs entirely local. Nothing leaves your machine.

---

## Download

Fade runs on macOS, Windows, and Linux.

| Platform | Requirement |
|----------|-------------|
| macOS | 13+ · Apple Silicon & Intel |
| Windows | 10 / 11 · x64 |
| Linux | x64 · `.deb` or `.AppImage` |

Releases coming soon.

---

## Build from source

```
npm install
npm run tauri build
```

Requires [Rust](https://rustup.rs) and [Node.js](https://nodejs.org).

```
npm run tauri dev   # development
```
