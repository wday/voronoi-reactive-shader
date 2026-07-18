use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 11;
pub const PARAM_SYNC_MODE: usize = 0;
pub const PARAM_SUBDIVISION: usize = 1;
pub const PARAM_LOOP_MS: usize = 2;
pub const PARAM_LOOP_FRAMES: usize = 3;
pub const PARAM_RATE: usize = 4;
pub const PARAM_DRY: usize = 5;
pub const PARAM_WET: usize = 6;
pub const PARAM_BLEND: usize = 7;
pub const PARAM_WARP_DEPTH: usize = 8;
pub const PARAM_WARP_RATE: usize = 9;
pub const PARAM_CONFINE: usize = 10;

/// Loop-length subdivision options: (label, beats).
const SUBDIVISIONS: [(&str, f32); 7] = [
    ("1/16", 0.25),
    ("1/8", 0.5),
    ("1/4", 1.0),
    ("1/2", 2.0),
    ("1 bar", 4.0),
    ("2 bars", 8.0),
    ("4 bars", 16.0),
];

/// Rate (playback speed) options: (label, ratio). Beat-ratio so warped/reversed
/// loops stay on the grid. 0 = freeze-frame, negatives = reverse.
const RATES: [(&str, f32); 7] = [
    ("-2x", -2.0),
    ("-1x", -1.0),
    ("-1/2x", -0.5),
    ("Freeze", 0.0),
    ("1/2x", 0.5),
    ("1x", 1.0),
    ("2x", 2.0),
];

/// Warp (Doppler) LFO period options: (label, beats).
const WARP_RATES: [(&str, f32); 5] = [
    ("1 bar", 4.0),
    ("1/2", 2.0),
    ("1/4", 1.0),
    ("1/8", 0.5),
    ("1/16", 0.25),
];

const MAX_LOOP_MS: f32 = 4000.0;
const MAX_LOOP_FRAMES: u32 = 239;
/// Max Doppler swing at Warp Depth = 1, in frames.
const MAX_WARP_FRAMES: f32 = 30.0;

fn option_elements<const N: usize>(items: &[(&str, f32); N]) -> Vec<(CString, f32)> {
    items
        .iter()
        .enumerate()
        .map(|(i, (name, _))| (CString::new(*name).unwrap(), i as f32 / (N as f32 - 1.0)))
        .collect()
}

