// ── params.rs ── FFGL parameter definitions ──
//
// Resolume exposes these as knobs/sliders in the effect panel.
// All FFGL params are f32 in [0.0, 1.0] — we map to useful ranges in code.
//
// Rust-isms:
//   `const X: usize = 0` — compile-time constant, like `#define X 0` in C.
//   `static` — global variable, like `static` in C. Lives for entire program.
//   `LazyLock` — initialized on first access (like Python's module-level code).
//     Needed because CString::new() isn't const — can't run at compile time.
//   `CString` — null-terminated string for C interop. Rust strings are NOT
//     null-terminated (they store length instead), so we need CString for GL/FFGL.
//   `..Default::default()` — fill remaining struct fields with defaults.
//     Like C's designated initializers: `{.name = "foo", /* rest zeroed */}`.
//   `#[derive(Debug)]` — auto-generate a debug print method (like __repr__).

use std::ffi::CString;
use std::sync::LazyLock;

use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};

pub const PARAM_LOOP_BEATS: usize = 0;
pub const PARAM_DECAY: usize = 1;
pub const PARAM_QUALITY: usize = 2;
pub const PARAM_DRY_WET: usize = 3;
pub const NUM_PARAMS: usize = 4;

/// Global parameter metadata — Resolume reads this to build the UI.
/// `LazyLock<[T; N]>` = lazily-initialized fixed-size array.
/// The `|| { ... }` is a closure (lambda) that runs once on first access.
static PARAMS: LazyLock<[SimpleParamInfo; NUM_PARAMS]> = LazyLock::new(|| {
    [
        SimpleParamInfo {
            name: CString::new("Loop Beats").unwrap(),
            param_type: ParameterTypes::Standard, // 0.0-1.0 slider
            default: Some(0.4),                   // → 4 beats via knob_to_beats()
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Decay").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0), // no feedback by default
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Quality").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(1.0), // pristine by default
            ..Default::default()
        },
        SimpleParamInfo {
            name: CString::new("Dry/Wet").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.0), // live input by default
            ..Default::default()
        },
    ]
});

pub fn param_info(index: usize) -> &'static dyn ParamInfo {
    &PARAMS[index]
}

/// Map 0.0-1.0 knob to discrete beat values: 1, 2, 4, 8, 16, 32.
/// Divides the [0,1] range into 6 equal zones.
/// 0.0-0.16 → 1, 0.17-0.33 → 2, 0.34-0.50 → 4, etc.
pub fn knob_to_beats(value: f32) -> u32 {
    const BEATS: [u32; 6] = [1, 2, 4, 8, 16, 32];
    let idx = ((value * (BEATS.len() as f32 - 0.01)) as usize).min(BEATS.len() - 1);
    BEATS[idx]
}

/// Runtime parameter storage — just a f32 array indexed by PARAM_* constants.
/// Resolume calls set_param(index, value) when the user moves a knob.
#[derive(Debug)]
pub struct LooperParams {
    values: [f32; NUM_PARAMS],
}

impl LooperParams {
    pub fn new() -> Self {
        Self {
            // [loop_beats, decay, quality, dry_wet] — match defaults above
            values: [0.4, 0.0, 1.0, 0.0],
        }
    }

    pub fn get(&self, index: usize) -> f32 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, value: f32) {
        self.values[index] = value;
    }

    pub fn loop_beats(&self) -> u32 {
        knob_to_beats(self.values[PARAM_LOOP_BEATS])
    }

    pub fn decay(&self) -> f32 {
        self.values[PARAM_DECAY]
    }

    pub fn quality(&self) -> f32 {
        self.values[PARAM_QUALITY]
    }

    pub fn dry_wet(&self) -> f32 {
        self.values[PARAM_DRY_WET]
    }
}
