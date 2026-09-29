# Mirror Transform — Dev Log

## 2026-03-11 — v1 implementation

Extracted the scale/rotate/swirl transform from the dream looper's ingest shader into a standalone FFGL effect. The dream looper applies these transforms internally on ingest, but the delay-line-module has no spatial transforms — chaining this effect between Send/Receive gives the same compound feedback transforms.

### What shipped
- 4 params: Scale (exponential), Rotation, Swirl, Mirror (on/off)
- Fragment shader: center → scale → swirl → rotate → uncenter → edge handle → sample
- Mirror mode: kaleidoscope fold via `1 - abs(mod(uv, 2) - 1)`
- Off mode: soft-clip fade to black at edges
- GL state save/restore (scissor, blend, depth)
- Registered in `plugins.json`

### Notes
- No FBO needed — single pass, stateless, renders into host FBO
- Shader compiled lazily on first draw to avoid GL context issues
- Plugin ID: `MrTx`

## 2026-08-30 — Catmull-Rom sampling + the identity-resample bug

Came out of live use: a zoom-tunnel feedback patch (1-frame varispeed loop, 99.9%
scale, 180° rotation) blurred out at depth, and **gentler zooms were blurrier than
aggressive ones at equal magnification**. That inversion is the tell — blur is
charged per *lap*, not per unit of zoom. Reaching a given magnification takes
`n = ln(M)/ln(1/s)` laps, so `n` scales as `1/(1-s)`: ~693 laps to halve at 99.9%
versus ~69 at 99%. Ten times the resampling passes for the same picture.

At 99.9% the per-lap displacement at radius r is `0.001·r`, so across most of a
1080p frame the image moves **less than half a texel per lap** — a bilinear tap
parked permanently in its blurriest regime while the content barely moves.

### Two problems, and the second was the bigger one

**1. Bilinear sampling.** Response at Nyquist for a fractional shift t is `|1-2t|`.
Raised to the power of several hundred laps, that annihilates fine detail.

**2. The soft-clip path was never an identity map.** This is the one that mattered:

```glsl
vec2 span = u_uv_scale - u_texel;
vec2 sample_uv = 0.5 * u_texel + clamp(transformed_uv, 0.0, 1.0) * span;
```

That squeezes `[0,1]` onto the outer-texel-**centre** span — `N-1` texels instead
of `N`. So every pixel lands at a fractional texel offset that varies across the
frame *even with no transform at all*, plus a ~0.05%/lap zoom-out nobody asked for.
Measured on a headless render: an identity Scale/Rotation gave `max|diff| = 180`
out of 255 against its own input, at matching alignment.

Introduced by the NPOT edge-seam fix — the intent (keep samples half a texel inside
the content) was right, the implementation rescaled where it should have clamped.
The **mirror path never had this**: its fold is additive (`half_t + ...`), so it was
already exactly identity. Only the soft-clip path was wrong.

### Implemented
- `sampleCubic()` — 16-tap Catmull-Rom, with every tap clamped into the content
  region. The 4x4 kernel reaches 2 texels past the sample point, so the old
  half-texel inset was not enough on its own to keep it out of the NPOT padding.
  At a whole-texel offset the weights are exactly `(0,1,0,0)`, so identity stays
  bit-exact. Used by both the mirror and soft-clip paths.
- Soft-clip mapping is now `clamp(transformed_uv * u_uv_scale, lo, hi)` — clamp,
  not rescale. Bit-exact identity for in-range uv, and strictly safer at the edges
  than the rescale it replaces.

### Measured (headless, 256x256 uniform-noise worst case, scale 0.999)

| variant | HF energy kept per lap | laps until 1% remains |
|---|---|---|
| before — bilinear + rescale | 0.516 | 7 |
| after — cubic + clamp | **0.970** | **149** |

~21x more usable tunnel depth. Contributions separate out as: bilinear→cubic alone
0.559→0.735 at scale 1.0; the mapping fix alone takes it to 1.000. **The mapping
bug was the dominant term** — cubic on its own would have left most of it.

Real footage has more low-frequency content than uniform noise, so absolute lap
counts will be higher; the ratio is the meaningful figure.

### Behaviour change to be aware of
The frame no longer shrinks by ~0.05% per lap, so a feedback patch tuned against
the old build will zoom very slightly slower for the same Scale value. Correct, but
not identical — worth re-checking any saved comp that leans on the zoom rate.

Cost: 16 texture fetches per pixel instead of 1. ~2 Gtexel/s at 1080p/60, which is
nothing on the target hardware, but it is a real increase if this plugin is ever run
somewhere weaker.

### Verification
- Both changed shaders compile-checked headlessly (moderngl, standalone GL context)
  before deploy — GLSL otherwise only compiles at runtime inside Resolume, where a
  syntax error shows up as a black or frozen output with no message.
- Identity pass-through verified bit-exact (`max|diff| = 0`, no shift, no flip).
- Built clean on MSVC. **Deploy blocked** — Resolume open.
  `make deploy PLUGIN=mirror_transform`.
