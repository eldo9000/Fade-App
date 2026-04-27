# TASK-7: Defence-in-depth for duckdb SQL string interpolation

## Goal
The duckdb COPY-statement construction at `convert/data.rs:212-215` no longer relies solely on single-quote escaping. Either: (a) the input/output paths are validated against a strict allowlist before interpolation, or (b) the SQL is constructed via a duckdb parameter-binding mechanism that doesn't require ad-hoc string escaping. Combined with TASK-2's path validation upstream, the duckdb pipeline has two independent layers of defence.

## Context
Current code at `convert/data.rs:211-215`:

```rust
// duckdb escapes single quotes by doubling them in SQL string literals.
let sql_in = input_path.replace('\'', "''");
let sql_out = output_path.replace('\'', "''");
let sql =
    format!("COPY (SELECT * FROM read_parquet('{sql_in}')) TO '{sql_out}' {format_clause};");
```

Single-quote escaping by doubling is correct per duckdb's SQL dialect docs for the basic case. The risk is layered:

1. **Backslash interpretation** — duckdb's SQL string literal can interpret escape sequences in some dialect modes. If `'\\'` is seen as a literal backslash in `STANDARD_CONFORMING_STRINGS=off`-equivalent mode, then `\'` could re-introduce a quote.
2. **Defence-in-depth** — even if single-quote escape is correct today, it's a single line of defence on the riskiest entry point in the data pipeline. A change in duckdb's parser, or a future schema where the `format_clause` becomes user-controlled, would re-open the gap.
3. **Upstream gap** — `data.rs::convert()` is called from `convert_file` (which TASK-2 will harden) and `run_operation`. Until TASK-2 lands, the input/output paths reaching this site can be largely arbitrary.

The cleanest defence is: don't construct SQL by string concatenation when the input is a path. duckdb's CLI binary doesn't support parameter binding directly, but the call already shells out — the escape logic could be replaced by:

- **Option A (preferred):** Validate that `input_path` and `output_path` contain only path-safe characters before constructing SQL (no `'`, no `\`, no `;`, no newline). Reject up-front with a clear error.
- **Option B:** Use the duckdb library directly via the `duckdb` crate (https://crates.io/crates/duckdb) which supports parameter binding. Larger change.

Option A pairs well with TASK-2 as a complementary belt-and-braces and is far smaller in scope.

Relevant files:
- `src-tauri/src/convert/data.rs:200-240` — the parquet→csv/json/tsv conversion path
- `src-tauri/src/convert/data.rs:140` — `dump_sqlite_table` (positive prior art — uses table-name escaping correctly)
- `src-tauri/src/lib.rs:439-535` — existing validators (reference)

## In scope
- Add a `validate_path_safe_for_sql(path: &str) -> Result<(), String>` helper to `convert/data.rs` (or to `lib.rs` if generally useful).
- Reject any path containing `'`, `\`, `;`, `\n`, `\r`, or null bytes. These are SQL-meta or line-terminator characters that have no business in a parquet/sqlite path.
- Call the validator at the top of the parquet conversion function (before the `replace('\'', "''")` lines), and at any other duckdb-CLI-string-interpolation site in the file.
- Keep the existing single-quote escape as a third layer (defence in depth).
- Tests:
  1. Valid path passes validation
  2. Path with `'` rejected
  3. Path with `\` rejected
  4. Path with `;` rejected
  5. Path with newline rejected

## Out of scope
- Migrating to the `duckdb` Rust crate (Option B). Large diff; defer until a separate refactor.
- Removing the existing `replace('\'', "''")` lines (they're now belt-and-braces; keep them).
- Touching `dump_sqlite_table` (line 140) — it's already correct (escapes table names properly).
- Path validation at the IPC entry (TASK-2).

## Steps
1. Read `convert/data.rs:140-280` for full context of the data pipeline.
2. Implement the validator at the top of the file (or in `lib.rs` as `pub(crate)`):
   ```rust
   fn validate_path_safe_for_sql(path: &str) -> Result<(), String> {
       const REJECTED: &[char] = &['\'', '\\', ';', '\n', '\r', '\0'];
       if let Some(c) = path.chars().find(|c| REJECTED.contains(c)) {
           return Err(format!(
               "Path contains character '{}' which is not safe for SQL interpolation: {}",
               c.escape_default(), path
           ));
       }
       Ok(())
   }
   ```
3. At the top of the parquet conversion function (before line 211), call:
   ```rust
   validate_path_safe_for_sql(input_path)?;
   validate_path_safe_for_sql(output_path)?;
   ```
4. Search the file for other `Command::new("duckdb")` or duckdb-related string-interpolation sites. Apply the same validation. Likely candidates: anywhere `format!` builds a SQL string from path data.
5. Add the five tests in the existing `#[cfg(test)] mod tests`.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::data`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "validate_path_safe_for_sql" src-tauri/src/convert/data.rs` returns 3+ matches (definition + 2 calls).
- The five tests pass.
- All existing data tests continue to pass.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- This task assumes TASK-2 (convert_file path validation) is either landed or in flight. The two are complementary: TASK-2 prevents most malformed paths from reaching here at all; TASK-7 hardens the specific SQL-interpolation site as defence-in-depth.
- The character set `' \ ; \n \r \0` is conservative. None are valid in any UNIX or Windows file path that this app would legitimately handle. The error message names the character so users understand what to fix.
- Do not remove the existing `replace('\'', "''")`. After this task, the pipeline is: validate (rejects most attacks) → escape (catches what slipped through). Two layers, both cheap.
- The static analyzer (Semgrep) cannot detect the duckdb dialect-specific escape nuance — it would only flag the `format!`-into-SQL pattern generically. Concern-based caught it because it understood the sink.
