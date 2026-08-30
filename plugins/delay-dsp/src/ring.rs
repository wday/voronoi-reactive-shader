//! Ring-buffer slot arithmetic for one channel's tape (spec v0.2 — absolute
//! addressing).
//!
//! Slots are addressed **absolutely** off a per-channel monotonic `frame_index`
//! that a barrier in `delay-core::dc_frame_tick` advances exactly once per host
//! frame. Both the write slot and the read slot are pure functions of that one
//! per-frame value, so a Tap reads the same content regardless of where it sits
//! in the stack or how many Tap/Write instances interleave (spec `TAP-READ-SLOT`,
//! `COMPOSE-ORDER`). This is the v0.2 fix for the old model, where the read slot
//! was derived from the live `write_pos` the Write mutated mid-frame — making a
//! `Tap→Write→Tap` sequence read one-frame-different content.
//!
//! Pulling the arithmetic here keeps `delay-write::draw_write`,
//! `delay-tap::draw_tap`, and the headless GL harness reconciled against one
//! definition, unit-testable without GL. Spec ref: `CORE-DEPTH`, `TAP-READ-SLOT`.

/// The ring geometry for one channel: just the loop length. `buf_size =
/// loop_length + 1`; the `+1` is the **guard slot** that keeps the read slot from
/// ever aliasing the write slot (see `CORE-DEPTH`). There is no stored write
/// position anymore — the slots are computed from `frame_index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    /// Delay length in frames, already clamped to `1..=max_loop`. Equals the
    /// realized delay exactly (spec v0.2 resolved the old `+1` off-by-one).
    pub loop_length: u32,
}

impl Ring {
    /// New ring with `loop_length` clamped to `1..=max_loop`. `max_loop` is
    /// `BUFFER_DEPTH - 1` in delay-core (119). The unit tests below exercise the
    /// clamp math generically and pass their own `max_loop`, so they are not tied
    /// to that value.
    pub fn new(loop_length: u32, max_loop: u32) -> Self {
        Self {
            loop_length: loop_length.clamp(1, max_loop.max(1)),
        }
    }

    /// Reconstruct the ring geometry from a channel's `buf_size` (as reported by
    /// `dc_buf_size`). The Tap uses this: it has no loop-length knob of its own
    /// and derives `loop_length = buf_size - 1` from the buffer the Write sized.
    /// `buf_size` must be `>= 2` (an allocated buffer always is).
    pub fn for_buffer(buf_size: u32) -> Self {
        Self {
            loop_length: buf_size.saturating_sub(1).max(1),
        }
    }

    /// Number of live layers the ring cycles through (`loop_length + 1`).
    #[inline]
    pub fn buf_size(&self) -> u32 {
        self.loop_length + 1
    }

    /// The slot the Write records into on a frame whose counter is `frame_index`.
    /// Mirrors `delay-write::draw_write`. `= frame_index mod buf_size`.
    #[inline]
    pub fn write_slot(&self, frame_index: u64) -> u32 {
        (frame_index % self.buf_size() as u64) as u32
    }

    /// The slot a Tap reads on a frame whose counter is `frame_index` — the
    /// oldest live layer, one full lap (`loop_length` frames) back. Mirrors
    /// `delay-tap::draw_tap`.
    ///
    /// Defined as the slot the Write will next advance into, i.e. `(write_slot +
    /// 1) mod buf_size`. Since `buf_size = loop_length + 1`, this equals the spec
    /// form `(frame_index − loop_length) mod buf_size`, and it is always `!=
    /// write_slot` (buf_size ≥ 2) — the read never aliases this frame's write, so
    /// draw order does not affect timing.
    #[inline]
    pub fn read_slot(&self, frame_index: u64) -> u32 {
        (self.write_slot(frame_index) + 1) % self.buf_size()
    }

