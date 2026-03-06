# Normalized Convolution Image Input

## Status: Requirements locked

## Origin
Knutsson & Westin 1993 "Normalized and Differential Convolution" (see `docs/references/knutsson-westin-normalized-convolution-1993.pdf`)

## Concept
Use normalized convolution to derive a **spatially-varying density/certainty field** from the input image. This field biases where Voronoi seed points cluster and how they behave, producing a Voronoi tessellation that loosely tracks the image structure while remaining noisy and drifting.

The performer's silhouette/features emerge from **cell density patterns**, not from color reproduction. The result is a Voronoi interpretation of the image, not a reconstruction.

## How It Works

### 1. Sparse sample the input image
- At each Voronoi cell, sample the input image at the seed point location
- Image brightness (or luminance) at each sample becomes the **certainty** value
- Sampling density is a controllable parameter

### 2. Normalized convolution → density field
- Apply NC (K&W eq. 12) to the sparse certainty samples:
  `densityField = (G * c) / (G * 1)` — a smooth reconstruction of local image "importance"
- The Gaussian applicability function controls the reconstruction radius
- High-certainty regions (bright/high-contrast) → dense cells
- Low-certainty regions (dark/flat) → sparse cells, large, noise-dominated

### 3. Density field biases Voronoi seeds
- **Seed attraction**: in high-density regions, seeds cluster tighter (effective scale increases)
- **Drift behavior**: low certainty → seeds drift freely (existing chaotic animation dominates). High certainty → seeds anchor toward image features
- The existing noise/drift system remains — certainty modulates its influence, not replaces it

### 4. Visual result
- Voronoi tessellation that loosely follows image content
- Always noisy, always drifting — never a clean reconstruction
- Performer's shape readable through cell density, not color

## Parameters

| Name | Description | Range | Default |
|------|-------------|-------|---------|
| `sampleDensity` | How many image samples per cell region | 0.0–1.0 | 0.5 |
| `imageInfluence` | How strongly the image certainty biases seed placement | 0.0–1.0 | 0.5 |
| `kernelRadius` | Applicability function width (reconstruction smoothness) | 0.01–0.5 | 0.1 |

Existing parameters (`density`, `driftSpeed`, `driftChaos`, etc.) continue to work — `imageInfluence` blends between pure noise (0) and image-driven (1).

## Implementation Approach
- Single-pass, computed inline in the Voronoi seed loop
- No multi-pass / persistent buffers needed for v1
- Sample `inputImage` at seed positions using `IMG_NORM_PIXEL`
- Compute luminance → certainty per seed
- Use certainty to modulate seed offset and drift amplitude

## Dependencies
- `webcam-image-input` feature (done — `inputImage` available)

## References
- Knutsson & Westin, CVPR 1993 — Normalized and Differential Convolution
- Key equations: Definition 2 (eq. 3), 0th-order interpolation (eq. 12)
