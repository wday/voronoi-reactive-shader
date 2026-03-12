# IFS Fractal Toolkit — Requirements

## Status: in progress

## Concept

Build fractal and chaotic patterns from video feedback using reusable primitive plugins composed in Resolume, rather than monolithic generators. The existing delay-line-module (Send/Receive/Tap) and mirror-transform (scale/rotate/swirl/mirror) already provide temporal feedback and spatial transforms. This feature adds two capabilities:

1. **X/Y translation on mirror-transform** — completes the affine transform primitive for IFS fractals
2. **Logistic feedback plugin** — introduces nonlinear dynamics (bifurcation, chaos) as a composable effect in the feedback chain
3. **Channel displacement plugin** — cross-channel coupling, the final ingredient for strange attractor dynamics

### What is an IFS?

An Iterated Function System defines a fractal as the attractor of N affine transforms applied repeatedly. Each transform is: scale + rotate + translate. Classic examples:

- **Cantor dust**: 2 copies, scale 0.33×, translate to left/right thirds
- **Sierpinski triangle**: 3 copies, scale 0.5×, translate to triangle vertices
- **Barnsley fern**: 4 copies with different probabilities (approximated by opacity)

### Architecture: composition over code

Each IFS "copy" is a Resolume layer with its own effect chain. The fractal emerges from the composition, not from a single plugin. This means:

- N copies = N layers, each with: Tap → mirror-transform (scale/rotate/translate)
- One layer with Send feeds the delay buffer
- Additive (or screen) blend mode on layers accumulates copies
- Feedback through the delay line iterates the function system

## Deliverables

### 1. Mirror-transform: add X/Y translation

Add two params to the existing mirror-transform plugin:

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 4 | Translate X | Standard | 0.0–1.0 → -1.0–+1.0 | 0.5 (0) | Horizontal offset in UV space |
| 5 | Translate Y | Standard | 0.0–1.0 → -1.0–+1.0 | 0.5 (0) | Vertical offset in UV space |

Translation applies after scale/swirl/rotate, before edge handling. This preserves the existing transform order (center → scale → swirl → rotate → uncenter) and adds offset before mirror/clip.

### 2. Example compositions

Resolume composition files (or documented recipes) showing how to build:

- Cantor dust (2-copy)
- Sierpinski triangle (3-copy)
- Fractal spiral (2-copy with rotation)

### 3. Logistic feedback plugin (new: `logistic-feedback`)

A per-pixel nonlinear map that introduces chaotic dynamics into the feedback chain. Based on the logistic map `x_{n+1} = r * x_n * (1 - x_n)`, the simplest equation that produces chaos.

#### Mathematical background

The logistic map's behavior is controlled entirely by the parameter `r`:

| r range | Behavior | Visual result in feedback |
|---------|----------|--------------------------|
| 0 – 1.0 | Collapse to 0 | Fades to black |
| 1.0 – 3.0 | Single fixed point | Converges to stable brightness `(r-1)/r` |
| 3.0 – 3.45 | Period-2 | Alternates between two brightness states |
| 3.45 – 3.57 | Period-4, 8, 16... | Increasingly complex periodic flicker |
| 3.57 – 4.0 | Chaos | Brightness never repeats, sensitive to initial value |
| ~3.83 | Period-3 window | Brief return to order inside chaos |
| 4.0 | Full chaos | Ergodic — visits every brightness eventually |

#### Spatial modulation of r

Uniform `r` creates global order/chaos. Spatial modulation creates *structured* chaos — regions of stability and instability coexisting in the same frame.

- **Edge mode**: Built-in Sobel gradient detector. `r_pixel = r_base + sobel_magnitude * sensitivity * (4.0 - r_base)`. Edges get pushed toward chaos while flat regions stay stable. Through feedback, edges dissolve into chaotic diffusion boundaries.
- **Radial mode**: `r_pixel = r_base + distance_from_center * sensitivity * (4.0 - r_base)`. Renders the bifurcation diagram as concentric rings — stable center, chaotic periphery.
- **Off**: Uniform `r` everywhere.

#### Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | R | Standard | 0.0–1.0 → 0.0–4.0 | 0.75 (3.0) | Bifurcation parameter — the main performance knob |
| 1 | Sensitivity | Standard | 0.0–1.0 | 0.0 | How much spatial mode pushes r toward 4.0 |
| 2 | Spatial Mode | Option | Off / Radial / Edge | Off | What drives spatial r variation |
| 3 | Dry/Wet | Standard | 0.0–1.0 | 1.0 | Mix logistic output with original |

