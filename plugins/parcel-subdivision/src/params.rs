use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 12;
pub const PARAM_SCALE: usize = 0;
pub const PARAM_WARP: usize = 1;
pub const PARAM_DEPTH: usize = 2;
pub const PARAM_REGULARITY: usize = 3;
pub const PARAM_BORDER: usize = 4;
pub const PARAM_INSET: usize = 5;
pub const PARAM_JITTER: usize = 6;
pub const PARAM_WARMTH: usize = 7;
pub const PARAM_INVERT: usize = 8;
pub const PARAM_CONTRAST: usize = 9;
pub const PARAM_DRIFT_X: usize = 10;
pub const PARAM_DRIFT_Y: usize = 11;

// Mirror plugins/parcel-subdivision/src/shaders/parcel.defaults.json.
const DEFAULTS: [f32; NUM_PARAMS] = [
    0.40, // scale
    0.55, // warp
    0.60, // depth
    0.50, // regularity
    0.40, // border
    0.30, // inset
    0.20, // jitter
    0.60, // warmth
    0.00, // invert
    0.55, // contrast
    0.12, // drift x
    0.17, // drift y
];

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    let std = |name: &str, def: f32| SimpleParamInfo {
        name: CString::new(name).unwrap(),
        param_type: ParameterTypes::Standard,
        default: Some(def),
        ..Default::default()
    };
    [
        std("Scale", DEFAULTS[PARAM_SCALE]),
        std("Warp", DEFAULTS[PARAM_WARP]),
        std("Depth", DEFAULTS[PARAM_DEPTH]),
        std("Regularity", DEFAULTS[PARAM_REGULARITY]),
        std("Border", DEFAULTS[PARAM_BORDER]),
        std("Inset", DEFAULTS[PARAM_INSET]),
        std("Jitter", DEFAULTS[PARAM_JITTER]),
        std("Warmth", DEFAULTS[PARAM_WARMTH]),
        std("Invert", DEFAULTS[PARAM_INVERT]),
        std("Contrast", DEFAULTS[PARAM_CONTRAST]),
        std("Drift X", DEFAULTS[PARAM_DRIFT_X]),
        std("Drift Y", DEFAULTS[PARAM_DRIFT_Y]),
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct ParcelParams {
    values: [f32; NUM_PARAMS],
}

impl ParcelParams {
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

    pub fn scale(&self) -> f32 {
        self.values[PARAM_SCALE]
    }
    pub fn warp(&self) -> f32 {
        self.values[PARAM_WARP]
    }
    pub fn depth(&self) -> f32 {
        self.values[PARAM_DEPTH]
    }
    pub fn regularity(&self) -> f32 {
        self.values[PARAM_REGULARITY]
    }
    pub fn border(&self) -> f32 {
        self.values[PARAM_BORDER]
    }
    pub fn inset(&self) -> f32 {
        self.values[PARAM_INSET]
    }
    pub fn jitter(&self) -> f32 {
        self.values[PARAM_JITTER]
    }
    pub fn warmth(&self) -> f32 {
        self.values[PARAM_WARMTH]
    }
    pub fn invert(&self) -> f32 {
        self.values[PARAM_INVERT]
    }
    pub fn contrast(&self) -> f32 {
        self.values[PARAM_CONTRAST]
    }
    pub fn drift_x(&self) -> f32 {
        self.values[PARAM_DRIFT_X]
    }
    pub fn drift_y(&self) -> f32 {
        self.values[PARAM_DRIFT_Y]
    }
}
