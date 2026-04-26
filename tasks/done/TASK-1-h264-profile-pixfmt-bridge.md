# TASK-1: Bridge H.264 profile to required pix_fmt in the Rust arg builder

## Goal
Picking H.264 codec with `pix_fmt = yuv422p` or `yuv444p` produces a working video file regardless of the user-selected `h264_profile`. The Rust arg builder either auto-promotes the profile to `high422` / `high444` when high-bit-depth chroma is selected, or rejects the combo with a clear error before invoking ffmpeg. A test in `matrix.rs` or `full_sweep.rs` confirms the previously-failing 660 combos now succeed (or at minimum, the load-bearing subset of yuv422p × {baseline, main, high} and yuv444p × {baseline, main, high}).

## Context
Fade's full permutation sweep (`src-tauri/tests/full_sweep.rs::video_full`, run 2026-04-25) surfaced that **660 of 900 H.264 combinations fail** with `Conversion failed!`. Root cause: H.264's `baseline`, `main`, and `high` profiles only accept `yuv420p`. The high-bit-depth chroma formats (`yuv422p`, `yuv444p`) require `high422` and `high444` profiles respectively. Fade currently exposes the user's profile choice in `ConvertOptions::h264_profile` and the user's pix_fmt choice in `ConvertOptions::pix_fmt`, but the arg builder (`src-tauri/src/args/video.rs`) treats them as independent and passes both straight to ffmpeg, which rejects the combination.

This is a real production bug — the UI lets the user pick a combo that can never work, and Fade returns a generic "Conversion failed!" with no guidance.

The fix has two acceptable shapes:

**(a) Auto-promote.** When `pix_fmt = yuv422p`, force `profile:v` to `high422` regardless of the user's profile choice. When `pix_fmt = yuv444p`, force `profile:v` to `high444`. yuv420p uses the user-selected profile as-is. Pros: silent fix, user gets the file they expected. Cons: silently overrides user input, which conflicts with Fade's general "what you set is what you get" stance.

**(b) Validate and error.** Detect the impossible combo and return a clear error before spawning ffmpeg: `"H.264 profile '{profile}' does not support {pix_fmt}; pick yuv420p or use a high-bit-depth profile"`. Pros: honest failure, user can fix. Cons: shifts the grace-handling burden to the UI (see TASK-2).

Pick (a) — auto-promote. Reasoning: this is a UI-encoder mismatch, not a user-intent ambiguity. If the user selects 4:2:2 or 4:4:4 chroma, they want that chroma; the profile choice is incidental UI exposure. Auto-promotion gives them the file they wanted. TASK-2 will additionally surface the implicit promotion in the UI.

Relevant files:
- `src-tauri/src/args/video.rs` — H.264 codec arg construction. Currently reads `opts.h264_profile` and `opts.pix_fmt` independently. The bridge logic goes here.
- `src-tauri/src/lib.rs` — `ConvertOptions` struct. `h264_profile: Option<String>` ("baseline" | "main" | "high"), `pix_fmt: Option<String>` ("yuv420p" | "yuv422p" | "yuv444p"). Doc comments need updating to reflect the auto-promotion.
- `src-tauri/tests/full_sweep.rs::h264_cases` — generates the failing matrix. After the fix, all 900 H.264 cases should pass (or fail for unrelated reasons — `crf=0` may still fail validly).
- `src-tauri/tests/matrix.rs` — pre-release smoke matrix. Add at minimum two cases: `yuv422p` with profile=`high` (must succeed via promotion to high422), `yuv444p` with profile=`high` (must succeed via promotion to high444).

## In scope
- `src-tauri/src/args/video.rs` — implement profile auto-promotion based on pix_fmt
- `src-tauri/src/lib.rs` — update doc comment on `h264_profile` to note auto-promotion when pix_fmt forces it
- `src-tauri/tests/matrix.rs` — add 2 cases: H.264 yuv422p+high and H.264 yuv444p+high (no `#[ignore]`; matrix is the smoke gate)
- A new unit test in `args/video.rs` test module: assert that `build_ffmpeg_video_args` emits `profile:v high422` when `pix_fmt = "yuv422p"` and the user picked any base profile

