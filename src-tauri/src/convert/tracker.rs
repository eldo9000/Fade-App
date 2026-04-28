//! Tracker / MIDI audio rendering.
//!
//! Two managed CLIs, gated by `which`:
//!
//! - `fluidsynth` for `.mid` (MIDI). Needs a SoundFont (`.sf2`). Lookup order:
//!   `FADE_SOUNDFONT` env var, `/usr/share/sounds/sf2/FluidR3_GM.sf2`,
//!   `/opt/homebrew/share/sounds/sf2/*.sf2`. Falls back to `timidity` (built-in
//!   patches, no soundfont needed) if fluidsynth is missing.
//! - `openmpt123` for `.mod/.xm/.it` (module trackers). Preferred over `xmp` —
//!   newer, broader format support. Falls back to `xmp -o` if absent.
//!
//! Pipeline shape: render to an intermediate WAV in a temp dir, then transcode
//! via ffmpeg to the requested audio format. WAV target short-circuits the
//! transcode step. Errors surface the raw stderr from the underlying tool
//! with a clear install hint including platform-appropriate package names.
//!
//! Not convertible: `.sf2` is a soundfont container, not an audio stream.
//! Kept as `todo: true` in FORMAT_GROUPS with an explanatory comment.

use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{tool_available, truncate_stderr, ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Window;

/// Trait abstracting the WAV → final-format transcode so `convert()` does
/// not need a `&Window`. The Tauri wrapper supplies a real implementation
/// that delegates to `convert::audio::run`; tests that only exercise the
/// pure render path (or that target WAV directly) can pass a no-op
/// implementation that errors if invoked.
pub trait AudioTranscoder {
    fn transcode(
        &mut self,
        input_wav: &str,
        output: &str,
        opts: &ConvertOptions,
        processes: Arc<Mutex<HashMap<String, Child>>>,
        cancelled: Arc<AtomicBool>,
    ) -> ConvertResult;
}

/// AudioTranscoder that always errors — for tests that don't exercise the
/// final-encode path (e.g. tracker tests that target WAV).
pub struct UnavailableAudio;
impl AudioTranscoder for UnavailableAudio {
    fn transcode(
        &mut self,
        _input_wav: &str,
        _output: &str,
        _opts: &ConvertOptions,
        _processes: Arc<Mutex<HashMap<String, Child>>>,
        _cancelled: Arc<AtomicBool>,
    ) -> ConvertResult {
        ConvertResult::Error("audio transcoder not available in this context".to_string())
    }
}

/// Resolve a usable SoundFont path, or return an install-hint error.
fn locate_soundfont() -> Result<PathBuf, String> {
    // 1. Explicit env override wins.
    if let Ok(p) = std::env::var("FADE_SOUNDFONT") {
        let pb = PathBuf::from(&p);
        if pb.is_file() {
            return Ok(pb);
        }
    }
    // 2. Common Linux package location.
    let linux_default = PathBuf::from("/usr/share/sounds/sf2/FluidR3_GM.sf2");
    if linux_default.is_file() {
        return Ok(linux_default);
    }
    // 3. Homebrew fluid-soundfont install path — glob the directory manually
    //    so we don't need to pull in a glob crate.
    let brew_dir = Path::new("/opt/homebrew/share/sounds/sf2");
    if let Ok(rd) = std::fs::read_dir(brew_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("sf2") {
                return Ok(p);
            }
        }
    }
    // 4. Intel Homebrew layout.
    let brew_intel = Path::new("/usr/local/share/sounds/sf2");
    if let Ok(rd) = std::fs::read_dir(brew_intel) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("sf2") {
                return Ok(p);
            }
        }
    }
    Err(
        "No SoundFont found. Set FADE_SOUNDFONT to an .sf2 file, or install one:\n  \
         brew install fluid-synth  (pulls a default sf2 on macOS)\n  \
         apt install fluid-soundfont-gm  (Debian/Ubuntu)\n\n\
         Alternative: install timidity which ships with built-in patches:\n  \
         brew install timidity    (macOS)\n  \
         apt install timidity     (Debian/Ubuntu)"
            .to_string(),
    )
}

