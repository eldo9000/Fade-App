# Sprint G — AI Tools

**Goal:** Implement all 5 AI tool scaffolds. Each requires a local model or managed install. No cloud APIs — runs entirely offline.

**Entry condition:** External tool detection + managed install pattern established. Decide on managed install mechanism (same approach as CorridorKey).

---

## TASK-G1: Audio Separation (stems)

**Scope:** Separate a mixed audio/video file into stems (vocals, drums, bass, other).

**Tool:** Demucs (Meta, MIT license) — Python CLI, runs locally.

**What to do:**
- Managed install: check for `demucs` in venv or PATH; offer one-click install via `pip install demucs`
- Command: `python -m demucs --mp3 --out <output_dir> <input>`
- Model: default `htdemucs` (4-stem); offer `htdemucs_ft` (fine-tuned) as quality preset
- Progress: parse Demucs stderr `%` progress
- Output: 4 files per stem in `<output_dir>/<model>/<track_name>/`
- Wire Tauri command `run_audio_separation`

**Done when:** Demucs separates a test file to 4 stems. Progress emits. CI green (gated on demucs presence).

---

## TASK-G2: Transcription (speech to text)

**Scope:** Transcribe audio/video to SRT, VTT, or plain text.

**Tool:** Whisper (OpenAI, MIT license) — Python CLI, runs locally via `whisper` or `faster-whisper`.

**What to do:**
- Managed install: check for `whisper` or `faster-whisper`; offer pip install
- Command: `whisper <input> --model medium --output_format srt --output_dir <dir>`
- Models: tiny/base/small/medium/large (expose in UI as quality presets)
- Language: auto-detect (default) or user-specified locale
- Output formats: SRT, VTT, TXT
- Wire Tauri command `run_transcription`

**Done when:** Whisper transcribes a short test clip to SRT. CI green (gated on whisper presence).

---

## TASK-G3: Translation (subtitle/text translation)

**Scope:** Translate an SRT/VTT/TXT file from any language to another.

**Tool:** Argos Translate (MIT) or Helsinki NLP models via ctranslate2 — all local, no API key.

**What to do:**
- Managed install: `pip install argostranslate`
- Download language pack on demand: `argostranslate.package.install_from_path(pkg)`
- Input: SRT/VTT/TXT file + source language + target language
- Preserve SRT timing; only translate text content
- Wire Tauri command `run_translation`

**Done when:** English SRT translates to Spanish SRT preserving timestamps. CI green (gated on argostranslate).

---

## TASK-G4: Colorize (black and white to color)

**Scope:** Colorize a grayscale video or image.

**Tool:** DeOldify (MIT) or `ddcolor` (Apache 2.0) — both run locally.

**What to do:**
- Evaluate: DeOldify (well-known, good quality) vs ddcolor (newer, faster)
- Managed install via pip
- For video: apply colorization frame-by-frame, reassemble with FFmpeg
- For image: single inference pass
- Progress: per-frame progress bar for video
- Wire Tauri command `run_colorize`

**Done when:** Grayscale image colorizes cleanly. CI green (gated on model presence).

---

## TASK-G5: Background Remover

**Scope:** Remove background from image or video (chroma-free, AI matte).

**Tool:** `rembg` (MIT) — Python CLI, runs locally via ONNX.

**What to do:**
- Managed install: `pip install rembg`
- Command: `rembg i <input> <output>` (image); for video: frame-by-frame with reassembly
- Output: PNG with alpha channel (image), MOV QTRLE with alpha (video)
- Models: u2net (default), u2netp (fast), isnet-general-use (high quality)
- Wire Tauri command `run_bg_remove`

**Done when:** Background removed from test image with clean alpha. CI green (gated on rembg presence).
