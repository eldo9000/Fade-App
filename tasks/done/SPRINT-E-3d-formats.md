# Sprint E — 3D Format Expansion

**Goal:** Unlock USD/USDZ, Alembic, Blender native, and STEP/IGES. First three use the existing Blender pipeline. STEP/IGES requires FreeCAD or OpenCASCADE.

**Entry condition:** `convert/model.rs` + `convert/model_blender.rs` stable. Blender path detection working (TASK-29 landed).

---

## TASK-E1: Blender native (.blend) input and output

**Scope:** Accept .blend as input; export to any model format via Blender Python.

**What to do:**
- `model_blender.rs` already uses Blender Python script for USD/ABC — extend script to handle .blend input
- For input: `bpy.ops.wm.open_mainfile(filepath=input)` then export to target format
- For output: any format → .blend via `bpy.ops.wm.save_as_mainfile(filepath=output)`
- Verify Blender version check (deferred per SESSION-STATUS Known Risks — implement now)
- Set `blend` to `live: true`
- Add sweep cases

**Done when:** .blend ↔ GLTF/OBJ/FBX converts cleanly. CI green.

---

## TASK-E2: USD and USDZ output

**Scope:** Enable USD/USDZ as live output formats (currently `building: true`).

**What to do:**
- Blender Python pipeline already in `model_blender.rs` — verify USD export: `bpy.ops.wm.usd_export(filepath=output)`
- USDZ: Blender supports `.usdz` export natively since Blender 3.5
- Set `usd` and `usdz` to `live: true`
- Add Blender version guard: require ≥ 3.5 for USDZ; emit version error if too old
- Add sweep cases (OBJ → USD, OBJ → USDZ)

**Done when:** USD/USDZ output passes sweep or emits Blender version error. CI green.

---

## TASK-E3: Alembic (.abc) input and output

**Scope:** Enable Alembic as a live format (currently `building: true`).

**What to do:**
- Blender Python: `bpy.ops.wm.alembic_export(filepath=output)` and `bpy.ops.wm.alembic_import(filepath=input)`
- Set `abc` to `live: true`
- Add sweep cases (OBJ → ABC, ABC → OBJ)

**Done when:** Alembic conversions pass sweep. CI green.

---

## TASK-E4: STEP and IGES (CAD formats)

**Scope:** Enable STEP (.stp/.step) and IGES (.igs/.iges) — currently `todo: true`.

**What to do:**
- Evaluate converters: FreeCAD CLI (`freecad --convert`), OpenCASCADE-based `occt_convert`, or `pythonOCC`
- FreeCAD: `FreeCAD --convert <input> <output>` supports STEP ↔ BREP, STEP → STL, IGES → STEP
- Add binary detection for `FreeCAD` or `freecad` (case varies by platform)
- Implement `convert_step_iges()` in `convert/model.rs`
- If no converter available: emit "CAD formats require FreeCAD"
- Set STEP/IGES to `live: true` (conditional on FreeCAD presence)
- Add sweep cases

**Done when:** STEP/IGES converts with FreeCAD present, or emits dependency error. CI green.
