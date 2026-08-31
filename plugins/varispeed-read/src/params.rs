use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const NUM_PARAMS: usize = 12;
pub const PARAM_CHANNEL: usize = 0;
pub const PARAM_SYNC_MODE: usize = 1;
pub const PARAM_SUBDIVISION: usize = 2;
pub const PARAM_LOOP_MS: usize = 3;
pub const PARAM_LOOP_FRAMES: usize = 4;
pub const PARAM_RATE: usize = 5;
pub const PARAM_DRY: usize = 6;
pub const PARAM_WET: usize = 7;
pub const PARAM_BLEND: usize = 8;
pub const PARAM_WARP_DEPTH: usize = 9;
pub const PARAM_WARP_RATE: usize = 10;
pub const PARAM_LOOP_MODE: usize = 11;

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

// Slider ranges only — the real cap is `depth - 1` computed at runtime from the
// core's depth (varispeed-read/src/lib.rs) and clamped in `loop_frames`. Tracks
// `BUFFER_DEPTH - 1` = 60, the N+1 stitch (VS-CAPACITY): at the deepest tap the
// read layer and the layer the Write is about to fill stay distinct.
//
// 60 frames = 1000 ms @60 fps, so the two maxima agree. Varispeed is the
// short-loop fractal box; long delay is DlyT/DlyW's job.
const MAX_LOOP_MS: f32 = 1000.0;
const MAX_LOOP_FRAMES: u32 = 60;
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
        // 0: Channel — the patch point. Read + Write on the same Channel form a
        //    loop over one tape. Two channels = two independent feedback networks,
        //    each with its own in-loop FX stack (VS-CHANNELS).
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
        // 1: Sync Mode — how Loop Length is specified.
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
        // 2: Subdivision (Loop Length in beats).
        SimpleParamInfo {
            name: CString::new("Subdivision").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(2.0 / 6.0), // 1/4
            elements: Some(option_elements(&SUBDIVISIONS)),
            ..Default::default()
        },
        // 3: Loop Ms.
        SimpleParamInfo {
            name: CString::new("Loop Ms").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(500.0),
            min: Some(1.0),
            max: Some(MAX_LOOP_MS),
            ..Default::default()
        },
        // 4: Loop Frames.
        SimpleParamInfo {
            name: CString::new("Loop Frames").unwrap(),
            param_type: ParameterTypes::Integer,
            default: Some(60.0),
            min: Some(1.0),
            max: Some(MAX_LOOP_FRAMES as f32),
            ..Default::default()
        },
        // 5: Rate — playback speed/direction (beat-ratio). Freeze=freeze-frame,
        //    negatives=reverse. Independent of the Write's Send=0 loop-freeze.
        SimpleParamInfo {
            name: CString::new("Rate").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(5.0 / 6.0), // 1x
            elements: Some(option_elements(&RATES)),
            ..Default::default()
        },
        // 6: Dry — gain on the live source into the FX chain.
        SimpleParamInfo {
            name: CString::new("Dry").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0),
            ..Default::default()
        },
        // 7: Wet — gain on the played-back loop.
        SimpleParamInfo {
            name: CString::new("Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.5),
            ..Default::default()
        },
        // 8: Blend Space — Linear (default) or Perceptual, as Delay Tap.
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
        // 9: Warp Depth — Doppler swing (0 = off).
        SimpleParamInfo {
            name: CString::new("Warp Depth").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0),
            ..Default::default()
        },
        // 10: Warp Rate — Doppler LFO period (beat-synced, bar-phase-locked).
        SimpleParamInfo {
            name: CString::new("Warp Rate").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(2.0 / 4.0), // 1/4
            elements: Some(option_elements(&WARP_RATES)),
            ..Default::default()
        },
        // 11: Loop Mode — where the Write lands relative to the read head.
        //     Both modes recirculate on exactly Loop Length frames at Rate 1x; they
        //     differ in what Rate != 1 does.
        //     Confined (default): the read plays a FIXED [0, Loop) window and the
        //     Write records the FX'd output back into the slot just read. There is
        //     no read/write rate mismatch, so ANY Rate accumulates in place at
        //     Send*Wet per lap (decaying, or sustaining at Send*Wet >= 1) without
        //     the compounding cascade. Never crosses the ring-lap seam. At most ONE
        //     Confined Read per channel — they collide on the loop-slot handoff.
        //     Free: the Write records at the moving cursor and the read floats
        //     `age` frames behind it, seeded to Loop Length - 1 (VS-ANCHOR) so Loop
        //     Length is the tap time. At Rate 1x that is a fixed Loop-length delay;
        //     at Rate != 1 read and write rates disagree, which is the compounding
        //     feedback cascade (experimental, unguarded). Free publishes nothing to
        //     the tape, so N Free Reads share one channel as N independent taps
        //     (VS-MULTITAP).
        SimpleParamInfo {
            name: CString::new("Loop Mode").unwrap(),
            param_type: ParameterTypes::Option,
            default: Some(1.0), // Confined — the in-place recirculating feedback path
            elements: Some(vec![
                (CString::new("Free").unwrap(), 0.0),
                (CString::new("Confined").unwrap(), 1.0),
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

/// Where the Write lands relative to the read head (Loop Mode param). See the
/// param-11 doc.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoopMode {
    /// Write at the moving cursor; the read floats `age` behind it (age model).
    Free,
    /// Fixed [0, Loop) window, in-place recirculating feedback.
    Confined,
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
                0.0,       // Channel: 1
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
                1.0,       // Loop Mode: Confined (in-place decaying feedback, default)
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

    /// Tape channel index. Read + Write on the same channel form a loop.
    pub fn channel(&self) -> u32 {
        if self.values[PARAM_CHANNEL] < 0.5 { 0 } else { 1 }
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

    /// Loop mode (Free / Confined), nearest of the 2 options.
    ///
    /// The old 3-option encoding (Free 0.0 / Confined 0.5 / Reverse 1.0) migrates
    /// sanely: 0.0 stays Free, and both 0.5 and 1.0 land on Confined.
    pub fn loop_mode(&self) -> LoopMode {
        if option_index(self.values[PARAM_LOOP_MODE], 2) == 0 {
            LoopMode::Free
        } else {
            LoopMode::Confined
        }
    }
}
