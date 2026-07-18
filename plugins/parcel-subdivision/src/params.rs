use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 9;
pub const PARAM_SCALE: usize = 0;
pub const PARAM_WARP: usize = 1;
pub const PARAM_DEPTH: usize = 2;
pub const PARAM_REGULARITY: usize = 3;
pub const PARAM_BORDER: usize = 4;
pub const PARAM_INSET: usize = 5;
pub const PARAM_JITTER: usize = 6;
pub const PARAM_WARMTH: usize = 7;
pub const PARAM_DRIFT: usize = 8;

// Mirror plugins/parcel-subdivision/src/shaders/parcel.defaults.json.
const DEFAULTS: [f32; NUM_PARAMS] = [
    0.40, // scale (shares P1/P2 terrain space)
    0.55, // warp
    0.60, // depth (subdivision levels; lowlands go deeper)
    0.50, // regularity (0 regular ↔ 1 irregular)
    0.40, // border weight
    0.30, // inset (parcel gap)
    0.20, // jitter (border wobble)
    0.60, // warmth
    0.15, // drift (0 = frozen)
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
        std("Drift", DEFAULTS[PARAM_DRIFT]),
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

    /// Drift 0..1 → phase advance per second. 0 freezes the terrain.
    pub fn drift(&self) -> f32 {
        self.values[PARAM_DRIFT]
    }
}
