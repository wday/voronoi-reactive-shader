use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 1;
pub const PARAM_SEND: usize = 0;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Send — record/dub level: how much of the current frame commits into
        //    the tape (`tape = Send*input`). **Send = 0 FREEZES** the loop: the
        //    Write stops advancing the record cursor and keeps the buffer, so the
        //    Read plays the captured loop back (varispeed). Not a wipe — the buffer
        //    is held, not cleared. Above 0 it records (dub level).
        SimpleParamInfo {
            name: CString::new("Send").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

pub struct WriteParams {
    values: [f32; NUM_PARAMS],
}

impl WriteParams {
    pub fn new() -> Self {
        Self { values: [1.0] }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = value.clamp(0.0, 1.0);
        }
    }

    /// Record/dub level. `0` = freeze (stop recording, keep the buffer).
    pub fn send(&self) -> f32 {
        self.values[PARAM_SEND]
    }
}
