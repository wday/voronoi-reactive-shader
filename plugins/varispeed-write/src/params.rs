use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 2;
pub const PARAM_CHANNEL: usize = 0;
pub const PARAM_SEND: usize = 1;

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Channel — the patch point. Write + Read on the same Channel form a
        //    loop over one tape. At most ONE Write per channel: `vc_write_tick`
        //    de-dups on frame id, so a second Write in the same host frame gets the
        //    same record_index back and overwrites the first (VS-MULTITAP).
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
        // 1: Send — record/dub level: how much of the current frame commits into
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
        Self {
            values: [
                0.0, // Channel: 1
                1.0, // Send: full record
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

    /// Tape channel index. Write + Read on the same channel form a loop.
    pub fn channel(&self) -> u32 {
        if self.values[PARAM_CHANNEL] < 0.5 { 0 } else { 1 }
    }

    /// Record/dub level. `0` = freeze (stop recording, keep the buffer).
    pub fn send(&self) -> f32 {
        self.values[PARAM_SEND]
    }
}
