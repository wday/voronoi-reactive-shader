// ── params.rs ── FFGL parameter definitions for Dream Looper ──

use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const PARAM_DRY: usize = 0;
pub const PARAM_WET: usize = 1;
pub const PARAM_TAP1: usize = 2;
pub const PARAM_TAP2: usize = 3;
pub const PARAM_TAP3: usize = 4;
pub const PARAM_TAP4: usize = 5;
pub const PARAM_FEEDBACK: usize = 6;
pub const PARAM_SHIFT_X: usize = 7;
pub const PARAM_SHIFT_Y: usize = 8;
pub const PARAM_ROTATION: usize = 9;
pub const PARAM_SCALE: usize = 10;
pub const PARAM_SWIRL: usize = 11;
pub const PARAM_HUE_SHIFT: usize = 12;
pub const PARAM_SAT_SHIFT: usize = 13;
pub const PARAM_MIRROR: usize = 14;
pub const PARAM_FOLD: usize = 15;
pub const PARAM_BPM: usize = 16;
pub const PARAM_SUBDIVISION: usize = 17;
pub const NUM_PARAMS: usize = 18;

static PARAMS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        SimpleParamInfo {
            name: CString::new("Dry").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1.0 mapped
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.25), // 0.5 mapped
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Tap 1").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1.0 mapped (0..1 → 0..2)
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Tap 2").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.35), // 0.7 mapped
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Tap 3").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.2), // 0.4 mapped
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Tap 4").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.1), // 0.2 mapped
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Feedback").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.85),
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
            name: CString::new("Swirl").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // center = no swirl
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
        // Defaults match the PARAMS declarations above
        Self {
            values: [
                0.5,   // dry
                0.25,  // wet
                0.5,   // tap 1
                0.35,  // tap 2
                0.2,   // tap 3
                0.1,   // tap 4
                0.85,  // feedback
                0.5,   // shift x
                0.5,   // shift y
                0.5,   // rotation
                0.5,   // scale
                0.5,   // swirl
                0.5,   // hue shift
                0.5,   // sat shift
                0.0,   // mirror
                1.0,   // fold
                0.467, // bpm
                0.5,   // subdivision
            ],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        self.values[index] = value;
    }

    /// Dry level (0..2). Live signal volume.
    pub fn dry(&self) -> f32 {
        self.values[PARAM_DRY] * 2.0
    }

    /// Wet level (0..2). Combined echo tap volume.
    pub fn wet(&self) -> f32 {
        self.values[PARAM_WET] * 2.0
    }

    /// Per-tier tap levels [T0, T1, T2, T3]. Each 0..2 (overdrivable).
    pub fn tap_levels(&self) -> [f32; 4] {
        [
            self.values[PARAM_TAP1] * 2.0,
            self.values[PARAM_TAP2] * 2.0,
            self.values[PARAM_TAP3] * 2.0,
            self.values[PARAM_TAP4] * 2.0,
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
    pub fn scale(&self) -> f32 {
        2.0_f32.powf(self.values[PARAM_SCALE] * 2.0 - 1.0)
    }

    /// Hue shift per feedback iteration. 0..1 param maps to ±0.5 hue (±180°).
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
