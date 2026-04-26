# TASK-4: Resolve the missing 1px.jpg fixture in lib.rs integration test

## Goal
The ignored test `lib.rs::tests::integration_image_convert_jpeg_to_png` either passes when run with `--include-ignored`, or has been removed entirely. No reference to a non-existent `src/tests/fixtures/1px.jpg` remains in the source tree.

## Context
During TASK-8 verification of the &Window decoupling refactor (committed as `c687310` on 2026-04-25), running `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored` surfaced one failure: `lib.rs::tests::integration_image_convert_jpeg_to_png` reads a fixture path `src/tests/fixtures/1px.jpg` that does not exist in the repository.

Verified facts (from OBSERVER-STATE Run 11):
- The test predates the &Window refactor — present since the initial source split commit `1b0a0ef`.
- The CI workflow (`.github/workflows/ci.yml`) runs `cargo test --lib` and `cargo test --test conversions -- --include-ignored`. The first does not include integration tests in `tests/`; the second targets a specific test file (`conversions.rs`). Neither path runs the broken `lib.rs` test, which is why it has lain dormant.
- The `src-tauri/src/tests/fixtures/` directory may not exist at all; the path is structurally suspicious because `tests/` lives at `src-tauri/tests/`, not `src-tauri/src/tests/`.

Two acceptable resolutions:

**(a) Restore the fixture.** Locate (or generate) a 1×1-pixel JPEG, write it to `src-tauri/src/tests/fixtures/1px.jpg`, and verify the test passes. A 1×1 JPEG can be generated with `magick -size 1x1 xc:red 1px.jpg` and is ~600 bytes.

**(b) Remove the test.** If the test is structurally redundant with what `src-tauri/tests/conversions.rs::image_png_to_webp` already covers (a smoke conversion through the real backend), delete the broken test entirely. Update any related test scaffolding it touches.

Pick (a) — restore the fixture. Reasoning: the test was written for a reason; we don't know what coverage it provided that conversions.rs doesn't. Cheaper to put back the missing 600-byte file than to remove a test and risk losing coverage we didn't audit. If the test still fails after the fixture is in place, *then* fall back to (b) and document why.

Relevant files:
- `src-tauri/src/lib.rs` — somewhere around line 3170 (per OBSERVER-STATE Run 11), in a `#[cfg(test)] mod tests` or similar. Contains the test function. The exact line should be confirmed by grep.
- `src-tauri/src/tests/fixtures/` — the directory the test expects. May or may not exist.
- `src-tauri/tests/conversions.rs::image_png_to_webp` — the parallel coverage in the integration test layer; useful to compare what each test asserts.

## In scope
- `src-tauri/src/tests/fixtures/1px.jpg` — generate or restore the fixture if (a) is taken
- If (a) fails after the fixture is placed: `src-tauri/src/lib.rs` test deletion is permitted, but only after fixture-restore is confirmed not to fix the test
- Any minor adjustment needed to the test's assertion if the test is structurally fine but has a stale assertion (e.g., asserting an exact byte count that's drifted)

## Out of scope
- Any change to non-test code in `lib.rs`
- Any change to `src-tauri/tests/*.rs` integration tests
- Any change to CI workflow — `--include-ignored` lib tests are intentionally not gated; that's a separate decision
- Adding new test cases beyond restoring this one to working condition
- Refactoring how `src-tauri/src/tests/fixtures/` is structured

## Steps
1. Grep for `integration_image_convert_jpeg_to_png` in `src-tauri/src/lib.rs`. Record: line number, full test body, what fixture path it reads, what it asserts.
2. Check whether `src-tauri/src/tests/fixtures/` exists. `ls src-tauri/src/tests/fixtures/ 2>&1`.
3. If the directory doesn't exist, create it: `mkdir -p src-tauri/src/tests/fixtures`.
4. Generate the 1×1 JPEG fixture: `magick -size 1x1 xc:red src-tauri/src/tests/fixtures/1px.jpg`. Verify file exists and is non-empty (should be ~600 bytes).
5. Run the test: `cargo test --manifest-path src-tauri/Cargo.toml --lib integration_image_convert_jpeg_to_png -- --include-ignored --nocapture`. Capture output.
6. **If the test passes:** done. Skip to step 8.
7. **If the test fails for a different reason (assertion drift, output path issue, etc.):** read the failure output. If the fix is trivial (a constant changed; use a matcher instead of exact bytes), apply it. If the failure is opaque or requires deeper investigation, halt and surface to user — do not delete the test without buy-in.
8. Confirm `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored` exits 0 with the test now passing (or still passing for the rest of the suite if step 7 led to a halt).
9. `cargo fmt` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `ls src-tauri/src/tests/fixtures/1px.jpg` shows the file exists, ~500–700 bytes.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib integration_image_convert_jpeg_to_png -- --include-ignored --nocapture` exits 0 with the test reported as `ok`.
- `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored` exits 0 (no other tests broken by the fixture introduction).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- A 1×1 red JPEG is a fine fixture; the test almost certainly only checks that the conversion produces a non-empty PNG, not pixel-level content.
- If the test references an OUTPUT path inside `src/tests/fixtures/` (writing the result there), that's a separate concern — the test should write to a tempdir, not the source tree. If you spot this, fix it as part of step 7 since it's in the same body. Don't expand beyond that.
- Don't add the fixture to `.gitignore`. A test fixture is an intentional checked-in artifact.
- If the fixture-restore approach fails AND the test turns out to be redundant with `conversions.rs::image_png_to_webp`, deleting the broken test is acceptable per the (b) fallback. Document the redundancy in the commit message in that case.
