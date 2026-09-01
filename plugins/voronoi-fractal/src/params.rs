//! Parameter block for Voronoi Fractal (requirements §Parameters).
//!
//! Every FFGL param is a normalised 0..1 Standard float; the accessors below map
//! each to its working range. Params 14..17 are deliberately adjacent so the
//! image-mapping group (Influence / Kernel / Cert Contrast / Cert Brightness)
//! reads as one block in the Resolume panel.

use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 21;
pub const PARAM_MODE: usize = 0;
pub const PARAM_DENSITY: usize = 1;
pub const PARAM_LAYER_SPREAD: usize = 2;
pub const PARAM_LAYER_MIX: usize = 3;
pub const PARAM_DEPTH: usize = 4;
pub const PARAM_COASTLINE: usize = 5;
pub const PARAM_DRIFT_SPEED: usize = 6;
pub const PARAM_BEAT_SYNC: usize = 7;
pub const PARAM_DRIFT_CHAOS: usize = 8;
pub const PARAM_WARP: usize = 9;
pub const PARAM_EDGE_WIDTH: usize = 10;
pub const PARAM_EDGE_GLOW: usize = 11;
pub const PARAM_COLOR_SHIFT: usize = 12;
pub const PARAM_COLOR_SAT: usize = 13;
pub const PARAM_IMAGE_INFLUENCE: usize = 14;
pub const PARAM_NC_KERNEL: usize = 15;
pub const PARAM_CERT_CONTRAST: usize = 16;
pub const PARAM_CERT_BRIGHTNESS: usize = 17;
pub const PARAM_BRIGHTNESS: usize = 18;
pub const PARAM_CONTRAST: usize = 19;
pub const PARAM_IMAGE_BLEND: usize = 20;

/// Beat Sync positions (FV-SYNC). Index 0 is free-run; the rest are the drift
/// period in bars. Mirrors the Subdivision idiom in delay-line-module.
const SYNC_PERIOD_BARS: [f32; 6] = [0.0, 4.0, 2.0, 1.0, 0.5, 0.25];

