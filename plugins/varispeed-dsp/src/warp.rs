//! Bar-phase-locked Doppler warp (VS-DOPPLER, requirements §7).
//!
//! A sinusoidal offset added to the read position. Driven by Resolume's
//! `host_beat.barPhase` (0..1 per bar), so the wobble is rhythmic, repeatable, and
//! re-syncs each bar. A *position* wobble makes the instantaneous *rate* oscillate
//! around nominal → that is the Doppler; the pitch swing scales with
//! `depth × cycles_per_bar`.

use std::f32::consts::TAU;

/// Doppler offset in frames.
///
/// - `bar_phase`  — `host_beat.barPhase`, in `[0,1)` across one bar.
/// - `warp_rate_beats` — LFO period as a beat count (e.g. `0.5` for a 1/8-note
///   wobble). `<= 0` disables (returns 0).
/// - `beats_per_bar` — bar length in beats (4.0 for 4/4).
/// - `depth_frames` — swing amplitude in frames.
///
/// One LFO cycle spans `warp_rate_beats` beats, i.e. `beats_per_bar /
/// warp_rate_beats` cycles per bar, so the phase is locked to the bar grid.
pub fn warp_offset(bar_phase: f32, warp_rate_beats: f32, beats_per_bar: f32, depth_frames: f32) -> f32 {
    if warp_rate_beats <= 0.0 || depth_frames == 0.0 || beats_per_bar <= 0.0 {
        return 0.0;
    }
    let cycles_per_bar = beats_per_bar / warp_rate_beats;
    let phase = (bar_phase * cycles_per_bar).fract(); // [0,1) within one LFO cycle
    depth_frames * (TAU * phase).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    #[test]
    fn zero_at_bar_start() {
        assert!(warp_offset(0.0, 0.5, 4.0, 10.0).abs() < EPS);
    }

    #[test]
    fn peak_at_quarter_cycle() {
        // 1/8-note wobble (0.5 beats) in 4/4 = 8 cycles/bar. A quarter-cycle is
        // 1/32 of a bar → sin peak = +depth.
        let v = warp_offset(1.0 / 32.0, 0.5, 4.0, 10.0);
        assert!((v - 10.0).abs() < EPS, "got {v}");
    }

    #[test]
    fn trough_at_three_quarter_cycle() {
        let v = warp_offset(3.0 / 32.0, 0.5, 4.0, 10.0);
        assert!((v + 10.0).abs() < EPS, "got {v}");
    }

    #[test]
    fn one_full_cycle_per_eighth_returns_to_zero() {
        // After 1/8 of a bar (one full cycle for a 1/8-note LFO) it is back to 0.
        assert!(warp_offset(1.0 / 8.0, 0.5, 4.0, 10.0).abs() < EPS);
    }

    #[test]
    fn disabled_when_rate_or_depth_zero() {
        assert_eq!(warp_offset(0.3, 0.0, 4.0, 10.0), 0.0);
        assert_eq!(warp_offset(0.3, 0.5, 4.0, 0.0), 0.0);
        assert_eq!(warp_offset(0.3, 0.5, 0.0, 10.0), 0.0);
    }

    #[test]
    fn amplitude_bounded_by_depth() {
        for i in 0..64 {
            let ph = i as f32 / 64.0;
            let v = warp_offset(ph, 0.5, 4.0, 7.0);
            assert!(v.abs() <= 7.0 + EPS);
        }
    }
}
