# Fade — Release & Auto-Updater

Fade uses Tauri v2's built-in updater plugin. Distribution is currently
**unsigned** (no Apple Developer ID). First-run on macOS requires the user
to run `xattr -cr /Applications/Fade.app` or for our build/release script
to strip the quarantine attribute before shipping. This is acceptable
because Fade is internal-only for now.

The updater itself is still cryptographically signed — just using Tauri's
own minisign keypair, not Apple notarization. The app will refuse to
install an update whose signature doesn't verify against the embedded
public key.

---

## How it works

1. On startup (after a 2-second delay so it doesn't block launch),
   the frontend calls `check()` from `@tauri-apps/plugin-updater`.
2. The plugin fetches `latest.json` from the configured endpoint:
   `https://github.com/eldo9000/Fade-App/releases/latest/download/latest.json`
3. If `latest.json` advertises a version newer than the current app,
   a banner appears in the window titlebar area.
4. On "Install & restart", the plugin downloads the `.app.tar.gz`
   bundle, verifies its `.sig` against the embedded public key,
   replaces `Fade.app` in place, and relaunches via
   `@tauri-apps/plugin-process`.

The public key is embedded in `src-tauri/tauri.conf.json`
under `plugins.updater.pubkey`. The matching private key is **not**
in the repo — see below.

---

## Keypair

Generated with `npx tauri signer generate -w ~/.tauri/fade.key`
(no password — this is an internal project; keep it simple).

- **Private key**: `~/.tauri/fade.key` — on the release maintainer's
  local machine. Never commit this file.
- **Public key**: `~/.tauri/fade.key.pub` — also committed inline
  into `tauri.conf.json` as the `pubkey` field.

To re-generate (e.g. key is lost or compromised — this will
invalidate all previously-signed updates, so every user will have
to manually reinstall once):

```sh
npx tauri signer generate -w ~/.tauri/fade.key
```

Then replace the `pubkey` field in `src-tauri/tauri.conf.json`
with the contents of `~/.tauri/fade.key.pub` and update the GitHub
Actions secret (see below).

---

## GitHub Actions secrets (for Block B — release CI)

The release workflow needs the private key to sign the built
`.app.tar.gz` bundle. Add these repo secrets in
**Settings → Secrets and variables → Actions**:

| Secret name                         | Value                                          |
| ----------------------------------- | ---------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`         | Contents of `~/.tauri/fade.key` (the file)     |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`| Empty string (we generated without a password) |

`tauri build` will auto-pick these up from the environment and
produce `Fade.app.tar.gz` + `Fade.app.tar.gz.sig` alongside the DMG.

---

## Expected release artifacts (what Block B must produce)

Each GitHub Release must attach:

1. **`Fade.app.tar.gz`** — the updater bundle (gzip-compressed tar
   of the `.app` directory). Produced automatically by
   `tauri build` because `bundle.createUpdaterArtifacts` is `true`
   in `tauri.conf.json`.
2. **`Fade.app.tar.gz.sig`** — minisign signature. Also produced
   automatically when `TAURI_SIGNING_PRIVATE_KEY` is set in the
   build environment.
3. **`latest.json`** — the updater manifest (see below).
4. **`Fade_<version>_aarch64.dmg`** — DMG for fresh installs
   (not used by the updater, but needed for first-time downloads).

---

## `latest.json` format

The updater endpoint is pinned to
`https://github.com/eldo9000/Fade-App/releases/latest/download/latest.json`.
GitHub serves the `latest` alias automatically, so CI doesn't need
to rewrite anything — just attach `latest.json` to each release.

Example:

```json
{
  "version": "0.2.0",
  "notes": "Bug fixes and new per-format controls.",
  "pub_date": "2026-04-18T12:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<contents of Fade.app.tar.gz.sig>",
      "url": "https://github.com/eldo9000/Fade-App/releases/download/v0.2.0/Fade.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "<contents of Fade.app.tar.gz.sig>",
      "url": "https://github.com/eldo9000/Fade-App/releases/download/v0.2.0/Fade.app.tar.gz"
    }
  }
}
```

Notes:

- `version` must match the `version` field in `tauri.conf.json`
  and `package.json` for that release.
- `signature` is the literal *contents* of the `.sig` file (not a
  URL, not base64-of-the-file — just the file text).
- `url` must point at the `.tar.gz` asset (not the DMG).
- If we only build for Apple Silicon, omit `darwin-x86_64` — the
  updater will just skip Intel machines cleanly. Right now we are
  macOS-only (no `windows` / `linux` entries).

---

## Release checklist

1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`,
   and `package.json` (all three must match).
2. Commit, tag `v<version>`, push.
3. CI (Block B, TBD) runs `npm run tauri build`, produces artifacts,
   creates a GitHub Release, attaches DMG + `.app.tar.gz` + `.sig`
   + generated `latest.json`.
4. Users running an older Fade see the update banner within 2s of
   next launch.

---

## CI workflow

The release pipeline lives at `.github/workflows/release.yml`. It runs
on `macos-14` (Apple Silicon) and produces the full set of artifacts
required by the updater contract above. Currently macOS aarch64 is the
only target — adding x86_64 or other platforms is a small matrix
extension.

### What it does

1. **Trigger.** Fires on any pushed tag matching `v*` (e.g. `v0.2.0`),
   or via manual `workflow_dispatch` with a tag input.
2. **Version sanity check.** Compares the tag (minus the `v`) against
   the `version` field in `src-tauri/tauri.conf.json`. Mismatches fail
   the job before any build work happens, which prevents shipping a
   release whose `latest.json` advertises a version the installed
   binary doesn't report.
3. **Toolchain setup.** Node 20 (with npm cache), Rust stable with
   the `aarch64-apple-darwin` target, and a cargo registry + target
   cache keyed on `src-tauri/Cargo.lock`.
4. **Build.** `npm ci` then
   `npm run tauri build -- --target aarch64-apple-darwin`, with
   `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   exported from repo secrets so Tauri signs the updater bundle
   inline.
5. **Ad-hoc codesign.** Runs `xattr -cr Fade.app` and
   `codesign --force --deep --sign - Fade.app` to strip the quarantine
   xattr and apply an ad-hoc signature. No Apple Developer ID is used.
6. **Re-pack + re-sign the updater tarball.** Tauri emits
   `Fade.app.tar.gz` *during* `tauri build`, which means the tarball
   produced by the build contains the **unsigned** `.app`. The
   workflow therefore deletes that tarball, re-tars the freshly
   signed `.app`, and re-invokes `tauri signer sign` against the new
   tarball so the `.sig` matches. The result: the DMG and the updater
   tarball both contain the ad-hoc signed `.app`, and the `.sig`
   verifies against the pubkey embedded in `tauri.conf.json`.
7. **Generate `latest.json`.** A small inline Node script reads the
   `.sig` file, assembles the manifest in the format documented above,
   uses `date -u` for `pub_date`, and pulls release notes from the
   annotated tag's body (falling back to
   `"See GitHub release notes."`).
