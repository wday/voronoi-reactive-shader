//! End-to-end pixel tests for the v3 delay line on a headless EGL/llvmpipe
//! context. Everything runs in ONE test function: the GL context and delay-core's
//! global registry are process-wide, so scenarios run sequentially and each
//! forces a fresh channel-0 buffer by using a distinct texture size (a size
//! change triggers delay-core's realloc, which clears every layer — the monotonic
//! `frame_index` is NOT reset, so slots keep advancing across scenarios, which is
//! fine: every read/write slot is computed mod buf_size from that counter).
//! Empty-buffer behaviour is checked on the never-written channel 1.

use delay_dsp::Ring;
use delay_gl_tests::{headless, solid_input_tex, Passes};

/// Sum of the RGB channels — used as a cheap "is this pixel lit?" probe.
fn lum(px: [u8; 4]) -> u32 {
    px[0] as u32 + px[1] as u32 + px[2] as u32
}

fn near(px: [u8; 4], want: [u8; 3], tol: i32) {
    for i in 0..3 {
        let d = (px[i] as i32 - want[i] as i32).abs();
        assert!(d <= tol, "channel {i}: got {} want {} (tol {tol}), px={px:?}", px[i], want[i]);
    }
}

/// Record a single `color` flash on frame 0 (Send=1), then step black frames with
/// Tap-BEFORE-Write ordering (the real stacking order), reading the Tap each frame
/// with the given `dry`/`wet`. Both the Tap and the Write tick the barrier with
/// the SAME per-frame id, so the channel's `frame_index` advances exactly once per
/// frame regardless of which ticks first (spec v0.2). Returns
/// `(frame_of_first_reproduction, pixel)`.
fn run_delay(
    p: &Passes,
    ch: usize,
    loop_length: u32,
    size: u32,
    color: [u8; 3],
    dry: f32,
    wet: f32,
    gamma: f32,
    fid0: u64,
) -> (u32, [u8; 4]) {
    let (w, h) = (size, size);
    let flash = solid_input_tex(w, h, [color[0], color[1], color[2], 255]);
    let black = solid_input_tex(w, h, [0, 0, 0, 255]);

    // Frame 0: Write only (advances frame_index, records the flash). No Tap yet.
    unsafe { p.record(ch, loop_length, flash, w, h, 1.0, fid0) };

    for f in 1..(loop_length + 6) {
        let fid = fid0 + f as u64;
        // Tap reads (ticks this frame) BEFORE this frame's Write records...
        let px = unsafe { p.tap_read(ch, black, w, h, dry, wet, gamma, fid) };
        if lum(px) > 30 {
            return (f, px);
        }
        // ...then this frame's Write (same frame_id: no extra advance) records black.
        unsafe { p.record(ch, loop_length, black, w, h, 1.0, fid) };
    }
    panic!("flash never reproduced for loop_length={loop_length}");
}

