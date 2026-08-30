# Slipgrid — Devlog

## 2026-07-12 — atom built, shader design reversed twice under measurement

Order of work: shader first (in its final home so the harness globs it), verified
headlessly against synthetic sources, then the Rust plugin, then register/build/deploy.

The interesting part is that **the shader design changed twice, both times because
a measurement contradicted the spec.** Recording it because the traps generalise
to any gather-based spatial atom.

### Trap 1 — a pixel-scale field probed at tile-scale spacing reads as zero

First implementation computed edge energy `E = |∇luma|` with a **2-texel** stencil,
then sampled `∇E` at points half a tile apart. At an 8×8 grid a tile is ~64px, so
those probes land in flat interior and read `E = 0` almost everywhere. The shader
fell through to its `mag < 1e-5` fallback on every hop, and **all four gravity
settings produced byte-identical output**.

Fix: measure `E` at the scale the tiles actually move in — a **half-tile** stencil.
`E` then peaks on tiles that *straddle* a contour and is ~0 on tiles wholly inside
or outside one.

### Trap 2 — gather cannot express attraction (the load-bearing one)

With `E` fixed, gravity did something, but nothing like the ask. Under loose
displacement (`out(T) = src(walk(T))`), **both step directions are wrong**:

- step **up**-gradient → every tile samples *from* an edge tile → edge content is
  duplicated and sprayed across the frame. Edges spread, they don't gather.
- step **down**-gradient → every tile samples from the flat region away from the
  nearest edge → the whole frame drains to background. The picture erases.

This is structural, not a sign error. Gather chooses where a tile reads *from*;
attraction is a claim about where tiles *land*. Worse, attraction means a **finite
supply** of tiles piling up somewhere — and loose displacement doesn't conserve
tiles, so bright content leaks everywhere instead of accumulating. Measured: at
full gravity the walk and attractor formulations both score *below* the do-nothing
baseline, and collapse the frame to **18/64 and 9/64 distinct source tiles**.

Fix: **Conserve mode** — a bijective swap. Pair the grid into dominoes (global
axis/distance/phase per round, so pairing is mutual *by construction*) and accept
a swap when it puts the brighter tile on the edgier one. The acceptance expression
`ΔL · ΔE · sign(gravity)` is symmetric in the pair, so both tiles reach the same
verdict without communicating — the pairing never throttles gravity, which was the
flaw in the mutual-agreement *walk* considered (and rejected) at spec time.

Result: 64/64 tiles conserved, and accretion works (see below).

### Trap 3 — the test image was lying (twice)

Two invalid measurements nearly sent this the wrong way:

1. **Correlated masks.** First metric asked "luminance on contour tiles", but in
   that test image the contour tiles *were* the bright tiles (a disc's rim), so the
   metric partly just measured content staying put. Both broken formulations scored
   well.
2. **Aliased stencil.** Second test used a fine checkerboard as the "edgy" region —
   with period exactly half a tile, which **aliases perfectly against the shader's
   half-tile stencil**. Every probe landed on the same phase, so the shader saw
   `E = 0`: it considered the checkerboard perfectly flat. Accretion was being
   measured onto a region the shader cannot see as an edge.

Final honest test: a bright **flat** square (luminance, no edges) plus a large
mid-gray **disc** (a true contour), with the edge mask built by replicating the
**shader's own `E` formula** — so the question is "did luminance accumulate on the
tiles the shader itself calls edges?", which can't beg the question.

That second failure is also a real property, now documented as a limit: this
detector sees **coarse contours, not texture busyness**. Fine detail does not attract.

### Result (final merged shader, Conserve, intensity=1, locality=3, iters=6)

Luminance on the shader's own edge tiles — baseline 91.8, frame mean 51.2:

| gravity | 0.0 | 0.5 | 1.0 | −1.0 |
|---|---|---|---|---|
| edge-tile luminance | 62.4 | 72.3 | **112.1** | 57.0 |

Gravity 0 lands near the frame mean (a random shuffle dilutes edge tiles — correct).
Gravity 1 climbs well above both the shuffle *and* the untouched baseline: bright
content is genuinely accreting onto contours. Gravity −1 repels.

Bijection: Conserve 64/64 distinct source tiles; Smear 14/64 (duplicates by design).
Identity exact at `Intensity = 0` and `Dry/Wet = 0` in both modes.

### Multi-pass was prototyped and rejected

Rounds 2..N of a single gather pass decide on **stale** luminance (a gather shader
can't know where content moved in an earlier round), so in principle `Iterations`
isn't a real annealing schedule. Built the correct version — FBO ping-pong, one
true involution round per pass — to see what it bought.

It bought nothing: **ping-pong 110.5 (6 passes) / 116.9 (12) vs 112.1 for the
single pass.** The single-pass composition is still a valid permutation with the
right bias. Dropped the FBOs; the atom stays a stateless single pass.

### Decisions taken with the user

- Name **Slipgrid**, `SlpG`, 16-byte name `"Slipgrid        "`.
- Frozen/seeded rather than self-animating; Seed is the automation handle.
- Loose displacement was the user's original call. Kept — as **Smear mode**, beside
  Conserve — after the evidence showed it can't do gravity. Mode knob is param 6.

### Status

Built clean (MSVC) and deployed to Resolume Extra Effects. **Not yet driven with
live footage** — the accretion numbers are all from synthetic test images.

## 2026-08-30 — Snap tile hops to whole texels

Found during a sweep for the resample bug fixed in `mirror-transform` the same day.

The shader carries `local = fract(v_uv * u_grid)` through untouched and rebuilds
the source coordinate as `(ti_new + local) / u_grid`, with the comment "a displaced
tile is a pixel-exact copy of its source". **That was only true when the grid
divides the frame.** The hop is `(ti_new - ti_home)/u_grid`, an integer number of
texels only if the tile size in pixels is an integer — grid 8 into 1920 is 240px
exactly, grid 7 is 274.3px. At any non-divisor grid every hop lands at a fractional
texel offset and bilinear resamples the whole tile.

Harmless as a one-shot effect (a half-texel softening nobody would notice). Not
harmless in a feedback loop, where the resample is applied once per lap and
compounds — the same mechanism that was blurring out zoom tunnels.

### Fix: snap, don't filter
```glsl
vec2 content_texel = u_texel / u_uv_scale;        // one source texel, in uv
vec2 hop = (ti - floor(tf)) / u_grid;
hop = round(hop / content_texel) * content_texel; // -> whole texels
vec2 src = v_uv + hop;
```
A whole-texel fetch has **zero** loss, which beats any interpolation filter — so
this is strictly better here than the Catmull-Rom used in `mirror-transform` and
`channel-displace`, where the offsets are genuinely continuous and can't be snapped.
Costs at most half a texel of tile-boundary placement, which is invisible.

### Measured (headless, 256x256 uniform noise, grid 7 — deliberately a non-divisor)
Fraction of output pixels that are *exactly* some source pixel (an interpolated
pixel essentially never matches one exactly on random content):

| | exact source pixels |
|---|---|
| before | 2.5% |
| after | **100%** |

So the permutation is now genuinely lossless for any grid value, and the docstring's
claim is true for the first time.

Built, deployed. Shader compile-checked headlessly before deploy.
