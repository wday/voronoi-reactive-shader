use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 4;
pub const PARAM_CHANNEL: usize = 0;
pub const PARAM_DRY: usize = 1;
pub const PARAM_WET: usize = 2;
pub const PARAM_BLEND: usize = 3;

/// Blend-space exponent for Linear mode — a cheap ~sRGB stand-in (see the
/// `Blend Space` param and `output.frag.glsl`). Perceptual mode uses 1.0.
pub const LINEAR_GAMMA: f32 = 2.2;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Channel — the patch point. Tap + Write on the same Channel form a loop.
        SimpleParamInfo {
            name: CString::new("Channel").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0),
            elements: Some(vec![
                (CString::new("1").unwrap(), 0.0),
                (CString::new("2").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 1: Dry — gain on the live source carried forward into the FX chain.
        //    This is the source's path INTO the FX stack: at 100% effect opacity
        //    the host does no mixing, so the source only survives if the Tap
        //    carries it. Dry=0 = pure recirculating feedback (no new material).
        SimpleParamInfo {
            name: CString::new("Dry").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
        // 2: Wet — gain on the delayed tape read (one full lap back). Feedback
        //    that gets re-processed by the downstream FX each lap. Hold high and
        //    pulse the Write's Send for dub echoes. Additive with Dry, not a
        //    crossfade, so the two are independent.
        SimpleParamInfo {
            name: CString::new("Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        // 3: Blend Space — the colour space the Dry/Wet sum happens in.
        //    Linear (default): decode to ~linear light, blend + clamp as real
        //    light, re-encode — cleaner colour mixing and additive highlight
        //    bloom. Perceptual: blend the sRGB-encoded values directly — the
        //    per-lap fade reads perceptually even (video-mixer look), and is the
        //    exact legacy behaviour. See output.frag.glsl (u_gamma).
        SimpleParamInfo {
            name: CString::new("Blend Space").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(1.0),
            elements: Some(vec![
                (CString::new("Perceptual").unwrap(), 0.0),
                (CString::new("Linear").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct TapParams {
    values: [f32; NUM_PARAMS],
}

impl TapParams {
    pub fn new() -> Self {
        Self {
            values: [
                0.0, // Channel: 1
                1.0, // Dry: full source
                0.5, // Wet: half feedback
                1.0, // Blend Space: Linear (default)
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

    pub fn channel(&self) -> usize {
        if self.values[PARAM_CHANNEL] < 0.5 { 0 } else { 1 }
    }

    pub fn dry(&self) -> f32 {
        self.values[PARAM_DRY]
    }

    pub fn wet(&self) -> f32 {
        self.values[PARAM_WET]
    }

    /// Blend-space exponent for the output pass: 1.0 = Perceptual (blend the
    /// encoded values directly, legacy), LINEAR_GAMMA = Linear light.
    pub fn gamma(&self) -> f32 {
        if self.values[PARAM_BLEND] < 0.5 { 1.0 } else { LINEAR_GAMMA }
    }
}