/// Spawn a tracker-renderer subprocess and drive it to completion, returning
/// the captured stderr and whether it exited successfully. The child is
/// registered in `processes` so `cancel_job` can kill it.
fn run_renderer(
    mut cmd: Command,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
) -> Result<(bool, String), String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn renderer: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    let stdout_handle = stdout.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for _ in reader.lines().map_while(Result::ok) {}
        })
    });

    let stderr_content = {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    };

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }

    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    Ok((success, stderr_content))
}

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// `audio_transcoder` is invoked when the requested output is a non-WAV
/// audio format and the rendered intermediate WAV needs final encoding.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    audio_transcoder: &mut dyn AudioTranscoder,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    progress(ProgressEvent::Started);

    let in_ext = Path::new(input)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_ext = Path::new(output)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // SoundFont containers aren't convertible — caught here instead of at the
    // routing layer so the error message is specific.
    if in_ext == "sf2" {
        return ConvertResult::Error(
            "SF2 files are SoundFont containers, not audio streams. \
             Use a .mid file together with an .sf2 to render audio."
                .to_string(),
        );
    }

    // Allowed audio targets — anything ffmpeg can write from a WAV source.
    const ALLOWED_TARGETS: &[&str] = &[
        "wav", "mp3", "flac", "ogg", "aac", "opus", "m4a", "aiff", "wma",
    ];
    if !ALLOWED_TARGETS.contains(&out_ext.as_str()) {
        return ConvertResult::Error(format!(
            "Unsupported tracker output format: {out_ext}. \
             Allowed: {}",
            ALLOWED_TARGETS.join(", ")
        ));
    }

    // Render to a temp WAV unless the target IS wav.
    #[cfg(unix)]
    let sandbox = {
        use std::os::unix::fs::PermissionsExt;
        match tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(std::env::temp_dir())
        {
            Ok(d) => d,
            Err(e) => return ConvertResult::Error(format!("failed to create temp sandbox: {e}")),
        }
    };
    #[cfg(not(unix))]
    let sandbox = match tempfile::TempDir::new_in(std::env::temp_dir()) {
        Ok(d) => d,
        Err(e) => return ConvertResult::Error(format!("failed to create temp sandbox: {e}")),
    };
    let tmp_wav_path = sandbox.path().join("render.wav");
    let tmp_wav = tmp_wav_path.to_string_lossy().to_string();
    let render_target = if out_ext == "wav" {
        output.to_string()
    } else {
        tmp_wav.clone()
    };

    // Dispatch to the right renderer.
    let render_res = match in_ext.as_str() {
        "mid" | "midi" => render_midi(&render_target, input, job_id, Arc::clone(&processes)),
        "mod" | "xm" | "it" | "s3m" => {
            render_module(&render_target, input, job_id, Arc::clone(&processes))
        }
        other => {
            return ConvertResult::Error(format!("Unsupported tracker input format: {other}"));
        }
    };
    let (success, stderr_content) = match render_res {
        Ok(t) => t,
        Err(e) => return ConvertResult::Error(e),
    };

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&tmp_wav_path);
        return ConvertResult::Cancelled;
    }

    if !success {
        let _ = std::fs::remove_file(&tmp_wav_path);
        return ConvertResult::Error(if stderr_content.trim().is_empty() {
            "tracker render failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        });
    }

    // WAV target: nothing more to do.
    if out_ext == "wav" {
        progress(ProgressEvent::Done);
        return ConvertResult::Done;
    }

    // Transcode WAV → target via the existing audio pipeline. We re-invoke
    // the audio converter rather than inlining ffmpeg args so format-specific
    // options (bitrate, vbr, etc.) flow through unchanged.
    progress(ProgressEvent::Phase("Encoding…".to_string()));
    progress(ProgressEvent::Percent(0.6));

    let audio_result = audio_transcoder.transcode(
        &tmp_wav,
        output,
        opts,
        Arc::clone(&processes),
        Arc::clone(cancelled),
    );

    // Best-effort cleanup of the intermediate WAV regardless of outcome.
    let _ = std::fs::remove_file(&tmp_wav_path);

    match audio_result {
        ConvertResult::Done => {
            progress(ProgressEvent::Done);
            ConvertResult::Done
        }
        other => other,
    }
}

/// AudioTranscoder backed by `convert::audio::run`. Lives in `run()` only —
/// anything that touches a `&Window` stays inside the wrapper.
struct WindowAudioTranscoder<'a> {
    window: &'a Window,
    job_id: String,
}

