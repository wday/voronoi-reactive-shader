# Varispeed — Development Log

Tracks build order and decisions. See [requirements.md](requirements.md) and
[plan.md](plan.md) for the spec and staged plan.

---

## 2026-07-08 — Stages 0–6 built

Full stage-by-stage build landed (per plan.md):

- **Stage 0 — `varispeed-dsp`** pure crate: `advance_age`/`sample_age` (free-float
  age model, `age += dr - rate`), `advance_head`/`sample_confined`/`confined_slot`
  (confined fixed-window loop), `warp_offset` (bar-phase-locked Doppler). Native
  unit tests green.
- **Stage 1 — `varispeed-core`** cdylib: single tape, full ring `BUFFER_DEPTH=240`,
  half-res RGBA16F storage, per-frame `host_time` barrier. Cross-DLL owner via
  `pluglib` loader. `delay-core` left untouched (VS-ISOLATION). ABI settled as
  `vc_write_tick / vc_record_index / vc_tex / vc_depth / vc_acquire / vc_release`
  plus confine handoff `vc_set_loop_slot / vc_loop_slot`.
- **Stage 2 — Varispeed Write (`VsWr`)**: pure overwrite `tape = Send*input` at
  `record_index mod depth`, half-res viewport, passthrough output. **Send=0 =
  freeze** (skip the tick; park the cursor — NOT the delay's wipe).
- **Stage 3 — Varispeed Read (`VsRd`)**: floating fractional read, `Dry*live +
  Wet*loop` in the selected blend space (Linear default via `u_gamma`).
- **Stage 4 — Warp + beat-ratio Rate**: Rate as a 7-way beat-ratio option
  (−2x…Freeze…2x); Doppler warp off `host_beat.barPhase`.
- **Confine mode**: added as Read param 10. Free = free-float scrub; Confined =
  fixed `[0, Loop)` window with in-place write-back (Read publishes its slot,
  Write records into it). Default shipped as **Free**.

Registered `varispeed_core`, `varispeed_read`, `varispeed_write` in `plugins.json`.
Windows MSVC build clean (DLLs staged 07-09 ~01:02). **Deploy blocked overnight**
— Resolume (`Avenue.exe`) held the DLLs locked.

---

## 2026-07-09 — Deploy + Confined-default fix

### Deployed the overnight build
Avenue was closed, so `make deploy PLUGIN=varispeed_core varispeed_write
varispeed_read` moved all three DLLs into `…/Resolume Avenue/Extra Effects/`.

### Investigated: "loops don't fully decay at Rate=2x"
Symptom (Free mode): with Wet=0.5, Send=0.1 (loop gain `Send*Wet=0.05`, should fade
in ~2 laps), loops kept circulating; new writes went dim but **old frames captured
at higher Send kept looping**.

**Root cause — architectural, not a math bug.** The Send math is correct
(`tape = Send*input`, per-lap gain `Send*Wet`), but that gain is only realised when
the read slot and the write-back slot coincide each frame. In **Free** mode the
Write always deposits at the newest slot (age 0, `record_index mod depth`, +1/frame)
while the Read's age head moves at `dr - rate` (−1/frame at Rate=2x). The two cursors
drift apart, so at Rate≠1 the Read replays ring slots the current write pass never
re-attenuates in place → captured (bright) content lingers until the write cursor
laps the whole tape. **Free is a scrub/playback mode; it does not implement
`Send*Wet` decay at Rate≠1** (matches its "resets each ring lap" doc).

**Confined** mode *does* decay at any Rate: the Read publishes `floor(head)` and the
Write records the FX'd output back into that exact slot → true in-place `Send*Wet`
recirculation. (`Send*Wet >= 1` crosses into sustain/overdub — same knob, by design.)

### Confirmed: subdivision change is not a buffer wipe
User saw 1/2 → 1/8 in Confined mode "reset the buffer." `varispeed-core` only
reallocs/clears on a **resolution** change (`vc_write_tick`), never on a length
change. The fixed `[0, Loop)` window simply re-anchors: shrinking keeps slots
`[0, L_small)` and drops the rest; growing later exposes never-recorded (black)
slots. Not a reseed.

### Change — made Confined the default (user decision: "make confined default,
everything should decay")
`plugins/varispeed-read/src/params.rs`:
- Confine `SimpleParamInfo.default`: `0.0 → 1.0` (new instances come up Confined).
- `ReadParams::new()` Confine initial value: `0.0 → 1.0`.
- Rewrote the Confine doc comment: Confined (default) = in-place `Send*Wet` decay at
  any Rate; Free = scrub/playback, no in-place decay at Rate≠1.

Rebuilt `varispeed_read.dll` clean (md5 `556a85d9…`). First deploy attempt failed
(`Avenue.exe` had reopened and relocked the DLL — note: process name is `Avenue.exe`,
so a `grep resolume` misses it). Redeployed once Avenue was closed; dst/src md5 match.

**Note:** Read effects already saved in a composition keep their stored Confine
value — set those to Confined manually or re-add them.

### Added: Reverse (ping-pong block) mode — Stage 7
Follow-on from the reverse discussion: Confined + `Rate<0` reverse-scrubs a buffer
being overwritten in place (glitch), and the brief coherent flash on 1x→-1x is the
residual forward-recorded window. Continuous *live* reverse is non-causal (one-block
latency) but not impossible — it's the audio-world **ping-pong / block-reverse**
architecture. User approved building it (latency fine; VRAM to spare given half-res).

Design decisions (asked): **grow ring 240→480** to hold two full-length blocks
(~2GB single tape) rather than cap reverse loop length; **honor Rate** so the frozen
block is a general block looper (-1x classic reverse, -2x/-0.5x reverse fast/slow,
+ = forward block delay, 0 = freeze-frame). See requirements §12 (VS-REV-*).

Implementation:
- `varispeed-core`: `BUFFER_DEPTH 240 → 480` (VS-REV-CAPACITY; ~2GB, ~8s).
- `varispeed-dsp`: new `sample_block(base, pos, len, depth)` — `sample_confined`
  offset to any block base, wraps within the block. +3 tests (base offset, block
  seam, reverse index walk `L-1..0`). 23 dsp tests green.
- `varispeed-read/params.rs`: Confine now 3-way `Free=0.0 / Confined=0.5 /
  Reverse=1.0`, default `0.5`. Replaced `confined() -> bool` with
  `loop_mode() -> LoopMode {Free,Confined,Reverse}`. **Encoding remap:** Confined
  moved `1.0 → 0.5`; comps saved today at `1.0` now read as Reverse — re-select.
- `varispeed-read/lib.rs`: reverse state (`active_block`, `rec_pos`, `play_pos`,
  `rev_len`); Reverse branch — publish forward record slot, read frozen block at
  `play_pos` (honors Rate), advance record metronome, swap every `L` and reset the
  play head to newest (`reverse_reset_pos`). Mode-aware `max_len` (Reverse capped at
  `depth/2`). Write unchanged (barrier-driven slot works for any mode).

Built all three clean (MSVC, first try). **Deploy blocked — `Avenue.exe` running.**
Close Avenue + `make deploy PLUGIN=varispeed_core varispeed_write varispeed_read`.
All uncommitted.

---

## Open / deferred

- Free-mode decay at Rate≠1 is inherent (scrub mode). If a decaying *free-floating*
  loop is ever wanted, that's a DSP change (write-back into the read slot in Free
  too — which collapses Free into Confine — or a global per-lap ring decay).
