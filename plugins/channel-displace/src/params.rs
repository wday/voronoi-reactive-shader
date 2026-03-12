use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 4;
pub const PARAM_AMOUNT: usize = 0;
pub const PARAM_PATTERN: usize = 1;
pub const PARAM_ANGLE: usize = 2;
pub const PARAM_DRY_WET: usize = 3;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Amount (coupling strength, 0–10% UV)
        SimpleParamInfo {
            name: CString::new("Amount").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
        // 1: Pattern (Cyclic / Mutual)
        SimpleParamInfo {
            name: CString::new("Pattern").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Cyclic
            elements: Some(vec![
                (CString::new("Cyclic").unwrap(), 0.0),
                (CString::new("Mutual").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 2: Angle (displacement direction, 0–360°)
        SimpleParamInfo {
            name: CString::new("Angle").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
        // 3: Dry/Wet
        SimpleParamInfo {
            name: CString::new("Dry/Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct DisplaceParams {
    values: [f32; NUM_PARAMS],
}

impl DisplaceParams {
    pub fn new() -> Self {
        Self {
            values: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = value.clamp(0.0, 1.0);
        }
    }

    /// Amount: 0.0 → 0.0, 1.0 → 0.1 (10% UV space)
    pub fn amount(&self) -> f32 {
        self.values[PARAM_AMOUNT] * 0.1
    }

    /// Pattern: 0 = cyclic (R←G, G←B, B←R), 1 = mutual (each ← avg of others)
    pub fn pattern(&self) -> i32 {
        if self.values[PARAM_PATTERN] > 0.5 { 1 } else { 0 }
    }

    /// Angle in radians: 0.0 → 0, 1.0 → 2π
    pub fn angle(&self) -> f32 {
        self.values[PARAM_ANGLE] * std::f32::consts::TAU
    }

    /// Dry/Wet: 0.0–1.0 direct
    pub fn dry_wet(&self) -> f32 {
        self.values[PARAM_DRY_WET]
    }
}
