# Varispeed — Requirements (v0.2)

A variable-rate playback head over a captured frame buffer, split into two FFGL
plugins so an FX chain can be inserted **inside** the loop. It plays a recorded
loop forwards, slower, faster, or frozen, with sub-frame-smooth motion and
beat-synced Doppler warble.

**Varispeed is the short-loop fractal box.** Loops cap at **1 second**; the
smoother dynamics of short loops are what make fractal and Euler-soup feedback
work. Long single-channel delay is `DlyT` / `DlyW`'s job, not this atom's.

`DlyT` / `DlyW` / `delay-core` are never modified by this feature (VS-ISOLATION).

---

## 1. Two plugins, one tape per channel

**Varispeed Read** (`VsRd`) is the playback head; **Varispeed Write** (`VsWr`) is
the recorder. FX go between them:

```
src -> VsRd -> FX... -> VsWr -> tape
```

That in-loop FX chain is what produces the fractal/feedback character — each lap
the recirculating content is re-processed. When frozen, the same chain just
processes the output for display.

**Two channels.** Read and Write pair on a shared tape selected by a **Channel**
param, exactly as `DlyT` / `DlyW` do. Two independent feedback networks, each with
its own in-loop FX stack. Varispeed's tapes are entirely its own — no `delay-core`
channel is ever touched.

## 2. Mental model — is the write head inside the window being read?

- **Stable:** nothing is re-recorded into the window being read, so the loop plays
  at its rate — pitched/slowed/frozen but steady.
- **Compounding:** read rate != write rate *while re-recording into the read
  window* -> every lap resamples the fed-back content by the rate ratio -> motion
  accelerates geometrically per lap while brightness decays by loop gain
  `Send*Wet`. `g<1` fades the cascade out; `g>=1` runs to temporal aliasing and
  clamped brightness runaway. Semi-chaotic; explored empirically.

`Rate = 1` collapses to an ordinary fixed delay: no resampling, no compounding.

## 3. Architecture — fixed ring, floating fractional read

- **Write is a metronome.** Exactly one live frame per host render frame into
  `record_index mod N`. The write never changes rate and never skips. Frozen
  (`Send = 0`) it simply does not tick, so the cursor parks — freeze needs no
  separate state.
- **Read is a floating float cursor**, owning all speed and direction. It never
  advances the write cursor.
- **Sub-frame interpolation** via Catmull-Rom across four ring layers.
- **Loop length `L` is a read-side window**, decoupled from ring depth `N`.

### Round-trip period

The Read draws before the Write, so within a host frame the Read sees
`record_index = R` and reads layer `R - age`, then the Write advances to `R+1` and
records there. Content therefore recirculates every **`age + 1`** frames. A
`L`-frame loop is `age = L - 1`, which is why `age` wraps mod `L` (range
`[0, L)`, round-trip `1..L`) and why `N = L_max + 1`.

## 4. The pinned-write reality

The write is pinned to the host render clock — one real frame in per render frame.

- **Capture -> freeze -> warp is the clean gesture.** Record at 1x, stop recording,
  then varispeed the frozen `L`-window forever with no accumulation.
- **Overdubbing into a warped loop cannot stay coherent** — a layer written at 1x
  against a read at `r x` resamples every lap. That is the compounding regime; it
  is the pinned-write reality, not a defect. Live `Rate != 1` is the opt-in
  experimental path.

## 5. Controls

**Varispeed Read** — 12 params:

| # | Control | Type | Meaning |
|---|---|---|---|
| 0 | **Channel** | option `1 / 2` | patch point; Read + Write on the same Channel form a loop |
| 1 | **Sync Mode** | option | how Loop Length is read: Subdivision / Ms / Frames |
| 2 | **Subdivision** | option | loop length in beats (1/16 .. 4 bars) |
| 3 | **Loop Ms** | int `1..1000` | loop length in ms |
| 4 | **Loop Frames** | int `1..60` | loop length in frames |
| 5 | **Rate** | option | `-2x -1x -1/2x Freeze 1/2x 1x 2x` |
| 6 | **Dry** | gain | live source into the FX chain |
| 7 | **Wet** | gain | played-back loop into the FX chain |
| 8 | **Blend Space** | option | Perceptual / Linear (default Linear) |
| 9 | **Warp Depth** | gain | Doppler swing amplitude |
| 10 | **Warp Rate** | option | beat-synced LFO period |
| 11 | **Loop Mode** | option | Free / Confined (see §6) |