#[test]
fn headless_delay_pipeline() {
    let _ctx = headless();
    let p = Passes::new();
    let mut fid: u64 = 1;

    // --- Scenario 1: realized delay is EXACTLY loop_length frames (spec v0.2
    //     absolute addressing resolved the old +1 off-by-one, CORE-DEPTH) —
    //     proven in actual pixels through the real shaders.
    for (loop_length, size) in [(2u32, 8u32), (3, 10), (5, 12), (12, 14)] {
        let (frame, _) = run_delay(&p, 0, loop_length, size, [255, 255, 255], 0.0, 1.0, 1.0, fid);
        assert_eq!(
            frame, loop_length,
            "loop_length={loop_length}: Tap should first reproduce the flash at frame loop_length"
        );
        fid += 100;
    }

    // --- Scenario 2: Send=1 reproduces the input color exactly (tape = 1*input)
    //     — record-math fidelity, not just "something lit".
    {
        let (frame, px) = run_delay(&p, 0, 4, 16, [200, 120, 40], 0.0, 1.0, 1.0, fid);
        assert_eq!(frame, 4);
        near(px, [200, 120, 40], 4);
        fid += 100;
    }

    // --- Scenario 3: Tap Wet is a linear gain on the delayed tape (output.frag).
    {
        let (_frame, px) = run_delay(&p, 0, 4, 18, [200, 200, 200], 0.0, 0.5, 1.0, fid);
        near(px, [100, 100, 100], 8); // ~0.5 * 200
        fid += 100;
    }

    // --- Scenario 4: Dry passthrough + empty-buffer wet-forcing (TAP-EMPTY-BUFFER).
    //     Channel 1 was never written: dc_buf_size==0, so Wet is forced to 0 and
    //     the output is dry*input with no garbage from an unallocated tape.
    {
        let input = solid_input_tex(20, 20, [80, 160, 240, 255]);
        let px = unsafe { p.tap_read(1, input, 20, 20, 1.0, 1.0, 1.0, fid + 1) };
        near(px, [80, 160, 240], 3);
    }

    // --- Scenario 5: Send is a pure overwrite (tape = Send*input, no buffer
    //     read), AND the frame barrier keeps a 2nd same-frame Write from advancing
    //     frame_index (COMPOSE-SINGLE-WRITER mechanism). Two Writes with the SAME
    //     frame_id: the first writes white (Send=1); the second (Send=0.5)
    //     overwrites the same slot to 0.5*white. Under v0.2 the write slot is
    //     frame_index % buf_size, so an unchanged frame_index means the same slot.
    {
        let (w, h) = (24u32, 24u32);
        let white = solid_input_tex(w, h, [200, 200, 200, 255]);
        let same_frame = fid;

        unsafe { p.record(0, 4, white, w, h, 1.0, same_frame) };
        let tex = delay_core::dc_tex(0);
        let bufsz = delay_core::dc_buf_size(0);
        let fi = delay_core::dc_frame_index(0);
        let slot = Ring::for_buffer(bufsz).write_slot(fi);
        let a = unsafe { p.read_layer(tex, slot, w, h) };
        near(a, [200, 200, 200], 3); // first writer wrote Send=1 * white

        unsafe { p.record(0, 4, white, w, h, 0.5, same_frame) };
        let fi2 = delay_core::dc_frame_index(0);
        assert_eq!(fi2, fi, "2nd same-frame writer must NOT advance frame_index");
        let b = unsafe { p.read_layer(tex, slot, w, h) };
        near(b, [100, 100, 100], 8); // overwrite: 0.5 * white
    }

    // --- Scenario 6: Blend Space = Linear (gamma 2.2) changes the Wet blend from
    //     a perceptual gain to a linear-light one. Same setup as Scenario 3
    //     (tape holds 200, Dry=0, Wet=0.5), but blended in linear light:
    //         out = 255 * (0.5 * (200/255)^2.2)^(1/2.2) ≈ 146
    //     vs the perceptual 0.5*200 = 100. A clean, large, analytic difference
    //     that proves the linear path is wired end-to-end through the real shader.
    {
        let (_frame, px) = run_delay(&p, 0, 4, 22, [200, 200, 200], 0.0, 0.5, 2.2, fid + 500);
        near(px, [146, 146, 146], 8);
        assert!(px[0] > 120, "linear Wet blend must be brighter than perceptual (~100)");
    }

    // --- Scenario 7: a loop_length change reseeds the whole tape to black, so a
    //     later expansion can't resurrect stale frames (CORE-LENGTH-RESEED). Fill
    //     a short (loop_length=8) ring so every one of its slots holds white, then
    //     change the length: slot 5 — white a moment ago — must read back black.
    {
        let (w, h) = (26u32, 26u32); // fresh size → realloc, clean start
        let white = solid_input_tex(w, h, [255, 255, 255, 255]);
        let black = solid_input_tex(w, h, [0, 0, 0, 255]);
        let mut f = fid + 700;
        for _ in 0..12 {
            unsafe { p.record(0, 8, white, w, h, 1.0, f) }; // 12 > buf_size(9): fills every slot
            f += 1;
        }
        let tex = delay_core::dc_tex(0);
        near(unsafe { p.read_layer(tex, 5, w, h) }, [255, 255, 255], 3); // slot 5 is white

        // Expand the loop: modulus (buf_size) changes 9 → 21, so the tape reseeds.
        unsafe { p.record(0, 20, black, w, h, 1.0, f) };
        near(unsafe { p.read_layer(tex, 5, w, h) }, [0, 0, 0], 3); // wiped (stale white without the fix)
    }

    // --- Scenario 8: the tape is resolution-independent on read, so the Write can
    //     store it downscaled (delay-write's TAPE_SCALE) and the Tap upscales for
    //     free via normalised uv + linear filtering. Also exercises the RGBA16F
    //     float format (values round-trip through half-float exactly at 8-bit in).
    //     Record a solid colour into a HALF-res tape, read it back at FULL res.
    {
        let (fw, fh) = (32u32, 32u32);
        let (tw, th) = (fw / 2, fh / 2); // tape stored at half resolution
        let color = [40u8, 210, 90];
        let src = solid_input_tex(tw, th, [color[0], color[1], color[2], 255]);
        let mut f = fid + 900;
        for _ in 0..6 {
            unsafe { p.record(0, 4, src, tw, th, 1.0, f) }; // fill the (loop_length=4) ring
            f += 1;
        }
        let black = solid_input_tex(fw, fh, [0, 0, 0, 255]);
        // Read at FULL res: the half-res tape upscales (solid colour ⇒ exact).
        let px = unsafe { p.tap_read(0, black, fw, fh, 0.0, 1.0, 1.0, f) };
        near(px, color, 4);
    }
}
