# Contributing to Fade

Thanks for your interest in contributing. Fade is a Tauri v2 macOS media
converter built on Svelte + Rust. This doc covers the essentials for getting
set up and landing changes.

## Required tools

- Node 20+
- Rust stable (via `rustup`)
- ffmpeg (`brew install ffmpeg`)
- imagemagick (`brew install imagemagick`)

## Local dev setup

```sh
git clone https://github.com/eldo9000/Fade-App.git
cd Fade-App
npm install
npm run tauri dev
```

## Running tests

```sh
# Frontend (Vitest)
npm test

# Rust backend
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Please run both before opening a PR.

## Code style

- Rust: `cargo fmt` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Frontend: Prettier defaults if configured; match surrounding style otherwise
- Keep changes focused; prefer small PRs over sweeping refactors

## Commit messages

Follow the existing conventional-commit-ish style visible in `git log`:

- `feat: ...` for new user-visible features
- `fix: ...` for bug fixes
- `refactor: ...` for internal restructuring
- `chore: ...` for tooling, deps, release plumbing
- `docs: ...` for documentation
- `ci: ...` for workflow changes

Scopes (e.g. `feat(ui): ...`, `fix(backend): ...`) are welcome but not required.

## PR flow

1. Fork the repo and create a feature branch off `main`.
2. Make your changes; keep commits tidy.
3. Ensure tests and linters pass locally.
4. Open a PR against `main`. Fill out the PR template (summary, why, test plan).
5. Address review feedback with follow-up commits; we'll squash on merge if
   appropriate.

## Related docs

- Release pipeline: [`docs/RELEASE.md`](docs/RELEASE.md)
- Changelog: [`CHANGELOG.md`](CHANGELOG.md)

## Questions

Open a discussion at https://github.com/eldo9000/Fade-App/discussions.
