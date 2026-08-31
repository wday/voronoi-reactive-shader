//! varispeed-dsp — pure, host- and GPU-independent logic for the Varispeed atom.
//!
//! Deterministic arithmetic, no GL and no FFGL, so it is unit-testable with a
//! plain `cargo test` (unlike the plugin cdylibs, which need the FFGL toolchain).
//! The GL/host-bound crates delegate to it:
//!   - Varispeed Read maps its float `read_pos` onto ring layers via [`Loop`].
//!   - the read shader gets its two layers + blend weight from [`Sample`].
//!   - the warp LFO (Doppler) comes from [`warp_offset`].
//!
//! Spec references (`VS-INTERP`, `VS-LOOP-WINDOW`, `VS-RATE`, `VS-DOPPLER`) point at
//! `features/varispeed/requirements.md`.

pub mod read_head;
pub mod warp;

pub use read_head::{advance_age, advance_head, anchor_age, confined_slot, sample_age, sample_confined, Sample};
pub use warp::warp_offset;
