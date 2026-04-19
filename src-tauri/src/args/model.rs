//! Argument builder for the `assimp` CLI (Open Asset Import Library).
//!
//! Assimp routes output format via an explicit format ID (`-f <id>`), not the
//! output-file extension. We map the file extension the user picks (obj, stl,
//! ply, gltf, glb, dae, fbx, 3ds, x3d) to assimp's canonical export ID
//! (objnomtl, stlb, plyb, gltf2, glb2, collada, fbxa, 3ds, x3d).
//!
//! Binary variants are preferred where they exist (`stlb`, `plyb`, `glb2`)
//! because they're smaller, faster to load, and lossless. ASCII variants are
//! still reachable via the file extension the user picked.
//!
//! Notes on coverage:
//!   - FBX write is ASCII-only (`fbxa`). Binary FBX requires the proprietary
//!     Autodesk FBX SDK, which assimp does not bundle.
//!   - USD/USDZ/BLEND are not supported by assimp and are not offered here.

use crate::ConvertOptions;

/// Map the chosen output extension to the assimp export format ID.
///
/// Returns `None` if the extension is not a supported 3D model output.
pub fn assimp_format_id(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "obj" => Some("obj"),
        "stl" => Some("stlb"),   // binary STL — smaller, faster
        "ply" => Some("plyb"),   // binary PLY
        "gltf" => Some("gltf2"), // glTF 2.0 (separate .bin + textures)
        "glb" => Some("glb2"),   // glTF 2.0 binary, self-contained
        "dae" => Some("collada"),
        "fbx" => Some("fbxa"), // FBX ASCII (binary FBX unavailable)
        "3ds" => Some("3ds"),
        "x3d" => Some("x3d"),
        _ => None,
    }
}

/// Build the assimp CLI args for a model conversion: `assimp export <in> <out> -f <id>`.
pub fn build_assimp_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let ext = opts.output_format.to_ascii_lowercase();
    let format_id = assimp_format_id(&ext).unwrap_or("obj");
    vec![
        "export".to_string(),
        input.to_string(),
        output.to_string(),
        "-f".to_string(),
        format_id.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(fmt: &str) -> ConvertOptions {
        ConvertOptions {
            output_format: fmt.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn maps_common_extensions() {
        assert_eq!(assimp_format_id("obj"), Some("obj"));
        assert_eq!(assimp_format_id("stl"), Some("stlb"));
        assert_eq!(assimp_format_id("ply"), Some("plyb"));
        assert_eq!(assimp_format_id("gltf"), Some("gltf2"));
        assert_eq!(assimp_format_id("glb"), Some("glb2"));
        assert_eq!(assimp_format_id("dae"), Some("collada"));
        assert_eq!(assimp_format_id("fbx"), Some("fbxa"));
        assert_eq!(assimp_format_id("3ds"), Some("3ds"));
        assert_eq!(assimp_format_id("x3d"), Some("x3d"));
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(assimp_format_id("GLB"), Some("glb2"));
        assert_eq!(assimp_format_id("Stl"), Some("stlb"));
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(assimp_format_id("blend"), None);
        assert_eq!(assimp_format_id("usd"), None);
        assert_eq!(assimp_format_id(""), None);
    }

    #[test]
    fn builds_export_command() {
        let args = build_assimp_args("/in/model.fbx", "/out/model.glb", &opts("glb"));
        assert_eq!(
            args,
            vec!["export", "/in/model.fbx", "/out/model.glb", "-f", "glb2"]
        );
    }

    #[test]
    fn unknown_format_falls_back_to_obj() {
        // Shouldn't happen in practice — caller already validates — but
        // ensure we emit a well-formed command rather than panicking.
        let args = build_assimp_args("a", "b", &opts("mystery"));
        assert_eq!(args[4], "obj");
    }
}