    /// Realized delay, in frames, between a frame being recorded and a Tap first
    /// reproducing it. As of spec v0.2 this equals `loop_length` exactly (the old
    /// `+1` off-by-one is resolved; the extra buffer layer is now the guard slot,
    /// not extra latency). The simulation test in this module derives the same
    /// number independently from slot identity.
    #[inline]
    pub fn realized_delay_frames(&self) -> u32 {
        self.loop_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent oracle for the realized delay: simulate the real per-frame
    /// loop under v0.2 absolute addressing. A shared `frame_index` advances once
    /// per frame (the barrier); each frame the Write records a unique marker into
    /// `write_slot(frame_index)` and the Tap reads `read_slot(frame_index)`. We
    /// return the frame offset at which the flash recorded on frame 0 is first
    /// read back. This re-derives the number from slot identity so it can be
    /// compared against `realized_delay_frames()`.
    fn simulate_first_reproduction(loop_length: u32, max_loop: u32) -> u32 {
        let ring = Ring::new(loop_length, max_loop);
        let buf_size = ring.buf_size();
        let mut slot_marker: Vec<Option<u32>> = vec![None; buf_size as usize];

        // Frame 0: barrier advances frame_index 0 -> 1; Write records the flash
        // (marker 0) into write_slot(1). (Matches the harness: frame_index starts
        // at 0 and the first tick advances it.)
        let mut frame_index: u64 = 1;
        slot_marker[ring.write_slot(frame_index) as usize] = Some(0);

        // Frames 1.. : barrier advances; Tap reads (same frame_index) BEFORE the
        // Write overwrites, matching Tap-before-Write stacking.
        for frame in 1..(buf_size * 4) {
            frame_index += 1;
            let read = ring.read_slot(frame_index);
            if slot_marker[read as usize] == Some(0) {
                return frame; // flash first reproduced here
            }
            slot_marker[ring.write_slot(frame_index) as usize] = Some(frame);
        }
        panic!("flash never reproduced within {} frames", buf_size * 4);
    }

    #[test]
    fn buf_size_is_loop_length_plus_one() {
        assert_eq!(Ring::new(2, 239).buf_size(), 3);
        assert_eq!(Ring::new(30, 239).buf_size(), 31);
        assert_eq!(Ring::new(239, 239).buf_size(), 240);
    }

    #[test]
    fn write_slot_wraps_at_buf_size() {
        let r = Ring::new(2, 239); // buf_size 3
        assert_eq!(r.write_slot(0), 0);
        assert_eq!(r.write_slot(1), 1);
        assert_eq!(r.write_slot(2), 2);
        assert_eq!(r.write_slot(3), 0); // wrapped
        assert_eq!(r.write_slot(4), 1);
    }

    #[test]
    fn read_slot_is_one_past_write_slot() {
        let r = Ring::new(3, 239); // buf_size 4
        // write 0->read 1, write 1->read 2, ... write 3->read 0
        for frame_index in 0..8u64 {
            let expected = (r.write_slot(frame_index) + 1) % r.buf_size();
            assert_eq!(r.read_slot(frame_index), expected);
        }
    }

    #[test]
    fn read_slot_never_aliases_write_slot() {
        // The guard slot (`+1` layer) guarantees the read never lands on the slot
        // this frame's Write is filling — so draw order can't affect timing.
        for loop_length in [1u32, 2, 5, 30, 239] {
            let r = Ring::new(loop_length, 239);
            for frame_index in 0..(r.buf_size() as u64 * 3) {
                assert_ne!(
                    r.read_slot(frame_index),
                    r.write_slot(frame_index),
                    "loop_length={loop_length} fi={frame_index}: read aliased write"
                );
            }
        }
    }

    #[test]
    fn read_slot_is_frame_index_minus_loop_length_mod_bufsize() {
        // The `(write_slot+1)` definition must equal the spec form
        // `(frame_index − loop_length) mod buf_size`.
        for loop_length in [1u32, 2, 5, 30, 239] {
            let r = Ring::new(loop_length, 239);
            let bs = r.buf_size() as u64;
            for frame_index in 0..(bs * 3) {
                let spec = ((frame_index % bs) + bs - (loop_length as u64 % bs)) % bs;
                assert_eq!(
                    r.read_slot(frame_index) as u64,
                    spec,
                    "loop_length={loop_length} fi={frame_index}"
                );
            }
        }
    }

    #[test]
    fn read_is_order_independent_within_a_frame() {
        // v0.2's central property: a Tap→Write→Tap sequence in one frame reads the
        // SAME slot from both Taps. Under absolute addressing the slots depend
        // only on `frame_index` (fixed once the barrier advanced it for the
        // frame), never on the mid-frame write — so any number of interleaved
        // Write calls between the two reads cannot change what a Tap reads.
        let r = Ring::new(5, 239);
        for frame_index in 0..20u64 {
            let tap_before = r.read_slot(frame_index);
            let _this_frames_write = r.write_slot(frame_index); // Write happens between
            let tap_after = r.read_slot(frame_index);
            assert_eq!(
                tap_before, tap_after,
                "fi={frame_index}: two Taps in one frame must read the same slot"
            );
        }
    }

    #[test]
    fn realized_delay_equals_loop_length() {
        // Spec v0.2: the realized delay is `loop_length` frames — exactly what the
        // knob asks for. This is the tripwire that must move in lockstep with the
        // spec if the addressing ever changes (it previously asserted
        // `loop_length + 1`, the pre-v0.2 off-by-one).
        for loop_length in [1u32, 2, 3, 7, 30, 100, 239] {
            let realized = simulate_first_reproduction(loop_length, 239);
            assert_eq!(
                realized, loop_length,
                "loop_length={loop_length}: realized delay should equal loop_length"
            );
            // The helper must agree with the independent simulation.
            assert_eq!(realized, Ring::new(loop_length, 239).realized_delay_frames());
        }
    }

    #[test]
    fn loop_length_is_clamped() {
        assert_eq!(Ring::new(0, 239).loop_length, 1); // below min -> 1
        assert_eq!(Ring::new(999, 239).loop_length, 239); // above max -> max
        assert_eq!(Ring::new(50, 239).loop_length, 50); // in range -> unchanged
    }

    #[test]
    fn for_buffer_recovers_loop_length() {
        for loop_length in [1u32, 2, 5, 239] {
            let bs = Ring::new(loop_length, 239).buf_size();
            assert_eq!(Ring::for_buffer(bs).loop_length, loop_length);
        }
    }
}
