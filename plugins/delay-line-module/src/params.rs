use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 7;
pub const PARAM_MODE: usize = 0;
pub const PARAM_CHANNEL: usize = 1;
pub const PARAM_SYNC_MODE: usize = 2;
pub const PARAM_SUBDIVISION: usize = 3;
pub const PARAM_DELAY_MS: usize = 4;
pub const PARAM_DELAY_FRAMES: usize = 5;
pub const PARAM_DECAY: usize = 6;

/// Subdivision options: (label, beats)
const SUBDIVISIONS: [(&str, f32); 7] = [
    ("1/16",   0.25),
    ("1/8",    0.5),
    ("1/4",    1.0),
    ("1/2",    2.0),
    ("1 bar",  4.0),
    ("2 bars", 8.0),
    ("4 bars", 16.0),
];

const MAX_DELAY_MS: f32 = 4000.0;
const MAX_DELAY_FRAMES: u32 = 239;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Mode
        SimpleParamInfo {
            name: CString::new("Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Read
            elements: Some(vec![
                (CString::new("Read").unwrap(), 0.0),
                (CString::new("Write").unwrap(), 1.0),
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
                (CString::new("2").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 2: Sync Mode
        SimpleParamInfo {
            name: CString::new("Sync Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0), // Subdivision
            elements: Some(vec![
                (CString::new("Subdivision").unwrap(), 0.0),
                (CString::new("Ms").unwrap(), 0.5),
                (CString::new("Frames").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 3: Subdivision
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
        // 4: Delay Ms (integer, 1–4000)
        SimpleParamInfo {
            name: CString::new("Delay Ms").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(500.0),
            min: Some(1.0),
            max: Some(MAX_DELAY_MS),
            ..Default::default()
        },
        // 5: Delay Frames (integer, 1–239)
        SimpleParamInfo {
            name: CString::new("Delay Frames").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(30.0),
            min: Some(1.0),
            max: Some(MAX_DELAY_FRAMES as f32),
            ..Default::default()
        },
        // 6: Decay (Write only — controls previous-iteration survival)
        SimpleParamInfo {
            name: CString::new("Decay").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Read,
    Write,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SyncMode {
    Subdivision,
    Ms,
    Frames,
}

pub struct DelayParams {
    values: [f32; NUM_PARAMS],
}

impl DelayParams {
    pub fn new() -> Self {
        Self {
            values: [
                0.0,    // Mode: Read
                0.0,    // Channel: 1
                0.0,    // Sync Mode: Subdivision
                2.0 / 6.0, // Subdivision: 1/4
                500.0,  // Delay Ms: 500ms (actual value)
                30.0,   // Delay Frames: 30 (actual value)
                0.0,    // Decay
            ],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = match index {
                PARAM_DELAY_MS => value.clamp(1.0, MAX_DELAY_MS),
                PARAM_DELAY_FRAMES => value.clamp(1.0, MAX_DELAY_FRAMES as f32),
                _ => value.clamp(0.0, 1.0),
            };
        }
    }

    pub fn mode(&self) -> Mode {
        if self.values[PARAM_MODE] < 0.5 {
            Mode::Read
        } else {
            Mode::Write
        }
    }

    pub fn channel(&self) -> usize {
        let v = self.values[PARAM_CHANNEL];
        if v < 0.5 { 0 } else { 1 }
    }

    pub fn sync_mode(&self) -> SyncMode {
        let v = self.values[PARAM_SYNC_MODE];
        if v < 0.33 {
            SyncMode::Subdivision
        } else if v < 0.67 {
            SyncMode::Ms
        } else {
            SyncMode::Frames
        }
    }

    pub fn subdivision_beats(&self) -> f32 {
        let v = self.values[PARAM_SUBDIVISION];
        let count = SUBDIVISIONS.len() as f32;
        let index = (v * count).min(count - 1.0) as usize;
        SUBDIVISIONS[index].1
    }

    pub fn delay_ms(&self) -> f32 {
        self.values[PARAM_DELAY_MS]
    }

    pub fn delay_frames_raw(&self) -> u32 {
        self.values[PARAM_DELAY_FRAMES].round() as u32
    }

    pub fn decay(&self) -> f32 {
        // Fourth-root curve: spreads the useful dub-echo range (0.90–0.99)
        // across most of the knob instead of cramming it into the last 10%.
        self.values[PARAM_DECAY].powf(0.25)
    }
}