- v1 non-goals still deferred (plan.md): seam crossfade, higher-order interp,
  de-strobe/multi-tap, live Freeze-control shape (requirements §9).
- Live validation of the Rate≠1 feedback cascade (`VsRd → FX → VsWr`) in Confined.
- Live validation of Reverse mode (Stage 7): `Rate=-1x` reverse tail with no manual
  freeze, one-block latency, blocks swapping every `L`.

## 2026-08-29 — Full-res tape (TAPE_SCALE 1.0) + BUFFER_DEPTH 480 → 240

Live-use feedback: deep feedback loops "blur into blobs, which stops fractal
generation". Varispeed Read/Write is what's actually being gigged now — the delay
suite has been retired from the rig.

### Diagnosis (spatial)

Same architecture and same failure as the delay line. Each lap: full-res → 2x2 box
downsample (`varispeed-write`, `TAPE_SCALE = 0.5`) → bilinear magnify on read. That
round trip has gain ~1.0 at DC but only **~0.35 at mid-high spatial frequencies**,
and 0 above the tape's Nyquist. Fine detail decays ~3x faster per lap than the
image as a whole, so structure survives and texture doesn't. Fractal generation
needs loop gain >= 1 in the band where the transform creates new detail.

### Diagnosis (temporal) — varispeed-only, NOT fixed here

