//! Delay-time conversion: the Write's Time controls -> a delay in frames.
//!
//! Mirrors `delay-write::DelayWrite::compute_delay_frames` (requirements
//! `WRITE-PARAM-TIME`). Pure arithmetic given the resolved inputs (BPM, the
//! estimated FPS, and the raw control values), so every branch — including the
//! `BPM <= 0` fallback, the rounding, and the clamp — is unit-testable.

/// How the tape length is specified. Mirrors `delay-write::params::SyncMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncMode {
    Subdivision,
    Ms,
    Frames,
}

/// Convert the Time controls to a delay in frames, clamped to `1..=max_frames`.
/// `max_frames` is `dc_buffer_depth()` (120) in the Write; delay-core then clamps
/// the effective loop length to `1..=119` (requirements `WRITE-PARAM-TIME`,
/// `CORE-DEPTH`).
///
/// - `Subdivision`: `round(subdivision_beats * (60 / bpm) * fps)`. If `bpm <= 0`
///   the result is `min(30, max_frames)` (the subdivision is ignored).
/// - `Ms`: `round((delay_ms / 1000) * fps)`.
/// - `Frames`: `frames_raw`, used directly.
pub fn delay_frames(
    mode: SyncMode,
    subdivision_beats: f32,
    delay_ms: f32,
    frames_raw: u32,
    bpm: f32,
    fps: f32,
    max_frames: u32,
) -> u32 {
    let d = match mode {
        SyncMode::Subdivision => {
            if bpm <= 0.0 {
                return 30u32.min(max_frames);
            }
            let beat_duration = 60.0 / bpm;
            let delay_secs = subdivision_beats * beat_duration;
            (delay_secs * fps).round() as u32
        }
        SyncMode::Ms => {
            let delay_secs = delay_ms / 1000.0;
            (delay_secs * fps).round() as u32
        }
        SyncMode::Frames => frames_raw,
    };
    d.clamp(1, max_frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: u32 = 120; // dc_buffer_depth()

    #[test]
    fn frames_mode_is_passthrough_clamped() {
        assert_eq!(delay_frames(SyncMode::Frames, 0.0, 0.0, 30, 120.0, 60.0, MAX), 30);
        assert_eq!(delay_frames(SyncMode::Frames, 0.0, 0.0, 0, 120.0, 60.0, MAX), 1); // min clamp
        assert_eq!(delay_frames(SyncMode::Frames, 0.0, 0.0, 999, 120.0, 60.0, MAX), MAX);
    }

    #[test]
    fn ms_mode_rounds_to_frames() {
        // 500 ms at 60 fps = 30 frames.
        assert_eq!(delay_frames(SyncMode::Ms, 0.0, 500.0, 0, 120.0, 60.0, MAX), 30);
        // 2000 ms at 60 fps = 120 frames -> hits the max (and, downstream, runs at 119).
        assert_eq!(delay_frames(SyncMode::Ms, 0.0, 2000.0, 0, 120.0, 60.0, MAX), 120);
        // Rounding: 16 ms at 60 fps = 0.96 frames -> rounds to 1.
        assert_eq!(delay_frames(SyncMode::Ms, 0.0, 16.0, 0, 120.0, 60.0, MAX), 1);
    }

    #[test]
    fn subdivision_mode_uses_bpm_and_fps() {
        // 1/4 note (1 beat) at 120 BPM = 0.5 s; at 60 fps = 30 frames.
        assert_eq!(delay_frames(SyncMode::Subdivision, 1.0, 0.0, 0, 120.0, 60.0, MAX), 30);
        // 1 bar (4 beats) at 120 BPM = 2 s; at 60 fps = 120 frames.
        assert_eq!(delay_frames(SyncMode::Subdivision, 4.0, 0.0, 0, 120.0, 60.0, MAX), 120);
        // FPS drives the conversion: same musical time at 30 fps halves the frames.
        assert_eq!(delay_frames(SyncMode::Subdivision, 1.0, 0.0, 0, 120.0, 30.0, MAX), 15);
    }

    #[test]
    fn subdivision_bpm_zero_falls_back_to_30() {
        // requirements WRITE-PARAM-TIME: BPM <= 0 -> 30 frames, subdivision ignored.
        assert_eq!(delay_frames(SyncMode::Subdivision, 16.0, 0.0, 0, 0.0, 60.0, MAX), 30);
        assert_eq!(delay_frames(SyncMode::Subdivision, 0.25, 0.0, 0, -5.0, 60.0, MAX), 30);
        // Fallback is itself bounded by max_frames.
        assert_eq!(delay_frames(SyncMode::Subdivision, 1.0, 0.0, 0, 0.0, 60.0, 10), 10);
    }

    #[test]
    fn result_is_clamped_into_range() {
        // Very long subdivision saturates at max_frames.
        assert_eq!(delay_frames(SyncMode::Subdivision, 16.0, 0.0, 0, 60.0, 60.0, MAX), MAX);
        // Never returns 0.
        assert!(delay_frames(SyncMode::Ms, 0.0, 1.0, 0, 120.0, 60.0, MAX) >= 1);
    }
}
