"""
Blender headless conversion script for Fade.

Invoked as:
  blender [input.blend] --background --python scripts/blender_convert.py \
          --python-exit-code 1 -- --input <src> --output <dst>

For non-.blend inputs the positional .blend arg is omitted; Blender starts
with an empty scene and the script imports from --input.
"""

import sys
import os


def _parse_args():
    after = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    args = {}
    i = 0
    while i < len(after) - 1:
        if after[i] in ("--input", "--output"):
            args[after[i][2:]] = after[i + 1]
            i += 2
        else:
            i += 1
    return args.get("input"), args.get("output")


def _ext(path):
    return os.path.splitext(path)[1].lstrip(".").lower()


def _import(path, in_ext):
    import bpy  # noqa: F401  (only importable inside Blender)

    if in_ext in ("usd", "usdc", "usda", "usdz"):
        result = bpy.ops.wm.usd_import(
            filepath=path,
            import_textures_mode="IMPORT_PACK",
            scale=1.0,
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"USD import failed: {result}")
        if len(bpy.data.objects) == 0:
            raise RuntimeError("USD import returned FINISHED but scene is empty")

    elif in_ext == "abc":
        result = bpy.ops.wm.alembic_import(
            filepath=path,
            as_background_job=False,
            set_frame_range=True,
            validate_meshes=True,
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"Alembic import failed: {result}")

    elif in_ext == "obj":
        result = bpy.ops.wm.obj_import(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"OBJ import failed: {result}")

    elif in_ext in ("gltf", "glb"):
        result = bpy.ops.import_scene.gltf(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"glTF import failed: {result}")

    elif in_ext == "stl":
        result = bpy.ops.wm.stl_import(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"STL import failed: {result}")

    elif in_ext == "ply":
        result = bpy.ops.wm.ply_import(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"PLY import failed: {result}")

    elif in_ext == "fbx":
        result = bpy.ops.import_scene.fbx(filepath=path, global_scale=1.0)
        if "FINISHED" not in result:
            raise RuntimeError(f"FBX import failed: {result}")

    elif in_ext == "dae":
        result = bpy.ops.wm.collada_import(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"Collada import failed: {result}")

    elif in_ext == "x3d":
        result = bpy.ops.import_scene.x3d(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"X3D import failed: {result}")

    else:
        raise RuntimeError(f"Unsupported input format: {in_ext}")


def _export(path, out_ext):
    import bpy  # noqa: F401

    if out_ext in ("usd", "usdc", "usda", "usdz"):
        result = bpy.ops.wm.usd_export(
            filepath=path,
            selected_objects_only=False,
            export_animation=False,
            export_textures=True,
            overwrite_textures=True,
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"USD export failed: {result}")

    elif out_ext == "abc":
        result = bpy.ops.wm.alembic_export(
            filepath=path,
            as_background_job=False,
            selected=False,
            start=1,
            end=1,
            xsamples=1,
            gsamples=1,
            uvs=True,
            normals=True,
            vcolors=False,
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"Alembic export failed: {result}")

    elif out_ext == "obj":
        result = bpy.ops.wm.obj_export(filepath=path, export_selected_objects=False)
        if "FINISHED" not in result:
            raise RuntimeError(f"OBJ export failed: {result}")

    elif out_ext == "gltf":
        result = bpy.ops.export_scene.gltf(
            filepath=path, export_format="GLTF_EMBEDDED"
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"glTF export failed: {result}")

    elif out_ext == "glb":
        result = bpy.ops.export_scene.gltf(filepath=path, export_format="GLB")
        if "FINISHED" not in result:
            raise RuntimeError(f"GLB export failed: {result}")

    elif out_ext == "stl":
        result = bpy.ops.wm.stl_export(filepath=path, ascii_format=False)
        if "FINISHED" not in result:
            raise RuntimeError(f"STL export failed: {result}")

    elif out_ext == "ply":
        result = bpy.ops.wm.ply_export(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"PLY export failed: {result}")

    elif out_ext == "fbx":
        result = bpy.ops.export_scene.fbx(
            filepath=path, use_selection=False, bake_anim=False
        )
        if "FINISHED" not in result:
            raise RuntimeError(f"FBX export failed: {result}")

    elif out_ext == "dae":
        result = bpy.ops.wm.collada_export(filepath=path)
        if "FINISHED" not in result:
            raise RuntimeError(f"Collada export failed: {result}")

    elif out_ext == "x3d":
        result = bpy.ops.export_scene.x3d(filepath=path, use_selection=False)
        if "FINISHED" not in result:
            raise RuntimeError(f"X3D export failed: {result}")

    else:
        raise RuntimeError(f"Unsupported output format: {out_ext}")


def main():
    input_path, output_path = _parse_args()
    if not input_path or not output_path:
        raise RuntimeError("--input and --output are required")

    in_ext = _ext(input_path)
    out_ext = _ext(output_path)

    if in_ext != "blend":
        _import(input_path, in_ext)

    _export(output_path, out_ext)

    print("FADE_BLENDER_OK")


main()
