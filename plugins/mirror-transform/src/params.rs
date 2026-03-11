use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 6;
pub const PARAM_SCALE: usize = 0;
pub const PARAM_ROTATION: usize = 1;
pub const PARAM_SWIRL: usize = 2;
pub const PARAM_MIRROR: usize = 3;
pub const PARAM_TRANSLATE_X: usize = 4;
pub const PARAM_TRANSLATE_Y: usize = 5;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Scale (0.5× to 2.0×, exponential)
        SimpleParamInfo {
            name: CString::new("Scale").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1.0×
            ..Default::default()
        },
        // 1: Rotation (-180° to +180°)
        SimpleParamInfo {
            name: CString::new("Rotation").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0°
            ..Default::default()
        },
        // 2: Swirl (-2.0 to +2.0 radians)
        SimpleParamInfo {
            name: CString::new("Swirl").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 3: Mirror (Off / On)
        SimpleParamInfo {
            name: CString::new("Mirror").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Off
            elements: Some(vec![
                (CString::new("Off").unwrap(), 0.0),
                (CString::new("On").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 4: Translate X (-1.0 to +1.0)
        SimpleParamInfo {
            name: CString::new("Translate X").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 5: Translate Y (-1.0 to +1.0)
        SimpleParamInfo {
            name: CString::new("Translate Y").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct TransformParams {
    values: [f32; NUM_PARAMS],
}

impl TransformParams {
    pub fn new() -> Self {
        Self {
            values: [0.5, 0.5, 0.5, 0.0, 0.5, 0.5],
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

    /// Scale: 0.0 → 0.5×, 0.5 → 1.0×, 1.0 → 2.0× (exponential)
    pub fn scale(&self) -> f32 {
        2.0_f32.powf(self.values[PARAM_SCALE] * 2.0 - 1.0)
    }

    /// Rotation in radians: 0.0 → -π, 0.5 → 0, 1.0 → +π
    pub fn rotation(&self) -> f32 {
        (self.values[PARAM_ROTATION] * 2.0 - 1.0) * std::f32::consts::PI
    }

    /// Swirl: 0.0 → -2.0, 0.5 → 0, 1.0 → +2.0
    pub fn swirl(&self) -> f32 {
        (self.values[PARAM_SWIRL] * 2.0 - 1.0) * 2.0
    }

    /// Mirror: true if on
    pub fn mirror(&self) -> bool {
        self.values[PARAM_MIRROR] > 0.5
    }

    /// Translate X: 0.0 → -1.0, 0.5 → 0, 1.0 → +1.0
    pub fn translate_x(&self) -> f32 {
        self.values[PARAM_TRANSLATE_X] * 2.0 - 1.0
    }

    /// Translate Y: 0.0 → -1.0, 0.5 → 0, 1.0 → +1.0
    pub fn translate_y(&self) -> f32 {
        self.values[PARAM_TRANSLATE_Y] * 2.0 - 1.0
    }
}
