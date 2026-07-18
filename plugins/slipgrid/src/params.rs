use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 9;
pub const PARAM_GRID_X: usize = 0;
pub const PARAM_GRID_Y: usize = 1;
pub const PARAM_INTENSITY: usize = 2;
pub const PARAM_LOCALITY: usize = 3;
pub const PARAM_EDGE_GRAVITY: usize = 4;
pub const PARAM_ITERATIONS: usize = 5;
pub const PARAM_MODE: usize = 6;
pub const PARAM_SEED: usize = 7;
pub const PARAM_DRY_WET: usize = 8;

const MAX_TILES: f32 = 64.0;
const MAX_LOCALITY: f32 = 16.0;
const MAX_ITERATIONS: f32 = 8.0;
const MAX_SEED: f32 = 1024.0;

// Host space is 0..1 for every param; the accessors below map to real ranges.
// Defaults: 8x8 grid, half the tiles moving, 2-tile hops, no gravity, 3 rounds,
// Conserve mode, fully wet.
const DEFAULT_GRID: f32 = (8.0 - 1.0) / (MAX_TILES - 1.0);
const DEFAULT_LOCALITY: f32 = 2.0 / MAX_LOCALITY;
const DEFAULT_ITERATIONS: f32 = (3.0 - 1.0) / (MAX_ITERATIONS - 1.0);

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        SimpleParamInfo {
            name: CString::new("Grid X").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(DEFAULT_GRID),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Grid Y").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(DEFAULT_GRID),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Intensity").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Locality").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(DEFAULT_LOCALITY),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Edge Gravity").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // bipolar: 0.5 is the neutral centre
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Iterations").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(DEFAULT_ITERATIONS),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0),
            elements: Some(vec![
                (CString::new("Conserve").unwrap(), 0.0),
                (CString::new("Smear").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Seed").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
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

pub struct SlipgridParams {
    values: [f32; NUM_PARAMS],
}

impl SlipgridParams {
    pub fn new() -> Self {
        Self {
            values: [
                DEFAULT_GRID,
                DEFAULT_GRID,
                0.5,
                DEFAULT_LOCALITY,
                0.5,
                DEFAULT_ITERATIONS,
                0.0,
                0.0,
                1.0,
            ],
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

    /// Tile grid: 0.0 → 1 tile, 1.0 → 64 tiles. Integer.
    pub fn grid(&self) -> [f32; 2] {
        [
            (1.0 + self.values[PARAM_GRID_X] * (MAX_TILES - 1.0)).round(),
            (1.0 + self.values[PARAM_GRID_Y] * (MAX_TILES - 1.0)).round(),
        ]
    }

    /// Intensity: fraction of the grid that participates. Direct 0..1.
    pub fn intensity(&self) -> f32 {
        self.values[PARAM_INTENSITY]
    }

    /// Locality: max displacement / swap distance, 0 → 16 tiles.
    pub fn locality(&self) -> f32 {
        self.values[PARAM_LOCALITY] * MAX_LOCALITY
    }

    /// Edge Gravity: bipolar. 0.5 → 0.0 (neutral), 0.0 → -1.0, 1.0 → +1.0.
    pub fn edge_gravity(&self) -> f32 {
        self.values[PARAM_EDGE_GRAVITY] * 2.0 - 1.0
    }

    /// Iterations: 1 → 8 rounds. Integer.
    pub fn iterations(&self) -> i32 {
        (1.0 + self.values[PARAM_ITERATIONS] * (MAX_ITERATIONS - 1.0)).round() as i32
    }

    /// Mode: 0 = Conserve (bijective swap), 1 = Smear (loose displacement).
    pub fn mode(&self) -> i32 {
        if self.values[PARAM_MODE] > 0.5 {
            1
        } else {
            0
        }
    }

    /// Seed: 0 → 1024. The permutation is frozen until this (or a param) changes.
    pub fn seed(&self) -> f32 {
        self.values[PARAM_SEED] * MAX_SEED
    }

    /// Dry/Wet: 0.0–1.0 direct.
    pub fn dry_wet(&self) -> f32 {
        self.values[PARAM_DRY_WET]
    }
}
