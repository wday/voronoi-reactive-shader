//! delay-dsp — pure, host- and GPU-independent logic for the v3 delay line.
//!
//! Everything here is deterministic arithmetic with no GL and no FFGL, so it is
//! unit-testable with a plain `cargo test`. The GL/host-bound crates delegate to
//! it:
//!   - `delay-core` uses [`Ring`] for the per-frame slot advance / read slot.
//!   - `delay-write` uses [`regen_curve`] and [`delay_frames`].
//!   - the headless integration harness reuses [`Ring`] to predict slots.
//!
//! Where a function mirrors code in another crate, the doc comment names the
//! source so the two stay reconciled. Spec references (`CORE-DEPTH`,
//! `WRITE-PARAM-TIME`, …) point at `features/delay-line-v3/requirements.sdoc`.

pub mod ring;
pub mod params;
pub mod timing;

pub use params::{clamp_unit, regen_curve};
pub use ring::Ring;
pub use timing::{delay_frames, SyncMode};
