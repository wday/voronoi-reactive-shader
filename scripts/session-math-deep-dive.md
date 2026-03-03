# Session: Shader Math Deep Dive

## Goal
Walk through the mathematical foundations of `shaders/voronoi_reactive.fs`
so I can understand it well enough to propose informed customizations.

## My Background
BS/MSc-level applied math, numerical computing, and statistics. Rusty but
solid fundamentals. Prefer building intuition over hand-wavy explanations —
show me the actual math and I'll follow.

## What to Cover

### 1. Hash Functions (lines 117–127)
- Why these specific constants? What makes a good hash for GPU use?
- Distribution properties — how uniform are the outputs?
- Why "no sine" matters (precision, portability)

### 2. Voronoi / Worley Noise (lines 158–204)
- F1 and F2 distance fields — geometric meaning, what F2−F1 actually measures
- Why the 3×3 neighborhood search is sufficient (Voronoi cell radius bounds)
- The seed animation model: circular drift vs chaotic drift, and how
  `driftChaos` interpolates between them
- How `smoothstep` on the blend factor affects the random walk continuity

### 3. Value Noise & Spatial Warp (lines 132–143, 215–219)
- Hermite interpolation: why `3t²−2t³` and not linear? What are the C1
  continuity implications?
- The two-octave warp: frequency ratio (3.0 vs 7.0), amplitude scaling (1.0
  vs 0.5) — is this a standard fBm pattern?
- How warp distortion feeds back into the Voronoi UV space

### 4. Multi-Layer Composition (lines 227–258)
- The `pow(layerSpread, layer)` scale progression — what kind of frequency
  cascade is this?
- Layer weighting with `pow(layerMix, layer)` — how this controls the
  contribution falloff
- How contrast and brightness transforms interact with the layer blend

### 5. Edge Detection & Glow (lines 242–249)
- `smoothstep` as a soft threshold — the math of the transition zone
- Edge width vs glow range: the `1 + edgeGlow * 4` multiplier
- HSV manipulation for the edge highlight (desaturated, full brightness)

## Approach
- Go function by function, starting from the building blocks (hash, noise)
  up to the composition (layers, edges, color)
- Use the actual code as the reference — annotate with math, not pseudocode
- When relevant, point out where different choices would produce different
  visual results (this sets up the customization session)

## Key Files
- `shaders/voronoi_reactive.fs` — the shader
- `CREDITS.md` — source attribution and links to original papers/articles
