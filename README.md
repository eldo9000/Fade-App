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

**Audio** — MP3, WAV, FLAC, OGG, AAC, Opus, M4A, WMA, ALAC, AC3, DTS, MIDI, MOD, XM, IT, SF2

**Video** — MP4, MOV, WebM, MKV, AVI, GIF, M4V, FLV, MPG, TS, 3GP, DivX, RMVB, ASF, WMV

**Codec presets** — H.264, H.265/HEVC, AV1, VP9, Apple ProRes, DNxHD, DNxHR, CineForm, QT Animation, Uncompressed, FFV1, XDCAM HD422/HD35, HAP, Theora, MPEG-2, MJPEG, Xvid, DV, MPEG-1

**Image** — JPEG, PNG, WebP, TIFF, BMP, AVIF

**3D** — OBJ, GLTF, GLB, STL, PLY, COLLADA, 3DS, X3D, FBX (ASCII)

---

## Intact video operations

No re-encode. Fast, lossless, container-level.

- **Cut / Extract** — trim to a single range with zero quality loss
- **Replace Audio** — swap an audio track into an existing video; auto-transcodes incompatible codecs
- **Rewrap** — repackage into MP4, MKV, MOV, or WebM without touching streams
- **Merge** — concatenate clips; fast stream-copy when streams match, re-encode when they don't
- **Extract Streams** — pull video, audio, or subtitle tracks out individually or all at once

---

## Processing operations

Re-encode. Filter chains, resolution / fps changes, timeline mutation.

- **Conform** — resolution, fps, color space, codec, bitrate, and audio normalization in one page
- **Silence Remover** — detect and strip silence with threshold, minimum duration, and pad controls
- **Chroma Key** — chromakey, colorkey, or hsvkey with despill correction; renders to MOV/WebM/PNG sequence/MKV

---

## Video filters

- **Rotate / Flip** — 90° CW/CCW, 180°, horizontal flip, vertical flip
- **Reverse** — reverse video and audio playback
- **Speed** — arbitrary playback rate via `setpts`
- **Fade** — fade-in and fade-out with configurable durations
- **Deinterlace** — yadif, yadif (double rate), bwdif
- **Denoise** — NLMeans with light / medium / strong presets

---

## Audio filters

- **Volume** — dB gain adjustment
- **Channel Tools** — stereo-to-mono, swap channels, mute left/right, mono-to-stereo
- **Pad Silence** — add silence to head or tail
- **Audio Offset** — delay audio track by milliseconds

---

## Frame operations

- **Thumbnail** — extract a representative frame at any timecode
- **Contact Sheet** — grid of keyframes across the full duration
- **Frame Export** — PNG / JPEG / WebP image sequence
- **Watermark** — overlay image with corner positioning, opacity, and scale controls

---

## Analysis

Reports, no output file.

- **Loudness & True Peak** — EBU R128 via `loudnorm` JSON report; integrated loudness, LRA, true peak
- **Audio Normalize** — EBU R128 two-pass, Peak, or ReplayGain
- **Cut Detection** — `scdet` or scene-select with post-filter on minimum shot length
- **Black Detection** — `blackdetect` ranges with configurable thresholds
- **VMAF** — perceptual quality score against a reference (HD / 4K / phone models)
- **FrameMD5** — per-frame hashing for video / audio / both, with diff mode

---

## File conversion

**Data** — JSON, CSV, TSV, XML, YAML, SQLite, Parquet, Jupyter

**Document** — HTML, PDF, TXT, MD

**Subtitle** — SRT, VTT, ASS, SSA, SBV, TTML — with lint (overlap, CPS, line length)

**Timeline** — EDL, FCPXML, OTIO, AAF (via OpenTimelineIO)

**Archive** — ZIP, TAR, GZ, 7z, CBR, CBZ, RAR

**Font** — TTF, OTF, WOFF, WOFF2

**Ebook** — EPUB, MOBI, AZW3, FB2, LIT

**Email** — EML, MBOX

---

## Workflow

- Visual waveform scrubber with in/out trim handles
- Waveform and spectrogram visualization
- Keyframe filmstrip timeline
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
