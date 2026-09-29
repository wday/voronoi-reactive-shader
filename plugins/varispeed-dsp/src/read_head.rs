//! The floating fractional read head, unified for **live** (moving write cursor)
//! and **frozen** (parked cursor) playback via an AGE model (VS-INTERP,
//! VS-LOOP-WINDOW, VS-RATE).
//!
//! `age` = how many frames behind the live write cursor the read is. Each render
//! frame the write cursor moves forward by `dr` (1 while recording, 0 while frozen)
//! and the read moves forward by `rate`, so `age += dr - rate`, wrapped into the
//! loop window `[0, len)`. The Read observes `dr` as `record_index - prev`, so
//! **freeze is implicit** — it never needs to know the Write's Send. Two useful
//! cases fall out with no special-casing:
//!   - live, rate=1 ⇒ `dr - rate = 0` ⇒ age constant ⇒ a fixed-length delay.
//!   - frozen (dr=0), rate=r ⇒ age moves at `-r` ⇒ the captured loop plays at r
//!     (reverse if r<0, freeze-frame if r=0), wrapping every `len`.

/// Four ring layers to fetch + the blend weight, for a Catmull-Rom cubic across
/// the read position (VS-INTERP): `out = catmull(prev, layer0, layer1, next, frac)`.
///
/// `layer0`/`layer1` bracket the read position exactly as they did under the old
/// linear `mix`; `prev` is one step before `layer0` and `next` one step after
/// `layer1`, both wrapped inside the loop window. At `frac == 0` a Catmull-Rom
/// returns `layer0` exactly, so integer reads (Rate +/-1x, +/-2x with no Warp) are
/// bit-identical to the linear version — only fractional reads change.
///
/// The outer taps wrap within the window like `layer1` always has, so at the seam
/// they pull from the opposite end. That widens the existing seam discontinuity
/// from one frame to two on each side; smoothing it is the deferred seam
/// crossfade, tracked separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub prev: u32,
    pub layer0: u32,
    pub layer1: u32,
    pub next: u32,
    pub frac: f32,
}

/// Advance the read `age` by one render frame.
/// - `dr`   — how far the write cursor moved this frame (`record_index - prev`;
///   1 while recording, 0 while frozen).
/// - `rate` — playback speed: `>1` faster, `0<r<1` slower, `0` freeze-frame,
///   `<0` reverse.
/// `age` changes by `dr - rate`, wrapped into the loop window `[0, len)`.
pub fn advance_age(age: f64, dr: f64, rate: f64, len: u32) -> f64 {
    (age + dr - rate).rem_euclid(len.max(1) as f64)
}

/// The `age` a Free-mode read parks at for an `len`-frame loop (VS-ANCHOR).
///
/// The Read draws before the Write, so within a host frame the Read sees
/// `record_index = R` and reads layer `R - age`, then the Write advances to `R + 1`
/// and records there. Content therefore recirculates every `age + 1` frames, so an
/// `len`-frame loop is `age = len - 1` — the top of the `[0, len)` range
/// [`advance_age`] wraps in.
///
/// Seeding `age` here is what makes Loop Length a directly settable tap time in
/// Free mode. Without it `age` stays at its initial 0 (at Rate 1 `dr - rate == 0`,
/// so nothing ever moves it), which is a 1-frame feedback loop with Loop Length
/// inert.
pub fn anchor_age(len: u32) -> f64 {
    (len.max(1) - 1) as f64
}

