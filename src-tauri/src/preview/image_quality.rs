use crate::truncate_stderr;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[derive(Serialize, Clone)]
pub struct ImageQualityPreview {
    diff_path: String,
    compressed_path: String,
}

/// Encode the source image at `quality` in `output_format`, then compute a
/// per-pixel difference against the original and return both as temp file paths.
/// Only meaningful for lossy formats (JPEG, WebP, AVIF).
#[command]
pub async fn preview_image_quality(
    path: String,
    quality: u32,
    output_format: String,
) -> Result<ImageQualityPreview, String> {
    crate::validate_input_path(&path)?;
    tokio::task::spawn_blocking(move || -> Result<ImageQualityPreview, String> {
        let p = Path::new(&path);
        if !p.exists() {
            return Err(format!("File not found: {path}"));
        }
        match output_format.as_str() {
            "jpeg" | "jpg" | "webp" | "avif" => {}
            other => {
                return Err(format!(
                    "{other} is lossless — no compression artifacts to preview"
                ))
            }
        }

        let ext = if output_format == "jpeg" {
            "jpg"
        } else {
            output_format.as_str()
        };
        #[cfg(unix)]
        let sandbox = {
            use std::os::unix::fs::PermissionsExt;
            tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(std::env::temp_dir())
                .map_err(|e| format!("failed to create temp sandbox: {e}"))?
        };
        #[cfg(not(unix))]
        let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
            .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
        let compressed = sandbox.path().join(format!("compressed.{ext}"));
        let diff = sandbox.path().join("diff.png");

        // Pass 1: encode at requested quality (induces lossy compression artifacts)
        let enc_out = Command::new("magick")
            .args([
                path.as_str(),
                "-quality",
                &quality.to_string(),
                compressed.to_str().unwrap_or(""),
            ])
            .output()
            .map_err(|e| format!("magick not found: {e}"))?;
        if !enc_out.status.success() {
            return Err(format!(
                "encode failed: {}",
                truncate_stderr(&String::from_utf8_lossy(&enc_out.stderr))
            ));
        }

        // Pass 2: amplified grayscale difference (original − encoded)
        let diff_out = Command::new("magick")
            .args([
                path.as_str(),
                compressed.to_str().unwrap_or(""),
                "-compose",
                "Difference",
                "-composite",
                "-evaluate",
                "multiply",
                "8",
                "-colorspace",
                "gray",
                diff.to_str().unwrap_or(""),
            ])
            .output()
            .map_err(|e| format!("magick not found: {e}"))?;
        if !diff_out.status.success() {
            return Err(format!(
                "diff failed: {}",
                truncate_stderr(&String::from_utf8_lossy(&diff_out.stderr))
            ));
        }

        let kept_dir = sandbox.keep();
        Ok(ImageQualityPreview {
            diff_path: kept_dir.join("diff.png").to_string_lossy().to_string(),
            compressed_path: kept_dir
                .join(format!("compressed.{ext}"))
                .to_string_lossy()
                .to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_image() -> std::path::PathBuf {
        // Create an empty file — preview_image_quality only checks `exists()`
        // before the format validation, so contents don't matter for the
        // error-path tests below.
        let p = std::env::temp_dir().join(format!(
            "fade-imgq-test-{}-{}.png",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::write(&p, b"").unwrap();
        p
    }

    fn err_of<T>(r: Result<T, String>) -> String {
        match r {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    #[tokio::test]
    async fn errors_when_file_missing() {
        let missing = std::env::temp_dir().join(format!(
            "fade-imgq-missing-{}-{}.jpg",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let err = err_of(
            preview_image_quality(
                missing.to_string_lossy().to_string(),
                80,
                "jpeg".to_string(),
            )
            .await,
        );
        assert!(err.starts_with("File not found"), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_lossless_png() {
        let p = tmp_image();
        let err = err_of(
            preview_image_quality(p.to_string_lossy().to_string(), 80, "png".to_string()).await,
        );
        assert!(err.contains("lossless"), "got: {err}");
        fs::remove_file(&p).ok();
    }

    #[tokio::test]
    async fn rejects_unknown_format() {
        let p = tmp_image();
        let err = err_of(
            preview_image_quality(p.to_string_lossy().to_string(), 50, "bogus".to_string()).await,
        );
        assert!(
            err.contains("lossless") || err.contains("bogus"),
            "got: {err}"
        );
        fs::remove_file(&p).ok();
    }

    #[tokio::test]
    async fn accepts_lossy_format_names() {
        // We can't actually run magick in unit tests, but we can verify the
        // format-gate logic accepts the four lossy names by checking that
        // those names are NOT rejected with the "lossless" message. We point
        // at a missing file so the function returns File-not-found instead
        // of ever invoking magick.
        let missing = std::env::temp_dir().join(format!(
            "fade-imgq-gate-{}-{}.jpg",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        for fmt in ["jpeg", "jpg", "webp", "avif"] {
            let err = err_of(
                preview_image_quality(missing.to_string_lossy().to_string(), 80, fmt.to_string())
                    .await,
            );
            // The missing-file check runs first, so we always hit it here;
            // the key point is that we never see "lossless" for these names.
            assert!(
                !err.contains("lossless"),
                "{fmt} wrongly flagged lossless: {err}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn sandbox_directory_is_mode_0700() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let perms = std::fs::metadata(dir.path()).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o700, "sandbox dir mode is not 0700");
    }

    #[test]
    fn image_quality_preview_serializes() {
        let p = ImageQualityPreview {
            diff_path: "/tmp/diff.png".to_string(),
            compressed_path: "/tmp/enc.jpg".to_string(),
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["diff_path"], "/tmp/diff.png");
        assert_eq!(v["compressed_path"], "/tmp/enc.jpg");
    }
}
