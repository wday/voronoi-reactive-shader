# Varispeed — Requirements (draft v0.1, 2026-07-08)

A new, **isolated** delay-line atom: a variable-rate playback head over a captured
frame buffer. It plays a recorded loop **forwards, backwards, faster, slower, or
frozen**, with sub-frame-smooth motion and beat-synced Doppler warble. Implements
the old `DEFER-SWEEP` idea (sweepable continuous time) but reframed around
*looping* rather than *delay*.

> **This does NOT touch the shipped model.** Delay Tap (`DlyT`), Delay Write
> (`DlyW`), and delay-core's current fixed-modulus addressing stay **exactly as
> they are** — simple, effective, validated, shipped. Varispeed is a separate atom
> so the proven echo can't regress into the experimental/chaotic path. See §2.

---

## 1. Why a separate atom

The current delay ties the ring modulus to `loop_length` and the Write only records
into that sub-range; the read is a fixed integer offset. Variable-rate playback
needs a **different tape topology** (a fixed full-depth ring with a *floating
fractional* read), and it opens a semi-chaotic feedback regime. Bolting that onto
`DlyT` would bloat it and put the stable echo one knob-twist from runaway. So:

- `DlyT` = "give me a clean echo." Varispeed = "detune / warp / scrub the loop."
- Different intents → different boxes. They may still share cross-DLL tape
  infrastructure (see §9 open questions), but Varispeed does **not** change `DlyT`
  / `DlyW` behaviour.

**Separate Read + Write modules (decided 2026-07-08).** Varispeed is a *pair*, not a
monolith, for the same reason the current delay is: FX must be insertable **between**
the read and the write (`source → Varispeed Read → FX… → Varispeed Write → tape`).
That in-loop FX chain is what produces the fractal / feedback chaos — each lap the
recirculating content is re-processed. (When frozen, the same FX chain just
processes the output for display instead of feeding back.)

**Single tape (decided 2026-07-08).** No multi-channel: Varispeed uses one shared
ring buffer, so there is **no Channel param**. Its Read and Write pair on that one
tape. (This also sidesteps any collision with `DlyW`'s channels — Varispeed's tape
is entirely its own.)

## 2. Mental model — two effects, only one accelerates

The crux the design turns on: **is the write head inside the loop window the read
is playing?**

- **Varispeed (stable):** nothing is being re-recorded into the window being read,
  so the loop just plays at its rate — pitched/reversed/slowed but steady. Warble
  appears only during a *transition* (changing rate live), then settles.
- **Feedback pitch-shift (accelerating):** read rate ≠ write rate *while
  re-recording into the read window* → every lap resamples the fed-back content by
  the rate ratio → motion accelerates geometrically per lap (`1.5×, 2.25×, 3.4×…`)
  while brightness decays by loop gain `Send·Wet`. `g<1` fades the cascade out;
  `g≥1` runs it to temporal aliasing (the video "ultrasonic" = motion faster than
  the frame rate → strobing) + clamped brightness runaway. Semi-chaotic; explore
  empirically.

`Rate = 1` collapses to the ordinary stable delay (no resampling, no compounding).

## 3. Architecture — fixed ring, floating fractional read

- **Write is a fixed metronome.** Exactly one live frame recorded per host render
  frame, into `write_slot = frame_index mod N` (N = ring depth, e.g. the current
  240). The write never changes rate and never skips — the tape is always a
  faithful, complete, constant-rate recording of the last N frames.
- **Read is a floating float cursor.** `read_pos += Rate` per frame (a float, may
  be negative). The read, not the write, owns all speed/direction. "Speeding up"
  means the read samples the recording at a stride > 1 (correct fast-forward, not a
  glitch); "slowing" means it advances < 1 frame and repeats/interpolates.
- **Sub-frame interpolation.** A fractional `read_pos` samples
  `mix(layer_floor, layer_floor+1, frac)` — linear between adjacent frames — so
  slow-mo and non-integer rates are smooth, not stuttered. (Higher-order interp is
  a possible later refinement; linear first.)
- **Loop length `L` is a read-side wrap window, decoupled from `N`.** `N` is only
  max capacity. A loop = `read_pos` cycling an `L`-frame window (`1 ≤ L ≤ N`) at
  `Rate`. Arbitrary loop lengths, exactly like today, expressed on the read side.

## 4. The pinned-write reality (a hard constraint, not a bug)

The write is **pinned to Resolume's render clock** — one real frame in per render
frame; we cannot make the host feed frames faster or slower. Consequences:

- **Capture → freeze → warp is the clean, primary gesture.** Record at 1×, stop
  recording, then varispeed the frozen `L`-window: stable pitched/reversed/warped
  loop of exactly `L` frames, forever, no accumulation.
