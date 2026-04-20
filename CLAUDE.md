# Fade — Claude Context

## Shared Standards

- **Engineering standards:** `~/Downloads/Github/Business-OS/standards/ENGINEERING.md` — session protocol, investigation logs, commit conventions, Known Patterns & Gotchas. Read at the start of any implementation session.
- **Observer Loop:** `~/Downloads/Github/Business-OS/standards/OBSERVER-LOOP.md` — the self-regulating pattern this project uses. Explains git notes, SESSION-STATUS, INVESTIGATION-LOG, OBSERVER-STATE, and the two skills below.
- **Design language & shared patterns:** `~/Downloads/Github/Libre-Apps/CLAUDE.md` — design tokens, Tauri 2 patterns, Svelte 5 patterns, cross-app conventions. Read before any UI or frontend work.

---

## Skills reference

- **`/observe-start`** — session-start briefing. Run this at the start of any work session. Auto-detects the current repo and outputs a single recommendation with a suggested next invocation. Stops — does not start any work.
- **`/observe-sync`** — observer agent. Run this after significant work to refresh `OBSERVER-STATE.md`. Synthesizes all artifacts (git notes, SESSION-STATUS, INVESTIGATION-LOG, Known Patterns, CI) into a coherent model of project health.

---

## What This Is

Fade is a media converter — convert, resize, and process images and video without leaving the desktop. Built with Tauri 2 + Svelte 5 + Rust. Ships with LibreWin OS.

---

## How to Run

```bash
npm install
npm run tauri dev
```

Dev server: port **1427**. Requirements: Rust toolchain, Node 22+, `ffmpeg` and `imagemagick` in PATH.

On macOS: `brew install ffmpeg imagemagick` before running.

---

## Tech Stack

- **Tauri 2** — window management, IPC, file system
- **Svelte 5** — frontend (runes only: `$state`, `$derived`, `$effect`, `$props`, `$bindable`)
- **Vite 8** + **Tailwind CSS 4** — build + styling
- **Rust (edition 2021)** — backend
- **FFmpeg + ImageMagick** — external CLI tools for conversion

---

## Key Docs

- `SPECS.md` — full product specification
- `ARCHITECTURE.md` — technical architecture decisions
- `INVESTIGATION-LOG.md` — active debugging arc (append-only)
- `KNOWN-BUG-CLASSES.md` — permanent known failure patterns

---

## Commit protocol — git notes required

After every `git commit` in this repo, attach a structured git note:

```bash
git notes add -m "app: fade
state: active | stable | fragile
context: <what you were in the middle of — one sentence>
deferred: <what you deliberately left incomplete — one sentence, or 'none'>
fragile: <what's nearby that could break — one sentence, or 'none'>
ci: green | red | unknown" HEAD
```

Then push the note:
```bash
git push origin refs/notes/commits
```

**This is not optional.** The observer agent reads these notes to maintain project awareness. A commit without a note is invisible to the observer.

If you forget on a prior commit: `git notes add <sha>` works retroactively.

---

## QC rules — enforced, no exceptions

**CI is the only truth.** A gate may not be marked PASSES in `SESSION-STATUS.md` until a CI run confirms it. Local results are hypotheses. CI results are facts.

**Read the pipeline before inserting into it.** Before adding code to an existing pipeline, read enough surrounding code to state what runs before this step, what runs after it, and what state it depends on.

**Diagnose the failing environment first.** When a test fails in CI, diagnose the CI failure before investigating local behavior. The simplest broken thing is fixed first. Do not investigate a deeper failure while a shallower one exists.

---

## Known Patterns & Gotchas

**Append immediately when you discover something non-obvious** — don't wait until session end. If interrupted, the finding must already be here. This section is permanent knowledge; it never gets archived. For temporary debugging findings, use `INVESTIGATION-LOG.md` instead.

**FFmpeg and ImageMagick must be in PATH — Tauri does not bundle them.** On macOS dev, set PATH explicitly in the shell that runs `tauri dev`. A missing binary produces a cryptic "command not found" from inside the Rust process, not a Tauri error.

**`validate_output_name()` before any CLI arg interpolation.** Any Tauri command that interpolates a user-supplied name into a CLI arg string must call `validate_output_name()` first. The validator rejects anything that isn't ASCII alphanumeric, `-`, or `_`.
