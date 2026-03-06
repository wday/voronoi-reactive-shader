# Normalized Convolution Image Input

## Status: Draft — requirements need refinement

## Origin
Knutsson & Westin 1993 "Normalized and Differential Convolution" (see `docs/references/knutsson-westin-normalized-convolution-1993.pdf`)

## Core Idea
Replace the hash-based grid point placement in the Voronoi shader with a normalized convolution pipeline that operates on sparse point data. This produces a soft Voronoi tessellation with smooth blended boundaries instead of hard nearest-neighbor edges.

## What Normalized Convolution Does
Given sparse samples with certainty values:
1. **Splat** — Place points into a sparse field. Certainty `c = 1` at sample locations, `c = 0` elsewhere.
2. **Blur** — Convolve both `c * T` (signal × certainty) and `c` (certainty alone) with a Gaussian kernel (the "applicability function").
3. **Normalize** — Divide: `result = (G * cT) / (G * c)`

This reconstructs a continuous field from sparse irregular samples, with adaptive smoothing (more smoothing where samples are sparse).

## Open Questions

### Point source
- [ ] Are points procedurally generated (random scatter, replacing the grid hash)?
- [ ] Or derived from an input image (e.g., feature points, thresholded samples)?
- [ ] Or both modes?

### Output interpretation
- [ ] Soft cell field (each pixel gets a blended cell identity) — replaces F1/F2 distance entirely
- [ ] Or point map generation only — feed reconstructed positions into traditional Voronoi distance
- [ ] How does this interact with the existing 3-layer system?

### Image input
- [ ] Does this feature add an image input to the shader (making it an image filter instead of a generator)?
- [ ] If so, what does the image provide — point positions? Cell colors? Certainty field?

### Kernel control
- [ ] Applicability function shape — Gaussian? Tunable alpha/beta as in K&W eq. 4?
- [ ] Should kernel width map to existing `edgeWidth` or be a new parameter?

### GPU implementation
- [ ] Single-pass approximation or multi-pass (splat → blur → normalize)?
- [ ] ISF supports persistent buffers — viable for multi-pass?
- [ ] Performance budget relative to current 3-layer Voronoi

## References
- Knutsson & Westin, CVPR 1993 — Normalized and Differential Convolution
- Key equations: Definition 2 (eq. 3), 0th-order interpolation (eq. 12)