**Varispeed Write** — 2 params:

| # | Control | Type | Meaning |
|---|---|---|---|
| 0 | **Channel** | option `1 / 2` | patch point |
| 1 | **Send** | gain | record level. `Send = 0` = freeze (stop writing, KEEP the buffer) |

There is no dedicated Freeze control: freezing is "not recording".

## 6. Loop Mode

Both modes give a round-trip period of exactly `L` frames at `Rate = 1`; they
differ in where the Write lands, and therefore in what `Rate != 1` does.

**Free** — the Write records at the moving cursor; the Read floats `age` frames
behind it. `age` is seeded to `L - 1` (§3) and moves by `dr - rate` per frame.
- `Rate = 1` recording: `dr - rate = 0`, so `age` holds -> a fixed `L`-frame delay.
- `Rate != 1` recording: read and write rates disagree -> the **compounding**
  cascade (§2). `age` sweeps and wraps every `L / |dr - rate|` frames.
- Frozen (`dr = 0`): `age` moves at `-rate`, so the captured `L`-window plays at
  `Rate` indefinitely, stable.

**Confined** — the Read publishes the slot it is reading and the Write records back
into that same slot, so feedback accumulates **in place** over a fixed `[0, L)`
window. There is no rate mismatch at any Rate, so `Rate != 1` gives a *stable*
varispeed loop that still accumulates FX lap over lap — the controlled counterpart
to Free's chaos. Never crosses the ring-lap seam.

## 7. Multi-tap — N Free Reads on one channel

Each Read instance owns its `age` / `head` state; a **Free** Read publishes nothing
back to the tape. So N Free Reads on one channel are N independent taps off one
recording, each with its own Loop Length (= tap time), Rate, Warp, Dry/Wet and
downstream FX.

```
layer 1:  src -> VsRd (L1) -> FX A -> VsWr      <- the only Write on this channel
layer 2:  src -> VsRd (L2) -> FX B -> out
layer 3:  src -> VsRd (L3) -> FX C -> out
```

Rules:
- **Exactly one Write per channel.** `vc_write_tick` de-dups on `frame_id`, so a
  second Write in the same host frame receives the same `record_index` and
  overwrites the first.
- **Only the Write's chain recirculates.** Other taps' FX are downstream-only and
  do not accumulate lap over lap. For that, use the second channel.
- **Confined Reads cannot share a channel** — they collide on the `loop_slot`
  handoff. Multi-tap is Free-only.
- All taps live within `N - 1` frames of the cursor.

## 8. Beat-sync

- **Rate is a beat-ratio** so warped loops stay on the grid: the loop repeats every
  `L / Rate`.
- **Doppler warp** oscillates the read *position*:
  `warp = WarpDepth * sin(2*pi * fract(barPhase * beats_per_bar / WarpRate_beats))`,
  driven by `host_beat.barPhase` (0..1 per bar). Bar-locked, so the wobble is
  rhythmic and re-syncs each bar: `Loop = 1/4, WarpRate = 1/8` warbles twice per
  beat. Assumes a 4-beat bar.

## 9. Requirements (testable)

- **VS-RING** — Playback addresses a fixed-depth ring (`write_slot = record_index
  mod N`); the read is a float independent of the write. Changing Rate or Loop
  Length neither reseeds nor reallocates the tape.
- **VS-INTERP** — A fractional read position returns a Catmull-Rom cubic across
  four consecutive frame layers: the two bracketing the position plus one either
  side to shape the curve. All four wrap inside the loop window. At `frac == 0` the
  cubic returns `layer0` exactly, so integer reads (Rate +/-1x, +/-2x, Warp 0) are
  bit-identical to a direct fetch. Overshoot is clamped to [0,1] in the shader —
  the tape stores encoded values and out-of-range results would recirculate.
- **VS-LOOP-WINDOW** — Loop Length `L` defines the read wrap window (`1 <= L <=
  N-1`), independent of `N` and of the write. Verifiable: a frozen `L`-frame loop
  played at `Rate` repeats with period `L / |Rate|` output frames.
- **VS-ROUNDTRIP** — At `Rate = 1` while recording, both Loop Modes recirculate
  content with a period of exactly `L` frames. Verifiable (dsp): Free's seeded
  `age` is `L - 1` and `advance_age` holds it there at `dr = rate = 1`.
