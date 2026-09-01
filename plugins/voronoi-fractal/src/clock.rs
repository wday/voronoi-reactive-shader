//! Drift clock (FV-DRIFT / FV-SYNC).
//!
//! Produces `anim_time`, a *cycle count* rather than a time in seconds: 1.0 is
//! one complete drift cycle. The shader turns it into circular orbits
//! (`TAU * anim_time`) and chaotic random-walk steps (`floor`/`fract`), so both
//! drift flavours land on the same grid.
//!
//! Free-run advances by `drift_speed / fps` per frame. Synced derives the cycle
//! count from the host's bar position. FFGL hands us `barPhase` in `[0,1)` but
//! no bar *index*, so bars are counted here by watching the phase wrap.

/// Phase must fall by more than this in one frame to count as a bar wrap.
/// Guards against jitter in the host's reported phase.
const WRAP_THRESHOLD: f32 = 0.5;

pub struct DriftClock {
    anim_time: f32,
    /// Bars completed since the last period change.
    bar_count: f32,
    last_bar_phase: f32,
    last_period: f32,
}

impl DriftClock {
    pub fn new() -> Self {
        Self {
            anim_time: 0.0,
            bar_count: 0.0,
            last_bar_phase: 0.0,
            last_period: 0.0,
        }
    }

    /// Advance one frame and return the current cycle count.
    ///
    /// - `bar_phase`   — `host_beat.barPhase`, `[0,1)` across one bar.
    /// - `period_bars` — drift period in bars; `<= 0` selects free-run.
    /// - `drift_speed` — free-run rate in cycles/second (ignored when synced).
    /// - `fps`         — assumed frame rate for the free-run integrator.
    pub fn tick(&mut self, bar_phase: f32, period_bars: f32, drift_speed: f32, fps: f32) -> f32 {
        // Engaging or changing sync restarts the bar count. The cycle count JUMPS
        // here, deliberately: snapping onto the beat grid is the point, and
        // preserving continuity instead would leave the drift phase offset by
        // wherever the bar happened to be when sync was engaged.
        if period_bars != self.last_period {
            self.bar_count = 0.0;
            self.last_period = period_bars;
            self.last_bar_phase = bar_phase;
        }

        if period_bars <= 0.0 {
            self.anim_time += drift_speed / fps.max(1.0);
        } else {
            if bar_phase < self.last_bar_phase - WRAP_THRESHOLD {
                self.bar_count += 1.0;
            }
            self.last_bar_phase = bar_phase;
            self.anim_time = (self.bar_count + bar_phase) / period_bars;
        }

        self.anim_time
    }

    /// FV-SYNC: seeds share one drift rate while locked, so the field lands
    /// together on the beat. Free-run keeps the per-seed rate randomisation.
    pub fn beat_locked(period_bars: f32) -> f32 {
        if period_bars > 0.0 {
            1.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;
    const FPS: f32 = 60.0;

    #[test]
    fn free_run_ignores_bar_phase() {
        let mut c = DriftClock::new();
        // Same drift_speed, wildly different phases → identical advance.
        let a = c.tick(0.0, 0.0, 60.0, FPS);
        let b = c.tick(0.75, 0.0, 60.0, FPS);
        assert!((a - 1.0).abs() < EPS, "got {a}");
        assert!((b - 2.0).abs() < EPS, "got {b}");
    }

    #[test]
    fn one_bar_period_completes_one_cycle_per_bar() {
        let mut c = DriftClock::new();
        c.tick(0.0, 1.0, 0.0, FPS);
        let quarter = c.tick(0.25, 1.0, 0.0, FPS);
        let three_q = c.tick(0.75, 1.0, 0.0, FPS);
        assert!((quarter - 0.25).abs() < EPS, "got {quarter}");
        assert!((three_q - 0.75).abs() < EPS, "got {three_q}");
    }

    #[test]
    fn bar_wrap_advances_the_counter() {
        let mut c = DriftClock::new();
        c.tick(0.0, 1.0, 0.0, FPS);
        c.tick(0.9, 1.0, 0.0, FPS);
        // Phase wraps 0.9 → 0.1: one bar elapsed, so cycle count passes 1.0.
        let after = c.tick(0.1, 1.0, 0.0, FPS);
        assert!((after - 1.1).abs() < EPS, "got {after}");
    }

    #[test]
    fn four_bar_period_is_a_quarter_cycle_per_bar() {
        let mut c = DriftClock::new();
        c.tick(0.0, 4.0, 0.0, FPS);
        c.tick(0.9, 4.0, 0.0, FPS);
        let after = c.tick(0.0, 4.0, 0.0, FPS); // one full bar elapsed
        assert!((after - 0.25).abs() < EPS, "got {after}");
    }

    #[test]
    fn quarter_bar_period_runs_four_cycles_per_bar() {
        let mut c = DriftClock::new();
        c.tick(0.0, 0.25, 0.0, FPS);
        let half = c.tick(0.5, 0.25, 0.0, FPS);
        assert!((half - 2.0).abs() < EPS, "half a bar = 2 cycles, got {half}");
    }

    #[test]
    fn sync_ignores_drift_speed() {
        let mut c = DriftClock::new();
        c.tick(0.0, 1.0, 0.0, FPS);
        let slow = c.tick(0.5, 1.0, 0.0, FPS);
        let mut d = DriftClock::new();
        d.tick(0.0, 1.0, 999.0, FPS);
        let fast = d.tick(0.5, 1.0, 999.0, FPS);
        assert!((slow - fast).abs() < EPS, "{slow} vs {fast}");
    }

    #[test]
    fn engaging_sync_snaps_to_the_bar_grid() {
        let mut c = DriftClock::new();
        // Free-run for a while so anim_time is at some arbitrary value...
        for _ in 0..37 {
            c.tick(0.0, 0.0, 2.0, FPS);
        }
        // ...then engage 1-bar sync mid-bar. The cycle count must be the bar
        // phase, not a continuation of the free-run value.
        let after = c.tick(0.4, 1.0, 0.0, FPS);
        assert!((after - 0.4).abs() < EPS, "expected snap to 0.4, got {after}");
    }

    #[test]
    fn period_change_rebases_the_bar_count() {
        let mut c = DriftClock::new();
        c.tick(0.0, 1.0, 0.0, FPS);
        c.tick(0.9, 1.0, 0.0, FPS);
        c.tick(0.1, 1.0, 0.0, FPS); // one bar elapsed → 1.1
        // Switching period restarts the count, so we are back inside cycle 0.
        let after = c.tick(0.1, 0.5, 0.0, FPS);
        assert!((after - 0.2).abs() < EPS, "got {after}");
    }

    #[test]
    fn small_backward_jitter_is_not_a_wrap() {
        let mut c = DriftClock::new();
        c.tick(0.50, 1.0, 0.0, FPS);
        // Host reports a slightly earlier phase; must not count a bar.
        let after = c.tick(0.49, 1.0, 0.0, FPS);
        assert!((after - 0.49).abs() < EPS, "got {after}");
    }

    #[test]
    fn beat_locked_flag_tracks_the_period() {
        assert_eq!(DriftClock::beat_locked(0.0), 0.0);
        assert_eq!(DriftClock::beat_locked(1.0), 1.0);
    }
}
