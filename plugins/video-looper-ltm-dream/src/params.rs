// ── params.rs ── FFGL parameter definitions for Dream Looper ──

use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const PARAM_TRAIL_LENGTH: usize = 0;
pub const PARAM_TRAIL_OPACITY: usize = 1;
pub const PARAM_WEIGHT_T0: usize = 2;
pub const PARAM_WEIGHT_T1: usize = 3;
pub const PARAM_WEIGHT_T2: usize = 4;
pub const PARAM_WEIGHT_T3: usize = 5;
pub const PARAM_SHIFT_X: usize = 6;
pub const PARAM_SHIFT_Y: usize = 7;
pub const PARAM_FEEDBACK: usize = 8;
pub const PARAM_ROTATION: usize = 9;
pub const PARAM_SCALE: usize = 10;
pub const PARAM_HUE_SHIFT: usize = 11;
pub const PARAM_SAT_SHIFT: usize = 12;
pub const PARAM_SWIRL: usize = 13;
pub const PARAM_MIRROR: usize = 14;
pub const PARAM_FOLD: usize = 15;
pub const PARAM_BPM: usize = 16;
pub const PARAM_SUBDIVISION: usize = 17;
pub const NUM_PARAMS: usize = 18;

static PARAMS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        SimpleParamInfo {
            name: CString::new("Trail Length").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Trail Opacity").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Weight T0").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.6),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Weight T1").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.3),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Weight T2").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.15),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Weight T3").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.08),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Shift X").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Shift Y").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Feedback").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.85),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Rotation").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = no rotation
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Scale").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = 1.0x (no zoom)
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Hue Shift").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = no shift
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Sat Shift").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = no shift
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Swirl").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = no swirl
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Mirror").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0), // off
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Fold").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0), // 1.0 = off (threshold at 1.0)
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("BPM").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.467), // 120 BPM (range 50-200)
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Subdivision").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1/4 note
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAMS[index]
}

pub struct DreamParams {
    values: [f32; NUM_PARAMS],
}

impl DreamParams {
    pub fn new() -> Self {
        Self {
            values: [0.5, 0.5, 0.6, 0.3, 0.15, 0.08, 0.5, 0.5, 0.85, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0, 1.0, 0.467, 0.5],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        self.values[index] = value;
    }

    /// How many tiers to sample (1-4). Knob 0.0 = 1 tier, 1.0 = all 4.
    pub fn active_tiers(&self) -> usize {
        let v = self.values[PARAM_TRAIL_LENGTH];
        ((v * 4.0).ceil() as usize).clamp(1, 4)
    }

    /// How far into each tier's history to reach (0..1 fraction of depth).
    /// Same knob as active_tiers — higher = more tiers AND deeper reach.
    pub fn trail_length(&self) -> f32 {
        self.values[PARAM_TRAIL_LENGTH]
    }

    /// Overall opacity weight for the trail compositing.
    pub fn trail_opacity(&self) -> f32 {
        self.values[PARAM_TRAIL_OPACITY]
    }

    /// Per-tier tap weights [T0, T1, T2, T3]. Each 0..1.
    pub fn tier_weights(&self) -> [f32; 4] {
        [
            self.values[PARAM_WEIGHT_T0],
            self.values[PARAM_WEIGHT_T1],
            self.values[PARAM_WEIGHT_T2],
            self.values[PARAM_WEIGHT_T3],
        ]
    }

    /// UV shift applied per feedback iteration.
    /// Returns (x, y) mapped from 0..1 param to ±0.5 UV offset.
    pub fn shift(&self) -> (f32, f32) {
        let x = (self.values[PARAM_SHIFT_X] - 0.5) * 1.0;
        let y = (self.values[PARAM_SHIFT_Y] - 0.5) * 1.0;
        (x, y)
    }

    /// Recursive feedback decay (0..1). Each echo is multiplied by this.
    pub fn feedback(&self) -> f32 {
        self.values[PARAM_FEEDBACK]
    }

    /// Z rotation per feedback iteration in radians.
    /// 0..1 param maps to ±π (±180° per echo).
    pub fn rotation(&self) -> f32 {
        (self.values[PARAM_ROTATION] - 0.5) * std::f32::consts::TAU
    }

    /// Scale per feedback iteration. Exponential: 0→0.5x, 0.5→1.0x, 1.0→2.0x.
    /// ~0.71 param gives 1/φ ≈ 1.618x (golden ratio zoom).
    pub fn scale(&self) -> f32 {
        2.0_f32.powf(self.values[PARAM_SCALE] * 2.0 - 1.0)
    }

    /// Hue shift per feedback iteration. 0..1 param maps to ±0.5 hue (±180°).
    /// Center = no shift. Full range cycles through entire spectrum per echo.
    pub fn hue_shift(&self) -> f32 {
        (self.values[PARAM_HUE_SHIFT] - 0.5) * 1.0
    }

    /// Saturation shift per feedback iteration. ±0.5 in HSV saturation.
    pub fn sat_shift(&self) -> f32 {
        (self.values[PARAM_SAT_SHIFT] - 0.5) * 1.0
    }

    /// Swirl: angular displacement proportional to distance from center.
    /// 0..1 param maps to ±2.0 radians max swirl.
    pub fn swirl(&self) -> f32 {
        (self.values[PARAM_SWIRL] - 0.5) * 4.0
    }

    /// Mirror mode: >0.5 = reflect at edges instead of clip to black.
    pub fn mirror(&self) -> bool {
        self.values[PARAM_MIRROR] > 0.5
    }

    /// Fold threshold: luminance above this folds back (triangle wave).
    /// 0..1 param maps to 0.1..1.0. At 1.0 = no folding (normal clamp).
    pub fn fold_threshold(&self) -> f32 {
        0.1 + self.values[PARAM_FOLD] * 0.9
    }

    /// BPM mapped from 0..1 param to 50..200.
    pub fn bpm(&self) -> f32 {
        50.0 + self.values[PARAM_BPM] * 150.0
    }

    /// Beat subdivision as a multiplier in beats.
    /// 0.25 = 1/16 note, 0.5 = 1/8, 1.0 = 1/4, 2.0 = 1/2, 4.0 = 1 measure.
    pub fn subdivision_beats(&self) -> f32 {
        let v = self.values[PARAM_SUBDIVISION];
        if v < 0.2 {
            0.25
        } else if v < 0.4 {
            0.5
        } else if v < 0.6 {
            1.0
        } else if v < 0.8 {
            2.0
        } else {
            4.0
        }
    }

    /// Delay in frames for the current BPM and subdivision (at 60 FPS).
    pub fn delay_frames(&self) -> u32 {
        let seconds = (60.0 / self.bpm()) * self.subdivision_beats();
        (seconds * 60.0).round() as u32
    }
}
