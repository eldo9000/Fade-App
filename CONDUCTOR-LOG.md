# Conductor Log — Blender headless backend for 3D format conversion

**Started:** 2026-04-21
**Goal:** USD, USDZ, Alembic (.abc), and Blender (.blend) conversion working via a Blender CLI backend alongside the existing assimp backend.
**Progress metric:** `cargo check` exit code (0 = all Rust compiles; nonzero = broken). Secondary: count of `todo: true` entries in App.svelte model format list (target: 0 of the 4 new formats).
**Tripwire threshold:** 3 consecutive steps with no metric advance → Stuck-Loop Protocol.

---

## Kickoff Brief

Implement Blender headless backend for 3D format conversion in Fade (Tauri 2 + Rust + Svelte 5).

### Mission

Add USD, USDZ, Alembic (.abc), and Blender (.blend) conversion support by adding a Blender CLI backend alongside the existing assimp backend.

### Research findings (authoritative — do not re-research)

- Target Blender 4.0+ only. No 3.x branching.
- `--python-exit-code 1` is mandatory or Blender exits 0 on Python exceptions
- Alembic ops require `as_background_job=False` — default is True, silent failure otherwise
- `.blend` input must be CLI positional arg: `blender input.blend --background --python script.py`
- USD import can silently succeed with empty scene — verify `len(bpy.data.objects) > 0` after import
- Sentinel pattern: Python script prints `FADE_BLENDER_OK` to stdout; Rust checks both exit code and sentinel
- DAE (Collada) stays routed through assimp — legacy/being removed from Blender
- 3DS stays assimp-only — requires separately installed extension in Blender 4.2+
- USDZ: Blender 4.0+ natively creates a proper zip archive when filepath ends in `.usdz`

### Operator names (Blender 4.x)

| Format | Import operator | Export operator |
|--------|----------------|----------------|
| USD/USDZ | `wm.usd_import` | `wm.usd_export` |
| Alembic | `wm.alembic_import` | `wm.alembic_export` |
| OBJ | `wm.obj_import` | `wm.obj_export` |
| GLTF/GLB | `import_scene.gltf` | `export_scene.gltf` (format='GLB' or 'GLTF_EMBEDDED') |
| STL | `wm.stl_import` | `wm.stl_export` |
| PLY | `wm.ply_import` | `wm.ply_export` |
| FBX | `import_scene.fbx` | `export_scene.fbx` |
| X3D | `import_scene.x3d` | `export_scene.x3d` |

### Files to create/modify

**New files:**
- `src-tauri/src/convert/model_blender.rs` — subprocess runner (mirrors model.rs shape)
- `src-tauri/src/args/model_blender.rs` — Blender path detection + command builder
- `scripts/blender_convert.py` — Python conversion script (ships with the app, path passed to Blender)

**Modify:**
- `src-tauri/src/convert/model.rs` — route USD/USDZ/ABC/BLEND input/output to Blender backend
- `src-tauri/src/convert/mod.rs` — expose model_blender module
- `src-tauri/src/args/mod.rs` — expose model_blender module
- `src/App.svelte` lines 1185-1190 — remove `todo: true` from USD, USDZ, Alembic, Blender entries
- `KNOWN-BUG-CLASSES.md` — add Alembic `as_background_job` and USD empty-scene gotchas

### Routing logic

A conversion goes through Blender if:
- Input format is one of: usd, usdc, usda, usdz, abc, blend
- OR output format is one of: usd, usdz, abc, blend

Otherwise stays on assimp.

### Rust subprocess runner design

- Spawns `blender` with constructed args
- Drains stdout looking for `FADE_BLENDER_OK` sentinel
- Captures stderr for failure messages
- Checks exit code AND sentinel
- Emits job-progress events at 0% start and 100% done

### Path detection (macOS priority)

1. `which blender` (PATH)
2. `/Applications/Blender.app/Contents/MacOS/Blender`
3. `~/Applications/Blender.app/Contents/MacOS/Blender`
4. Windows: `C:\Program Files\Blender Foundation\Blender*\blender.exe` glob
5. Linux: `/usr/bin/blender`, `/usr/local/bin/blender`

Error message must match ffmpeg/assimp pattern:
`"blender not found: {e}\n\nInstall with:\n  macOS:   brew install --cask blender\n  Linux:   apt install blender (or snap/flatpak)\n  Windows: winget install BlenderFoundation.Blender"`

---

## Step 1 — 2026-04-21 — implement Blender backend (all files)

- **Status:** IN FLIGHT — awaiting CI run 24728731018
- **Mission:** Brief §all — create all new files and modify existing ones per the kickoff brief
- **Worker did:**
  - Created `scripts/blender_convert.py` — headless Python script with full import/export operator routing and `FADE_BLENDER_OK` sentinel
  - Created `src-tauri/src/args/model_blender.rs` — `find_blender()`, `needs_blender()`, `build_blender_args()`, `blender_not_found_msg()` with platform-gated `#[cfg]` path search
  - Created `src-tauri/src/convert/model_blender.rs` — subprocess runner mirroring model.rs, drains stdout for sentinel, fails distinctly if sentinel absent on exit 0
  - Modified `model.rs` to delegate to Blender backend when `needs_blender()` is true
  - Modified `convert/mod.rs` and `args/mod.rs` to expose new modules
  - Removed `todo: true` from usd/usdz/abc/blend in App.svelte; STEP/IGES remain
  - Appended BC-003 and BC-004 to `KNOWN-BUG-CLASSES.md`
- **Conductor verified:** All 3 new files exist. `todo: true` removed from exactly usd/usdz/abc/blend (STEP/IGES untouched). `cargo check` re-run locally: exit 0 in 2.15s.
- **Repo delta:** commit `9b13f41` on main; git notes pushed
- **Metric:** (pending — awaiting CI run 24726537004)
- **Next:** (pending)