## Out of scope
- Any UI changes to `VideoOptions.svelte` — that's TASK-2
- `full_sweep.rs` — it's the diagnostic that found the bug; verify with it locally but don't add cases (it's #[ignore] anyway)
- H.265, AV1, VP9, or any non-H.264 codec
- Any change to `ConvertOptions::h264_profile` or `::pix_fmt` field types
- Documenting the bridge logic in ARCHITECTURE.md or anywhere outside the source

## Steps
1. Read `src-tauri/src/args/video.rs` end to end. Find the H.264 codec branch — likely a `match codec.as_deref()` or similar dispatching on `opts.codec`. Note where `h264_profile` and `pix_fmt` are read.
2. Add a helper (private) `fn h264_effective_profile(profile: Option<&str>, pix_fmt: Option<&str>) -> &'static str` that returns:
   - `"high422"` if `pix_fmt == Some("yuv422p")`
   - `"high444"` if `pix_fmt == Some("yuv444p")`
   - The user's profile (default `"high"`) otherwise — passed through `match` to map the option string to a `&'static str` (`"baseline"`, `"main"`, `"high"`).
3. In the H.264 branch, replace the direct read of `opts.h264_profile` with a call to `h264_effective_profile(opts.h264_profile.as_deref(), opts.pix_fmt.as_deref())`. The `-profile:v` arg uses the returned value.
4. Update the doc comment on `ConvertOptions::h264_profile` in `src-tauri/src/lib.rs:233` to add: `// Auto-promoted to high422/high444 when pix_fmt is yuv422p/yuv444p.`
5. Add a unit test inside `args/video.rs`'s existing `#[cfg(test)] mod tests`:
   - One case: `h264_profile = Some("baseline".into())`, `pix_fmt = Some("yuv422p".into())`. Build args. Assert the `profile:v` value is `"high422"`.
   - One case: `h264_profile = Some("main".into())`, `pix_fmt = Some("yuv444p".into())`. Assert `profile:v` is `"high444"`.
   - One case: `h264_profile = Some("high".into())`, `pix_fmt = Some("yuv420p".into())`. Assert `profile:v` is `"high"` (no promotion).
6. Add 2 cases to `src-tauri/tests/matrix.rs::video_matrix` (the `#[ignore]` video matrix function): `mp4_h264_yuv422p_high` and `mp4_h264_yuv444p_high`. Use codec=h264, crf=28, preset=ultrafast, the relevant pix_fmt, and h264_profile=high.
7. Build: `cargo build --manifest-path src-tauri/Cargo.toml`. Must succeed.
8. Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`. Unit tests must pass.
9. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test matrix video_matrix -- --include-ignored --nocapture`. The 2 new cases must produce non-empty output files.
10. Local-only verification (do NOT add to CI or check in): run a slice of `full_sweep::video_full::h264` to confirm the previously-failing yuv422p/yuv444p combos now pass.
11. `cargo fmt` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0; new unit tests in `args::video` pass.
- `cargo test --manifest-path src-tauri/Cargo.toml --test matrix video_matrix -- --include-ignored --nocapture` exits 0 with the 2 new H.264 cases producing non-empty output files (each ≥ 1000 bytes).
- `grep "h264_effective_profile" src-tauri/src/args/video.rs` returns 2 lines: the helper definition and one call site inside the H.264 branch.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- The auto-promotion does NOT silently rewrite the user's `ConvertOptions::h264_profile` — only the emitted ffmpeg arg differs. The frontend's stored value is unchanged. This matters if the UI reads back the chosen profile.
- ffmpeg accepts `-profile:v high422` and `-profile:v high444` for libx264; both have been valid for years. No version gate is needed.
- If the existing arg builder constructs the H.264 args via a string concatenation rather than a Vec push, the helper still applies — just call it before the concat.