- **Overdubbing *into* a warped loop cannot stay coherent.** A layer written at 1×
  against a read at `r×` resamples every lap → the compounding regime. This is
  messy in analog varispeed too; it is the pinned-write reality, not a defect to
  design away. Live `Rate ≠ 1` is therefore the **opt-in experimental** path.

## 5. Controls (conceptual)

Split across the two modules. No Channel param (single tape, §1).

**Varispeed Read** (the playback head — owns all time/position):

| Control | Type | Meaning |
|---|---|---|
| **Loop Length (L)** | time | read wrap window; Sync Mode {Subdivision, Ms, Frames} like `DlyW`'s Time; `1..N` |
| **Rate** | signed ratio | playback speed/direction: `0` = freeze-frame, `1` = normal, `2` = double, `0.5` = half, `−1` = reverse. Beat-ratio by default (see §7), Free mode optional |
| **Warp Depth** | gain | Doppler wobble amount (frames / fraction of a beat the read position swings) |
| **Warp Rate** | subdivision | beat-synced LFO frequency of the Doppler wobble (§7) |
| **Dry / Wet** | gains | as `DlyT`: source injection vs played-back loop (two independent gains) |
| **Blend Space** | option | Perceptual / Linear, as `DlyT` (default Linear) |

**Varispeed Write** (the recorder — full-ring):

| Control | Type | Meaning |
|---|---|---|
| **Send / Record** | gain | record/dub level (as `DlyW`). **Send=0 = freeze** (stop writing, KEEP the buffer). There is NO dedicated Freeze control — freezing is just "not recording." Exact control shape (continuous gain vs toggle) is deferred (§9). |

## 6. Regimes (what each combination does)

| Goal | Record | Rate | Result |
|---|---|---|---|
| Clean echo, any length | live | 1 | equals `DlyT` (use `DlyT`) |
| **Stable pitched/reversed loop, length L** | **frozen** | r | plays L at speed r forever — the headline gesture |
| Doppler warble → settle | live → freeze | r | transient warble during handoff, then stable |
| Accelerating cascade | live | r | compounding chaos (opt-in) |

## 7. Beat-sync

- **Rate as beat-ratio (default).** Since `Loop Length` is a musical window,
  `Rate ∈ {±1/4, ±1/2, ±1, ±2, …}` keeps warped/reversed loops on the grid: the
  loop repeats every `L / Rate`. A **Free** mode allows deliberate drift /
  polyrhythm.
- **Doppler around a subdivision.** A separate oscillating warp on the read
  *position*: `read_pos = center(L,Rate) + WarpDepth · sin(warp_phase · 2π)`, where
  `warp_phase = fract(host_beat.barPhase · beats_per_bar / WarpRate_beats)` — driven
  by Resolume's **`host_beat.barPhase`** (0..1 per bar, confirmed present in
  ffgl-core, alongside `bpm`). Because it is bar-locked, the wobble is rhythmic,
  repeatable, and re-syncs each bar — set `Loop = 1/4`, `WarpRate = 1/8` for a warble
  twice per beat. A sinusoidal *position* wobble makes the instantaneous *rate*
  oscillate around nominal → that is the Doppler; pitch swing scales with
  `WarpDepth × WarpRate`. (Assumes 4/4 / 4-beat bar for subdivision→cycles-per-bar;
  refine if odd meters matter.)

## 8. Requirements (testable)

- **VS-RING** — Playback SHALL address a fixed-depth ring (`write_slot =
  frame_index mod N`); the read SHALL be a float `read_pos` independent of the
  write. Changing Rate/Loop Length SHALL NOT reseed or reallocate the tape.
- **VS-INTERP** — A fractional `read_pos` SHALL return a **Catmull-Rom cubic**
  across four consecutive frame layers: the two bracketing the position, plus one
  either side to shape the curve. All four wrap inside the loop window. At
  `frac == 0` the cubic returns the `layer0` frame exactly, so integer reads
  (Rate ±1x, ±2x with Warp Depth 0) are bit-identical to a direct fetch.

  > **Amended 2026-08-30.** Was a linear blend of the two bracketing layers.
  > Upgraded because linear `mix()` is a pure two-tap average, so at fractional
  > rates it low-passed the loop a little on *every lap* — a second per-lap blur
  > alongside the (now removed) half-res spatial one, active whenever Rate is
  > fractional or Warp Depth > 0. The cubic is both smoother between frames and
  > slightly sharper, so it addresses the judder and the detail loss together.
  > Overshoot is clamped to [0,1] in the shader: the tape stores encoded values
  > and out-of-range results would otherwise recirculate. See `devlog.md`.
