// ── params.rs ── FFGL parameter definitions for Dream Looper ──

use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const PARAM_TRAIL_LENGTH: usize = 0;
pub const PARAM_TRAIL_OPACITY: usize = 1;
pub const NUM_PARAMS: usize = 2;

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
            values: [0.5, 0.5],
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
}
