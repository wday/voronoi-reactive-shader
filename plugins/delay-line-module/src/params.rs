use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 4;
pub const PARAM_MODE: usize = 0;
pub const PARAM_CHANNEL: usize = 1;
pub const PARAM_SUBDIVISION: usize = 2;
pub const PARAM_FEEDBACK: usize = 3;

/// Subdivision options: (label, beats)
const SUBDIVISIONS: [(& str, f32); 7] = [
    ("1/16",   0.25),
    ("1/8",    0.5),
    ("1/4",    1.0),
    ("1/2",    2.0),
    ("1 bar",  4.0),
    ("2 bars", 8.0),
    ("4 bars", 16.0),
];

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Mode
        SimpleParamInfo {
            name: CString::new("Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Receive
            elements: Some(vec![
                (CString::new("Receive").unwrap(), 0.0),
                (CString::new("Send").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 1: Channel
        SimpleParamInfo {
            name: CString::new("Channel").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Channel 1
            elements: Some(vec![
                (CString::new("1").unwrap(), 0.0),
                (CString::new("2").unwrap(), 1.0 / 3.0),
                (CString::new("3").unwrap(), 2.0 / 3.0),
                (CString::new("4").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 2: Subdivision (Receive only)
        SimpleParamInfo {
            name: CString::new("Subdivision").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(2.0 / 6.0), // 1/4 note
            elements: Some(
                SUBDIVISIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| {
                        (CString::new(*name).unwrap(), i as f32 / 6.0)
                    })
                    .collect(),
            ),
            ..Default::default()
        },
        // 3: Feedback (Receive only)
        SimpleParamInfo {
            name: CString::new("Feedback").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Receive,
    Send,
}

pub struct DelayParams {
    values: [f32; NUM_PARAMS],
}

impl DelayParams {
    pub fn new() -> Self {
        Self {
            values: [0.0, 0.0, 2.0 / 6.0, 0.5],
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

    pub fn mode(&self) -> Mode {
        if self.values[PARAM_MODE] > 0.5 { Mode::Send } else { Mode::Receive }
    }

    pub fn channel(&self) -> usize {
        let v = self.values[PARAM_CHANNEL];
        ((v * 4.0).min(3.0)) as usize
    }

    pub fn subdivision_beats(&self) -> f32 {
        let v = self.values[PARAM_SUBDIVISION];
        let count = SUBDIVISIONS.len() as f32;
        let index = (v * count).min(count - 1.0) as usize;
        SUBDIVISIONS[index].1
    }

    pub fn feedback(&self) -> f32 {
        self.values[PARAM_FEEDBACK]
    }
}
