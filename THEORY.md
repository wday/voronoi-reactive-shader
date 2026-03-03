# Shader Math Deep Dive

Mathematical foundations of `shaders/voronoi_reactive.fs`, walked through
function-by-function from building blocks up to final composition.

Reference: [session-math-deep-dive.md](scripts/session-math-deep-dive.md)

---

## 1. Hash Functions (lines 117–127)

### What they do

Both `hash1` and `hash2` map a 2D input to a pseudorandom output (scalar or
2-component) in [0, 1). They're the source of all randomness in the shader.

### The construction, step by step

Take `hash1`:

```glsl
vec3 p3 = fract(vec3(p.xyx) * 0.1031);       // (1) scale + fold
p3 += dot(p3, p3.yzx + 33.33);                // (2) mix via dot product
return fract((p3.x + p3.y) * p3.z);           // (3) collapse + fold
```

**Step (1)** — `p.xyx` promotes the 2D input to 3D by repeating x. The multiply
by `0.1031` rescales the input into a fractional range, and `fract()` wraps it
to [0, 1). This is the first "folding" — it maps the infinite integer lattice
into a bounded domain.

**Step (2)** — The dot product against a *permuted* version of itself (`p3.yzx`)
creates cross-component mixing. The `+ 33.33` offset ensures that even inputs
near zero produce large intermediate values. This is the *avalanche* step —
small input changes cascade into large output changes.

**Step (3)** — A final multiply-and-`fract` collapses back to a scalar. The
multiplication is the key nonlinearity: it creates high-frequency variation that
`fract` then wraps.

### Why these constants?

The constants (0.1031, 0.1030, 0.0973, 33.33) are empirically tuned by Dave
Hoskins. The design goals:

- **Irrational-ish ratios** — the three scale factors are close but not equal
  (0.1031, 0.1030, 0.0973). If they were identical, the swizzled components
  would be correlated. The slight differences break symmetry.
- **Large offset (33.33)** — prevents the dot product from being near-zero for
  small inputs, which would kill the avalanche effect.
- **No special mathematical significance** — unlike, say, the golden ratio
  constants in some hashes. These were found by visual inspection of
  distribution quality on GPU.

### Why "no sine"?

The classic GPU hash is
`fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453)`. The problems:

1. **Precision** — `sin()` on large inputs depends on the GPU's trig
   implementation. Different GPUs compute `sin(78432.7)` differently. The
   high-frequency part of the sine curve amplifies these errors, so the "random"
   output can vary across hardware.
2. **Periodicity** — `sin` is periodic. For large enough inputs the hash starts
   repeating in visible patterns.
3. **Speed** — on some mobile GPUs, `sin` is a multi-cycle instruction.
   Multiply-and-`fract` is uniformly fast.

Hoskins' hash uses only `fract`, `dot`, `+`, and `*` — all of which are exact
(or nearly exact) across GPU architectures. The result: portable, consistent
randomness.

### Distribution quality

This class of hash isn't cryptographically uniform, but it's *visually*
uniform — the autocorrelation is low enough that human eyes can't detect
patterns. For Voronoi seed placement, what matters is:

- No visible clustering or alignment artifacts
- Smooth variation as the cell coordinate changes by 1

Both hold well for this hash. If you ran a chi-squared test on a 256×256 grid
of outputs you'd see minor deviations from perfect uniformity, but nothing that
shows up in the rendered output.

### `hash2` differences

```glsl
vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
```

The three *different* scale constants mean the three intermediate components
decorrelate faster. The output extraction also differs:

```glsl
return fract((p3.xx + p3.yz) * p3.zy);
```

This produces *two* outputs from the same intermediate state by using different
swizzle patterns (`p3.xx + p3.yz` vs `p3.zy`). The two output components are
not statistically independent (they share the same intermediate `p3`), but
they're uncorrelated enough for Voronoi seed placement.

---

## 2. Voronoi / Worley Noise (lines 158–204)

### F1 and F2: geometric meaning

The shader evaluates Voronoi on a grid. Each integer cell `(i, j)` gets a *seed
point* (random position inside the cell). For any query point `p`:

- **F1** = distance to the nearest seed
- **F2** = distance to the second-nearest seed

Geometrically:

- **F1** defines the classic Voronoi diagram. The locus `F1 = 0` is the set of
  seed points themselves. The Voronoi *cell* for seed `s` is
  `{p : F1(p) = |p - s|}` — the region closer to `s` than to any other seed.
