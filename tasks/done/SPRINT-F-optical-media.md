# Sprint F — Optical Media (DVD / Blu-ray)

**Goal:** Implement DVD rip, DVD authoring, and Blu-ray rip. All scaffolded. Heavy external tool dependency.

**Entry condition:** Core FFmpeg pipeline stable. External tool detection pattern established.

---

## TASK-F1: DVD rip

**Scope:** Rip a DVD to MP4/MKV/HEVC.

**What to do:**
- Tool chain: `HandBrakeCLI` (preferred — handles CSS, menu skipping, chapter detection) or `dvdbackup` + FFmpeg
- Detect `HandBrakeCLI` binary; emit clear error if missing
- UI: input is a DVD drive path or ISO file; output format selector (MP4/MKV); quality preset (RF 20/22/24)
- Wire `HandBrakeCLI --input <dvd> --output <out> --preset "H.264 MKV 1080p30"` with progress parsing
- HandBrake progress: parse `Encoding: task 1 of 1, <N.NN> %` from stdout
- Add basic test with ISO fixture if available; otherwise integration test only

**Done when:** DVD ISO rip to MKV works end-to-end. Progress emits. CI green (test gated on HandBrake presence).

---

## TASK-F2: DVD authoring

**Scope:** Author a DVD from video files.

**What to do:**
- Tool chain: `dvdauthor` + `mkisofs`/`genisoimage`
- Workflow: FFmpeg transcode → MPEG-2/AC3 → dvdauthor XML → ISO
- Detect both `dvdauthor` and `mkisofs`/`genisoimage`
- UI: input list of video files, output ISO path, optional chapter markers
- Implement `create_dvd_iso()` in new `operations/dvd_author.rs`
- Wire transcode step: `-vcodec mpeg2video -b:v 5000k -vf scale=720:480 -acodec ac3 -b:a 192k`
- Add smoke test

**Done when:** Single video file authors to playable ISO structure. CI green (gated on dvdauthor presence).

---

## TASK-F3: Blu-ray rip

**Scope:** Rip a Blu-ray disc or ISO to HEVC/MKV.

**What to do:**
- Tool chain: `HandBrakeCLI` (handles BD+ via libbluray) or `MakeMKV` CLI
- HandBrake with `--input <bd_path>` and `--preset "H.265 MKV 1080p30"`
- UI: same shape as DVD rip (F1) — drive or ISO input, output format
- Reuse progress parsing from F1
- Note: BD+ DRM requires libbluray; document this limitation

**Done when:** Blu-ray ISO rips to MKV. CI green (gated on HandBrake presence with libbluray).

---

## TASK-F4: Web Video preset

**Scope:** One-click "optimize for web" output from any video.

**What to do:**
- Preset: H.264 + AAC, crf 23, `faststart` moov atom (critical for streaming), 1080p max, 2-pass optional
- Wire as a named preset in `convert/video.rs` — not a new file, just a preset bundle
- FFmpeg flags: `-c:v libx264 -crf 23 -preset slow -movflags +faststart -c:a aac -b:a 128k -vf scale=-2:1080`
- UI: single-click card in the video conversion page
- Add sweep case

**Done when:** Any video → web-optimized MP4 with `faststart` flag verified. CI green.