- **VS-LOOP-WINDOW** — Loop Length `L` SHALL define the read wrap window
  (`1 ≤ L ≤ N`), independent of `N` and of the write. Verifiable: a frozen
  `L`-frame loop played at `Rate` repeats with period `L / |Rate|` output frames.
- **VS-RATE** — Rate SHALL support `0` (freeze-frame), `1` (normal), `>1` (faster),
  `0<r<1` (slower/interpolated), and `<0` (reverse). Rate=1 SHALL be identical to a
  fixed-offset delay of `L`.
- **VS-FREEZE** — With recording disabled, the captured `L`-window SHALL play at
  `Rate` indefinitely with **no** temporal accumulation (stable). Verifiable: a
  frozen loop at any `Rate` returns to a bit-identical frame every `L / |Rate|`
  output frames (modulo interpolation at fractional positions).
- **VS-LIVE-WARP** — With recording enabled and `Rate ≠ 1`, the loop MAY exhibit
  the accelerating feedback cascade (§2). This is permitted/experimental, not
  guarded; documented as such.
- **VS-BEATSYNC-RATE** — In beat-ratio mode, Rate SHALL snap to musical ratios so a
  frozen loop stays grid-aligned; a Free mode SHALL allow continuous rate.
- **VS-DOPPLER** — Warp SHALL modulate `read_pos` sinusoidally with `WarpDepth`
  amplitude at a `WarpRate` subdivision, phase-driven by the host beat so the
  wobble is beat-locked and repeatable.
- **VS-ISOLATION** — Varispeed SHALL NOT change the behaviour of `DlyT`, `DlyW`, or
  delay-core's existing fixed-modulus addressing. Verifiable: the delay-line
  Tier-1/Tier-2 suites pass unchanged.
- **VS-SEAM** — The loop wrap point (read_pos `L → 0`) is a content discontinuity
  (a normal looper seam) and is NOT smoothed in v1; documented.
- **VS-STORAGE** — The tape SHALL be a **half-res (0.5×) RGBA16F** texture array of
  `N` layers, reusing the delay line's approach (`CORE-FORMAT` + `WRITE-TAPE-SCALE`):
  the Write records at half resolution, the Read upscales via normalised uv + linear
  filtering (which also gives the fractional-frame interpolation a free bilinear
  spatial pass). Float removes banding; half-res keeps VRAM ~1 GB. Validated live in
  the delay line (looks good, freed substantial VRAM per Resolume stats).

## 9. Open questions / decisions

**Resolved 2026-07-08:**
- **Plugin structure = separate Read + Write pair** (not a monolith), so FX insert
  between them for the feedback/fractal chaos (§1).
- **Single tape, no multi-channel** — Varispeed owns one ring buffer of its own; no
  Channel param; never shares a tape with `DlyW` (§1).
- **Beat-phase available** — `FFGLData.host_beat` is `{ bpm, barPhase }`; `barPhase`
  (0..1 per bar) is confirmed present in ffgl-core (used in example-raw). So the
  Doppler LFO and beat-ratio Rate are **bar-phase-locked**, not just frequency-
  locked (VS-DOPPLER, §7). No `host_time` free-run fallback needed.

**Also resolved 2026-07-08:**
- **Core infrastructure = new `varispeed-core` cdylib** — clone the delay-core
  cross-DLL pattern (loader, barrier, alloc) for a single full-ring tape. Full
  isolation; the shipped `dc_*` / delay-core stay untouched.
- **Capture = continuous** — the tape always holds the last `N` frames; there is no
  arm/grab gesture. Freezing is just Send→0 (stop writing, keep buffer); the Read
  loops the last `L`.
- **No dedicated Freeze control** — freeze = "not recording" (Send=0). Its exact
  control shape is the one remaining open below.
- **Ring depth `N` = 240** (4 s @60 fps), matching the delay line.
- **Storage = half-res (0.5×) RGBA16F**, reusing the delay line's `CORE-FORMAT` +
  `WRITE-TAPE-SCALE` (validated live in Resolume — looks good, freed substantial
  VRAM per Resolume stats). See VS-STORAGE.

**Still open:**
1. **Freeze/Send control shape.** Reuse `DlyW`'s continuous Send gain (0=freeze), or
   a simpler record on/off toggle? Deferred — decide once the spike is playable.
   (The *behaviour* — Send=0 keeps the buffer and stops writing — is fixed; only the
   knob shape is open.)

## 10. Non-goals (v1)

- Seam-crossfade / beat-matched loop stitching (VS-SEAM left raw).
- Clean overdub into a warped loop (physically messy under the pinned write, §4).
- Higher-order interpolation, motion-blur multi-tap to de-strobe fast-forward
  (possible later refinement, the temporal cousin of the spatial downsample filter).