- **F2 − F1** measures how close the query point is to a *cell boundary*. On the
  boundary itself, the two nearest seeds are equidistant, so `F2 − F1 = 0`. Deep
  inside a cell, `F2 − F1` is large. This is what the shader uses for edge
  detection (line 235: `edgeDist = vor.y - vor.x`).

The key insight: **F2 − F1 is a smooth scalar field whose zero set is exactly
the Voronoi edge network.** That's why it's so useful — you get edges "for free"
from the distance computation, no derivative or finite-difference needed.

### Why the 3×3 neighborhood search is sufficient

The loop (lines 167–201) checks a 3×3 grid of cells centered on the cell
containing the query point. Is this always enough?

The seed for cell `(i, j)` is confined to `seedBase ∈ [0.1, 0.9]` within the
cell (line 174: `seedHash * 0.8 + 0.1`). The drift can push the seed further —
up to about `±0.35` from circular drift or `±0.35` from chaotic drift (since
`0.7 * 0.5 = 0.35`). Worst case, a seed sits at approximately:

```
seedBase + drift ∈ [0.1 - 0.35, 0.9 + 0.35] = [-0.25, 1.25]
```

So a seed can wander at most ~0.25 units outside its home cell. The query point
`localP ∈ [0, 1)`. The maximum distance at which a seed in a non-neighbor cell
could be the nearest seed would require it to be closer than any seed in the 3×3
neighborhood — but seeds two cells away are at minimum `1 - 0.25 = 0.75` units
from the cell boundary, while there's always a seed within the 3×3 block that's
closer. So 3×3 is sufficient with comfortable margin.

If you removed the `* 0.8 + 0.1` margin clamping and let seeds occupy the full
[0, 1] range, or if drift amplitudes were larger, you'd need a 5×5 search.

### Seed animation model

Each seed has two motion modes:

**Circular drift** (lines 177–178):

```glsl
float angle = 6.2832 * seedHash.x + animTime * (0.3 + seedHash.y * 0.7);
vec2 circularDrift = 0.35 * vec2(cos(angle), sin(angle));
```

This is simple circular motion: `(r·cos(ωt + φ), r·sin(ωt + φ))` with:

- Phase `φ = 2π · seedHash.x` — different starting angle per cell
- Angular velocity `ω = 0.3 + seedHash.y · 0.7` — varies per cell in
  `[0.3, 1.0]`, so seeds orbit at different speeds
- Radius `r = 0.35`

Result: smooth, predictable orbits. Visually calm and periodic.

**Chaotic drift** (lines 181–186):

```glsl
float phase = floor(animTime * 0.3);
float blend = fract(animTime * 0.3);
blend = blend * blend * (3.0 - 2.0 * blend);   // smoothstep
vec2 rA = hash2(cellPos + vec2(phase * 17.3, phase * 7.1)) - 0.5;
vec2 rB = hash2(cellPos + vec2((phase+1.0) * 17.3, (phase+1.0) * 7.1)) - 0.5;
vec2 chaoticDrift = mix(rA, rB, blend) * 0.7;
```

This is a *keyframe random walk*:

- Time is quantized into phases at rate `0.3 × animTime`
- At each phase boundary, a new random target (`rB`) is generated by hashing
  `cellPos + f(phase)`
- The seed smoothly interpolates from the previous target (`rA`) to the new
  target
- The `blend * blend * (3.0 - 2.0 * blend)` is the Hermite smoothstep — same
  function used in the value noise. It ensures C1 continuity at the keyframe
  transitions (velocity = 0 at each keyframe), so there's no visible "snapping."

Without the smoothstep, you'd have linear interpolation between random
targets — C0 continuous but with visible velocity discontinuities (sharp
direction changes). The smoothstep makes the motion feel organic.

The `* 17.3` and `* 7.1` offsets on the phase ensure that the hash inputs for
different phases are well-separated in input space, avoiding correlated
consecutive targets.

**The chaos blend** (line 188):

```glsl
vec2 drift = mix(circularDrift, chaoticDrift, driftChaos);
```

Linear interpolation between the two modes. At `driftChaos = 0`: pure circular
orbits. At `driftChaos = 1`: pure random walk. At `0.3` (default): mostly
circular with some random perturbation — the seeds wobble around their orbits.

---

## 3. Value Noise & Spatial Warp (lines 132–143, 215–219)

### Hermite interpolation: why 3t² − 2t³?

In `valueNoise2`, line 135:

