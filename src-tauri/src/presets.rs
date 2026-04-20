//! User-saved conversion presets, persisted as JSON alongside the shared
//! `librewin_common` preset file.
//!
//! Why not use `librewin_common::{read_presets, write_presets}` directly:
//! the shared `FadePreset` struct is pinned via a tagged git dep and doesn't
//! carry `normalize_loudness`, so saves from the frontend silently dropped
//! that field (ARCH-009). A local `StoredPreset` extends the shape with
//! `normalize_loudness: Option<bool>`; Shelf's reader ignores unknown
//! fields and only reads (never writes) so the added field survives
//! round-trips through both apps.
//!
//! File locking: `save_preset` / `delete_preset` RMW the JSON file.
//! Without a lock two concurrent saves race and one write is lost
//! (CONC-005). An `fs2` advisory exclusive lock on a sidecar `.lock`
//! file serializes the RMW.

use fs2::FileExt;
use librewin_common::config::fade_presets_path;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use tauri::command;

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Locally-extended preset shape. Superset of `librewin_common::FadePreset`
/// (adds `normalize_loudness`). Serializes to the same JSON file; Shelf
/// reads into the narrower struct and ignores the extra field.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StoredPreset {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub output_format: String,
    #[serde(default)]
    pub quality: Option<u32>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub normalize_loudness: Option<bool>,
}

fn presets_path() -> PathBuf {
    fade_presets_path()
}

fn lock_path() -> PathBuf {
    let mut p = presets_path();
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "fade-presets.json".to_string());
    p.set_file_name(format!("{name}.lock"));
    p
}

/// Runs `f` while holding an exclusive advisory lock on a sidecar lock file,
/// serializing concurrent preset RMW across threads and processes.
fn with_preset_lock<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let lock_path = lock_path();
    if let Some(dir) = lock_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create preset dir: {e}"))?;
    }
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("open preset lock: {e}"))?;
    lock_file
        .lock_exclusive()
        .map_err(|e| format!("acquire preset lock: {e}"))?;
    let out = f();
    let _ = FileExt::unlock(&lock_file);
    out
}