`read.frag.glsl` also does `mix(b0, b1, u_frac)` between the two bracketing frames.
That is required for smooth fractional playback (VS-INTERP), but in a feedback loop
it is a **second lowpass**, and it is independent of tape resolution:

| condition | `u_frac` | temporal blur |
|---|---|---|
| Rate ±1x, ±2x, Warp Depth 0 | always 0 | none |
| Rate ±1/2x | alternates 0 / 0.5 | hard 2-frame average every other frame |
| any Warp Depth > 0 | continuous | always on |

So the blobbiest patches will be the Warp-driven and 1/2x ones, and full res alone
won't rescue those. A Smooth/Nearest interp switch on the Read would (at the cost
of judder) — logged as a candidate for `features/delay-lens/`.

### The depth trade

`BUFFER_DEPTH = 480` was never loop length — `MAX_LOOP_FRAMES` was already 239. The
480 exists so Reverse (ping-pong) can hold two full blocks at once. So depth has to
stay ≈ `2 * MAX_LOOP_FRAMES`, and going full-res at 480 would be 7.96 GB at
1080p — too much beside Resolume on a 12 GB card.

Single tape at 1920x1080, RGBA16F (8 B/texel):

| tape | depth | max loop | VRAM |
|---|---|---|---|
| half (was) | 480 | 239 fr / 4 s | 1.99 GB |
| **full (now)** | **240** | **120 fr / 2 s** | **3.98 GB** |

User ruling: take the 2 s max loop. Actual use is 1/16..1/4 note (or 2-3 frames for
tight feedback) at 1080p/60, which tops out near 60 frames at 60 BPM — 2x headroom.

### Implemented
- **varispeed-core**: `BUFFER_DEPTH` 480 → 240, with the `2 * MAX_LOOP_FRAMES`
  invariant written down at the const so Reverse doesn't silently break if either
  moves later. Half-res doc references corrected.
- **varispeed-write**: `TAPE_SCALE` 0.5 → 1.0. Constant only — the sizing math,
  viewport and `vc_write_tick` dims all key off it, and the core takes whatever
  dims it is handed.
- **varispeed-read**: `MAX_LOOP_MS` 4000 → 2000, `MAX_LOOP_FRAMES` 239 → 120, noted
  as tracking the core's depth. `read.frag.glsl` comment corrected (the read is now
  a 1:1 fetch, no spatial upscale).

### Verification
- `cargo test -p varispeed-dsp`: 23/23 pass. Its `480` literals are test-local
  depths passed to `sample_block`, not tied to `BUFFER_DEPTH`.
- All three DLLs built clean on MSVC.
- **Deploy blocked** — Resolume open, holding the DLLs. All three must go together:
  core at depth 240 with a Read still allowing a 239-frame loop would break the
  two-block Reverse invariant. Close Resolume, then
  `make deploy PLUGIN=varispeed_core` (and `varispeed_write`, `varispeed_read`).

### Watch on first play
The downscale was also providing free anti-aliasing. At full res, 2-3 frame loops
may shimmer more. Don't revert for that — it's what the Bloom side of the Delay
Lens Focus knob is for.

### 2026-08-29 (same day) — corrected an off-by-one I introduced

Shipped `MAX_LOOP_FRAMES = 119`, caught on review. Wrong: 119 was me preserving
the old `239 = 240 - 1` arithmetic instead of re-deriving the constraint.

