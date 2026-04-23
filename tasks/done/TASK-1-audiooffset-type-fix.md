# TASK-1: Fix AudioOffset offset_ms type: i64 → i32

## Goal
`src-tauri/src/operations/audio_offset.rs` declares `offset_ms` as `i64`; after this task it is `i32`, and the ts-rs generated `OperationPayload.ts` reflects `number` instead of `bigint` for the `audio_offset` variant.

## Context
The `AudioOffset` struct in `audio_offset.rs` holds an `offset_ms: i64` field. ts-rs maps Rust `i64` to TypeScript `bigint`, so `OperationPayload.ts` currently reads:

```ts
{ "type": "audio_offset", input_path: string, offset_ms: bigint, output_path: string, }
```

`bigint` is not interchangeable with `number` in TypeScript. Any future frontend caller that constructs this variant and passes a plain `number` for `offset_ms` will have a type mismatch — the browser will accept it at runtime (JS is untyped), but the ts-rs codegen contract is broken. The fix: change the field to `i32`. Audio offset values are in milliseconds; `i32` covers ±2,147,483,647 ms (±~596 hours), which is sufficient for any practical use.

**No frontend callers currently construct `audio_offset` payloads** — a grep of `src/` (excluding `src/lib/types/generated/`) for both `offset_ms` and `audio_offset` returns no results. The operation is backend-only with no UI wiring yet. There is nothing to update on the frontend side beyond the generated type.

**ts-rs regeneration:** ts-rs generates TypeScript bindings as part of the test suite (via `#[test]` functions in the Rust code, or via a build script). Running `cargo test --manifest-path src-tauri/Cargo.toml` will regenerate the bindings into `src/lib/types/generated/`. After regeneration, verify the file on disk changed.

**Relevant files:**
- `src-tauri/src/lib.rs` — the `OperationPayload` enum at line ~1098 has an `AudioOffset` variant with `offset_ms: i64`; **this is the field ts-rs reads to generate the TypeScript type**
- `src-tauri/src/operations/audio_offset.rs` — the `run()` function takes `offset_ms: i64` as a parameter (line 23); must match the enum variant
- `src/lib/types/generated/OperationPayload.ts` — generated file, contains the `audio_offset` variant with `offset_ms: bigint`

## In scope
- `src-tauri/src/lib.rs` — change `offset_ms: i64` to `offset_ms: i32` in the `AudioOffset` enum variant (~line 1098); also update the destructuring/call site at ~line 1465 where `*offset_ms` is passed to `audio_offset::run()`
- `src-tauri/src/operations/audio_offset.rs` — change the `offset_ms: i64` parameter to `offset_ms: i32` (~line 23)
- `src/lib/types/generated/OperationPayload.ts` — updated automatically by ts-rs test run; verify it changed

## Out of scope
- Frontend `.svelte` or `.js` files — no callers exist; nothing to update
- All other Rust files not listed above

## Steps
1. Read `src-tauri/src/lib.rs` around lines 1090–1110 to find the `AudioOffset` variant in `OperationPayload` and confirm the `offset_ms: i64` field location.
2. Read `src-tauri/src/operations/audio_offset.rs` in full.
3. In `src-tauri/src/lib.rs`: change `offset_ms: i64` to `offset_ms: i32` in the `AudioOffset` enum variant.
4. In `src-tauri/src/operations/audio_offset.rs`: change the `offset_ms: i64` parameter to `offset_ms: i32`.
5. Search `src-tauri/src/lib.rs` for the call site where `audio_offset::run()` is called with `*offset_ms` (around line 1465) and confirm it still compiles — no cast needed since both sides are now `i32`.
6. Run `cargo test --manifest-path src-tauri/Cargo.toml` to compile, run tests, and trigger ts-rs regeneration.
7. Read `src/lib/types/generated/OperationPayload.ts` and confirm the `audio_offset` variant now reads `offset_ms: number` instead of `offset_ms: bigint`.
8. Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
9. Run `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.

## Success signal
- `cargo test` exits 0.
- `src/lib/types/generated/OperationPayload.ts` contains `offset_ms: number` (not `bigint`) in the `audio_offset` variant.
- `cargo clippy -D warnings` exits 0.
- `cargo fmt --check` exits 0.

## Notes
The `as f64` casts in the function body are safe with `i32` — no logic changes needed. The only edit in `audio_offset.rs` is the field type declaration.
