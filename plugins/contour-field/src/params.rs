use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 9;
pub const PARAM_SCALE: usize = 0;
pub const PARAM_WARP: usize = 1;
pub const PARAM_CONTOURS: usize = 2;
pub const PARAM_LINE_WEIGHT: usize = 3;
pub const PARAM_ELEVATION: usize = 4;
pub const PARAM_JITTER: usize = 5;
pub const PARAM_BREAKUP: usize = 6;
pub const PARAM_WARMTH: usize = 7;
pub const PARAM_DRIFT: usize = 8;

// Defaults mirror plugins/contour-field/src/shaders/contour.defaults.json so the
// Resolume plugin and the harness agree on the shipped look.
const DEFAULTS: [f32; NUM_PARAMS] = [
    0.45, // scale
    0.55, // warp
    0.35, // contours
    0.45, // line weight
    0.50, // elevation
    0.35, // jitter
    0.20, // breakup
    0.60, // warmth
    0.15, // drift (slow geological drift; 0 = frozen)
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
        std("Contours", DEFAULTS[PARAM_CONTOURS]),
        std("Line Weight", DEFAULTS[PARAM_LINE_WEIGHT]),
        std("Elevation", DEFAULTS[PARAM_ELEVATION]),
        std("Jitter", DEFAULTS[PARAM_JITTER]),
        std("Break Up", DEFAULTS[PARAM_BREAKUP]),
        std("Warmth", DEFAULTS[PARAM_WARMTH]),
        std("Drift", DEFAULTS[PARAM_DRIFT]),
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct ContourParams {
    values: [f32; NUM_PARAMS],
}

impl ContourParams {
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

    // Aesthetic params pass straight through 0..1 — the shader maps them to real
    // ranges via mix(), so host and harness share one mapping.
    pub fn scale(&self) -> f32 {
        self.values[PARAM_SCALE]
    }
    pub fn warp(&self) -> f32 {
        self.values[PARAM_WARP]
    }
    pub fn contours(&self) -> f32 {
        self.values[PARAM_CONTOURS]
    }
    pub fn line_weight(&self) -> f32 {
        self.values[PARAM_LINE_WEIGHT]
    }
    pub fn elevation(&self) -> f32 {
        self.values[PARAM_ELEVATION]
    }
    pub fn jitter(&self) -> f32 {
        self.values[PARAM_JITTER]
    }
    pub fn breakup(&self) -> f32 {
        self.values[PARAM_BREAKUP]
    }
    pub fn warmth(&self) -> f32 {
        self.values[PARAM_WARMTH]
    }

    /// Drift 0..1 → phase advance per second. 0 freezes the terrain.
    pub fn drift(&self) -> f32 {
        self.values[PARAM_DRIFT]
    }
}
