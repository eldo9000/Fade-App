# TASK-22: Mark stale OPEN entries CONFIRMED in INVESTIGATION-LOG

## Goal
`INVESTIGATION-LOG.md` accurately reflects the current state of all findings. Four OPEN entries from 2026-04-28 that have since been resolved by committed fixes are updated to CONFIRMED with their closing commits. The log is the source of truth for open work; stale OPENs create false urgency.

## Context
Three sprints landed on 2026-04-28 (TASK-1 through TASK-21, all CI-green). Several INVESTIGATION-LOG entries were written as OPEN during the sweep run (TASK-15) and were subsequently fixed by implementation tasks, but the log was not updated after each fix landed. The following entries are stale:

**Should become CONFIRMED:**
- `h264 lossless (crf=0) requires high444 profile` — Fixed by commit `ea93db0` (TASK-19: forced yuv444p/high444 when crf=0 in args/video.rs).
- `h265 (libx265) rejects h264 profile names emitted by shared arg path` — Fixed by commit `fa74e91` (TASK-18: added h265_effective_profile(), split h264|h265 branch).

**Partially resolved — update with nuance:**
- `DNxHD fixed-bitrate-resolution coupling unguarded` — Commit `7ee89bf` (TASK-20) added a resolution guard in `convert/video.rs` matching the DNxHR guard pattern. The guard covers the user-facing case (explicit resolution set below 1280×720). The 64×64 test-fixture sweep failures remain because the fixture passes no explicit resolution — the guard only fires when `opts.resolution` is set. Add a CONFIRMED line noting the guard was added and the fixture-only gap, but the user-facing case is protected.

**Leave as OPEN (genuinely unresolved):**
- `DNxHR resolution guard bypassed at arg-builder layer` — Doc comment added (TASK-20) but architectural gap remains. Leave OPEN.
- `HAP encoder absent` and `HAP resolution divisibility` — Environment issues, no code fix possible without custom FFmpeg build. Leave OPEN.
- `libtheora absent` — Same class as HAP. Leave OPEN.

**Already has CONFIRMED:** `libaom-av1 absent` — already marked CONFIRMED in the log. Do not touch.

The OPEN entry from 2026-04-25 (`dispatch 1/3 — encoder-constraint class undocumented`) already has a same-day CONFIRMED entry below it. Leave both untouched.

## In scope
- `INVESTIGATION-LOG.md` — append CONFIRMED lines for the three entries described above

## Out of scope
- Any source file
- Any test file
- SESSION-STATUS.md
- OBSERVER-STATE.md
- Entries not listed above

## Steps
1. Read `INVESTIGATION-LOG.md` in full to confirm the exact text of each entry.
2. For the h264 lossless OPEN entry, append immediately after it:
   ```
   2026-04-28 | CONFIRMED | h264 lossless (crf=0) — forced yuv444p + high444 profile in args/video.rs when crf=0; commit ea93db0 (TASK-19)
   ```
3. For the h265 profile OPEN entry, append immediately after it:
   ```
   2026-04-28 | CONFIRMED | h265 profile mismatch — added h265_effective_profile(), split h264|h265 branch in args/video.rs; commit fa74e91 (TASK-18)
   ```
4. For the DNxHD OPEN entry, append immediately after it:
   ```
   2026-04-28 | CONFIRMED | DNxHD resolution guard added in convert/video.rs for explicit-resolution path (commit 7ee89bf, TASK-20); 64×64 fixture bypass remains (guard requires opts.resolution to be set)
   ```
5. Do not touch any other entry.

## Success signal
- `grep -c "CONFIRMED" INVESTIGATION-LOG.md` returns a higher count than before (3 new CONFIRMED lines added).
- `grep "h264 lossless" INVESTIGATION-LOG.md` shows both the OPEN line and a CONFIRMED line.
- `grep "h265.*profile" INVESTIGATION-LOG.md` shows both the OPEN line and a CONFIRMED line.
- `grep "DNxHD" INVESTIGATION-LOG.md` shows both the OPEN line and a CONFIRMED line.
- The file has no unintended modifications to other entries.