- **VS-ANCHOR** — Free's `age` is seeded to `L - 1` on instance start, on a latched
  Loop Length change, and on entry to Free mode, so Loop Length is a directly
  settable, patch-recallable tap time rather than a drift-accumulated state.
- **VS-RATE** — Rate supports `0` (freeze-frame), `1` (normal), `>1` (faster),
  `0<r<1` (slower), `<0` (reverse scrub). Rate=1 is identical to a fixed delay of
  `L`.
- **VS-FREEZE** — With recording disabled the captured `L`-window plays at `Rate`
  indefinitely with no temporal accumulation. Verifiable: a frozen loop returns to
  a bit-identical frame every `L / |Rate|` output frames, modulo interpolation.
- **VS-LIVE-WARP** — With recording enabled and `Rate != 1`, Free may exhibit the
  accelerating cascade (§2). Permitted and unguarded; documented as experimental.
- **VS-CONFINED-STABLE** — In Confined mode the Write records into the slot the
  Read published, so there is no rate mismatch: `Rate != 1` accumulates FX without
  the compounding resample cascade.
- **VS-DOPPLER** — Warp modulates the read position sinusoidally with `WarpDepth`
  amplitude at a `WarpRate` subdivision, phase-driven by the host beat so the
  wobble is beat-locked and repeatable.
- **VS-CHANNELS** — `varispeed-core` holds `NUM_CHANNELS = 2` independent tapes.
  Read and Write address one by Channel param. A tape allocates lazily on its first
  recording Write, so an unused channel costs no VRAM. An out-of-range channel is a
  no-op, never a panic.
- **VS-MULTITAP** — N Free Reads on one channel operate independently (§7). A Free
  Read never writes tape state. Verifiable: two Reads at different Loop Lengths on
  one channel produce different output from one recording.
- **VS-CAPACITY** — `N = 61`, `L_max = 60` = **1.0 s @60fps**. `N = L_max + 1` is
  the stitch guard: at `age = L_max - 1` the read layer and the write layer differ
  by `L_max < N`, so they never alias.
- **VS-STORAGE** — The tape is a **full-res RGBA16F** texture array of `N` layers
  (`TAPE_SCALE = 1.0`). Float removes banding; full-res avoids the ~0.35 round-trip
  gain loss at high spatial frequencies that mushed tight fractal feedback into
  blobs within a few laps. **965 MiB per channel at 1080p; 1.88 GiB for both.**
- **VS-SEAM** — The loop wrap point is a content discontinuity (a normal looper
  seam) and is not smoothed.
- **VS-ISOLATION** — Varispeed never changes `DlyT`, `DlyW`, or delay-core.
  Verifiable: the delay-line Tier-1/Tier-2 suites pass unchanged.

## 10. Non-goals

- **Reverse ping-pong block mode.** Removed: it forced `N = 2 * L_max` (doubling
  VRAM for a mode rarely reached for), suits long video-clip loops rather than this
  atom's short-loop fractal role, and needed more tuning than it had. `Rate < 0`
  still reverse-scrubs the buffer. Reverse *feedback* is out of scope.
- More than 2 channels (a 4x4 grid needs per-channel tape-resolution scaling —
  see §11).
- Seam crossfade; clean overdub into a warped loop; higher-order interpolation;
  motion-blur multi-tap to de-strobe fast-forward.

## 11. Headroom

Growth past 2 channels is bounded by fill rate before VRAM: 2 channels is 4 FFGL
instances plus in-loop FX, all full-frame per host frame. If more are wanted, the
lever is **per-channel tape resolution** — a Write param promoting `TAPE_SCALE`
from a constant. A channel displayed in a 4x4 grid cell (480x270) stores at 1/4
scale for 119 MiB, making 16 channels ~1.9 GiB. The half-res detail loss that
motivated `TAPE_SCALE = 1.0` only applies when tape resolution is below *display*
resolution, which a grid cell is not.

## 12. Open

- **Send control shape.** Continuous gain (as now) or a record on/off toggle. The
  behaviour is fixed (`Send = 0` keeps the buffer and stops writing); only the knob
  shape is undecided.
- **Blend Space** is a set-and-forget param occupying a playable slot. Candidate for
  removal (hard-coding Linear) if the knob count needs contracting further.
