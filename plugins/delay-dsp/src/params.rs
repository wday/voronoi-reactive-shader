//! Parameter response curves and clamps, shared by the plugins.
//!
//! Mirrors `delay-write::params` (`regen()`) and the `[0,1]` clamps applied in
//! both plugins' `set`. Kept pure so the curves are unit-tested independently of
//! the FFGL parameter machinery.

/// Regen response: fourth-root curve. Spreads the useful dub range (~0.90–0.99
/// effective) across most of the knob travel. Mirrors `WriteParams::regen`
/// (requirements `WRITE-PARAM-REGEN`). Input and output are in `[0,1]`.
#[inline]
pub fn regen_curve(raw: f32) -> f32 {
    raw.powf(0.25)
}

/// Standard `[0,1]` gain clamp used for Send / Dry / Wet and the raw Regen knob
/// (requirements `WRITE-PARAM-SEND`, `TAP-PARAM-DRY`, `TAP-PARAM-WET`).
#[inline]
pub fn clamp_unit(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regen_curve_endpoints_and_monotonic() {
        assert_eq!(regen_curve(0.0), 0.0);
        assert_eq!(regen_curve(1.0), 1.0);
        // Fourth root pushes small raw values UP (a gentle low end).
        assert!(regen_curve(0.5) > 0.5);
        // Strictly increasing across the knob.
        let mut prev = -1.0;
        for i in 0..=100 {
            let v = regen_curve(i as f32 / 100.0);
            assert!(v > prev, "regen_curve must be strictly increasing");
            prev = v;
        }
    }

    #[test]
    fn regen_curve_spreads_dub_range() {
        // The design intent: the useful 0.90–0.99 *effective* decay lands across
        // a wide swath of the knob. raw^0.25 = 0.9 at raw ~= 0.6561, and = 0.99
        // at raw ~= 0.9606, so ~30% of the knob covers 0.90–0.99.
        assert!((regen_curve(0.6561) - 0.9).abs() < 1e-3);
        assert!((regen_curve(0.9606) - 0.99).abs() < 1e-3);
    }

    #[test]
    fn clamp_unit_bounds() {
        assert_eq!(clamp_unit(-0.5), 0.0);
        assert_eq!(clamp_unit(1.5), 1.0);
        assert_eq!(clamp_unit(0.3), 0.3);
    }
}
