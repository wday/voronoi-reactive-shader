# IFS Fractal Toolkit — Dev Log

## 2026-03-11 — Requirements and plan

Decided against building a monolithic IFS plugin. The existing modular architecture (delay-line + mirror-transform) already provides the primitives for IFS — each Resolume layer acts as one "copy" of the iterated function. Only missing piece: X/Y translation in mirror-transform.

Deliverables: translation params on mirror-transform, logistic-feedback plugin, channel-displace plugin, and documented example compositions.

## 2026-03-11 — Logistic feedback plugin added

Added logistic-feedback plugin to scope. The logistic map `x = r*x*(1-x)` applied per-channel gives bifurcation control over feedback dynamics with one knob (R). Spatial modulation of r via built-in Sobel edge detection creates structured chaos — stable flat regions, chaotic edges that dissolve through feedback iterations. Radial mode renders the bifurcation diagram as concentric rings. Plugin is stateless (no FBO) — the delay-line provides temporal iteration.

Added channel-displace plugin to scope. Cross-channel UV displacement — R sampled at offset driven by G, etc. Creates strange attractor dynamics through feedback by introducing variable coupling (the third ingredient of chaos alongside iteration and nonlinearity). Cyclic and mutual coupling patterns. Small displacement range (0–10% UV) compounds through feedback iterations.

## 2026-08-30 — channel-displace: Catmull-Rom sampling

Found during a sweep for the resample bug fixed in `mirror-transform` the same day.

`channel-displace` was clean on the identity path (it samples `v_uv` unchanged for
`original`, and verified bit-exact at Amount = 0). But the three displaced fetches
sample at `v_uv + dir * channel_value` — content-driven and freely sub-pixel, so a
bilinear tap sits at a fractional offset essentially always.

One-shot that is invisible. In a feedback loop it is applied once per lap and
compounds: bilinear's Nyquist response for a fractional shift `t` is `|1-2t|`, so
fine detail decays geometrically with lap count.

Unlike `slipgrid`, the offsets here **cannot be snapped** to whole texels — the
displacement is the effect. So the answer is a better filter: the same 16-tap
Catmull-Rom added to `mirror-transform`, with every tap clamped inside the texture
edge (the 4x4 kernel reaches 2 texels out, so the pre-existing half-texel inset was
not sufficient on its own). Only the three displaced fetches use it; `original`
stays on the cheap path because it is already exact.

### Measured (headless, 256x256 uniform noise, Amount 0.02)

| | HF energy kept per lap | laps until 1% remains |
|---|---|---|
| bilinear | 0.607 | 9 |
| cubic | **0.761** | **17** |

Identity (Amount = 0) verified bit-exact, `max|diff| = 0`.

**Honest scope:** this roughly doubles usable feedback depth, it does not solve it.
There was no mapping bug here (unlike `mirror-transform`, where the fix took HF
retention from 0.516 to 0.970), so all that is available is the filter improvement.
0.761/lap is still lossy, and inherently so — a continuous displacement resamples,
full stop. Pairing it with in-loop sharpening or grain (see `features/delay-lens/`)
is what would push it further.

Cost: the three displaced fetches go from 1 tap to 16 each, so 48 instead of 3 per
pixel — roughly 6 Gtexel/s at 1080p/60, comfortable on the target GPU but a real
increase. The standard 9-bilinear-tap Catmull-Rom optimisation is available if this
ever needs to run somewhere weaker; the 16-tap point-sample form was chosen for
being obviously correct.

Built, deployed. Shader compile-checked headlessly before deploy.

## 2026-08-30 — NPOT padding: channel-displace + logistic-feedback

Both plugins ignored `u_uv_scale` entirely and sampled raw `uv` across `[0,1]`,
assuming the input texture's hardware size equals the frame size. Every other
shader in the repo corrects for this; these two were the holdouts.

Prompted by an observed artifact: **channel-displace introducing unexpected lines.**
That is exactly what this bug produces. The edge clamp reads

```glsl
clamp(uv, 0.5 * texel, 1.0 - 0.5 * texel)
```

so when the texture is padded, `1.0 - 0.5*texel` is the last texel of the **padded**
texture — black. A channel displaced off-frame therefore clamps to *black* instead
of edge-extending, which is the precise opposite of the comment's stated intent
("clamps to edge-extend instead of black"). Result: black streaks along the frame
edge wherever the displacement pushes a channel out.

`logistic-feedback` had the same mapping bug plus a second one: its Sobel taps
stepped by `u_texel_size` (= 1/hardware) while being added to a content-space uv.
Once `sampleContent` scales by `u_uv_scale`, the step has to be one *content*
texel — `u_texel_size / u_uv_scale` — or the edge detector probes the wrong
distance.

### Measured (headless, 200px content in a 256px texture, u_uv_scale = 0.78125)

Fraction of the output that is fully black, and max deviation from an exact copy:

| | max abs diff | fully-black pixels |
|---|---|---|
| channel-displace, Amount 0 — before | 255 | 38.4% |
| channel-displace, Amount 0 — after | **0** | **0.0%** |
| channel-displace, Amount 0.05 — before | 255 | 40.4% |
| channel-displace, Amount 0.05 — after | 255 (the effect working) | **0.0%** |
| logistic-feedback, Dry/Wet 0 — before | 255 | 38.4% |
| logistic-feedback, Dry/Wet 0 — after | **0** | **0.0%** |

38.4% is exactly `1 - (200/256)²`, i.e. the padding region — confirming the
mechanism rather than just the symptom.

### Note on whether this was live
The fix is a **no-op when the host allocates exact-size textures** (`u_uv_scale ==
1`), which is normal on modern GPUs, and the unpadded path was re-verified
unchanged (identity still bit-exact, cubic still 0.761 HF/lap). So this may have
been latent rather than active. If the reported lines persist after this deploy,
padding was not the cause and the next suspect is the ordinary edge-extend
behaviour at the clamp, which is real and by design.

Built, deployed. Shaders compile-checked headlessly before deploy.
