use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 11;
pub const PARAM_SCALE: usize = 0;
pub const PARAM_PERSPECTIVE: usize = 1;
pub const PARAM_ROT_X: usize = 2;
pub const PARAM_ROT_Y: usize = 3;
pub const PARAM_ROT_Z: usize = 4;
pub const PARAM_ANAMORPH: usize = 5;
pub const PARAM_SWIRL: usize = 6;
pub const PARAM_TRANSLATE_X: usize = 7;
pub const PARAM_TRANSLATE_Y: usize = 8;
pub const PARAM_EDGES: usize = 9;
pub const PARAM_FOLD: usize = 10;

// Option element values are spread across the host's 0..1 parameter range (the
// convention the other plugins here use), and decoded back to the shader's 0..3
// selector by `edges()`.
pub const EDGE_SOFT_CLIP: f32 = 0.0;
pub const EDGE_MIRROR_PLANE: f32 = 1.0 / 3.0;
pub const EDGE_MIRROR_TILE: f32 = 2.0 / 3.0;
pub const EDGE_MIRROR_BOX: f32 = 1.0;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Scale (0.5x to 2.0x, exponential)
        SimpleParamInfo {
            name: CString::new("Scale").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1.0x
            ..Default::default()
        },
        // 1: Perspective (camera distance; 0 is near-orthographic)
        SimpleParamInfo {
            name: CString::new("Perspective").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // d ~ 2.1
            ..Default::default()
        },
        // 2: Rotate X (-180 to +180 degrees)
        SimpleParamInfo {
            name: CString::new("Rotate X").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 3: Rotate Y (-180 to +180 degrees)
        SimpleParamInfo {
            name: CString::new("Rotate Y").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 4: Rotate Z (-180 to +180 degrees, rigid/aspect-correct)
        SimpleParamInfo {
            name: CString::new("Rotate Z").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 5: Anamorph Rot (-180 to +180 degrees, raw uv space)
        SimpleParamInfo {
            name: CString::new("Anamorph Rot").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 6: Swirl (-2.0 to +2.0 radians)
        SimpleParamInfo {
            name: CString::new("Swirl").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 7: Translate X (-1.0 to +1.0)
        SimpleParamInfo {
            name: CString::new("Translate X").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 8: Translate Y (-1.0 to +1.0)
        SimpleParamInfo {
            name: CString::new("Translate Y").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 0
            ..Default::default()
        },
        // 9: Edges (soft clip / three mirror folds)
        SimpleParamInfo {
            name: CString::new("Edges").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Soft Clip
            elements: Some(vec![
                (CString::new("Soft Clip").unwrap(), EDGE_SOFT_CLIP),
                (CString::new("Mirror Plane").unwrap(), EDGE_MIRROR_PLANE),
                (CString::new("Mirror Tile").unwrap(), EDGE_MIRROR_TILE),
                (CString::new("Mirror Box").unwrap(), EDGE_MIRROR_BOX),
            ]),
            ..Default::default()
        },
        // 10: Fold Tile (0.25x to 4.0x, exponential)
        SimpleParamInfo {
            name: CString::new("Fold Tile").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5), // 1.0x
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct Transform3DParams {
    values: [f32; NUM_PARAMS],
}

impl Transform3DParams {
    pub fn new() -> Self {
        Self {
            values: [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0, 0.5],
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

    /// Scale: 0.0 -> 0.5x, 0.5 -> 1.0x, 1.0 -> 2.0x (exponential)
    pub fn scale(&self) -> f32 {
        2.0_f32.powf(self.values[PARAM_SCALE] * 2.0 - 1.0)
    }

    /// Camera distance in world units, where the frame is 1.0 tall.
    ///
    /// 0.0 -> 12.6 (near-orthographic: an X tilt reads as a vertical squash with
    /// no vanishing point), 0.5 -> 2.1, 1.0 -> 0.6 (wide-angle tunnel). Cubic
    /// rather than exponential: the useful range is all at the near end.
    pub fn perspective(&self) -> f32 {
        let v = 1.0 - self.values[PARAM_PERSPECTIVE];
        0.6 + 12.0 * v * v * v
    }

    /// Rotate X in radians: 0.0 -> -pi, 0.5 -> 0, 1.0 -> +pi
    pub fn rot_x(&self) -> f32 {
        (self.values[PARAM_ROT_X] * 2.0 - 1.0) * std::f32::consts::PI
    }

    /// Rotate Y in radians: 0.0 -> -pi, 0.5 -> 0, 1.0 -> +pi
    pub fn rot_y(&self) -> f32 {
        (self.values[PARAM_ROT_Y] * 2.0 - 1.0) * std::f32::consts::PI
    }

    /// Rotate Z in radians: 0.0 -> -pi, 0.5 -> 0, 1.0 -> +pi
    pub fn rot_z(&self) -> f32 {
        (self.values[PARAM_ROT_Z] * 2.0 - 1.0) * std::f32::consts::PI
    }

    /// Anamorphic rotation in radians: 0.0 -> -pi, 0.5 -> 0, 1.0 -> +pi
    pub fn anamorph(&self) -> f32 {
        (self.values[PARAM_ANAMORPH] * 2.0 - 1.0) * std::f32::consts::PI
    }

    /// Swirl: 0.0 -> -2.0, 0.5 -> 0, 1.0 -> +2.0
    pub fn swirl(&self) -> f32 {
        (self.values[PARAM_SWIRL] * 2.0 - 1.0) * 2.0
    }

    /// Translate X: 0.0 -> -1.0, 0.5 -> 0, 1.0 -> +1.0
    pub fn translate_x(&self) -> f32 {
        self.values[PARAM_TRANSLATE_X] * 2.0 - 1.0
    }

    /// Translate Y: 0.0 -> -1.0, 0.5 -> 0, 1.0 -> +1.0
    pub fn translate_y(&self) -> f32 {
        self.values[PARAM_TRANSLATE_Y] * 2.0 - 1.0
    }

    /// Edges mode as the shader's 0..3 selector: 0 soft clip, 1 mirror plane,
    /// 2 mirror tile, 3 mirror box. Thresholds sit midway between the four
    /// element values (0, 1/3, 2/3, 1).
    pub fn edges(&self) -> f32 {
        let v = self.values[PARAM_EDGES];
        if v < 1.0 / 6.0 {
            0.0
        } else if v < 0.5 {
            1.0
        } else if v < 5.0 / 6.0 {
            2.0
        } else {
            3.0
        }
    }

    /// Mirror cell size in frames: 0.0 -> 0.25x, 0.5 -> 1.0x, 1.0 -> 4.0x
    pub fn fold(&self) -> f32 {
        2.0_f32.powf(self.values[PARAM_FOLD] * 4.0 - 2.0)
    }
}
