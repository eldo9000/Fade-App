# TASK-3: Migrate remaining inline run() wrappers to emitter helpers

## Goal
`video.rs`, `audio.rs`, and `archive.rs` run() wrappers no longer contain inline `ProgressEvent` closures — they delegate to helpers in `convert/mod.rs`, consistent with the 12 modules migrated in `da6ca88`.

## Context
In commit `da6ca88` (`refactor(convert): extract window_progress_emitter helper; dedup 12 run() wrappers`), a `window_progress_emitter()` helper was added to `src-tauri/src/convert/mod.rs`. It encapsulates the boilerplate: clone the window, build a closure that emits `JobProgress` payloads for `ProgressEvent::Started`, `Phase`, `Percent`, and `Done`. 12 of the 15 modules were migrated to use it.

Three modules were explicitly excluded with a comment in `mod.rs` (around line 51):
> "Callers that batch Phase+Percent (e.g. `video::run`, `audio::run`) must NOT use this helper — their closure logic cannot be expressed here."

The "batching" behavior in `video.rs` and `audio.rs`: when `ProgressEvent::Phase(msg)` fires, the message is stored in `pending_phase` instead of emitted immediately. When the next `ProgressEvent::Percent(p)` fires, both the message and percent are emitted together as a single `JobProgress` payload. This allows the frontend to show "2s elapsed — 45%" in one update rather than two.

`archive.rs` does NOT batch — its inline closure matches `window_progress_emitter`'s structure almost exactly, except for a dynamic initial message ("Extracting…" vs "Repacking…" depending on `operation`). This is already supported by `window_progress_emitter`'s `starting_msg: &str` parameter. However: archive.rs passes percent in the 0–100 range to its closure (`.clamp(0.0, 100.0)`), while `window_progress_emitter` applies `(p * 100.0).clamp(0.0, 100.0)` (expecting 0.0–1.0 input). Before migrating archive.rs, verify which scale the archive `convert()` function actually emits.

The plan:
1. Add a `window_progress_emitter_batched(window, job_id) -> impl FnMut(ProgressEvent)` helper to `convert/mod.rs` that implements the Phase+Percent batching behavior. This mirrors what video.rs and audio.rs do inline.
2. Migrate `video.rs` and `audio.rs` run() to use `window_progress_emitter_batched`.
3. Verify the percent scale in `archive.rs` convert() calls, then migrate `archive.rs` run() to use `window_progress_emitter` (with the correct starting_msg and matching scale).
4. Update the `mod.rs` comment that says video/audio "must NOT" use the helper — it will now be outdated.

Read `src-tauri/src/convert/mod.rs` in full (it is short) before editing. Read `video.rs`, `audio.rs`, and `archive.rs` run() bodies in full. Do not assume the batching behavior is identical across video and audio — verify before writing the helper.

## In scope
- `src-tauri/src/convert/mod.rs` — add `window_progress_emitter_batched()`, update comment
- `src-tauri/src/convert/video.rs` — run() wrapper only (lines 124–156)
- `src-tauri/src/convert/audio.rs` — run() wrapper only
- `src-tauri/src/convert/archive.rs` — run() wrapper only

## Out of scope
- Any `convert()` function body in any module — the pure conversion functions must not be touched
- `lib.rs`
- Any frontend Svelte files
- Any test files
- Any module other than the four listed

## Steps
1. Read `src-tauri/src/convert/mod.rs` in full — understand the existing `window_progress_emitter` signature, behavior, and the "must NOT" comment.
2. Read `src-tauri/src/convert/video.rs` run() (lines 124–156) — note the exact Phase+Percent batching logic.
3. Read `src-tauri/src/convert/audio.rs` run() — confirm it is identical to video.rs in structure.
4. Read `src-tauri/src/convert/archive.rs` run() — note the `initial_message` dynamic value and the percent scale (`p.clamp(0.0, 100.0)` vs `(p * 100.0).clamp(0.0, 100.0)`). Search for where `ProgressEvent::Percent` is called in `archive::convert()` to confirm the input scale.
5. Add `pub fn window_progress_emitter_batched(window: &tauri::Window, job_id: &str) -> impl FnMut(ProgressEvent)` to `convert/mod.rs`. The closure must buffer Phase messages and flush them on the next Percent event, exactly as the inline closures do.
6. Replace the inline closure in `video.rs` run() with a call to `crate::convert::window_progress_emitter_batched(window, job_id)`.
7. Replace the inline closure in `audio.rs` run() identically.
8. For `archive.rs`: if the convert() function emits Percent in 0.0–1.0 range, migrate to `window_progress_emitter` with the dynamic starting_msg. If it emits in 0–100 range, either fix the scale in convert() (preferred) or document why the migration is blocked and leave archive inline with a comment.
9. Update the "must NOT" comment in `mod.rs` to reflect the new state.
10. Run `cargo check` from `src-tauri/` — must pass with no new errors or warnings.
11. Commit the change.

## Success signal
`cargo check` exits 0. `grep -n "pending_phase\|move |ev: ProgressEvent" src-tauri/src/convert/video.rs src-tauri/src/convert/audio.rs` returns no matches (inline closures are gone). `grep -n "window_progress_emitter" src-tauri/src/convert/video.rs src-tauri/src/convert/audio.rs` shows the new helper call in each.

## Notes
If video.rs and audio.rs batching logic is not identical, write two helpers instead of one, or parameterize. Do not force a shared abstraction that requires changing behavior.

If the archive.rs percent scale issue blocks clean migration, note it in the commit message and leave archive.rs inline with a `// TODO(batch-scale): archive emits 0-100` comment rather than silently changing archive behavior.

Commit message: `refactor(convert): migrate remaining run() wrappers to emitter helpers`
