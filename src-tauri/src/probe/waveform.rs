use crate::probe_duration;
use crate::AppState;
use serde::Serialize;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{command, Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct WaveformData {
    pub amplitudes: Vec<f32>,
    /// HSL hue (0-360) for each bar, derived from per-chunk dominant frequency.
    pub hues: Vec<u32>,
}

#[derive(Serialize, Clone)]
pub struct WaveformResult {
    pub job_id: String,
    pub data: Option<WaveformData>,
    pub error: Option<String>,
    pub cancelled: bool,
}

/// Upper bound on frontend-requested bucket count. 1600 is the point where an
/// SVG `<rect>` per bar still reconciles cheaply; above that both IPC payload
/// and DOM cost scale faster than perceived waveform detail.
const MAX_BUCKETS: usize = 1600;
const MIN_BUCKETS: usize = 100;

/// When `probe_duration` can't determine length (corrupt header, exotic
/// container), fall back to a hint representing roughly this many seconds of
/// audio at the decode sample rate. Too small and short files collapse into
/// fewer buckets than requested; too big and the last bucket silently absorbs
/// everything for files longer than the hint. 120s matches typical clip
/// previews; files past the hint still produce a valid waveform because the
/// final bucket accepts the remainder.
const FALLBACK_DURATION_HINT_S: u64 = 120;

/// Zero-crossing rate → HSL hue.
/// At 8 000 Hz sample rate, ZCR × 4 000 ≈ dominant frequency in Hz.
/// Bass (red/orange) → mids (yellow/green) → hi-hats (cyan/blue).
fn zcr_to_hue(zcr: f32) -> u32 {
    let clamped = zcr.clamp(0.0, 1.0);
    (clamped * 240.0) as u32
}

/// Streaming bucket accumulator — never buffers the full PCM stream.
///
/// Samples flow in one-at-a-time via `push`. Each sample is folded into the
/// current bucket's running `sum_sq` + zero-crossing counter. When the
/// current bucket reaches `samples_per_bucket`, subsequent samples advance to
/// the next bucket (except on the final bucket, which absorbs any overflow so
/// that under-estimated duration hints don't drop data).
///
/// Memory: `O(n)` — only the n bucket accumulators, never the raw PCM stream.
struct BucketAccumulator {
    n: usize,
    samples_per_bucket: u64,
    sum_sq: Vec<f64>,
    count: Vec<u64>,
    crossings: Vec<u32>,
    prev_sign: Vec<Option<bool>>,
    cur_idx: usize,
    cur_count: u64,
}

impl BucketAccumulator {
    fn new(n: usize, samples_per_bucket: u64) -> Self {
        Self {
            n,
            samples_per_bucket: samples_per_bucket.max(1),
            sum_sq: vec![0.0; n],
            count: vec![0; n],
            crossings: vec![0; n],
            prev_sign: vec![None; n],
            cur_idx: 0,
            cur_count: 0,
        }
    }

    fn push(&mut self, s: f32) {
        if self.cur_count >= self.samples_per_bucket && self.cur_idx + 1 < self.n {
            self.cur_idx += 1;
            self.cur_count = 0;
        }
        let idx = self.cur_idx;
        let sign = s >= 0.0;
        if let Some(prev) = self.prev_sign[idx] {
            if prev != sign {
                self.crossings[idx] += 1;
            }
        }
        self.prev_sign[idx] = Some(sign);
        self.sum_sq[idx] += f64::from(s) * f64::from(s);
        self.count[idx] += 1;
        self.cur_count += 1;
    }

    fn finish(self) -> WaveformData {
        if self.count.iter().all(|&c| c == 0) {
            return WaveformData {
                amplitudes: vec![],
                hues: vec![],
            };
        }
        let last = self.count.iter().rposition(|&c| c > 0).unwrap_or(0);
        let len = last + 1;
        let mut amplitudes = Vec::with_capacity(len);
        let mut hues = Vec::with_capacity(len);
        for i in 0..len {
            let c = self.count[i];
            if c == 0 {
                amplitudes.push(0.0);
                hues.push(0);
                continue;
            }
            let mean_sq = self.sum_sq[i] / c as f64;
            amplitudes.push(mean_sq.sqrt() as f32);
            let zcr = self.crossings[i] as f32 / c as f32;
            hues.push(zcr_to_hue(zcr));
        }
        let max = amplitudes.iter().copied().fold(0.0f32, f32::max);
        if max > 0.0 {
            for a in &mut amplitudes {
                *a /= max;
            }
        }
        WaveformData { amplitudes, hues }
    }
}

/// Drive f32le samples from `reader` into `sink`. Reads in 64 KiB blocks; if
/// a block ends mid-sample, the trailing 1-3 bytes carry over to the next
/// read. Never holds more than 64 KiB of PCM at once.
///
/// `cancelled` is polled once per 64 KiB block; if tripped, the loop exits
/// early (and the caller is responsible for killing the child that's feeding
/// the reader).
fn stream_samples<R: Read, F: FnMut(f32)>(
    mut reader: R,
    mut sink: F,
    cancelled: &AtomicBool,
) -> io::Result<bool> {
    let mut buf = [0u8; 64 * 1024];
    let mut carry = [0u8; 3];
    let mut carry_n: usize = 0;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Ok(true);
        }
        let nread = reader.read(&mut buf)?;
        if nread == 0 {
            break;
        }
        let mut i = 0usize;
        if carry_n > 0 {
            let need = 4 - carry_n;
            if nread >= need {
                let mut w = [0u8; 4];
                w[..carry_n].copy_from_slice(&carry[..carry_n]);
                w[carry_n..].copy_from_slice(&buf[..need]);
                sink(f32::from_le_bytes(w));
                i = need;
                carry_n = 0;
            } else {
                carry[carry_n..carry_n + nread].copy_from_slice(&buf[..nread]);
                carry_n += nread;
                continue;
            }
        }
        while i + 4 <= nread {
            let w = [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]];
            sink(f32::from_le_bytes(w));
            i += 4;
        }
        let rem = nread - i;
        if rem > 0 {
            carry[..rem].copy_from_slice(&buf[i..nread]);
            carry_n = rem;
        }
    }
    Ok(false)
}

