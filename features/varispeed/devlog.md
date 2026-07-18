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