impl AudioTranscoder for WindowAudioTranscoder<'_> {
    fn transcode(
        &mut self,
        input_wav: &str,
        output: &str,
        opts: &ConvertOptions,
        processes: Arc<Mutex<HashMap<String, Child>>>,
        cancelled: Arc<AtomicBool>,
    ) -> ConvertResult {
        crate::convert::audio::run(
            self.window,
            &self.job_id,
            input_wav,
            output,
            opts,
            processes,
            cancelled,
        )
    }
}

/// Render MIDI/module-tracker input to the requested audio format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Rendering tracker…");
    let mut transcoder = WindowAudioTranscoder {
        window,
        job_id: job_id.to_string(),
    };
    convert(
        input,
        output,
        opts,
        &mut emit,
        &mut transcoder,
        job_id,
        processes,
        &cancelled,
    )
}

fn render_midi(
    target_wav: &str,
    input: &str,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
) -> Result<(bool, String), String> {
    if tool_available("fluidsynth") {
        let sf = locate_soundfont()?;
        let mut cmd = Command::new("fluidsynth");
        // -ni  : no-shell, no interactive
        // -F   : render to file (WAV)
        // -r   : sample rate
        cmd.args([
            "-ni",
            "-F",
            target_wav,
            "-r",
            "44100",
            sf.to_string_lossy().as_ref(),
            input,
        ]);
        return run_renderer(cmd, job_id, processes);
    }
    if tool_available("timidity") {
        let mut cmd = Command::new("timidity");
        // -Ow : output WAV, -o : output path
        cmd.args(["-Ow", "-o", target_wav, input]);
        return run_renderer(cmd, job_id, processes);
    }
    Err("No MIDI renderer found. Install one:\n  \
         brew install fluid-synth   (macOS, recommended)\n  \
         apt install fluidsynth     (Debian/Ubuntu)\n\n\
         Or, for a SoundFont-free fallback:\n  \
         brew install timidity      (macOS)\n  \
         apt install timidity       (Debian/Ubuntu)"
        .to_string())
}

fn render_module(
    target_wav: &str,
    input: &str,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
) -> Result<(bool, String), String> {
    if tool_available("openmpt123") {
        let mut cmd = Command::new("openmpt123");
        // --render   : offline render
        // -o <path>  : output WAV path
        // --force    : overwrite existing file
        cmd.args(["--render", "--force", "-o", target_wav, input]);
        return run_renderer(cmd, job_id, processes);
    }
    if tool_available("xmp") {
        let mut cmd = Command::new("xmp");
        // -o <path>  : render to file
        // --wav      : WAV output
        cmd.args(["-o", target_wav, "--wav", input]);
        return run_renderer(cmd, job_id, processes);
    }
    Err("No tracker module renderer found. Install one:\n  \
         brew install libopenmpt    (macOS, ships openmpt123)\n  \
         apt install openmpt123     (Debian/Ubuntu)\n\n\
         Fallback:\n  \
         brew install xmp           (macOS)\n  \
         apt install xmp            (Debian/Ubuntu)"
        .to_string())
}