#### Shader

Single-pass fragment shader. Per-pixel:
1. Sample input luminance (or per-channel)
2. Compute spatial r modifier (Sobel or radial, based on mode)
3. Apply logistic map: `x = r * x * (1.0 - x)`
4. Mix with original per dry/wet
5. Output

The Sobel filter uses a 3×3 kernel on the input texture — standard edge detection, no extra passes or FBOs needed.

#### Performability

- **R** is the star — sweep from 2.5 → 4.0 during a set for order-to-chaos transition
- **Sensitivity** is "set and forget" — determines spatial character
- **Spatial Mode** is a preset selector, switch between sets
- **Dry/Wet** for blending intensity

#### Per-channel vs luminance

Apply the logistic map independently to R, G, B. Each channel has different initial brightness, so they bifurcate at different `r` thresholds — you get color separation at the onset of chaos without any extra work.

### 4. Channel displacement plugin (new: `channel-displace`)

Cross-channel coupling — one color channel's value influences another channel's spatial sampling. This is the video analog of coupled differential equations (Lorenz, Rössler) where cross-variable dependency creates strange attractor dynamics.

#### How it works

Each color channel samples the input at a slightly different UV position, offset by another channel's brightness:

```
R samples at (uv + G_brightness * amount * direction)
G samples at (uv + B_brightness * amount * direction)
B samples at (uv + R_brightness * amount * direction)
```

The cyclic coupling R→G→B→R creates rotational asymmetry — channels chase each other through color space. Through feedback iterations, this produces:

- **Low coupling**: Subtle chromatic aberration, prismatic fringing at edges
- **Medium coupling**: Channels separate into swirling color fields, ghostly RGB echoes
- **High coupling**: Full decorrelation, chaotic color dynamics

The displacement range is intentionally small (0–10% of UV space) because feedback compounds it. Even 1% per frame creates massive separation after 30 iterations.

#### Coupling patterns

- **Cyclic (R→G→B)**: R displaced by G, G by B, B by R. Asymmetric, creates directional color spiraling. Analogous to Lorenz coupling.
- **Mutual (R↔G↔B)**: Each channel displaced by average of the other two. Symmetric, creates outward color diffusion.

#### Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Amount | Standard | 0.0–1.0 → 0.0–0.1 UV | 0.0 | Coupling strength |
| 1 | Pattern | Option | Cyclic / Mutual | Cyclic | Which channels drive which |
| 2 | Angle | Standard | 0.0–1.0 → 0°–360° | 0.0 | Direction of displacement |
| 3 | Dry/Wet | Standard | 0.0–1.0 | 1.0 | Mix with original |

#### Shader

Single-pass fragment shader. Per-pixel:
1. Sample input at current UV to get channel values
2. Compute per-channel UV offset based on coupling pattern and angle
3. Re-sample input at offset UVs, one per channel
4. Composite R from sample 1, G from sample 2, B from sample 3
5. Mix with original per dry/wet

No FBO, no extra passes. Three texture lookups instead of one.

#### Performability

- **Amount** is the main knob — sweep from 0 to introduce color separation
- **Angle** rotates the displacement direction — creates different spatial patterns
- **Pattern** is a mode switch, set per composition
- Pairs naturally with logistic-feedback: coupling + nonlinearity = chaos

#### Interaction with other primitives

| Chain position | Effect |
|---|---|
| Before mirror-transform | Channels separate, then get mirror-folded — color-banded kaleidoscopes |
| After mirror-transform | Uniform spatial transform, then channels diverge — ghostly color echoes |
| Between Send/Receive | Coupling compounds through feedback — strange attractor dynamics |
| With logistic-feedback | Nonlinearity + coupling = the two essential ingredients for chaos |

## The complete chaos toolkit

Four composable, stateless plugins covering the three ingredients of chaotic dynamics:

| Plugin | Role | Chaos ingredient |
|--------|------|-----------------|
| delay-line-module | Temporal feedback loop | **Iteration** |
| mirror-transform | Affine spatial transforms + edge handling | Spatial structure |
| logistic-feedback | Per-pixel bifurcation map | **Nonlinearity** |
| channel-displace | Cross-channel UV coupling | **Coupling** |

## Constraints

- Existing mirror-transform behavior unchanged at default param values (translate 0,0)
- Logistic feedback must be stateless (no FBO, no temporal buffer — the delay-line provides iteration)
- Must work with delay-line-module Send/Receive/Tap chain
- Per-channel logistic map (not luminance-only) for richer color dynamics