static PARAM_INFOS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        // 0: Sync Mode — how Loop Length is specified.
        SimpleParamInfo {
            name: CString::new("Sync Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.0),
            elements: Some(vec![
                (CString::new("Subdivision").unwrap(), 0.0),
                (CString::new("Ms").unwrap(), 0.5),
                (CString::new("Frames").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
        // 1: Subdivision (Loop Length in beats).
        SimpleParamInfo {
            name: CString::new("Subdivision").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(2.0 / 6.0), // 1/4
            elements: Some(option_elements(&SUBDIVISIONS)),
            ..Default::default()
        },
        // 2: Loop Ms.
        SimpleParamInfo {
            name: CString::new("Loop Ms").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(500.0),
            min: Some(1.0),
            max: Some(MAX_LOOP_MS),
            ..Default::default()
        },
        // 3: Loop Frames.
        SimpleParamInfo {
            name: CString::new("Loop Frames").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(60.0),
            min: Some(1.0),
            max: Some(MAX_LOOP_FRAMES as f32),
            ..Default::default()
        },
        // 4: Rate — playback speed/direction (beat-ratio). Freeze=freeze-frame,
        //    negatives=reverse. Independent of the Write's Send=0 loop-freeze.
        SimpleParamInfo {
            name: CString::new("Rate").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(5.0 / 6.0), // 1x
            elements: Some(option_elements(&RATES)),
            ..Default::default()
        },
        // 5: Dry — gain on the live source into the FX chain.
        SimpleParamInfo {
            name: CString::new("Dry").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
        // 6: Wet — gain on the played-back loop.
        SimpleParamInfo {
            name: CString::new("Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        // 7: Blend Space — Linear (default) or Perceptual, as Delay Tap.
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
        // 8: Warp Depth — Doppler swing (0 = off).
        SimpleParamInfo {
            name: CString::new("Warp Depth").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
        // 9: Warp Rate — Doppler LFO period (beat-synced, bar-phase-locked).
        SimpleParamInfo {
            name: CString::new("Warp Rate").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(2.0 / 4.0), // 1/4
            elements: Some(option_elements(&WARP_RATES)),
            ..Default::default()
        },
        // 10: Confine — how the loop relates to the tape.
        //     Confined (default): the read plays a FIXED [0, Loop] window and the
        //     Write records the FX'd output back into the slot just read → feedback
        //     recirculates in place at Send*Wet per lap, so loops decay (or, at
        //     Send*Wet >= 1, sustain/overdub) at ANY Rate. No ring-lap reset.
        //     Free: the read floats over the full ring anchored to the live write
        //     cursor (free-floating varispeed scrub). Because the read and write
        //     cursors drift apart at Rate != 1, feedback is NOT an in-place
        //     Send*Wet decay there — old ring content replays until the write
        //     cursor laps the whole tape. Use Free for scrub/playback.
        //     Reverse (ping-pong block): two Loop-length blocks in the ring. One
        //     records the live signal FORWARD (1x metronome) while the other is
        //     frozen and played at Rate (-1x = classic reverse delay; the Rate knob
        //     also gives reverse-fast/slow and forward block delay). The blocks
        //     swap every Loop frames, so reverse feedback runs continuously WITHOUT
        //     a manual freeze — at the inherent one-block latency. Feedback (a
        //     decaying reversed tail) rides the normal Read->FX->Write chain.
        SimpleParamInfo {
            name: CString::new("Confine").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(0.5), // Confined — the in-place recirculating feedback path
            elements: Some(vec![
                (CString::new("Free").unwrap(), 0.0),
                (CString::new("Confined").unwrap(), 0.5),
                (CString::new("Reverse").unwrap(), 1.0),
            ]),
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAM_INFOS[index]
}

#[derive(Clone, Copy, PartialEq)]
pub enum SyncMode {
    Subdivision,
    Ms,
    Frames,
}

/// How the read relates to the tape (Confine param). See the param-10 doc.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoopMode {
    /// Free-floating full-ring scrub (age model).
    Free,
    /// Fixed [0, Loop) window, in-place recirculating feedback.
    Confined,
    /// Ping-pong block reverse: record-forward block + frozen block played at Rate.
    Reverse,
}

/// Pick the nearest option index for an Option value encoded as `i/(n-1)`.
fn option_index(value: f32, n: usize) -> usize {
    ((value * (n as f32 - 1.0)).round() as usize).min(n - 1)
}

pub struct ReadParams {
    values: [f32; NUM_PARAMS],
}

impl ReadParams {
    pub fn new() -> Self {
        Self {
            values: [
                0.0,       // Sync Mode: Subdivision
                2.0 / 6.0, // Subdivision: 1/4
                500.0,     // Loop Ms
                60.0,      // Loop Frames
                5.0 / 6.0, // Rate: 1x
                1.0,       // Dry
                0.5,       // Wet
                1.0,       // Blend Space: Linear
                0.0,       // Warp Depth
                2.0 / 4.0, // Warp Rate: 1/4
                0.5,       // Confine: Confined (in-place decaying feedback, default)
            ],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        if index < NUM_PARAMS {
            self.values[index] = match index {
                PARAM_LOOP_MS => value.clamp(1.0, MAX_LOOP_MS),
                PARAM_LOOP_FRAMES => value.clamp(1.0, MAX_LOOP_FRAMES as f32),
                _ => value.clamp(0.0, 1.0),
            };
        }
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
        SUBDIVISIONS[option_index(self.values[PARAM_SUBDIVISION], SUBDIVISIONS.len())].1
    }

    pub fn loop_ms(&self) -> f32 {
        self.values[PARAM_LOOP_MS]
    }

    pub fn loop_frames_raw(&self) -> u32 {
        self.values[PARAM_LOOP_FRAMES].round() as u32
    }

    /// Playback rate ratio (signed; 0 = freeze-frame, <0 = reverse).
    pub fn rate(&self) -> f32 {
        RATES[option_index(self.values[PARAM_RATE], RATES.len())].1
    }

    pub fn dry(&self) -> f32 {
        self.values[PARAM_DRY]
    }

    pub fn wet(&self) -> f32 {
        self.values[PARAM_WET]
    }

    /// Blend-space exponent: 1.0 = Perceptual, 2.2 = Linear.
    pub fn gamma(&self) -> f32 {
        if self.values[PARAM_BLEND] < 0.5 { 1.0 } else { 2.2 }
    }

    /// Doppler swing in frames (0 = off).
    pub fn warp_depth_frames(&self) -> f32 {
        self.values[PARAM_WARP_DEPTH] * MAX_WARP_FRAMES
    }

    /// Doppler LFO period in beats.
    pub fn warp_rate_beats(&self) -> f32 {
        WARP_RATES[option_index(self.values[PARAM_WARP_RATE], WARP_RATES.len())].1
    }

    /// Confine mode (Free / Confined / Reverse), nearest of the 3 options.
    pub fn loop_mode(&self) -> LoopMode {
        match option_index(self.values[PARAM_CONFINE], 3) {
            0 => LoopMode::Free,
            1 => LoopMode::Confined,
            _ => LoopMode::Reverse,
        }
    }
}