/// Extract an N-point RMS waveform plus per-bar frequency hue.
///
/// Streams f32le PCM from ffmpeg stdout and folds it into N bucket
/// accumulators on the fly — memory stays at `O(n)` regardless of file
/// length (previously `O(file_length)`, ~115 MB for a 1h file at 8 kHz).
///
/// `draft` drops decode sample rate to 2 000 Hz (4× less decode + pipe data).
/// RMS envelope is near-identical; only ZCR hue accuracy degrades (Nyquist
/// falls from 4 kHz to 1 kHz). Only engaged on heavy files — audio-only items
/// always run at full fidelity.
///
/// Job-based: spawns a worker thread, registers the ffmpeg child under
/// `job_id` so `cancel_job` can kill it, and emits the final result on
/// `analysis-result:{job_id}`. Returns immediately.
#[command]
pub fn get_waveform(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    path: String,
    draft: bool,
    buckets: Option<usize>,
) -> Result<(), String> {
    let n = buckets.unwrap_or(500).clamp(MIN_BUCKETS, MAX_BUCKETS);
    let ar_str = if draft { "2000" } else { "8000" };
    let ar: u64 = if draft { 2000 } else { 8000 };

    // Register cancellation flag before spawning the thread.
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let duration_s = probe_duration(&path).unwrap_or(0.0);
        let expected_samples = if duration_s > 0.0 {
            (duration_s * ar as f64) as u64
        } else {
            FALLBACK_DURATION_HINT_S * ar
        };
        let samples_per_bucket = (expected_samples / n as u64).max(1);

        let emit = |payload: WaveformResult| {
            let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
        };
        let cleanup = || {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        };

        let spawn_result = Command::new("ffmpeg")
            .args(["-i", &path, "-ac", "1", "-ar", ar_str, "-f", "f32le", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match spawn_result {
            Ok(c) => c,
            Err(e) => {
                cleanup();
                emit(WaveformResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(format!("ffmpeg not found: {e}")),
                    cancelled: false,
                });
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.wait();
                cleanup();
                emit(WaveformResult {
                    job_id: job_id.clone(),
                    data: Some(WaveformData {
                        amplitudes: vec![],
                        hues: vec![],
                    }),
                    error: None,
                    cancelled: false,
                });
                return;
            }
        };

        // Register the child so `cancel_job` can kill it.
        {
            let mut map = processes.lock();
            map.insert(job_id.clone(), child);
        }

        // Close the cancel TOCTOU window.
        if cancelled.load(Ordering::SeqCst) {
            let mut map = processes.lock();
            if let Some(c) = map.get_mut(&job_id) {
                let _ = c.kill();
            }
        }

        let mut accum = BucketAccumulator::new(n, samples_per_bucket);
        let stream_result = stream_samples(stdout, |s| accum.push(s), &cancelled);

        // Retrieve the child back so we can wait for it.
        let child_opt = {
            let mut map = processes.lock();
            map.remove(&job_id)
        };
        if let Some(mut c) = child_opt {
            // If cancelled, make sure the child is killed.
            if cancelled.load(Ordering::SeqCst) {
                let _ = c.kill();
            }
            let _ = c.wait();
        }

        cleanup();

        match stream_result {
            Ok(true) => {
                emit(WaveformResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: None,
                    cancelled: true,
                });
            }
            Ok(false) => {
                if cancelled.load(Ordering::SeqCst) {
                    emit(WaveformResult {
                        job_id: job_id.clone(),
                        data: None,
                        error: None,
                        cancelled: true,
                    });
                } else {
                    emit(WaveformResult {
                        job_id: job_id.clone(),
                        data: Some(accum.finish()),
                        error: None,
                        cancelled: false,
                    });
                }
            }
            Err(e) => {
                emit(WaveformResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(format!("waveform read failed: {e}")),
                    cancelled: false,
                });
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn zcr_to_hue_low_maps_to_red() {
        assert_eq!(zcr_to_hue(0.0), 0);
    }

    #[test]
    fn zcr_to_hue_mid_maps_to_green() {
        assert_eq!(zcr_to_hue(0.5), 120);
    }

    #[test]
    fn zcr_to_hue_high_maps_to_blue() {
        assert_eq!(zcr_to_hue(1.0), 240);
    }

    #[test]
    fn zcr_to_hue_clamps_out_of_range_inputs() {
        assert_eq!(zcr_to_hue(-1.0), 0, "negative clamps to 0 -> red");
        assert_eq!(zcr_to_hue(2.0), 240, "above 1.0 clamps to blue");
        assert_eq!(zcr_to_hue(f32::NAN), 0, "NaN clamps via clamp semantics");
    }

    #[test]
    fn zcr_to_hue_never_exceeds_240() {
        for i in 0..=100 {
            let z = i as f32 / 100.0;
            let h = zcr_to_hue(z);
            assert!(h <= 240, "zcr={z} produced hue {h}");
        }
    }

    #[test]
    fn waveform_data_serializes_as_expected_shape() {
        let w = WaveformData {
            amplitudes: vec![0.1, 0.5, 1.0],
            hues: vec![0, 120, 240],
        };
        let v: serde_json::Value = serde_json::to_value(&w).unwrap();
        assert!(v["amplitudes"].is_array());
        assert!(v["hues"].is_array());
        assert_eq!(v["amplitudes"].as_array().unwrap().len(), 3);
        assert_eq!(v["hues"][1].as_u64(), Some(120));
    }

    #[test]
    fn waveform_result_serializes_with_job_id() {
        let r = WaveformResult {
            job_id: "abc".to_string(),
            data: None,
            error: None,
            cancelled: true,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["job_id"], "abc");
        assert_eq!(v["cancelled"], true);
        assert!(v["data"].is_null());
    }

    #[test]
    fn accumulator_empty_returns_empty_waveform() {
        let acc = BucketAccumulator::new(10, 100);
        let w = acc.finish();
        assert!(w.amplitudes.is_empty());
        assert!(w.hues.is_empty());
    }

    #[test]
    fn accumulator_silence_produces_zero_rms_each_bucket() {
        let mut acc = BucketAccumulator::new(10, 100);
        for _ in 0..1000 {
            acc.push(0.0);
        }
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 10);
        assert_eq!(w.hues.len(), 10);
        for a in &w.amplitudes {
            assert_eq!(*a, 0.0);
        }
        // All samples have sign true; no crossings.
        for h in &w.hues {
            assert_eq!(*h, 0);
        }
    }

    #[test]
    fn accumulator_alternating_square_wave_is_maximum_zcr() {
        let mut acc = BucketAccumulator::new(5, 20);
        for i in 0..100 {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            acc.push(s);
        }
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 5);
        // RMS of ±1 alternation is 1.0; after normalization still 1.0.
        for a in &w.amplitudes {
            assert!((*a - 1.0).abs() < 1e-6, "expected ~1.0, got {a}");
        }
        // 19 crossings per bucket of 20 samples -> zcr = 0.95 -> hue = 228.
        for h in &w.hues {
            assert_eq!(*h, 228, "zcr 0.95 should map to hue 228");
        }
    }

    #[test]
    fn accumulator_last_bucket_absorbs_overflow_samples() {
        // Hint = 100, actual = 200. First 9 buckets fill exactly;
        // bucket 9 absorbs the remaining 110 samples.
        let mut acc = BucketAccumulator::new(10, 10);
        for i in 0..200 {
            // Mark last-bucket samples with a distinct value so we can detect
            // they were not dropped.
            let s = if i < 90 { 0.5 } else { 1.0 };
            acc.push(s);
        }
        // Inspect counts before finish() normalization.
        assert_eq!(acc.count[0], 10);
        assert_eq!(acc.count[8], 10);
        assert_eq!(acc.count[9], 110, "final bucket must absorb remainder");
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 10);
        // Final bucket is mostly 1.0s -> larger RMS than early 0.5 buckets.
        assert!(w.amplitudes[9] > w.amplitudes[0]);
    }

    #[test]
    fn accumulator_short_stream_trims_trailing_empty_buckets() {
        let mut acc = BucketAccumulator::new(10, 100);
        // Only 3 samples — far fewer than one bucket's worth.
        acc.push(0.5);
        acc.push(0.5);
        acc.push(0.5);
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 1, "trailing empty buckets trimmed");
        assert_eq!(w.hues.len(), 1);
    }

    #[test]
    fn accumulator_normalizes_to_unit_peak() {
        let mut acc = BucketAccumulator::new(3, 10);
        for _ in 0..10 {
            acc.push(0.1);
        }
        for _ in 0..10 {
            acc.push(0.5);
        }
        for _ in 0..10 {
            acc.push(0.3);
        }
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 3);
        let peak = w.amplitudes.iter().copied().fold(0.0f32, f32::max);
        assert!(
            (peak - 1.0).abs() < 1e-6,
            "peak should be normalized to 1.0"
        );
    }

    #[test]
    fn accumulator_crossings_reset_between_buckets() {
        // Regression guard: if a bucket's prev_sign leaks into the next
        // bucket's first sample, the first flip after a bucket boundary
        // would be double-counted. Split pattern ensures this would be
        // detectable: [+,+,+, -,-,-] across two buckets = 0 crossings each.
        let mut acc = BucketAccumulator::new(2, 3);
        acc.push(1.0);
        acc.push(1.0);
        acc.push(1.0);
        acc.push(-1.0);
        acc.push(-1.0);
        acc.push(-1.0);
        assert_eq!(acc.crossings[0], 0);
        assert_eq!(acc.crossings[1], 0);
    }

    fn encode_f32le(samples: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(samples.len() * 4);
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    #[test]
    fn stream_samples_parses_whole_word_stream() {
        let input = encode_f32le(&[0.25, -0.5, 0.75, -1.0]);
        let mut out = Vec::new();
        let flag = AtomicBool::new(false);
        let cancelled = stream_samples(Cursor::new(input), |s| out.push(s), &flag).unwrap();
        assert!(!cancelled);
        assert_eq!(out, vec![0.25, -0.5, 0.75, -1.0]);
    }

    /// `Read` impl that only returns `chunk` bytes per call, forcing the
    /// carryover path when chunk isn't a multiple of 4.
    struct ChunkingReader {
        data: Vec<u8>,
        chunk: usize,
        pos: usize,
    }
    impl Read for ChunkingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            let end = (self.pos + self.chunk)
                .min(self.data.len())
                .min(self.pos + buf.len());
            let n = end - self.pos;
            buf[..n].copy_from_slice(&self.data[self.pos..end]);
            self.pos = end;
            Ok(n)
        }
    }

    #[test]
    fn stream_samples_handles_byte_boundary_splits() {
        let samples: Vec<f32> = (0..16).map(|i| i as f32 * 0.25 - 2.0).collect();
        let bytes = encode_f32le(&samples);
        // Every non-multiple-of-4 chunk size exercises the carry path.
        for chunk in [1, 2, 3, 5, 6, 7, 9, 13] {
            let reader = ChunkingReader {
                data: bytes.clone(),
                chunk,
                pos: 0,
            };
            let mut out = Vec::new();
            let flag = AtomicBool::new(false);
            stream_samples(reader, |s| out.push(s), &flag).unwrap();
            assert_eq!(
                out, samples,
                "chunk size {chunk} dropped or reordered samples"
            );
        }
    }

    #[test]
    fn stream_samples_handles_empty_stream() {
        let mut out: Vec<f32> = Vec::new();
        let flag = AtomicBool::new(false);
        stream_samples(Cursor::new(Vec::<u8>::new()), |s| out.push(s), &flag).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn stream_samples_drops_trailing_non_word_bytes() {
        // Three trailing bytes with no fourth byte — cannot form an f32.
        let mut input = encode_f32le(&[0.5]);
        input.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let mut out = Vec::new();
        let flag = AtomicBool::new(false);
        stream_samples(Cursor::new(input), |s| out.push(s), &flag).unwrap();
        assert_eq!(out, vec![0.5]);
    }

    #[test]
    fn stream_samples_exits_early_when_cancelled() {
        let samples: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let bytes = encode_f32le(&samples);
        let flag = AtomicBool::new(true);
        let mut out = Vec::new();
        let cancelled = stream_samples(Cursor::new(bytes), |s| out.push(s), &flag).unwrap();
        assert!(cancelled, "pre-set cancel flag should return true");
        assert!(
            out.is_empty(),
            "no samples should be read when cancelled before first read"
        );
    }

    #[test]
    fn stream_plus_accumulator_end_to_end() {
        // Sine wave: 400 samples at 1 Hz in a 1-second 400 Hz imaginary clock.
        let samples: Vec<f32> = (0..400)
            .map(|i| (2.0 * std::f32::consts::PI * i as f32 / 40.0).sin())
            .collect();
        let bytes = encode_f32le(&samples);
        let mut acc = BucketAccumulator::new(4, 100);
        let flag = AtomicBool::new(false);
        stream_samples(Cursor::new(bytes), |s| acc.push(s), &flag).unwrap();
        let w = acc.finish();
        assert_eq!(w.amplitudes.len(), 4);
        // Sine RMS is stable across full cycles -> normalized amplitudes ≈ 1.
        for a in &w.amplitudes {
            assert!(
                (*a - 1.0).abs() < 0.05,
                "expected sine RMS near 1.0 after normalization, got {a}"
            );
        }
        // Each 100-sample bucket spans 2.5 full cycles -> 5 crossings per cycle
        // × 2.5 ≈ ~5 crossings. ZCR ≈ 0.05, hue ≈ 12. Loose bound to avoid
        // floating-point brittleness.
        for h in &w.hues {
            assert!(*h < 40, "low-frequency sine should map to red-ish hue");
        }
    }
}
