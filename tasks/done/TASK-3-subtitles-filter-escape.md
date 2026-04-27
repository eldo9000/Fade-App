# TASK-3: Escape input path for ffmpeg `subtitles=` filter

## Goal
The `subtitles=` filter at `args/video.rs:141` correctly escapes the input path per ffmpeg filter-graph rules. Paths containing `:` (Windows drive letters, time-code-style components), apostrophes, commas, and brackets no longer break the filter graph or alter its meaning.

## Context
The current code at `src-tauri/src/args/video.rs:140-142`:

```rust
if opts.output_format == "mkv" && opts.mkv_subtitle.as_deref() == Some("burn") {
    filters.push(format!("subtitles={}", input));
}
```

`input` is the user-supplied input path. It's spliced raw into ffmpeg's `-vf` argument string. ffmpeg parses `-vf` arguments as a colon-separated key=value list per filter, with a separate level of escaping inside each value.

Real-world breakage:
- **Windows drive letters:** `C:\Users\me\video.mp4` — the `:` after `C` terminates the filter argument early; ffmpeg parses `\Users\me\video.mp4` as a separate filter-key. Hard fail with confusing error.
- **Apostrophes / single quotes:** `My 'Wedding' Video.mp4` — alters the ffmpeg quoting state.
- **Commas:** ffmpeg uses `,` as the filter separator inside `-vf`. A comma in the input path splits into a second filter.
- **Square brackets:** `[A]` denotes a filter input/output stream label.

The ffmpeg filter-escaping rules require:
1. **Inside filter argument** (this layer): backslash-escape `\`, `:`, `'`.
2. **At the filter description level** (outer layer): single-quote-wrap the whole argument; or backslash-escape `,`, `[`, `]`, `;`.

The right fix here is to apply both layers of escaping, then single-quote-wrap the result.

The existing test at `args/video.rs` line ~882 covers only `subtitles=in.mkv` — the trivial case. New cases needed.

Relevant files:
- `src-tauri/src/args/video.rs:140-142` — the filter line
- `src-tauri/src/args/video.rs` `#[cfg(test)] mod tests` — add new tests
- ffmpeg source: `libavfilter/graphparser.c` — reference for escaping rules

## In scope
- Add a private helper `fn escape_for_subtitles_filter(path: &str) -> String` to `args/video.rs`.
- Replace the current `format!("subtitles={}", input)` with `format!("subtitles={}", escape_for_subtitles_filter(input))`.
- Tests:
  1. `subtitles_filter_escapes_colon_in_windows_path` (input: `C:\Users\me\v.mkv`)
  2. `subtitles_filter_escapes_apostrophe` (input: `/u/me/My's Video.mkv`)
  3. `subtitles_filter_escapes_comma` (input: `/u/me/A,B.mkv`)
  4. `subtitles_filter_escapes_brackets` (input: `/u/me/[A]Video.mkv`)
  5. `subtitles_filter_simple_path_unchanged_or_safely_quoted` (regression: existing trivial-case behaviour preserved or now wrapped in safe quotes)

## Out of scope
- Other filter strings in `args/video.rs` (`tpad`, `format`, etc.) — they use numeric or known-safe-enum values, not user paths. Audit them in a follow-up if desired.
- The `args/image.rs` filter chain — image module doesn't accept user-content interpolation in filters today.
- Any change to `mkv_subtitle` handling other than `Some("burn")`.
- Path validation at the IPC layer (TASK-2 handles that).

## Steps
1. Read `args/video.rs:120-160` for full context of the filter chain. Note that filters are joined with `,` at line 144 (`filters.join(",")`) — the outer-layer comma escaping is therefore needed.
2. Implement the escape helper:
   ```rust
   /// Escape a value for use inside an ffmpeg filter argument.
   /// Applies inner-layer escaping for `\`, `:`, `'` then single-quote-wraps
   /// the whole value (outer-layer protection against `,`, `[`, `]`, `;`).
   fn escape_for_subtitles_filter(path: &str) -> String {
       let inner: String = path
           .chars()
           .flat_map(|c| match c {
               '\\' | ':' | '\'' => vec!['\\', c],
               _ => vec![c],
           })
           .collect();
       format!("'{}'", inner)
   }
   ```
3. Replace line 141 with:
   ```rust
   filters.push(format!("subtitles={}", escape_for_subtitles_filter(input)));
   ```
4. Add the five tests inside the existing `#[cfg(test)] mod tests` block. Build a `ConvertOptions` with `output_format = "mkv"`, `mkv_subtitle = Some("burn")`, and the relevant `input` path; assert the resulting `-vf` arg list contains the correctly-escaped subtitles filter.
5. Update or remove the prior trivial-case test at line ~882 if its expectation no longer matches (the new behaviour wraps even simple paths in single quotes — that's correct). Adjust assertion to expect `subtitles='in.mkv'`.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "escape_for_subtitles_filter" src-tauri/src/args/video.rs` returns 2+ matches (definition + call site).
- The five new tests pass.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video` exits 0.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- ffmpeg's filter escaping is two-layer because filter graphs nest. The outer `-vf` layer joins filters with `,`; the inner argument layer uses `:` for key=value pairs and `'` for quoting. This helper handles both layers in one shot by escaping the inner specials and then single-quote-wrapping.
- Single-quote-wrapping every input is safe even for trivial paths — ffmpeg accepts `subtitles='in.mkv'` identically to `subtitles=in.mkv`. Don't over-optimise.
- Static analysis missed this finding because it scanned for `Command::arg(format!(...))` shell-arg patterns. The injection is one level deeper — `format!` into a filter-string then passed as a single arg.
- Threat model: this is correctness-first (Windows paths break today), security-second (input path is constrained by file-picker, but TASK-2's `validate_no_traversal` doesn't sanitise punctuation).
