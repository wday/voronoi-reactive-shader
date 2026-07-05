use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 4;
pub const PARAM_CHANNEL: usize = 0;
pub const PARAM_TAP_OFFSET: usize = 1;
pub const PARAM_MULTI_TAP: usize = 2;
pub const PARAM_BUFFER_MIX: usize = 3;

const MAX_TAPS: u32 = 8;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Channel
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
        // 1: Tap Offset (0 = newest .. 1 = oldest, across the loop)
        SimpleParamInfo {
            name: CString::new("Tap Offset").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
        // 2: Multi-tap count (Stage 5 — currently single-tap; param reserved)
        SimpleParamInfo {
            name: CString::new("Multi-tap").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(1.0),
            min: Some(1.0),
            max: Some(MAX_TAPS as f32),
            ..Default::default()
        },
        // 3: Buffer Mix (output dry/wet: dry = own input, wet = tapped buffer)
        SimpleParamInfo {
            name: CString::new("Buffer Mix").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
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
                1.0, // Tap Offset: oldest
                1.0, // Multi-tap: 1
                1.0, // Buffer Mix: full wet
            ],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = match index {
                PARAM_MULTI_TAP => value.clamp(1.0, MAX_TAPS as f32),
                _ => value.clamp(0.0, 1.0),
            };
        }
    }

    pub fn channel(&self) -> usize {
        if self.values[PARAM_CHANNEL] < 0.5 { 0 } else { 1 }
    }

    pub fn tap_offset(&self) -> f32 {
        self.values[PARAM_TAP_OFFSET]
    }

    #[allow(dead_code)] // wired for Stage 5 (multi-tap)
    pub fn multi_tap(&self) -> u32 {
        self.values[PARAM_MULTI_TAP].round() as u32
    }

    pub fn buffer_mix(&self) -> f32 {
        self.values[PARAM_BUFFER_MIX]
    }
}