// Mirror plugins/voronoi-fractal/src/shaders/voronoi.defaults.json.
const DEFAULTS: [f32; NUM_PARAMS] = [
    1.00, // mode           → Fractal
    0.25, // density        → ~9
    0.17, // layer spread   → ~2.0
    0.50, // layer mix
    0.40, // depth          → 3 levels
    0.50, // coastline bias
    0.17, // drift speed    → ~0.5
    0.00, // beat sync      → Off
    0.30, // drift chaos
    0.00, // warp
    0.27, // edge width     → ~0.04
    0.40, // edge glow
    0.00, // color shift
    0.70, // color sat
    0.60, // image influence
    0.18, // nc kernel      → ~0.1
    0.18, // cert contrast  → ~1.0
    0.60, // cert brightness
    0.50, // brightness     → 1.0
    0.50, // contrast       → 1.0
    0.00, // image blend
];

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    let std = |name: &str, def: f32| SimpleParamInfo {
        name: CString::new(name).unwrap(),
        param_type: ParameterTypes::Standard,
        default: Some(def),
        ..Default::default()
    };
    [
        std("Mode", DEFAULTS[PARAM_MODE]),
        std("Density", DEFAULTS[PARAM_DENSITY]),
        std("Layer Spread", DEFAULTS[PARAM_LAYER_SPREAD]),
        std("Layer Mix", DEFAULTS[PARAM_LAYER_MIX]),
        std("Depth", DEFAULTS[PARAM_DEPTH]),
        std("Coastline Bias", DEFAULTS[PARAM_COASTLINE]),
        std("Drift Speed", DEFAULTS[PARAM_DRIFT_SPEED]),
        std("Beat Sync", DEFAULTS[PARAM_BEAT_SYNC]),
        std("Drift Chaos", DEFAULTS[PARAM_DRIFT_CHAOS]),
        std("Warp", DEFAULTS[PARAM_WARP]),
        std("Edge Width", DEFAULTS[PARAM_EDGE_WIDTH]),
        std("Edge Glow", DEFAULTS[PARAM_EDGE_GLOW]),
        std("Color Shift", DEFAULTS[PARAM_COLOR_SHIFT]),
        std("Color Sat", DEFAULTS[PARAM_COLOR_SAT]),
        std("Image Influence", DEFAULTS[PARAM_IMAGE_INFLUENCE]),
        std("NC Kernel", DEFAULTS[PARAM_NC_KERNEL]),
        std("Cert Contrast", DEFAULTS[PARAM_CERT_CONTRAST]),
        std("Cert Brightness", DEFAULTS[PARAM_CERT_BRIGHTNESS]),
        std("Brightness", DEFAULTS[PARAM_BRIGHTNESS]),
        std("Contrast", DEFAULTS[PARAM_CONTRAST]),
        std("Image Blend", DEFAULTS[PARAM_IMAGE_BLEND]),
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct VoronoiParams {
    values: [f32; NUM_PARAMS],
}

impl VoronoiParams {
    pub fn new() -> Self {
        Self { values: DEFAULTS }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = value.clamp(0.0, 1.0);
        }
    }

    /// FV-MODE: 0.0 = Layered, 1.0 = Fractal. Passed to the shader as a float
    /// flag rather than a branch selector so both paths stay in one program.
    pub fn fractal_mode(&self) -> f32 {
        if self.values[PARAM_MODE] >= 0.5 {
            1.0
        } else {
            0.0
        }
    }

    pub fn density(&self) -> f32 {
        2.0 + self.values[PARAM_DENSITY] * 28.0
    }

    pub fn layer_spread(&self) -> f32 {
        1.5 + self.values[PARAM_LAYER_SPREAD] * 2.5
    }

    pub fn layer_mix(&self) -> f32 {
        self.values[PARAM_LAYER_MIX]
    }

    /// FV-HIER: 1..6 discrete levels.
    pub fn depth(&self) -> f32 {
        (1.0 + self.values[PARAM_DEPTH] * 5.0).round()
    }

    pub fn coastline(&self) -> f32 {
        self.values[PARAM_COASTLINE]
    }

    pub fn drift_speed(&self) -> f32 {
        self.values[PARAM_DRIFT_SPEED] * 3.0
    }

    /// FV-SYNC: period in bars, or 0.0 for free-run.
    pub fn sync_period_bars(&self) -> f32 {
        let n = SYNC_PERIOD_BARS.len();
        let idx = (self.values[PARAM_BEAT_SYNC] * (n - 1) as f32).round() as usize;
        SYNC_PERIOD_BARS[idx.min(n - 1)]
    }

    pub fn drift_chaos(&self) -> f32 {
        self.values[PARAM_DRIFT_CHAOS]
    }

    pub fn warp(&self) -> f32 {
        self.values[PARAM_WARP]
    }

    pub fn edge_width(&self) -> f32 {
        self.values[PARAM_EDGE_WIDTH] * 0.15
    }

    pub fn edge_glow(&self) -> f32 {
        self.values[PARAM_EDGE_GLOW]
    }

    pub fn color_shift(&self) -> f32 {
        self.values[PARAM_COLOR_SHIFT]
    }

    pub fn color_sat(&self) -> f32 {
        self.values[PARAM_COLOR_SAT]
    }

    pub fn image_influence(&self) -> f32 {
        self.values[PARAM_IMAGE_INFLUENCE]
    }

    pub fn nc_kernel(&self) -> f32 {
        0.01 + self.values[PARAM_NC_KERNEL] * 0.49
    }

    pub fn cert_contrast(&self) -> f32 {
        0.1 + self.values[PARAM_CERT_CONTRAST] * 4.9
    }

    pub fn cert_brightness(&self) -> f32 {
        self.values[PARAM_CERT_BRIGHTNESS]
    }

    pub fn brightness(&self) -> f32 {
        self.values[PARAM_BRIGHTNESS] * 2.0
    }

    pub fn contrast(&self) -> f32 {
        self.values[PARAM_CONTRAST] * 2.0
    }

    pub fn image_blend(&self) -> f32 {
        self.values[PARAM_IMAGE_BLEND]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_positions_map_to_bar_periods() {
        let mut p = VoronoiParams::new();
        p.set(PARAM_BEAT_SYNC, 0.0);
        assert_eq!(p.sync_period_bars(), 0.0, "position 0 is free-run");
        p.set(PARAM_BEAT_SYNC, 1.0);
        assert_eq!(p.sync_period_bars(), 0.25, "top position is 1/4 bar");
        p.set(PARAM_BEAT_SYNC, 0.6);
        assert_eq!(p.sync_period_bars(), 1.0, "0.6 of 5 steps rounds to index 3");
    }

    #[test]
    fn depth_is_discrete_one_to_six() {
        let mut p = VoronoiParams::new();
        p.set(PARAM_DEPTH, 0.0);
        assert_eq!(p.depth(), 1.0);
        p.set(PARAM_DEPTH, 1.0);
        assert_eq!(p.depth(), 6.0);
    }

    #[test]
    fn mode_is_a_hard_switch_at_half() {
        let mut p = VoronoiParams::new();
        p.set(PARAM_MODE, 0.49);
        assert_eq!(p.fractal_mode(), 0.0);
        p.set(PARAM_MODE, 0.5);
        assert_eq!(p.fractal_mode(), 1.0);
    }
}
