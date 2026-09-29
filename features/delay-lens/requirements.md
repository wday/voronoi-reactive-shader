# Delay Lens — Requirements

## Goal

Give the Delay Tap a set of **degrade knobs** that model what a real camera-feedback
loop does to an image on every lap: chromatic aberration, vignette, sensor grain,
rolling shutter, and a bipolar focus/blur. Five continuous, performable controls on
the read head, fused into the pass that already exists.

## Motivation

Two things converged.

The delay's tape was stored at half resolution, and the resulting per-lap blur was
mushing tight fractal feedback into blobs (diagnosed and fixed in
`features/delay-line-v3/devlog.md`, 2026-08-29 — the tape is now full-res). But the
blur was not purely a bug: **goopy fluid feedback looked good because of it.** The
fix removed a character that was worth keeping.

The right conclusion is not to put the blur back into the tape. It's to make
softening one of several *intentional, swept* degrades on the read path — and to
notice that blur is the least interesting of them.

### The signal-processing reason this matters

Blur is the only degrade on the list that **destroys information**. A lowpass has
round-trip loop gain < 1 at high spatial frequencies, so detail decays to nothing
and fractal generation stalls. Chromatic aberration, lens distortion, shear and
grain **displace or add** detail instead: loop gain stays ≈ 1 and the structure
just moves somewhere else each lap. That is how camera feedback builds structure
rather than dissolving it, and it is why these belong in a feedback loop
specifically, not just in a look-dev toolbox.

## Design decisions (locked 2026-08-29)

| Decision | Choice | Rationale |
|---|---|---|
| **In-suite, not insert FX** | All five fused into the Tap's existing fragment pass | Anything between Tap and Write is already inside the loop — that's what the split is for. But every insert is another pass through Resolume's **8-bit inter-effect FBO**, and that quantization compounds *every lap*. Four insert plugins = four quantizations per lap; four fused knobs = zero extra passes. For 2-3 frame feedback that's the difference between detail surviving ~20 laps and ~5. Playability points the same way. |
| **On the Tap, not the Write** | Read head | Instant response when swept — a Write-side degrade only affects new writes, so a sweep takes a full lap to be heard. Also falls out correctly: `output.frag.glsl` blends `dry*live + wet*buf`, so degrade applied to `buf` leaves the Dry injection clean. The loop rots; the source doesn't. |
| **Mental model** | **Write is the tape, Tap is the lens** | Names the split for someone who has to learn it mid-set. Medium vs. optics. |
| **Param placement** | Appended after the existing 4 | Resolume caches param descriptors per `unique_id`, and `DlyT` is hardcoded (`delay-tap/src/lib.rs:211`). Appending keeps saved comps' indices valid. Reordering would scramble them. |
| **Not included** | Lens distortion, bloom/halation, exposure/AGC | Distortion overlaps `mirror-transform` (already has scale/translate) — don't duplicate an atom. Bloom is genuinely multi-pass (threshold + separable blur) and deserves its own plugin. AGC needs a luminance readback. |

## The five knobs

Each is specified by **what it does over laps**, not what it does to a still frame.
That is the only description that predicts the result.

### L1 — Focus / Bloom (bipolar, centre = off)

Sharpen one way, blur the other, clean full-res at centre. ~5 taps.

- **Bloom** side restores the character the half-res tape used to provide, but
  continuously and at full resolution. Also the fix for any aliasing/shimmer the
  full-res tape introduces at 2-3 frame loops (the downscale had been giving free
  anti-aliasing).
- **Focus** side pushes round-trip HF gain *past 1.0*, so detail grows instead of
  decaying. This is the direct lever on "fractal generation stalls".
- Risk: at high Focus the loop can run away into checkerboard/grid. `output.frag.glsl`'s
  `clamp(0,1)` bounds it. Treated as available character, not a fault.

### L2 — Aberration (bipolar)

Per-channel radial scale about **frame centre**, R and B displaced in opposite
directions. ~3 taps (2 extra).

In a loop this is a *differential zoom*: R/G/B walk different scale trajectories
and separate into nested rainbow shells over successive laps. Substantially more
generative than the fringe a single frame shows. Bipolar so the shells can be
driven inward or outward.

**Must be anchored at frame centre**, not (0,0) — same lesson as every other zoom
knob in this repo (see `reference_center_anchor_scaling`).

### L3 — Vignette

Radial luminance falloff. One `smoothstep`.

In a loop this is a **spatial gain map**: gain ≈ 1 at centre, < 1 at the edges. That
gives the feedback a stable attractor basin and stops the frame saturating
edge-to-edge. Expected to be the single most effective stabilizer of the five, and
most of why real camera feedback holds together at all.

### L4 — Grain

Per-frame hash noise added to the tape read. ~3 lines.

Once a lowpass has driven HF energy to zero there is nothing left for the transform
to amplify. Grain continuously reseeds the band the loop grows structure from. It
complements L1's Focus rather than competing with it: Focus sets the loop's HF gain,
Grain provides something for that gain to act on.

Reuse the sine-free `hash` from `shaders/voronoi_reactive.fs:160-172` (keep the
Dave Hoskins credit line), consistent with `slipgrid`.

### L5 — Shear (rolling shutter)

Per-row layer offset on the tape read: `layer - v_uv.y * shear`, wrapped mod
`buf_size`. ~5 lines.

Nearly free given the architecture. The tape is a `GL_TEXTURE_2D_ARRAY`, and array
layers **do not filter across `r`** — the coordinate snaps to the nearest layer. So a
per-row offset yields discrete row bands each drawn from a different frame: true
slit-scan, not an approximation of one.

Scales with loop length, which makes it musical — subtle at a 3-frame loop, a large
temporal smear at a 1/4 note.

Needs `buf_size` in the shader to wrap correctly in ring space; the Rust side
already has it.

## Constraints

- **C1** — Tap goes from 4 params to 9. Comparable to `contour-field` (9). Append only.
- **C2** — Single pass. No new FBOs, no ping-pong. Worst case ~10 texture fetches
  vs. 2 today; negligible at 1080p/60 on the target hardware.
- **C3** — Every knob's zero/centre position must be **bit-identical to current
  behaviour**, so an existing comp that hasn't touched them looks unchanged.
- **C4** — Degrade applies to the tape read (`buf`) only, never to `live`.
- **C5** — Works in both blend spaces (`u_gamma` 1.0 and 2.2). Decide per knob
  whether it acts before or after the gamma decode; grain and vignette are
  light-domain operations and should act on decoded values in linear mode.

## Open questions

- **Q1** — Should Focus/Bloom be one bipolar knob or two? Bipolar is fewer params
  and reads better as a performance control, but the two directions have quite
  different tap counts. Leaning bipolar.
- **Q2** — Does Aberration want a separate angle/mode (radial vs. linear)? Radial
  only for v1; linear is what `channel-displace` already does.
- **Q3** — Grain: static per-frame, or advected by the loop? Static first.

## Verification

- Tier-2 headless (`delay-gl-tests`): each knob at zero/centre → output bit-identical
  to the current Tap (C3). Then one behavioural assertion per knob (e.g. Aberration
  at max separates a white disc's channels by the expected radius).
- Live Resolume check with real footage at a 3-frame and a 1/4-note loop — the
  knobs are judged by what they do over laps, which headless can't show.
