# Development Log — Webcam / Image Input

## 2026-03-06 — Feature inception

### Context
Prerequisite for normalized-convolution-image-input feature. Need video input (webcam tracking a performer) to feed into the Voronoi shader.

### Research
- Checked existing ISF effect shaders in `ffgl-rs/ffgl-isf/isf-extras/` — 29 shaders use `"TYPE": "image"` inputs
- `hsv.fs` is a clean minimal example: declares `inputImage` in ISF JSON, samples via `IMG_THIS_PIXEL(inputImage)`
- Build pipeline (`isf_glsl_preprocess.rs`) already handles `IMG_PIXEL` → `texture()` rewriting
- ISF prefix macros define `IMG_THIS_PIXEL`, `IMG_NORM_PIXEL`, `IMG_THIS_NORM_PIXEL`
- No Rust/FFGL build changes needed — just ISF metadata + shader code

### Decisions
- Direction: convert shader to effect (not maintain dual generator+effect yet — can rewind with git)
- v1 scope TBD — need to decide integration mode (blend, color source, reactive)

## 2026-03-06 — v1 implemented and deployed

### Changes
- Locked v1 scope: blend/tint mode only, prove the pipeline
- Updated ISF metadata: category `["Fx", "Generator"]`, added `inputImage` (image) and `blendAmount` (float 0–1)
- Added 2 lines to `main()`: sample input image, mix with Voronoi output
- Created `Makefile` for repeatable WSL→Windows build+deploy workflow
- Built FFGL DLL successfully, deployed to Resolume Avenue Extra Effects
- Tested in Resolume: working — webcam input blends with Voronoi output via Image Blend slider

### Files changed
- `shaders/voronoi_reactive.fs` — ISF metadata + blend logic
- `Makefile` — new, build/deploy/preview targets
