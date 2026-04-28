# TASK-13: Add HAP sub-format cases to `full_sweep.rs`

## Goal
`tests/full_sweep.rs` covers HAP, HAP Q, and HAP Alpha sub-formats end-to-end. The encoder-constraint class (BC-005) has equivalent sweep coverage to DNxHD and CineForm. Any encoder-constraint bugs in the HAP pipeline surface in the next sweep run rather than in user reports.

## Context
BC-005 in `KNOWN-BUG-CLASSES.md` documents the encoder-constraint pattern. Three confirmed instances led to the class:
- AVIF speed cap (libheif limit)
- DNxHR resolution minimum (≥ 1280×720)
- H.264 profile/pix_fmt impossible combos

Recent sweep commits added DNxHD bitrate and CineForm quality cases (`1dbf064`). HAP remains unswept.

HAP is a video codec with three sub-formats:
- **HAP** — base codec, RGB
- **HAP Q** — higher quality, RGB+alpha intermediate
- **HAP Alpha** — alpha-channel support

ffmpeg exposes these via `-vcodec hap -format hap|hap_q|hap_alpha`. The `hap_format` field in `ConvertOptions` selects the sub-format. Likely encoder constraints to surface:
- Resolution divisibility — HAP requires width and height multiples of 4
- Pixel format — HAP requires `rgba` for alpha variants, `rgb24` otherwise
- Container compatibility — HAP needs MOV; AVI/MKV with HAP may fail

The sweep-test pattern in `full_sweep.rs` is Cartesian — combinations of options that should "just work". When they don't, the failure surfaces a constraint to add to the arg builder + UI disable list.

Relevant files:
- `src-tauri/tests/full_sweep.rs` — current sweep; reference DNxHD/CineForm cases for the pattern
- `src-tauri/src/args/video.rs` — HAP arg construction; check current behaviour
- `KNOWN-BUG-CLASSES.md` — BC-005 entry

## In scope
- Add HAP, HAP Q, HAP Alpha sweep cases to `full_sweep.rs`. For each:
  - Try common resolutions: 1920×1080 (multiple of 4), 1280×720, 640×480, plus an "off" case like 1023×769 (not multiple of 4) to surface the divisibility constraint.
  - Try MOV container (canonical) plus MP4 if HAP-in-MP4 is intended to be supported.
  - For HAP Alpha, include a fixture with alpha if available; otherwise note in the test that it's testing the encoder-arg path, not actual alpha preservation.
- Mark all new cases `#[ignore]` per the existing sweep convention (manual-only; CI runs only the smoke matrix).
- For each constraint surfaced (test fails), file follow-up TODOs in INVESTIGATION-LOG.md and add a guard to the arg builder.

## Out of scope
- Surfacing fixes for any HAP constraints found — that's a follow-up arc.
- Adding HAP cases to `matrix.rs` (the smoke gate). Only `full_sweep.rs` needs them.
- Sweeping any other codec (e.g. ProRes, Cinepak) — separate task if desired.
- UI changes to the HAP options panel (`src/lib/VideoOptions.svelte` or wherever).

## Steps
1. Read `tests/full_sweep.rs` end-to-end. Find the DNxHD case set added in `1dbf064` to use as a template.
2. Identify the helper for "build a `ConvertOptions` with codec=hap, format=X". If a helper exists, extend it; otherwise build inline test cases.
3. For each of HAP, HAP Q, HAP Alpha, add cases:
   ```rust
   // HAP base — common resolutions
   ConvertOptionsCase {
       codec: "hap",
       hap_format: Some("hap"),
       resolution: "1920x1080",
       container: "mov",
       expected: Pass,
   },
   // HAP base — non-divisible-by-4 (likely fail; surface constraint)
   ConvertOptionsCase {
       codec: "hap",
       hap_format: Some("hap"),
       resolution: "1023x769",
       container: "mov",
       expected: Pass, // mark as Pass; if fails, that's the finding
   },
   // HAP Q
   ConvertOptionsCase { ..., hap_format: Some("hap_q"), ..., },
   // HAP Alpha
   ConvertOptionsCase { ..., hap_format: Some("hap_alpha"), ..., },
   ```
   Use the actual struct shape from `full_sweep.rs` — the above is illustrative.
4. Mark each new case `#[ignore]` if added as separate tests; if added to a Cartesian helper, the helper is already `#[ignore]`.
5. Run the new sweep cases manually:
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep -- --include-ignored hap
   ```
6. For each case that fails, capture the failure mode (ffmpeg stderr) and append a one-line entry to `INVESTIGATION-LOG.md` per the project's INVESTIGATION-LOG protocol. Example:
   ```
   2026-04-26 | OPEN | HAP encoder rejects non-multiple-of-4 resolution (1023x769) — full_sweep
   ```
7. `cargo fmt --manifest-path src-tauri/Cargo.toml`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep -i "hap" src-tauri/tests/full_sweep.rs` returns 6+ matches (at least 2 per sub-format).
- Running `cargo test --include-ignored hap` exits 0 if all cases pass, OR produces one or more INVESTIGATION-LOG entries documenting the surfaced constraints.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- Sweep run output is documented in INVESTIGATION-LOG.md if any failures appear.

## Notes
- This task is sweep coverage, not feature implementation. The point is to *surface* constraints — failures are findings, not bugs.
- HAP is a niche codec used mainly in VJ / live performance contexts. Real-world user impact of constraints is low; sweep coverage is mainly defensive.
- If all cases pass, that's a positive signal — HAP encoder args are well-behaved. Document in SESSION-STATUS as a closed BC-005 sub-area.
- The pattern documented in BC-005 is "UI presents invalid encoder-option combinations". Adding HAP sweep coverage extends prevention to this codec; further UI-disable hardening (matching what was done for H.264 profile/pix_fmt at `50c89cb`) is out of scope here.
