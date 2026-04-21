//! Progress-event rate limiter scaffolding.
//!
//! ffmpeg emits `out_time_ms=...` once per encoded frame — at 60fps that's
//! 60 webview wakeups per second per active job. `RateLimiter` gates each
//! emission by two independent thresholds:
//!
//!   * `min_interval` — wall-clock gap since the last accepted emission
//!   * `min_delta` — absolute change in the tracked value since the last
//!     accepted emission
//!
//! An emission is accepted when **both** thresholds are crossed (or when
//! there is no prior emission). The first `should_emit` call always accepts
//! so the UI sees a 0% tick at job start.
//!
//! Wired into the canonical `run_ffmpeg` in `operations/mod.rs`; replaces
//! the previous unbounded per-line emit so a 60fps encode no longer drives
//! 60 webview wakeups per second.

use std::time::{Duration, Instant};

pub(crate) struct RateLimiter {
    min_interval: Duration,
    min_delta: f32,
    last_emit: Option<Instant>,
    last_value: Option<f32>,
}

impl RateLimiter {
    pub(crate) fn new(min_interval: Duration, min_delta: f32) -> Self {
        Self {
            min_interval,
            min_delta,
            last_emit: None,
            last_value: None,
        }
    }

    /// Decide whether to emit at `now` for tracked value `value`. Updates
    /// internal state when accepting, leaves it untouched when rejecting.
    pub(crate) fn should_emit(&mut self, now: Instant, value: f32) -> bool {
        let accept = match (self.last_emit, self.last_value) {
            (None, _) | (_, None) => true,
            (Some(t), Some(v)) => {
                now.duration_since(t) >= self.min_interval && (value - v).abs() >= self.min_delta
            }
        };
        if accept {
            self.last_emit = Some(now);
            self.last_value = Some(value);
        }
        accept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_emission_always_accepted() {
        let mut rl = RateLimiter::new(Duration::from_millis(100), 1.0);
        assert!(rl.should_emit(Instant::now(), 0.0));
    }

    #[test]
    fn rejects_when_interval_too_short() {
        let mut rl = RateLimiter::new(Duration::from_millis(100), 0.1);
        let t0 = Instant::now();
        assert!(rl.should_emit(t0, 10.0));
        // 50 ms < 100 ms interval — rejected even though delta (5.0) crosses.
        assert!(!rl.should_emit(t0 + Duration::from_millis(50), 15.0));
    }

    #[test]
    fn rejects_when_delta_too_small() {
        let mut rl = RateLimiter::new(Duration::from_millis(10), 5.0);
        let t0 = Instant::now();
        assert!(rl.should_emit(t0, 10.0));
        // 100 ms > 10 ms interval — but delta (1.0) below threshold (5.0).
        assert!(!rl.should_emit(t0 + Duration::from_millis(100), 11.0));
    }

    #[test]
    fn accepts_when_both_thresholds_crossed() {
        let mut rl = RateLimiter::new(Duration::from_millis(100), 5.0);
        let t0 = Instant::now();
        assert!(rl.should_emit(t0, 0.0));
        assert!(rl.should_emit(t0 + Duration::from_millis(200), 10.0));
    }

    #[test]
    fn rejected_emission_does_not_reset_baseline() {
        let mut rl = RateLimiter::new(Duration::from_millis(100), 5.0);
        let t0 = Instant::now();
        assert!(rl.should_emit(t0, 0.0));
        // Rejected — delta too small.
        assert!(!rl.should_emit(t0 + Duration::from_millis(200), 1.0));
        // Next check still compares against the original 0.0 baseline, so
        // a +4.0 move from the baseline still fails delta even though it's
        // +3.0 above the rejected value.
        assert!(!rl.should_emit(t0 + Duration::from_millis(400), 4.0));
        // But +6.0 from baseline crosses.
        assert!(rl.should_emit(t0 + Duration::from_millis(600), 6.0));
    }

    #[test]
    fn negative_delta_is_treated_as_magnitude() {
        let mut rl = RateLimiter::new(Duration::from_millis(10), 5.0);
        let t0 = Instant::now();
        assert!(rl.should_emit(t0, 50.0));
        // Backwards jump — abs(delta) >= 5.0 still accepts.
        assert!(rl.should_emit(t0 + Duration::from_millis(20), 40.0));
    }
}
