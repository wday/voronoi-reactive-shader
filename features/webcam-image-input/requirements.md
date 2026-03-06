# Webcam / Image Input (Effect Mode)

## Status: v1 scope locked

## Goal
Convert the Voronoi shader from a generator to an image filter (effect) so it can receive a video source (webcam, media clip, etc.) from Resolume's layer routing.

## What changes

### ISF metadata
- Change `"CATEGORIES"` from `["Generator"]` to include an effect category (e.g. `["Fx"]`)
- Add image input: `{"NAME": "inputImage", "TYPE": "image"}`

### Shader code
- Sample input image via `IMG_THIS_PIXEL(inputImage)` (current pixel) or `IMG_NORM_PIXEL(inputImage, coord)` (arbitrary UV)
- Decide how the input image integrates with the Voronoi output (see open questions)

### No FFGL/build changes expected
- The `ffgl-rs` build pipeline already supports image inputs — `isf_glsl_preprocess.rs` handles `IMG_PIXEL`, and ISF prefix macros define `IMG_THIS_PIXEL`, `IMG_NORM_PIXEL`, etc.

## Open questions

### v1 scope (locked)
- [x] **Blend/tint** — Voronoi output blended with input image. Proves the pipeline end-to-end.
- [x] Defer reactive/NC integration to the `normalized-convolution-image-input` feature
- [x] Add a `blendAmount` float parameter to control mix between Voronoi and input image

### Future (out of scope for v1)
- **Color source** — Voronoi cells colored by sampling the input image at cell center
- **Reactive** — Input image drives Voronoi parameters (density, drift, edge glow) per-pixel
- Switchable modes

### Generator preservation
- Git handles this — generator version lives in history, can be recovered or branched if needed

## Dependencies
- None for v1 (standalone ISF/GLSL change)
- Downstream: `normalized-convolution-image-input` feature depends on this

## References
- Existing effect examples: `ffgl-rs/ffgl-isf/isf-extras/hsv.fs` (clean image input pattern)
- ISF macros: `ffgl-rs/build-common/src/isf_prefix.glsl`