/// Resolve the read to two ring layers + interp weight.
/// - `record_index` — the write cursor (`vc_record_index`).
/// - `age`   — frames behind the live cursor (already advanced; wrapped here too).
/// - `warp`  — Doppler offset in frames; positive = newer (less age).
/// - `len`   — loop window length; `depth` — ring depth.
///
/// Interpolates within the loop window; at the seam the oldest frame blends back to
/// the newest (VS-SEAM — a raw discontinuity, not smoothed).
pub fn sample_age(record_index: u64, age: f64, warp: f64, len: u32, depth: u32) -> Sample {
    let len = len.max(1);
    let depth = depth.max(1);
    // warp shifts the read position; positive warp = newer = less age.
    let e = (age - warp).rem_euclid(len as f64);
    let a0 = e.floor();
    let frac = (e - a0) as f32;
    // Four consecutive ages for the cubic. Higher age = older, so `prev` (age0 - 1)
    // is the NEWER neighbour and `next` (age0 + 2) the older one. All wrap mod len
    // at the seam, like age1 always has.
    let l = len as i128;
    let age_prev = (a0 as i128 - 1).rem_euclid(l);
    let age0 = a0 as i128;
    let age1 = (a0 as i128 + 1).rem_euclid(l); // wrap oldest→newest at the seam
    let age_next = (a0 as i128 + 2).rem_euclid(l);
    // Frame at age `a` lives in ring layer (record_index - a) mod depth. i128 keeps
    // the subtraction well-defined before the ring has filled (early frames read
    // cleared/black slots — the warm-up, as in the delay line).
    let layer = |a: i128| ((record_index as i128 - a).rem_euclid(depth as i128)) as u32;
    Sample {
        prev: layer(age_prev),
        layer0: layer(age0),
        layer1: layer(age1),
        next: layer(age_next),
        frac,
    }
}

/// Advance the confined-loop play head by `rate` frames, wrapped into `[0, len)`.
/// Unlike [`advance_age`] there is no `dr` term: the head is a fixed play position
/// over a fixed window, so it moves purely at `rate` (Confine mode).
pub fn advance_head(head: f64, rate: f64, len: u32) -> f64 {
    (head + rate).rem_euclid(len.max(1) as f64)
}

/// Confined-loop read (Confine mode): a FIXED window of `len` slots at ring layers
/// `[0, len)`, sampled at float position `pos` (wraps mod len) with cubic
/// (Catmull-Rom) interpolation, wrapping across the seam. The Write records back into `floor(head)` of this
/// same window, so feedback recirculates in place and accumulates (like the delay's
/// sustain) instead of marching across the full ring — no ring-lap reset.
pub fn sample_confined(pos: f64, len: u32, depth: u32) -> Sample {
    let len = len.max(1);
    let depth = depth.max(1);
    let w = pos.rem_euclid(len as f64);
    let i0 = w.floor();
    let frac = (w - i0) as f32;
    let i = i0 as i64;
    let n = len as i64;
    let slot = |k: i64| ((i + k).rem_euclid(n) as u32) % depth;
    Sample { prev: slot(-1), layer0: slot(0), layer1: slot(1), next: slot(2), frac }
}

