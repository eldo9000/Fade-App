# TASK-27: Sweep re-baseline — confirm post-BC-005 state

## Goal
`full_sweep video_full` and `refactored_av_sweep` both run to completion. Results are logged. Only env-blocked failures (HAP encoder absent, libtheora absent) and the known 64×64 fixture-shape failures (DNxHD/DNxHR with no explicit resolution) remain as OPEN. All other failures from the original 173-failure baseline are CONFIRMED fixed. SESSION-STATUS reflects the 0.6.6 sweep closure.

## Context
The video sweep originally surfaced 173 failures. BC-005 fixes in TASK-18–21 + TASK-23 resolved:
- 27 h265 profile-name failures (TASK-18)
- 120 h264-lossless failures (TASK-19)
- 9 av1 encoder-absent failures (TASK-21, switched to libsvtav1)

Expected remaining failures after fixes:
- **HAP: ~15 cases** (env-blocked — HAP encoder absent from Homebrew FFmpeg 8.1)
- **libtheora: ~1 case** (env-blocked — same class)
- **DNxHD/DNxHR 64×64 fixture: some cases** (fixture-shape — guard fires only on explicit resolution; the 64×64 sweep fixture sets no resolution)
- **h265-lossless: some cases** (explicitly deferred)

Run both sweeps. Classify each failure against the documented expected set. Any failure NOT in the expected set is a regression — flag it as a new OPEN entry and stop.

## In scope
- Running the tests (read-only)
- Appending to `INVESTIGATION-LOG.md`
- Updating `SESSION-STATUS.md`

## Out of scope
- Fixing any failures
- Touching any source file (`args/`, `convert/`, `src/`)
- Touching any test file
- Adding new sweep cases

## Steps
1. From `src-tauri/`, run: `cargo test --test full_sweep video_full -- --ignored --nocapture 2>&1`
   Capture full output. Count total cases, passed, failed.
2. From `src-tauri/`, run: `cargo test --test refactored_av_sweep -- --ignored --nocapture 2>&1`
   Capture full output. Count total cases, passed, failed.
3. For each failure in video_full:
   - Match against the expected set (HAP absent, libtheora absent, DNxHD/DNxHR 64×64, h265-lossless).
   - If it matches: no new log entry needed (these are already documented OPENs).
   - If it does NOT match: append `YYYY-MM-DD | OPEN | video_full regression — <case-name>: <error> — unexpected, not in expected set` and stop. Surface to dispatcher.
4. For refactored_av_sweep: same classification. Any unexpected failure = stop.
5. If all failures match the expected set: append to `INVESTIGATION-LOG.md`:
   `YYYY-MM-DD | CONFIRMED | 0.6.6 sweep re-baseline — video_full: N total, M failed (all env-blocked/fixture-shape/deferred, none new); refactored_av_sweep: P total, Q failed. BC-005 fixes confirmed held.`
6. Update `SESSION-STATUS.md`: add a 0.6.6 line at the top of `## Next action` with failure counts and confirmed-no-regressions verdict.
7. Do NOT commit (dispatcher commits).

## Success signal
- Both sweeps complete without Rust panics.
- All failures match the expected set.
- INVESTIGATION-LOG.md has a CONFIRMED entry for the re-baseline.
- SESSION-STATUS.md has the 0.6.6 line.

## Notes
- If `video_full` surfaces a case name containing "hap", "theora", "dnxhd_br", "dnxhr", or "h265_lossless" in its failure list, that's expected.
- If `video_full` surfaces anything else in the failure list, that's a regression — stop and surface.
- Test timeout: video_full can take several minutes (spawns ffmpeg per case). Use a generous timeout (600000ms).
