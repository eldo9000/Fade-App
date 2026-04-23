# TASK-1: Expand assetProtocol scope to cover Windows non-C drives

## Goal
`src-tauri/tauri.conf.json` has an `assetProtocol.scope` that allows media preview from any Windows drive letter (D:\, E:\, etc.), not just paths under `$HOME`.

## Context
Fade uses Tauri 2's `assetProtocol` to serve local media files to the frontend for preview. The current scope in `tauri.conf.json` is:

```json
"assetProtocol": {
  "enable": true,
  "scope": [
    "$HOME/**",
    "$TEMP/**",
    "/Volumes/**",
    "/media/**",
    "/mnt/**"
  ]
}
```

On macOS, `$HOME` covers `/Users/username/**` and `/Volumes/**` covers external disks. On Linux, `/media/**` and `/mnt/**` cover mounts. On Windows, `$HOME` typically resolves to `C:\Users\username`, so files on the C drive under the user profile work. However, files on secondary Windows drives (D:\, E:\, F:\, etc.) fall outside any of the current scope patterns and are blocked — Tauri's assetProtocol will return a 403/forbidden for asset requests to those paths.

**The fix:** Add a Windows-compatible glob pattern that covers all drive letters. In Tauri 2's scope system, Windows paths use forward slashes in glob context. The correct pattern to match any drive letter root on Windows is `[A-Z]:\\**` (backslash-escaped for JSON) or `[A-Z]://**`. Check the Tauri 2 documentation or existing issues to confirm the exact syntax Tauri 2 uses for Windows drive glob patterns before writing the value.

**Important:** Do not remove any existing scope entries. Only add new entries for Windows drives. The existing macOS and Linux entries must remain intact.

**No code changes needed** — this is a JSON config change only. No Rust or frontend files need to be touched.

**Relevant file:**
- `src-tauri/tauri.conf.json` — the only file to modify

## In scope
- `src-tauri/tauri.conf.json` — add Windows drive pattern(s) to `assetProtocol.scope`

## Out of scope
- Any Rust source files
- Any frontend files
- `src-tauri/capabilities/` — do not add or modify capability files
- Any other config files

## Steps
1. Read `src-tauri/tauri.conf.json` in full to understand the current structure.
2. Determine the correct Tauri 2 glob syntax for Windows drive letters. In Tauri 2, `assetProtocol.scope` entries are glob patterns evaluated by the `glob` crate. For Windows drives, the pattern `[A-Z]:\\**` (matching `C:\anything`, `D:\anything`, etc.) is the standard form. Confirm by checking whether Tauri 2 uses the `glob` crate's path matching conventions.
3. Add the Windows drive pattern to the `assetProtocol.scope` array. A single entry covering all drives is preferred over 26 individual entries.
4. Verify the JSON is valid (no trailing commas, correct nesting).
5. Run `cargo check --manifest-path src-tauri/Cargo.toml` to confirm the build still compiles (tauri.conf.json is parsed at build time).

## Success signal
- `src-tauri/tauri.conf.json` `assetProtocol.scope` contains a new entry matching Windows drive patterns (e.g., `[A-Z]:\\**` or equivalent).
- All existing scope entries (`$HOME/**`, `$TEMP/**`, `/Volumes/**`, `/media/**`, `/mnt/**`) are still present.
- `cargo check` exits 0.

## Notes
Tauri 2 processes `assetProtocol` scope entries as glob patterns using the `glob` crate (same as capability `allow` lists). On Windows, path separators are normalized to forward slashes in some contexts — if `[A-Z]:\\**` does not work, `[A-Z]://**` is the forward-slash equivalent. Add both if uncertain; they are non-overlapping.
