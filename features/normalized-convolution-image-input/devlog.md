# Development Log — Normalized Convolution Image Input

## 2026-03-06 — Feature inception

### Context
Idea surfaced during discussion about deconvolution. Recalled Knutsson & Westin 1993 normalized convolution algorithm — random sparse sampling + Gaussian blur + normalization to reconstruct continuous fields from incomplete data.

### Connection to voronoi shader
Current shader uses grid-aligned hash points (Worley noise). NC could replace this with:
- Truly irregular point placement (no grid artifacts)
- Soft/blended cell boundaries instead of hard F1/F2 edges
- Spatially varying density via certainty field
- GPU-friendly splat-blur-slice pipeline

### Decisions
- Downloaded reference paper to `docs/references/knutsson-westin-normalized-convolution-1993.pdf`
- Created feature directory with draft requirements
- Requirements still have significant open questions — need to clarify point source, output interpretation, and whether this adds image input capability

## 2026-03-06 — v1 implemented and deployed

### Requirements clarified
- NC does NOT replace Voronoi — it generates a spatially-varying certainty field from the input image
- Certainty (luminance) biases seed placement: high certainty → tighter cells, anchored seeds
- Low certainty → sparse cells, noise-dominated drift
- Result: Voronoi tessellation that loosely tracks image content while staying noisy and drifting

### Implementation
- Added `imageInfluence` (0–1) and `kernelRadius` (0.01–0.5) parameters
- Added `imageCertainty()` helper — samples input image luminance at seed positions
- Modified `voronoiLayer`: each seed samples image at its world position, certainty modulates:
  - Drift amplitude: `driftScale = 1.0 - cert * 0.8` (high cert → 80% less drift)
  - Effective distance: `dist / (1.0 + cert * 2.0)` (high cert → up to 3× tighter cells)
- NC accumulation in seed loop: Gaussian-weighted certainty across neighbors for smooth density field
- At `imageInfluence = 0`, output identical to previous version

### Files changed
- `shaders/voronoi_reactive.fs` — NC logic, new parameters, `imageCertainty()` helper
- `features/normalized-convolution-image-input/requirements.md` — locked
- `features/normalized-convolution-image-input/plan.md` — created

### Tested
- Built and deployed to Resolume Avenue via `make release`
- Confirmed working with webcam input