The delay line's `BUFFER_DEPTH - 1` is a genuine N+1 stitch — `buf_size =
loop_length + 1`, the read slot must not alias the slot the writer owns. Varispeed
has no such rule. Its caps are computed per mode at runtime in
`varispeed-read/src/lib.rs:162-165` (`Reverse => depth/2`, `_ => depth-1`) and
clamped in `loop_frames`; `MAX_LOOP_FRAMES` is only the slider's declared range.
Reverse is the binding case and `depth/2 = 120` is reachable — two 120-frame blocks
tile the 240-layer ring exactly.

The old 239 was inherited from the delay's convention and, at depth 480, happened
to sit one below Reverse's cap of 240, so nothing ever surfaced it. Second tell
missed at the time: 120 frames @60 fps is exactly 2000 ms, so `MAX_LOOP_MS = 2000`
with `MAX_LOOP_FRAMES = 119` disagreed — Ms mode would ask for 120 and get 119.

Now `MAX_LOOP_FRAMES = 120`, with the no-`-1` reasoning written at the constant so
it doesn't get "corrected" back. varispeed-core's comment fixed to
`2 * MAX_LOOP_FRAMES`.

### 2026-08-30 — first live impression of the full-res tape

Warp reads completely differently. With the half-res tape the Doppler smear was
competing with the per-lap spatial blur and mostly lost; at full res the spatial
detail survives the lap, so the temporal `mix()` between bracketing frames now
shows up as a distinct, legible doppler rather than mush. User: "warp is insane
with deep feedback loops, the doppler is so apparent."

Bears directly on the deferred Smooth/Nearest interp switch: the temporal blend
was logged as a *second lowpass to fix*, but with the spatial loss removed it
reads as the effect doing its job. Keep the switch on the list as an option for
crunchier reads, not as a defect. Re-evaluate after more time on it.

## 2026-08-30 — Catmull-Rom temporal interpolation (VS-INTERP upgrade)

Follow-on from the full-res tape. Two motivations converged:

1. **Judder.** Fractional Rate read jerky where Frames-mode-at-1x looked smooth.
2. **The temporal lowpass.** Linear `mix(b0, b1, frac)` is a pure two-tap average,
   so at fractional rates it softened the loop on *every lap* — the second blur
   flagged when the spatial one was removed, active whenever Rate is fractional or
   Warp Depth > 0.

Cubic addresses both at once: Catmull-Rom is smoother between frames *and* has a
mild sharpening lobe where linear only averages. That is why it was chosen over
the seam crossfade, which only removes a periodic artifact.

### Implemented
- **varispeed-dsp**: `Sample` grows from `{layer0, layer1, frac}` to
  `{prev, layer0, layer1, next, frac}`. `layer0`/`layer1` keep their exact former
  meaning, so every pre-existing test still asserts the same thing. All three
  samplers updated — `sample_age` (age space: `prev` is age0-1, the *newer*
  neighbour), `sample_confined`, `sample_block` — each wrapping all four taps
  inside its own window/block via `rem_euclid`.
- **read.frag.glsl**: `catmull(p0,p1,p2,p3,t)`, result clamped to [0,1] because the
  tape stores encoded values and overshoot would recirculate round the loop.
- **shader.rs**: four scalar uniforms, **not** a `float[4]`. `uniform_loc` returns
  `glGetUniformLocation` raw (-1 on a miss) and `glUniform*(-1, ...)` is a silent
  no-op, so an array-name mismatch on some driver would have frozen the read on
  layer 0 with no error. Rust side still takes `[f32; 4]`.

### Properties worth knowing
- **Integer rates are unchanged.** At `frac == 0` Catmull-Rom returns `p1` exactly,
  so Rate ±1x/±2x with Warp 0 is bit-identical to before. Only fractional reads move.
- **The seam gets slightly wider.** The outer taps wrap like `layer1` always has, so
  at the seam they pull from the opposite end of the window: the discontinuity now
  spans two frames each side instead of one. Accepted — seam crossfade is still
  deferred and is the proper fix.
- **Does not rescue very short loops.** At `len = 3` there are only three distinct
  images; four taps have nothing to work with and the seam is a third of the window.
  Rate ≠ 1 wants 20-60 frames. That judder is content starvation, not filter quality.

### Verification
- `cargo test -p varispeed-dsp`: **28/28** (23 pre-existing + 5 new covering the
  four-tap layout, seam wrap in age space, backward wrap at window/block start, and
  the degenerate `len = 1` case where all four taps alias).
- `varispeed_read` built clean on MSVC; new shader source confirmed present in the
  DLL. GLSL only compiles at runtime in Resolume, so a shader error would surface
  as a black or frozen read — worth a look on first load.
- **Deploy blocked** (Resolume open). `make deploy PLUGIN=varispeed_read`.
