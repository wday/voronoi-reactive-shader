# Implementation Plan — Webcam Image Input v1

## Steps

### 1. Create feature branch
- Branch from current main

### 2. Update ISF metadata in `shaders/voronoi_reactive.fs`
- Change `"CATEGORIES"` from `["Generator"]` to `["Fx", "Generator"]`
- Add `inputImage` to INPUTS: `{"NAME": "inputImage", "TYPE": "image"}`
- Add `blendAmount` float param (0.0–1.0, default 0.0) to control mix

### 3. Modify shader `main()`
- Sample input image: `vec4 inputColor = IMG_THIS_PIXEL(inputImage);`
- After final Voronoi color is computed, blend: `color = mix(color, inputColor.rgb, blendAmount);`
- Preserve alpha handling

### 4. Test in browser preview
- Verify shader compiles and renders in the ISF preview harness
- With no image input, `blendAmount = 0.0` should produce identical output to current generator

### 5. Build FFGL plugin
- Run existing build pipeline
- Confirm it compiles without changes to Rust/FFGL code

### 6. Test in Resolume
- Load as effect on a layer with webcam source
- Verify `inputImage` receives the layer input
- Sweep `blendAmount` from 0 (pure Voronoi) to 1 (pure input)
- Confirm all existing parameters still work

### 7. Commit

## Risk
- Low. Only touching ISF metadata + 3-4 lines of GLSL. Build pipeline proven with 29 existing effect shaders.
- Fallback: `blendAmount = 0.0` is functionally identical to the generator version.
