# Session: Video-Reactive Voronoi — Problem Spec & Design

## Goal
Design how to integrate live video (dual cameras) into the Voronoi shader so
that video content influences the generative visuals — edge detection driving
cell structure, camera feeds modulating color/density/warp, etc.

## Starting Vision
- Dual camera inputs (e.g., performer cam + audience cam, or two angles)
- Edge detection on the video feeds
- Video edges influence the Voronoi map — seed placement, density, distortion,
  or cell coloring driven by what the cameras see
- The result should feel like a living reactive visual, not just an overlay

## Design Questions to Work Through

### 1. Input Architecture
- How do dual camera feeds enter the ISF pipeline? (ISF `image` inputs,
  FFGL texture inputs, or external preprocessing?)
- Resolution and frame rate considerations — what can the GPU handle in
  real time?
- Do we preprocess video on CPU (OpenCV edge detection) or do edge detection
  in the shader?

### 2. Edge Detection Approach
- Sobel, Canny, or simpler luminance gradient in GLSL?
- Single-pass vs multi-pass (ISF supports persistent buffers / PASSES)
- Thresholding and sensitivity — how to make this a tunable parameter

### 3. Video → Voronoi Mapping Strategies
Explore these as distinct modes or combinable layers:
- **Density modulation**: edge density in a region → local Voronoi cell density
- **Seed attraction**: Voronoi seeds migrate toward detected edges
- **Warp field**: video edges generate a distortion field that warps the
  Voronoi UV space
- **Color injection**: sample video color into Voronoi cells
- **Opacity/blend**: video regions control where generative vs passthrough
  dominates

### 4. Dual Camera Blending
- How to combine two feeds — split screen, crossfade, difference, or
  independent influence channels?
- Could one camera drive structure (edges/density) while the other drives
  color?

### 5. Performance Budget
- Target: real-time 1080p 60fps on mid-range GPU
- What's the cost of per-pixel edge detection + Voronoi with video sampling?
- Where to make quality/performance tradeoffs

### 6. Parameter Design
- What new ISF inputs are needed?
- How do these interact with the existing controls (density, warp, drift)?
- What should be exposed for live performance control vs set-and-forget?

## Approach
- Start with the simplest viable integration (single camera, luminance edges
  → warp field) and design incrementally toward the full vision
- Identify what can stay in pure GLSL/ISF vs what needs external tooling
- Produce a phased implementation plan with clear milestones

## Key Files
- `shaders/voronoi_reactive.fs` — current shader (starting point)
- `CREDITS.md` — source attribution
- `preview/` — browser preview harness (useful for prototyping)
- `ffgl-rs/` — FFGL plugin build pipeline (deployment target)