```glsl
f = f * f * (3.0 - 2.0 * f);   // f ← 3f² - 2f³
```

This is the Hermite basis function `h(t) = 3t² - 2t³`, mapping `[0,1] → [0,1]`
with:

- `h(0) = 0`, `h(1) = 1` (interpolation)
- `h'(0) = 0`, `h'(1) = 0` (C1 continuity)

Why not linear (`f = f`)? With linear interpolation between grid values, the
derivative is *discontinuous* at integer lattice boundaries. You'd see visible
seams where the gradient jumps. The Hermite smoothstep guarantees the noise field
has continuous first derivatives everywhere, so the warp displacement varies
smoothly.

Why not `6t⁵ − 15t⁴ + 10t³` (Perlin's improved smoothstep)? That gives C2
continuity (continuous second derivatives), which matters for lighting
computations where you differentiate the noise to get normals. For a
displacement warp like this, C1 is sufficient — the human eye doesn't detect
second-derivative discontinuities in color fields.

### The noise itself

```glsl
vec2 a = hash2(i) - 0.5;
vec2 b = hash2(i + vec2(1,0)) - 0.5;
vec2 c = hash2(i + vec2(0,1)) - 0.5;
vec2 d = hash2(i + vec2(1,1)) - 0.5;
return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
```

This is *bilinear interpolation* (with Hermite-smoothed coordinates) of random
vectors at the four corners of the integer cell. Each corner gets a random 2D
offset in `[-0.5, 0.5]`. The `- 0.5` centering means the noise has zero mean —
important for a warp displacement, so the average UV position doesn't shift.

Note: this is *value noise*, not *gradient noise* (Perlin noise). Value noise
interpolates random *values*; gradient noise interpolates random *gradients*
dotted with position offsets. Value noise has more low-frequency energy and a
slightly "blobby" character. For a spatial warp, this is fine — you want broad,
smooth distortions, not the fine directional detail that gradient noise provides.

### The two-octave warp (lines 216–218)

```glsl
vec2 warpOffset = valueNoise2(uv * 3.0 + TIME * 0.08);
warpOffset += valueNoise2(uv * 7.0 - TIME * 0.05) * 0.5;
uv += warpOffset * warp * 0.25;
```

Two octaves of noise are summed:

| Octave | Frequency | Amplitude | Time drift |
|--------|-----------|-----------|------------|
| 1      | 3.0       | 1.0       | +0.08      |
| 2      | 7.0       | 0.5       | −0.05      |

**Is this standard fBm?** Close but not quite. In standard fractional Brownian
motion you'd have:

- Frequency ratio (lacunarity) = 2.0 (each octave doubles)
- Amplitude ratio (gain/persistence) = 0.5 (each octave halves)

Here the frequency ratio is 7/3 ≈ 2.33, and the amplitude ratio is 0.5. So the
lacunarity is higher than standard fBm but the persistence is the same. The
effect: the second octave adds detail at a frequency that's more than double the
first, giving a slightly more "stretched" quality to the warp than standard fBm
would.

The opposite time drift directions (`+0.08` vs `−0.05`) mean the two octaves
slide past each other over time, creating a slowly evolving, non-repeating
distortion pattern.

**Feedback into Voronoi UV space**: The warp modifies `uv` *before* it enters
the Voronoi computation. This means the Voronoi cells themselves get
distorted — their boundaries curve and wobble. The warp doesn't change the cell
topology (same number of cells, same neighbors), just the shapes. At high `warp`
values the cells can look like biological/organic structures rather than
geometric polygons.

---

## 4. Multi-Layer Composition (lines 227–258)

### Scale progression

```glsl
float scale = density * pow(layerSpread, fl);   // fl = 0, 1, 2
```

With defaults `density = 8`, `layerSpread = 2`:

| Layer | Scale | Cells across screen |
|-------|-------|---------------------|
| 0     | 8     | ~8                  |
| 1     | 16    | ~16                 |
| 2     | 32    | ~32                 |

This is a *geometric frequency cascade* — each layer has `layerSpread×` more
cells than the previous one. It's the same principle as fBm octaves, but applied
to Voronoi patterns instead of noise.

With `layerSpread = 2.0` you get a standard octave doubling. Higher values (up
to 4.0) space the layers further apart in frequency, making them more visually
distinct. Lower values (down to 1.5) pack them closer, creating a denser, more
textured look.

### Layer weighting

```glsl
float layerWeight = pow(layerMix, fl);
if (layer == 0) layerWeight = 1.0;
```

| Layer | Weight (layerMix = 0.5) |
|-------|-------------------------|
| 0     | 1.0 (forced)            |
| 1     | 0.5                     |
| 2     | 0.25                    |

Layer 0 is always full-strength. Subsequent layers contribute with exponentially
decreasing weight. At `layerMix = 0`: only layer 0 is visible (single-scale
Voronoi). At `layerMix = 1`: all three layers contribute equally.

The normalization at line 258:

```glsl
color /= max(totalWeight, 0.001);
```

divides by `1 + layerMix + layerMix²`, keeping the overall brightness roughly
constant regardless of `layerMix`.

### Contrast and brightness

```glsl
layerColor = clamp((layerColor - 0.5) * contrast + 0.5, 0.0, 1.0);  // per-layer
color *= brightness;   // after blending
color *= tint.rgb;     // final tint
```

The contrast transform is an affine map around the midpoint 0.5:

```
output = contrast × (input − 0.5) + 0.5
```

At `contrast = 1`: identity. At `contrast > 1`: values are pushed away from
mid-gray (darks get darker, lights get lighter). At `contrast < 1`: values
compress toward mid-gray.

Note that contrast is applied *per layer before blending*, while brightness is
applied *after blending*. This ordering matters — if you swapped them, high
contrast on individual layers would clip before blending, losing information.
Doing contrast first and then brightness means you can independently control the
dynamic range (contrast) and overall level (brightness).

---

## 5. Edge Detection & Glow (lines 241–249)

### `smoothstep` as a soft threshold

```glsl
float edgeFactor = 1.0 - smoothstep(0.0, max(edgeWidth, 0.001), edgeDist);
```

Recall `edgeDist = F2 - F1`, which is 0 on cell boundaries and positive inside
cells. The `smoothstep(0, w, d)` function returns:

```
           ⎧ 0                          if d ≤ 0
s(d) =     ⎨ 3(d/w)² − 2(d/w)³         if 0 < d < w
           ⎩ 1                          if d ≥ w
```

So `1 - smoothstep(...)` is 1 on the boundary and falls to 0 over a distance of
`edgeWidth`. This creates a soft edge line with width controlled by `edgeWidth`.

### The glow layer

```glsl
float glowRange = edgeWidth * (1.0 + edgeGlow * 4.0);
float glowFactor = (1.0 - smoothstep(0.0, glowRange, edgeDist)) * edgeGlow;
float totalEdge = clamp(max(edgeFactor, glowFactor), 0.0, 1.0);
```

The glow is a *second, wider* smoothstep threshold with range
`edgeWidth × (1 + 4·edgeGlow)`. At default `edgeGlow = 0.4`:

```
glowRange = edgeWidth × 2.6
```

So the glow extends about 2.6× further than the hard edge. The glow factor is
also scaled by `edgeGlow`, so it's dimmer than the core edge.

The `max(edgeFactor, glowFactor)` ensures the core edge is never dimmed by the
glow. Visually: a bright core line with a softer halo around it.

### Edge color

```glsl
vec3 edgeRGB = hsv2rgb(vec3(hue, colorSat * 0.2, 1.0));
```

Same hue as the cell interior, but:

- Saturation dropped to 20% of `colorSat` — much more desaturated (closer to
  white)
- Value (brightness) set to 1.0 — full brightness

This creates bright, pale edge lines that "glow" against the more saturated cell
interiors. The hue continuity means edges share the color family of their cell
rather than being a flat white.

---

## Customization Hooks

Places where different choices produce visually distinct results:

| What to change | Where | Effect |
|---|---|---|
| Seed confinement range | Line 174, `0.8 + 0.1` | Wider range → more irregular cells, but may need 5×5 search |
| Drift radius | Lines 178, 186 | Larger → more motion, cells can "swap" neighbors |
| Noise type for warp | Lines 132–143 | Swap value noise for curl noise → divergence-free warp, more fluid-like |
| fBm octave count | Lines 216–218 | More octaves → finer warp detail |
| Distance metric | Line 191, `length()` | Manhattan (`abs(d.x)+abs(d.y)`) → diamond cells; Chebyshev (`max(abs(d.x),abs(d.y))`) → square cells |
| Edge function | Line 235 | Use `F1` alone → distance-field shading instead of edges |
| Layer count | Line 227 | More layers → richer texture but heavier GPU cost |
| Smoothstep → quintic | Line 135 | C2 continuity, subtly smoother warp |