- Multi-tap / multiple simultaneous read heads.

## 11. Relationship to prior work

- Implements/replaces delay-line `DEFER-SWEEP` (sweepable continuous time), reframed
  as looping. The one-line `DEFER-SWEEP` stub in `delay-line-v3/requirements.sdoc`
  should point here once this solidifies.
- Reuses concepts from the shipped delay: Channel patching, Dry/Wet two-gain blend,
  Blend Space (Linear default), beat-synced Sync Mode for time values.
- Shares the RGBA16F / reduced-resolution tape thinking (delay `CORE-FORMAT` /
  `WRITE-TAPE-SCALE`) if it reuses delay-core storage.

## 12. Reverse mode — ping-pong block loop (addendum, 2026-07-09)

**Motivation.** Confined feedback plays cleanly only over a *frozen* buffer;
Confined + `Rate<0` reverse-scrubs a buffer being overwritten in place, so reverse
feedback needs a manual freeze (a brief coherent flash on 1x→-1x is the residual
forward-recorded content). Reverse of a *live* stream is non-causal — the reversed
version of interval `[t, t+L]` cannot be emitted until `t+L`. That is a one-block
latency, not an impossibility; it is exactly how audio reverse delays work
(ping-pong / block-reverse). This mode automates the record→freeze→play-reverse
gesture so reverse feedback runs continuously.

**Model.** A third value of the **Confine** param: `Free / Confined / Reverse`. In
Reverse, two `L`-length blocks occupy ring layers `[0, L)` and `[L, 2L)`. One block
is **active** — the Write records the live signal FORWARD into it at a 1×/frame
metronome (the Read publishes the forward record slot via the existing
`vc_set_loop_slot` barrier). The other block is **frozen** and played by the Read at
`Rate` (`-1x` = classic reverse; the Rate knob also gives reverse-fast/slow, and
`+` rates a forward block delay; `0` = freeze-frame). The blocks **swap** every `L`
frames; on swap the just-recorded block becomes the frozen playback source and the
reverse head resets to the newest frame (`L-1` for reverse rates). Feedback (a
decaying reversed tail) rides the normal `Read → FX → Write` chain: the reverse
playback enters the Read output via `Wet`, gets recorded into the active block via
`Send`, so the tail decays at `Send·Wet` per block.

**Requirements (testable):**
- **VS-REV-PINGPONG** — Reverse mode SHALL maintain two `L`-length blocks; exactly
  one records forward while the other plays at `Rate`, swapping every `L` recorded
  frames. Verifiable (dsp): the reverse play head walks block-layer indices
  `L-1, L-2, …, 0` at `Rate=-1`.
- **VS-REV-CAPACITY** — Reverse SHALL require `2L ≤ N`; `N` is **240** so a
  full-length block (`L ≤ 120`) fits twice. Free/Confined get the longer `N - 1`
  max for free. Changing `L` restarts the ping-pong (re-anchors the two blocks) but
  SHALL NOT reseed the tape (per `VS-RING`).

  > **Amended 2026-08-29.** `N` was 480 (`L ≤ 239`). Halved to pay for the tape
  > going full-res (`WRITE-TAPE-SCALE` = 1.0): half-res storage cost ~0.35
  > round-trip gain at high spatial frequencies, which mushed tight fractal
  > feedback into blobs within a few laps. Full-res at `N = 240` is ~3.98 GB at
  > 1080p; at `N = 480` it would be 7.96 GB, too much beside Resolume on a 12 GB
  > card. Accepted trade: max loop 4 s → 2 s, against observed live use of
  > 1/16..1/4 note. See `devlog.md` 2026-08-29.
  >
  > Note the slider max is `N/2 = 120` exactly, with **no `-1`**: the `depth - 1`
  > form is the delay line's N+1 stitch and does not apply here (the caps are
  > computed per mode at runtime in `varispeed-read/src/lib.rs`).
- **VS-REV-LATENCY** — Reverse output lags input by one block (`L` frames); this is
  inherent (non-causal reversal) and accepted, matching a hardware reverse delay.
- **VS-REV-FEEDBACK** — The reversed tail SHALL decay at `Send·Wet` per block (no
  manual freeze); `Send·Wet ≥ 1` sustains, as with Confined.
- **VS-REV-SEAM** — The swap boundary and per-block wrap are content discontinuities,
  not smoothed in v1 (same tolerance as `VS-SEAM`).

**Encoding note.** Confine options are `Free=0.0, Confined=0.5, Reverse=1.0`
(default `0.5` = Confined). This remaps the short-lived 2-value encoding where
Confined was `1.0`; a composition saved with Confine `1.0` will now read as Reverse
— re-select the mode.