/// The integer slot the confined play head sits on — the slot the Write records
/// back into (Confine mode).
pub fn confined_slot(head: f64, len: u32) -> u32 {
    (head.rem_euclid(len.max(1) as f64)).floor() as u32 % len.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_age_is_top_of_the_window() {
        // Round trip is `age + 1` frames, so an L-frame loop parks at L-1 — the
        // largest age `advance_age`'s mod-L wrap can hold.
        assert_eq!(anchor_age(30), 29.0);
        assert_eq!(anchor_age(1), 0.0); // 1-frame loop == read the just-written frame
        assert_eq!(anchor_age(0), 0.0); // degenerate len clamps, never underflows
    }

    #[test]
    fn seeded_age_holds_at_rate_one() {
        // VS-ROUNDTRIP: recording at 1x, the seeded age is a fixed point, so the
        // loop recirculates on exactly L frames instead of drifting.
        for len in [1u32, 2, 7, 30, 60] {
            let a = anchor_age(len);
            assert!((advance_age(a, 1.0, 1.0, len) - a).abs() < 1e-9, "len {len}");
        }
    }

    #[test]
    fn seeded_age_reads_the_oldest_frame_in_the_window() {
        // age L-1 is the oldest frame of the L-window; its interp partner wraps to
        // the newest (the seam), and at frac 0 the cubic returns layer0 exactly.
        let s = sample_age(100, anchor_age(8), 0.0, 8, 61);
        assert_eq!(s.layer0, (100 - 7) % 61); // oldest of the 8-window, mod ring
        assert_eq!(s.layer1, 100 % 61); // seam: wraps to the newest
        assert_eq!(s.frac, 0.0);
    }

    #[test]
    fn round_trip_slot_never_aliases_the_write_at_max_len() {
        // VS-CAPACITY: N = L_max + 1. At the deepest tap the read layer and the
        // layer the Write is about to fill (R+1) must stay distinct.
        let (depth, len, r) = (61u32, 60u32, 1000u64);
        let read = sample_age(r, anchor_age(len), 0.0, len, depth).layer0;
        let write = ((r + 1) % depth as u64) as u32;
        assert_ne!(read, write);
    }

    #[test]
    fn cubic_taps_are_four_consecutive_ages() {
        // age 2.25 in an 8-frame window: taps at ages 1,2,3,4 (prev is NEWER).
        // Frame at age a lives at layer (record_index - a).
        let s = sample_age(100, 2.25, 0.0, 8, 240);
        assert_eq!(s.prev, 99); // age 1
        assert_eq!(s.layer0, 98); // age 2
        assert_eq!(s.layer1, 97); // age 3
        assert_eq!(s.next, 96); // age 4
        assert!((s.frac - 0.25).abs() < 1e-6);
    }

    #[test]
    fn cubic_outer_taps_wrap_at_the_seam() {
        // age 7.5 in an 8-frame window straddles the seam: layer1 wraps back to the
        // newest frame and `next` follows it, so the cubic never reads outside the
        // window (which would be a different loop's content).
        let s = sample_age(100, 7.5, 0.0, 8, 240);
        assert_eq!(s.prev, 94); // age 6
        assert_eq!(s.layer0, 93); // age 7 — oldest
        assert_eq!(s.layer1, 100); // age 0 — wrapped to newest
        assert_eq!(s.next, 99); // age 1
    }

    #[test]
    fn confined_cubic_taps_wrap_within_window() {
        // Forward wrap at the top of the window.
        let s = sample_confined(7.5, 8, 240);
        assert_eq!(s.prev, 6);
        assert_eq!(s.layer0, 7);
        assert_eq!(s.layer1, 0);
        assert_eq!(s.next, 1);
        // Backward wrap at the bottom: prev must come from the window END, never
        // from layer -1 (which would land outside the confined window).
        let s = sample_confined(0.5, 8, 240);
        assert_eq!(s.prev, 7);
        assert_eq!(s.layer0, 0);
        assert_eq!(s.layer1, 1);
        assert_eq!(s.next, 2);
    }


    #[test]
    fn degenerate_window_aliases_every_tap() {
        // len 1: nothing to interpolate; all four taps collapse onto the one frame
        // and frac is 0, so the cubic returns it exactly.
        let s = sample_confined(0.0, 1, 240);
        assert_eq!((s.prev, s.layer0, s.layer1, s.next), (0, 0, 0, 0));
        assert!(s.frac.abs() < 1e-6);
    }

    #[test]
    fn confined_window_maps_to_low_layers() {
        // Fixed window [0, len): slot = floor(pos) mod len, ring layer == slot.
        let s = sample_confined(2.25, 8, 240);
        assert_eq!(s.layer0, 2);
        assert_eq!(s.layer1, 3);
        assert!((s.frac - 0.25).abs() < 1e-6);
    }

    #[test]
    fn confined_seam_wraps_within_window() {
        let s = sample_confined(7.5, 8, 240);
        assert_eq!(s.layer0, 7);
        assert_eq!(s.layer1, 0); // wraps to window start, never into the rest of the ring
    }




    #[test]
    fn confined_head_advances_and_wraps() {
        let h = advance_head(7.5, 1.0, 8);
        assert!((h - 0.5).abs() < 1e-9);
        assert_eq!(confined_slot(7.5, 8), 7);
        assert_eq!(confined_slot(0.2, 8), 0);
    }

    #[test]
    fn confined_reverse_head_wraps() {
        let h = advance_head(0.5, -1.0, 8);
        assert!((h - 7.5).abs() < 1e-9);
    }

    #[test]
    fn newest_frame_at_age_zero() {
        // age 0 = the just-written frame (record_index mod depth).
        let s = sample_age(100, 0.0, 0.0, 8, 240);
        assert_eq!(s.layer0, 100);
        assert_eq!(s.frac, 0.0);
    }

    #[test]
    fn fractional_age_blends_two_adjacent_frames() {
        // age 2.5 blends frame (100-2) with the one newer (100-1)... wait: age0=2,
        // age1=3 → layers 98 and 97 (older side). frac 0.5.
        let s = sample_age(100, 2.5, 0.0, 8, 240);
        assert_eq!(s.layer0, 98); // 100 - 2
        assert_eq!(s.layer1, 97); // 100 - 3
        assert!((s.frac - 0.5).abs() < 1e-6);
    }

    #[test]
    fn seam_blends_oldest_back_to_newest() {
        // len 8, age 7.5: age0=7 (oldest) → 100-7=93; age1=(7+1)%8=0 (newest) → 100.
        let s = sample_age(100, 7.5, 0.0, 8, 240);
        assert_eq!(s.layer0, 93);
        assert_eq!(s.layer1, 100);
        assert!((s.frac - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ring_wraps_modulo_depth() {
        // record_index small: 3 - 5 wraps into the (cleared) top of the ring.
        let s = sample_age(3, 5.0, 0.0, 8, 240);
        assert_eq!(s.layer0, (240 + 3 - 5) as u32); // 238
    }

    #[test]
    fn warp_shifts_toward_newer() {
        // age 3, warp +1 ⇒ effective age 2 (one frame newer).
        let s = sample_age(100, 3.0, 1.0, 8, 240);
        assert_eq!(s.layer0, 98); // 100 - 2
    }

    #[test]
    fn live_rate_one_holds_constant_age() {
        // dr=1 (recording), rate=1 ⇒ age unchanged ⇒ fixed delay.
        let a = advance_age(3.0, 1.0, 1.0, 8);
        assert!((a - 3.0).abs() < 1e-9);
    }

    #[test]
    fn frozen_forward_playback_decreases_age() {
        // dr=0 (frozen), rate=1 ⇒ age moves -1 (plays the loop forward), wrapping.
        let a = advance_age(0.0, 0.0, 1.0, 8);
        assert!((a - 7.0).abs() < 1e-9); // 0 - 1 wraps to 7
    }

    #[test]
    fn frozen_reverse_increases_age() {
        // dr=0, rate=-1 ⇒ age moves +1 (reverse).
        let a = advance_age(6.0, 0.0, -1.0, 8);
        assert!((a - 7.0).abs() < 1e-9);
    }

    #[test]
    fn freeze_frame_holds_age_when_frozen() {
        // dr=0, rate=0 ⇒ age constant ⇒ a single held frame.
        assert!((advance_age(2.3, 0.0, 0.0, 8) - 2.3).abs() < 1e-9);
    }

    #[test]
    fn frozen_forward_completes_a_loop() {
        // Play a frozen len-4 loop forward; ages visit 0,3,2,1,0,… (mod 4).
        let mut age = 0.0;
        let seen: Vec<u32> = (0..4)
            .map(|_| {
                let l0 = sample_age(100, age, 0.0, 4, 240).layer0;
                age = advance_age(age, 0.0, 1.0, 4);
                l0
            })
            .collect();
        assert_eq!(seen, vec![100, 97, 98, 99]); // ages 0,3,2,1 → 100,97,98,99
    }
}
