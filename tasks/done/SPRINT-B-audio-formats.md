# Sprint B — Audio Format Expansion

**Goal:** Unlock the 4 audio output formats currently marked `building` or `todo`. All are FFmpeg-passthrough — the hard work is verifying codec availability in CI and wiring the UI correctly.

**Entry condition:** Audio conversion pipeline stable. `full_sweep` audio cases passing.

---

## TASK-B1: AIFF output

**Scope:** Enable AIFF as a live output format (currently `building: true`).

**What to do:**
- AIFF is PCM-in-AIFF container — FFmpeg supports natively (`-f aiff`)
- Set `aiff` to `live: true` in audio format picker
- Add AIFF output cases to `full_sweep` audio section
- Verify round-trip: WAV → AIFF → WAV sample-accurate

**Done when:** AIFF output cases pass in sweep. CI green.

---

## TASK-B2: Vorbis output (.ogg stream, not container)

**Scope:** Enable Vorbis as a distinct output option (currently `todo`).

**What to do:**
- Note: OGG container already live. Vorbis here means explicit `-c:a libvorbis` selection
- Check CI: `ffmpeg -codecs | grep vorbis` — libvorbis should be present
- If present: set `vorbis` to `live: true`, wire codec in `convert/audio.rs`
- If absent: emit "Vorbis requires FFmpeg with libvorbis"
- Add sweep cases

**Done when:** Vorbis output cases pass or emit dependency error. CI green.

---

## TASK-B3: Dolby Digital+ (EAC3)

**Scope:** Enable EAC3 output (currently `todo`).

**What to do:**
- Check CI: `ffmpeg -codecs | grep eac3`
- FFmpeg ships `eac3` encoder natively in most builds
- Set `ddp` / `eac3` to `live: true`, wire `-c:a eac3` in audio converter
- Add bitrate options (384k, 448k, 640k standard for EAC3)
- Add sweep cases

**Done when:** EAC3 output cases pass. CI green.

---

## TASK-B4: Dolby TrueHD

**Scope:** Enable TrueHD output (currently `todo`).

**What to do:**
- Check CI: `ffmpeg -codecs | grep truehd` — TrueHD encoder (`mlp`) availability varies
- TrueHD is typically lossless PCM encoded in MLP container; requires MKV or raw `.thd` output
- If encoder present: wire `-c:a truehd` with MKV container output, add sweep cases
- If absent: mark as env-blocked in INVESTIGATION-LOG (same pattern as HAP)

**Done when:** TrueHD either converts cleanly or emits env-blocked diagnostic. CI green.