fn read_stored() -> Vec<StoredPreset> {
    let path = presets_path();
    let mut s = String::new();
    match File::open(&path).and_then(|mut f| f.read_to_string(&mut s)) {
        Ok(_) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn write_stored(presets: &[StoredPreset]) -> Result<(), String> {
    let path = presets_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(presets).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

#[command]
pub fn list_presets() -> Vec<StoredPreset> {
    read_stored()
}

#[command]
#[allow(clippy::too_many_arguments)]
pub fn save_preset(
    name: String,
    media_type: String,
    output_format: String,
    quality: Option<u32>,
    codec: Option<String>,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
    normalize_loudness: Option<bool>,
) -> Result<StoredPreset, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Preset name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("Preset name too long (max 64 chars)".to_string());
    }

    let preset = StoredPreset {
        id: uuid_v4(),
        name,
        media_type,
        output_format,
        quality,
        codec,
        bitrate,
        sample_rate,
        normalize_loudness,
    };

    with_preset_lock(|| {
        let mut presets = read_stored();
        presets.push(preset.clone());
        write_stored(&presets)?;
        Ok(preset.clone())
    })
}

#[command]
pub fn delete_preset(id: String) -> Result<(), String> {
    with_preset_lock(|| {
        let mut presets = read_stored();
        let before = presets.len();
        presets.retain(|p| p.id != id);
        if presets.len() == before {
            return Err(format!("Preset not found: {id}"));
        }
        write_stored(&presets)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    // Serialize tests that share the on-disk preset file so they don't
    // clobber each other. Matches the lockfile's cross-process guarantee
    // at the in-process level.
    fn test_mutex() -> &'static Mutex<()> {
        static M: OnceLock<Mutex<()>> = OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
    }

    /// Snapshot + restore the real presets file so tests don't lose user data.
    struct PresetGuard {
        backup: Option<Vec<u8>>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl PresetGuard {
        fn new() -> Self {
            let lock = match test_mutex().lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let backup = std::fs::read(presets_path()).ok();
            let _ = std::fs::remove_file(presets_path());
            PresetGuard {
                backup,
                _lock: lock,
            }
        }
    }

    impl Drop for PresetGuard {
        fn drop(&mut self) {
            match &self.backup {
                Some(bytes) => {
                    let _ = std::fs::write(presets_path(), bytes);
                }
                None => {
                    let _ = std::fs::remove_file(presets_path());
                }
            }
        }
    }

    #[test]
    fn uuid_v4_is_36_chars_with_hyphens() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn uuid_v4_is_unique_across_many_calls() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            assert!(seen.insert(uuid_v4()), "uuid collision");
        }
    }

    #[test]
    fn save_preset_rejects_empty_name() {
        let _g = PresetGuard::new();
        let err = save_preset(
            "".to_string(),
            "image".to_string(),
            "jpeg".to_string(),
            Some(80),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("empty"),
            "expected empty-name error, got {err}"
        );
    }

    #[test]
    fn save_preset_rejects_whitespace_only_name() {
        let _g = PresetGuard::new();
        let err = save_preset(
            "   \t  ".to_string(),
            "image".to_string(),
            "jpeg".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn save_preset_rejects_overlong_name() {
        let _g = PresetGuard::new();
        let long = "x".repeat(65);
        let err = save_preset(
            long,
            "image".to_string(),
            "jpeg".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("too long"),
            "expected too-long error, got {err}"
        );
    }

    #[test]
    fn save_preset_persists_normalize_loudness() {
        let _g = PresetGuard::new();
        let saved = save_preset(
            "Voice test".to_string(),
            "audio".to_string(),
            "mp3".to_string(),
            None,
            None,
            Some(128),
            Some(44_100),
            Some(true),
        )
        .expect("save");

        let all = list_presets();
        let back = all
            .iter()
            .find(|p| p.id == saved.id)
            .expect("saved preset present");
        assert_eq!(back.normalize_loudness, Some(true));
        assert_eq!(back.bitrate, Some(128));
    }

    #[test]
    fn save_preset_normalize_loudness_none_is_preserved() {
        let _g = PresetGuard::new();
        let saved = save_preset(
            "No-norm".to_string(),
            "audio".to_string(),
            "flac".to_string(),
            None,
            None,
            None,
            Some(44_100),
            None,
        )
        .expect("save");

        let back = list_presets()
            .into_iter()
            .find(|p| p.id == saved.id)
            .expect("present");
        assert_eq!(back.normalize_loudness, None);
    }

    #[test]
    fn concurrent_saves_dont_lose_entries() {
        let _g = PresetGuard::new();
        let threads = 16;
        let per_thread = 5;

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                std::thread::spawn(move || {
                    for i in 0..per_thread {
                        save_preset(
                            format!("t{t}-i{i}"),
                            "audio".to_string(),
                            "mp3".to_string(),
                            None,
                            None,
                            Some(128),
                            Some(44_100),
                            Some(i % 2 == 0),
                        )
                        .expect("save");
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("join");
        }

        let all = list_presets();
        assert_eq!(
            all.len(),
            threads * per_thread,
            "expected all {} concurrent saves to persist, got {}",
            threads * per_thread,
            all.len()
        );

        let mut ids = HashSet::new();
        for p in &all {
            assert!(ids.insert(p.id.clone()), "duplicate id {}", p.id);
        }
    }

    #[test]
    fn delete_preset_removes_by_id() {
        let _g = PresetGuard::new();
        let saved = save_preset(
            "Delete me".to_string(),
            "audio".to_string(),
            "mp3".to_string(),
            None,
            None,
            Some(128),
            Some(44_100),
            None,
        )
        .expect("save");

        delete_preset(saved.id.clone()).expect("delete");
        assert!(list_presets().iter().all(|p| p.id != saved.id));
    }

    #[test]
    fn delete_preset_missing_id_errors() {
        let _g = PresetGuard::new();
        let err = delete_preset("no-such-id".to_string()).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn stored_preset_reads_legacy_json_without_normalize_loudness() {
        let _g = PresetGuard::new();
        // Simulate JSON written by an older build / by Shelf (narrower shape).
        let legacy = r#"[{
            "id": "legacy-1",
            "name": "Legacy",
            "media_type": "audio",
            "output_format": "mp3",
            "quality": null,
            "codec": null,
            "bitrate": 192,
            "sample_rate": 44100
        }]"#;
        std::fs::create_dir_all(presets_path().parent().expect("parent")).expect("mkdir");
        std::fs::write(presets_path(), legacy).expect("write legacy");

        let all = list_presets();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "legacy-1");
        assert_eq!(all[0].normalize_loudness, None);
        assert_eq!(all[0].bitrate, Some(192));
    }
}