8. **Create GitHub Release.** Uses
   `softprops/action-gh-release@v2` to publish a non-draft,
   non-prerelease release under the pushed tag, attaching:
   - `Fade_<version>_aarch64.dmg`
   - `Fade.app.tar.gz`
   - `Fade.app.tar.gz.sig`
   - `latest.json`

### Required repo configuration

- Secrets: `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (see the previous section).
- Permissions: the job declares `contents: write`, which is all
  `softprops/action-gh-release` needs to create a release and upload
  assets via the ambient `GITHUB_TOKEN`.

### Cutting a release

1. Bump `version` in `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml`, and `package.json` (all three must match).
2. Commit.
3. Tag with the `v`-prefix form and push:
   ```sh
   git tag -a v0.2.0 -m "Fade v0.2.0"
   git push origin main --follow-tags
   ```
4. Watch the "Release" workflow in the GitHub Actions tab. On
   success, the release is live at
   `https://github.com/eldo9000/Fade-App/releases/tag/v0.2.0` and the
   `latest.json` endpoint starts serving it immediately.

### Testing the workflow manually

Use `workflow_dispatch` from the Actions tab (or
`gh workflow run release.yml -f tag=v0.2.0`) to re-run the release
against an existing tag without pushing a new one. The version sanity
check still applies — the tag version must match
`src-tauri/tauri.conf.json` or the job fails fast.
