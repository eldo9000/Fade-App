# TASK-1: Clamp AVIF speed to libheif's actual cap of 9

## Goal
`ConvertOptions::avif_speed` is clamped to 9 in the Rust arg builder, the doc comment is corrected to say `0–9`, and the ImageOptions.svelte speed slider is capped at `max="9"`. `avif_speed = 10` can no longer be stored or emitted.

## Context
Fade's `full_sweep.rs` diagnostic (run 2026-04-25) found that every AVIF conversion with `avif_speed = 10` fails. Root cause: libheif's `heic:speed` define accepts 0–9; 10 is out-of-range and ImageMagick rejects it. `ConvertOptions::avif_speed` is currently documented `// 0-10`, the UI slider uses `min="0" max="10"`, and the arg builder emits whatever value arrives without clamping. This is a UI→backend mismatch — the user can select a value that always fails.

Relevant files:
- `src-tauri/src/lib.rs` line 269: `pub avif_speed: Option<u32>, // 0-10` — doc comment is wrong
- `src-tauri/src/args/image.rs` lines 175–185: the AVIF branch that builds `heic:speed={}` — no clamp applied
- `src/lib/ImageOptions.svelte` lines 238–244: the speed slider (`min="0" max="10"`)
- `src-tauri/src/args/image.rs` existing test `image_args_avif_speed_and_chroma` around line 349 — verify it still passes after the clamp

## In scope
- `src-tauri/src/args/image.rs` — clamp `s` to 9 before emitting `heic:speed={}`. Add a unit test asserting that `avif_speed = 10` emits `heic:speed=9`.
- `src-tauri/src/lib.rs` — update doc comment on `avif_speed` from `// 0-10` to `// 0–9; clamped at 9 (libheif limit)`
- `src/lib/ImageOptions.svelte` — change `max="10"` to `max="9"` and update the `10-0` denominator in the range-pct style to `9-0`

## Out of scope
- Any change to how `avif_chroma` works
- Any change to other image formats
- Any change to `ConvertOptions` field types (stays `Option<u32>`)
- Any change to `full_sweep.rs` or `matrix.rs`

## Steps
1. Read `src-tauri/src/args/image.rs` lines 175–190 (the AVIF branch). Confirm the current emit: `heic:speed={}` with `s` directly from `opts.avif_speed`.
2. In the AVIF branch, change `s` to `s.min(9)` before the format! call. The result should emit `heic:speed={}` with the clamped value.
3. Update `src-tauri/src/lib.rs` line 269 doc comment: `// 0–9; clamped at 9 (libheif limit)`.
4. Add a unit test to `src-tauri/src/args/image.rs`'s `#[cfg(test)] mod tests`:
   - Name: `image_args_avif_speed_clamp_at_9`
   - Set `avif_speed = Some(10)`. Build args. Assert the arg list contains `"heic:speed=9"` and does NOT contain `"heic:speed=10"`.
5. Run `cargo test --manifest-path src-tauri/Cargo.toml --lib args::image`. All tests must pass including the existing `image_args_avif_speed_and_chroma` and the new clamp test.
6. Read `src/lib/ImageOptions.svelte` lines 238–244. Change `max="10"` to `max="9"`. Change the style denominator `(10-0)` to `(9-0)` in the range-pct calculation.
7. Verify the tooltip text at the fieldset (currently `"0 slowest / best · 10 fastest / worst"`) — update it to `"0 slowest / best · 9 fastest / worst"`.
8. Run `npm run check` (Svelte typecheck) if available.
9. `cargo fmt --manifest-path src-tauri/Cargo.toml` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `grep "heic:speed" src-tauri/src/args/image.rs` shows the `.min(9)` clamp applied.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib args::image` exits 0 with the new clamp test passing.
- `grep 'max="9"' src/lib/ImageOptions.svelte` returns a match.
- `grep '0-10\|10-0\|max="10"' src/lib/ImageOptions.svelte` returns no matches (old values gone).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- The clamp is defensive-only — the UI will never send 10 after the slider change, but backend protection is correct regardless.
- Do not change the field type to `Option<u8>` or otherwise restructure `ConvertOptions`. Only the doc comment changes.
- The existing test at line ~349 uses `avif_speed: Some(6)` — no behavior change there, just verify it still passes.
