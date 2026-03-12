use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 4;
pub const PARAM_R: usize = 0;
pub const PARAM_SENSITIVITY: usize = 1;
pub const PARAM_SPATIAL_MODE: usize = 2;
pub const PARAM_DRY_WET: usize = 3;

pub const MODE_OFF: f32 = 0.0;
pub const MODE_RADIAL: f32 = 0.5;
pub const MODE_EDGE: f32 = 1.0;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: R (bifurcation parameter, 0.0–4.0)
        SimpleParamInfo {
            name: CString::new("R").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.75), // 3.0
            ..Default::default()
        },
        // 1: Sensitivity (spatial r modulation strength)
        SimpleParamInfo {
            name: CString::new("Sensitivity").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
        // 2: Spatial Mode (Off / Radial / Edge)
        SimpleParamInfo {
            name: CString::new("Spatial Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Off
            elements: Some(vec![
                (CString::new("Off").unwrap(), MODE_OFF),
                (CString::new("Radial").unwrap(), MODE_RADIAL),
                (CString::new("Edge").unwrap(), MODE_EDGE),
            ]),
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

pub struct LogisticParams {
    values: [f32; NUM_PARAMS],
}

impl LogisticParams {
    pub fn new() -> Self {
        Self {
            values: [0.75, 0.0, 0.0, 1.0],
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

    /// R: 0.0 → 0.0, 0.75 → 3.0, 1.0 → 4.0
    pub fn r(&self) -> f32 {
        self.values[PARAM_R] * 4.0
    }

    /// Sensitivity: 0.0–1.0 direct
    pub fn sensitivity(&self) -> f32 {
        self.values[PARAM_SENSITIVITY]
    }

    /// Spatial mode: 0 = off, 1 = radial, 2 = edge
    pub fn spatial_mode(&self) -> i32 {
        let v = self.values[PARAM_SPATIAL_MODE];
        if v > 0.75 {
            2 // edge
        } else if v > 0.25 {
            1 // radial
        } else {
            0 // off
        }
    }

    /// Dry/Wet: 0.0–1.0 direct
    pub fn dry_wet(&self) -> f32 {
        self.values[PARAM_DRY_WET]
    }
}
