# Varispeed — Implementation Plan (2026-07-08)

Built stage-by-stage like the delay line, each stage independently verifiable. Pure
logic and headless-GL are tested natively on WSL/llvmpipe; the FFGL cdylibs are
Windows-only (`make build`), validated live in Resolume.

Naming: **Varispeed Read** (`VsRd`, `"Varispeed Read  "`), **Varispeed Write**
(`VsWr`, `"Varispeed Write "`), shared **`varispeed-core`** (`varispeed_core.dll`).

---

## Stage 0 — `varispeed-dsp` pure crate (host/GPU-independent math) ← START HERE
Single source of truth for the read-head math, natively unit-testable (mirrors
`delay-dsp`). No GL, no ffgl.
- `Ring { depth: N }`, `Loop { anchor, len L }`.
- `Loop::sample(pos) -> Sample { layer0, layer1, frac }` — map a floating position
  (frames since loop start; may exceed L or be negative) to two adjacent ring layers
  **within the L-window** (wrap mod L) + interp weight. This is VS-INTERP + VS-LOOP-
  WINDOW in pure form.
- `advance(pos, rate) -> pos` (reverse/fractional), and beat-ratio helpers.
- `warp_offset(bar_phase, warp_rate_beats, beats_per_bar, depth) -> f32` — the
  bar-phase-locked Doppler (VS-DOPPLER).
- **Verify:** `cargo test -p varispeed-dsp` — integer pos ⇒ frac 0; seam wrap
  (pos=L−0.5 blends last↔first); reverse wrap; warp periodicity + zero at phase 0;
  Rate=1 reduces to a fixed offset (VS-RATE tie to a plain delay).

## Stage 1 — `varispeed-core` cdylib (cross-DLL tape + barrier)
Clone the delay-core pattern (loader via `pluglib`, per-frame barrier on
`host_time`, lazy alloc). Differences from delay-core:
- **Single tape** (no channels), **full ring** `write_slot = frame_index mod N`.
- **Half-res (0.5×) RGBA16F** storage (VS-STORAGE), reusing delay's alloc +
  `clear_texture_array`.
- ABI: `vc_frame_tick`, `vc_tex`, `vc_frame_index`, `vc_depth`, `vc_acquire/release`.
- **Verify:** exports present; a headless test allocs + reads back a written layer.
- Leaves `delay-core` / `dc_*` completely untouched (VS-ISOLATION).

## Stage 2 — Varispeed Write (`VsWr`)
Full-ring recorder: `tape[frame_index mod N] = Send*input`, half-res viewport (reuse
`TAPE_SCALE`), passthrough output. **Send=0 = freeze** (skip the write pass, keep the
buffer — NOT the delay's wipe). Continuous capture (always last N frames).
- **Verify (Windows/live):** records; Send=0 holds the buffer static.

## Stage 3 — Varispeed Read (`VsRd`)
Floating fractional read head. Params: Loop Length (Sync Mode), Rate, Dry, Wet, Blend
Space. New read shader: sample `layer0`/`layer1` (from `varispeed-dsp`), lerp by
`frac`, then `clamp(Dry*live + Wet*loop)` in the chosen blend space (reuse the
`u_gamma` output logic). `read_pos` accumulated per frame by Rate.
- **Verify (Tier-2 + live):** frozen loop plays at Rate; Rate=1 == fixed delay;
  reverse/slow/fast; Rate=0 freeze-frame.

## Stage 4 — Warp (Doppler) + beat-ratio Rate
Add Warp Depth + Warp Rate (subdivision) driving `read_pos` via `warp_offset` off
`host_beat.barPhase`; Rate as beat-ratio (+ Free mode).
- **Verify (live):** bar-locked warble; `Loop=1/4, WarpRate=1/8` wobbles twice/beat.

## Stage 5 — Tier-2 headless-GL harness (`varispeed-gl-tests` or extend existing)
Surfaceless EGL: real varispeed-core ring + real read shader; assert interpolated
pixels (a fractional read between two known-colour layers returns the lerp; frozen
loop periodicity; reverse).

## Stage 6 — Windows build + deploy + live exploration
`make build`/`deploy` all three (`varispeed_core.dll` beside `VsRd`/`VsWr`). Live:
frozen warped loops (the headline), reverse, beat-locked Doppler, then the **live
Rate≠1 feedback cascade** with FX inserted (`VsRd → FX → VsWr`) — the fractal chaos.

---

## Stage 7 — Reverse (ping-pong block) mode (added 2026-07-09)
Third **Confine** value `Reverse` (requirements §12). Ring depth `N` grown 240→480
so two full `L`-blocks fit. New pure `sample_block(base, pos, len, depth)` in
`varispeed-dsp` (offset cousin of `sample_confined`); Read owns the ping-pong state
(active block, forward record metronome `rec_pos`, reverse play head `play_pos`,
swap every `L`). Read publishes the forward record slot; Write is unchanged (records
into whatever slot the barrier hands it). Playback honours Rate (reverse/fwd/fast/
slow); feedback tail decays at `Send·Wet` per block via the normal chain.
- **Verify:** `cargo test -p varispeed-dsp` — block base offset, block seam wrap,
  reverse index walk (all green). Live: `Rate=-1x` reverse tail with no manual
  freeze; one-block latency; blocks swap every `L`.

## Cross-cutting
- **Isolation (VS-ISOLATION):** never edit `dc_*`/`delay-*`; the delay Tier-1/Tier-2
  suites must stay green.
- **Deferred (v1 non-goals):** seam crossfade, clean overdub-into-warp, higher-order
  interp, de-strobe multi-tap, multi-tap reads, live Freeze-control shape (§9).
- Update `delay-line-v3/requirements.sdoc`'s `DEFER-SWEEP` stub to point here once
  Stage 3 lands.
