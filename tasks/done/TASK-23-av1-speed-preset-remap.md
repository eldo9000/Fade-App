# TASK-23: Remap av1_speed from -cpu-used to -preset for libsvtav1

## Goal
When codec is "av1", `args/video.rs` emits `-preset` (libsvtav1's speed parameter) instead of `-cpu-used` (libaom-av1's speed parameter). The `av1_speed` user option has a visible effect on encode speed. The silent no-op introduced when AV1 was switched to libsvtav1 (TASK-21) is closed.

## Context
TASK-21 switched the AV1 encoder from `libaom-av1` to `libsvtav1` in `args/video.rs`. The encoder name change was a one-line fix and worked correctly, but a side-effect was noted: the AV1 codec arm still emits `-cpu-used` for speed control, which is a libaom-av1-specific flag. libsvtav1 silently ignores `-cpu-used` and uses `-preset` (0–13) for speed control instead. As a result, the `av1_speed` user option currently has no effect.

Parameter mapping:
- libaom-av1: `-cpu-used 0–8` (0 = slowest/best quality, 8 = fastest/lowest quality)
- libsvtav1: `-preset 0–13` (0 = slowest/best, 13 = fastest/lowest)

The `av1_speed` field in `ConvertOptions` likely holds a value in the libaom-av1 range (0–8) or a normalized range. The fix is to change the emitted flag name from `-cpu-used` to `-preset` in the AV1 arm. If the value range differs significantly (0–8 vs 0–13), scale linearly: `preset = round(speed * 13 / 8)`. If the field already accepts 0–13 or a user-visible normalized range, use it directly after reading the actual field definition.

Relevant files:
- `src-tauri/src/args/video.rs` — AV1 arm in `build_ffmpeg_video_args()` or equivalent where `-cpu-used` is emitted
- `src-tauri/src/lib/types/generated/ConvertOptions.ts` or the Rust struct — check the av1_speed field range/type

## In scope
- `src-tauri/src/args/video.rs` — change `-cpu-used` to `-preset` in AV1 arm; scale value if needed
- `INVESTIGATION-LOG.md` — no new entry needed (this is a follow-up to the already-CONFIRMED libaom-av1 entry)

## Out of scope
- Any UI changes to the AV1 speed slider range
- Changing the `av1_speed` field type or range in `ConvertOptions`
- libtheora or HAP encoder issues — separate env problems
- Any file not listed under In scope

## Steps
1. Read `src-tauri/src/args/video.rs` — find the AV1 arm and locate where `-cpu-used` is emitted. Note the field name used (`av1_speed`, `speed`, `cpu_used`, etc.) and the type.
2. Read the `ConvertOptions` Rust struct definition to confirm the field name, type, and expected range for the AV1 speed parameter.
3. Replace `-cpu-used` with `-preset` in the AV1 arm. If the field range is 0–8 (libaom), scale to 0–13 for libsvtav1: `let preset = (speed as f32 * 13.0 / 8.0).round() as u32`. If the field is already 0–13 or None, use directly.
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "cpu-used" src-tauri/src/args/video.rs` returns no matches in production code (only in comments if any).
- `grep "preset" src-tauri/src/args/video.rs` returns a match in the AV1 arm.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All lib tests pass.

## Notes
- If `-cpu-used` appears nowhere in the file (the AV1 arm may use a different variable name like `cpu_used` passed as a flag string), search for the string "cpu" in the file first.
- libsvtav1 preset 8 is the default (balanced speed/quality). If the user passes no av1_speed, the default behaviour should be reasonable — confirm the default is handled (e.g., `None` → no `-preset` flag, letting libsvtav1 use its own default).
- Do not add a new INVESTIGATION-LOG entry — this fix is a follow-up to the already-CONFIRMED `libaom-av1 absent` entry.
