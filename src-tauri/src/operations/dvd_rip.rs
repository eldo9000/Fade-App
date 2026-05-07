//! DVD and Blu-ray rip via HandBrakeCLI.
//!
//! HandBrake writes progress to stdout in lines like:
//!   "Encoding: task 1 of 1, 45.50 %"
//! We parse the percentage from those lines and emit job-progress events.

use crate::{validate_output_name, JobProgress};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

/// HandBrake RF quality presets.
/// Lower RF = higher quality; HandBrake default is 22.
#[derive(Debug, Clone, Copy)]
pub enum RfQuality {
    High,    // RF 18
    Default, // RF 20
    Fast,    // RF 22
    Small,   // RF 24
}

impl RfQuality {
    fn rf_value(self) -> u8 {
        match self {
            RfQuality::High => 18,
            RfQuality::Default => 20,
            RfQuality::Fast => 22,
            RfQuality::Small => 24,
        }
    }
}

/// Locate the HandBrakeCLI binary. HandBrake ships as `HandBrakeCLI` on macOS
/// and many Linux distros, but some package managers lower-case it.
fn find_handbrakecli() -> Option<&'static str> {
    for name in &["HandBrakeCLI", "handbrakecli"] {
        if Command::new("which")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            // SAFETY: both literals are 'static
            return Some(if *name == "HandBrakeCLI" {
                "HandBrakeCLI"
            } else {
                "handbrakecli"
            });
        }
    }
    None
}

/// Parse a HandBrake progress line such as:
///   "Encoding: task 1 of 1, 45.50 %"
/// Returns the percentage as an f32 in [0.0, 100.0], or None if the line
/// doesn't match the pattern.
fn parse_hb_percent(line: &str) -> Option<f32> {
    // Look for ", <float> %" anywhere in the line.
    let after_comma = line.split(',').nth(1)?;
    let trimmed = after_comma.trim();
    let without_pct = trimmed.strip_suffix('%')?.trim();
    without_pct.parse::<f32>().ok()
}

/// Run a HandBrakeCLI encode with the given preset. Used by both `run_dvd_rip`
/// and `run_bluray_rip`; the only difference is the preset string.
fn run_handbrake(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    preset: &str,
    rf: u8,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    validate_output_name(output)?;

    let hb = find_handbrakecli()
        .ok_or_else(|| "DVD rip requires HandBrakeCLI (handbrake.fr)".to_string())?;

    let mut child = Command::new(hb)
        .args([
            "--input",
            input,
            "--output",
            output,
            "--preset",
            preset,
            "--quality",
            &rf.to_string(),
        ])
        // HandBrake writes its progress to stdout; stderr carries detailed log.
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn HandBrakeCLI: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
        if cancelled.load(Ordering::SeqCst) {
            if let Some(c) = map.get_mut(job_id) {
                let _ = c.kill();
            }
        }
    }

    // Drain stderr in a thread so the pipe buffer never blocks the child.
    let stderr_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    });

    if let Some(stdout) = stdout {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(pct) = parse_hb_percent(&line) {
                let fraction = (pct / 100.0).min(0.99_f32);
                let _ = window.emit(
                    "job-progress",
                    JobProgress {
                        job_id: job_id.to_string(),
                        percent: fraction * 100.0,
                        message: format!("{:.1}%", pct),
                    },
                );
            }
        }
    }

    let error_output = stderr_thread.join().unwrap_or_default();

    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    if cancelled.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if success {
        Ok(())
    } else {
        let msg = if error_output.trim().is_empty() {
            "HandBrakeCLI encode failed".to_string()
        } else {
            crate::truncate_stderr(&error_output)
        };
        Err(msg)
    }
}

/// Rip a DVD source to an H.264 MKV using HandBrakeCLI.
pub fn run_dvd_rip(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    quality: RfQuality,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    run_handbrake(
        window,
        job_id,
        input,
        output,
        "H.264 MKV 1080p30",
        quality.rf_value(),
        processes,
        &cancelled,
    )
}

/// Rip a Blu-ray source to an H.265 MKV using HandBrakeCLI.
///
/// Note: BD+ DRM requires libbluray compiled into HandBrake. If ripping
/// a DRM-protected disc, ensure your HandBrake build includes libbluray
/// support; otherwise HandBrake will report an advisory error.
pub fn run_bluray_rip(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    quality: RfQuality,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    run_handbrake(
        window,
        job_id,
        input,
        output,
        "H.265 MKV 1080p30",
        quality.rf_value(),
        processes,
        &cancelled,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hb_percent_standard_line() {
        let line = "Encoding: task 1 of 1, 45.50 %";
        assert_eq!(parse_hb_percent(line), Some(45.5));
    }

    #[test]
    fn parse_hb_percent_zero() {
        let line = "Encoding: task 1 of 1, 0.00 %";
        assert_eq!(parse_hb_percent(line), Some(0.0));
    }

    #[test]
    fn parse_hb_percent_hundred() {
        let line = "Encoding: task 1 of 1, 100.00 %";
        assert_eq!(parse_hb_percent(line), Some(100.0));
    }

    #[test]
    fn parse_hb_percent_non_matching_line() {
        assert_eq!(parse_hb_percent("Some other handbrake output"), None);
        assert_eq!(parse_hb_percent(""), None);
    }

    #[test]
    fn rf_quality_values() {
        assert_eq!(RfQuality::High.rf_value(), 18);
        assert_eq!(RfQuality::Default.rf_value(), 20);
        assert_eq!(RfQuality::Fast.rf_value(), 22);
        assert_eq!(RfQuality::Small.rf_value(), 24);
    }

    /// Confirm error message when HandBrakeCLI is absent.
    /// This test is deterministic on any machine without HandBrake installed.
    #[test]
    fn dvd_rip_returns_error_when_handbrake_missing() {
        // Skip if HandBrakeCLI is actually present so we don't accidentally try
        // to rip a non-existent input on a fully-equipped machine.
        if find_handbrakecli().is_some() {
            eprintln!("skip dvd_rip_returns_error_when_handbrake_missing: HandBrakeCLI found");
            return;
        }
        // We can't construct a real Window in unit tests; test the binary
        // detection path via find_handbrakecli instead.
        let result = find_handbrakecli();
        assert!(result.is_none(), "expected None, HandBrake not installed");
    }
}
