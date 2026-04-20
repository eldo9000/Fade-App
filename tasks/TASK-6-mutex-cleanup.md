# TASK-6 — Mutex `.unwrap()` cleanup

**Size:** Small. One session, ~30 min.
**Prerequisites:** None. Fresh agent, no context needed.
**Dependencies:** Independent of all other TASK files.

---

## Project context

**Fade** is a desktop media converter (images, video, audio, and data formats) built with **Tauri 2 + Svelte 5 + Rust**. Backend uses FFmpeg and ImageMagick as external CLI tools. The repo is at `/Users/eldo/Downloads/Github/Fade-App`.

Tech:
- Rust edition 2021, MSRV 1.77
- Workspace in `src-tauri/`
- Clippy workspace lints: `all = warn`, `unwrap_used = warn`, `await_holding_lock = deny`
- CI runs `cargo clippy --all-targets -- -D warnings` (so `warn` becomes hard fail)

## The problem

The Rust backend holds two shared mutable maps inside `AppState`:

```rust
// src-tauri/src/lib.rs:37-40
pub struct AppState {
    pub processes: Arc<Mutex<HashMap<String, Child>>>,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}
```

Ten call sites use `.lock().unwrap()`, which:
1. Triggers `clippy::unwrap_used` (a lint this project treats as warn + `-D warnings` ⇒ hard fail)
2. Panics with a useless message if the mutex is poisoned
3. Contradicts the stated lint policy

Mutex poisoning only occurs if a thread panics while holding the lock. For these short HashMap critical sections it's essentially impossible, but the codebase should still be lint-clean and use informative panics.

## Scope — exactly 10 call sites

**In scope:**
- `src-tauri/src/lib.rs` — 6 sites at lines **544, 663, 720, 727, 1028, 1515**
- `src-tauri/src/operations/merge.rs` — 2 sites at lines **220, 259**
- `src-tauri/src/operations/mod.rs` — 2 sites at lines **148, 187**

All are of the shape:
```rust
let mut map = state.cancellations.lock().unwrap();
// or
let mut map = processes.lock().unwrap();
// or
let map = state.cancellations.lock().unwrap();
```

**Out of scope — do NOT touch:**
- Any other `.unwrap()` or `.expect()` in the codebase
- Any `unwrap()` inside `#[cfg(test)]` or `#[test]` functions (tests are allowed to unwrap)
- Any business logic, any operation handler bodies, any data flow

## Prescribed fix

Replace each site with `.expect("<descriptive message>")`. The message should name which mutex poisoned:

```rust
// Before
let mut map = state.cancellations.lock().unwrap();

// After
let mut map = state.cancellations.lock().expect("cancellations mutex poisoned");
```

For the `processes` mutex:
```rust
let mut map = processes.lock().expect("processes mutex poisoned");
```

Do not introduce helper functions, trait implementations, or parking_lot. The fix is purely a textual substitution at the 10 identified sites. Keep the rest of the line unchanged (binding name, mutability, whether it's `let mut` or `let`).

## Procedure

1. **Verify the baseline.** From the repo root:
   ```bash
   cd /Users/eldo/Downloads/Github/Fade-App
   cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -20
   ```
   Note the current warning/error state. If clippy already fails, do NOT assume your cleanup fixes whatever else is broken.

2. **Apply the 10 edits.** Use the Edit tool with exact line matches. Verify each edit by spot-reading the line after.

3. **Re-run clippy.**
   ```bash
   cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
   ```
   The 10 `unwrap_used` warnings should be gone. No new errors.

4. **Run backend unit tests.**
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml --lib
   ```
   All tests must pass. Tests using `.unwrap()` inside `#[test]` functions are untouched and must still pass.

5. **Run formatter check.**
   ```bash
   cargo fmt --manifest-path src-tauri/Cargo.toml --check
   ```
   Must pass. If it fails, run `cargo fmt --manifest-path src-tauri/Cargo.toml` and re-verify.

6. **Commit.**
   ```bash
   git add -A
   git commit -m "refactor(rust): replace mutex unwrap with expect at 10 sites

   Silences clippy::unwrap_used warnings. Mutex poisoning now produces
   an informative panic naming the affected state map."
   ```

7. **Add the git note (required by project protocol).**
   ```bash
   git notes add -m "app: fade
   state: stable
   context: cleaned all 10 mutex .lock().unwrap() sites to .expect(<name>)
   deferred: none
   fragile: none
   ci: unknown" HEAD
   ```

8. **Push.**
   ```bash
   git push origin main
   git push origin refs/notes/commits
   ```

## Verification — definition of done

All of the following must be true before you report green:

- [ ] All 10 listed call sites now use `.expect(...)` with a descriptive message naming the mutex
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --lib` all tests pass
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check` exits 0
- [ ] No files outside `src-tauri/src/lib.rs`, `src-tauri/src/operations/merge.rs`, `src-tauri/src/operations/mod.rs` were modified
- [ ] Commit is on `main`, git note attached, both pushed

## Gotchas

- **Line numbers may shift during the task.** Use the content around the line to verify, not just the number. If you edit line 544 first, line 663 may now be 663 (no-op insertion), but if you accidentally added/removed a line it would shift. Prefer `Edit` with enough surrounding context to make `old_string` unique.

- **Do not use `replace_all`.** Each site has its own surrounding context (different binding names, `mut` vs no-`mut`, different mutex fields). Apply each edit deliberately.

- **CI is the only truth.** Local `cargo clippy` is a hypothesis; CI confirms. After pushing, check `gh run list --limit 1` to confirm the run went green. If red, investigate — do not mark this task done.

- **The `-D warnings` flag turns every warning into an error.** If clippy still flags something after your changes, the warning is something else — read the error, diagnose, and either fix in scope if trivial or halt and report.

## On failure

If clippy still fails after the 10 edits, do NOT widen scope. Report what's failing with the exact clippy output. The user will decide whether to expand scope or file a follow-up task.

If a test fails that was previously passing, check whether you accidentally modified test-module `unwrap()`s. Tests should be untouched.
