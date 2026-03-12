# Flow Plugin Family — Dev Log

## 2026-03-11 — Feature setup

- Created feature artifacts from `flow-plugin-family.md` spec (drafted in web chat)
- `stereo_depth.fs` confirmed in project root (not `shaders/`) — registered in plugins.json as-is
- Build order: flow-inject → flow-euler → flow-lagrange

## 2026-03-12 — Phase 1 & 2: ISF shaders + stereo depth refactor + spout-publish

### stereo_depth.fs — refactored to single-input hstacked mode
- **Design decision:** Resolume Avenue can't route two separate sources to one ISF effect (that's Arena/Wire only). FFGL supports 2-input "mixer" plugins but that's the wrong abstraction.
- Refactored to take a single side-by-side (hstacked) input: left camera in left half, right camera in right half
- Added `sampleLeft()`/`sampleRight()` helpers that remap UV into each half
- Texel size adjusted: `px.x` doubled since each camera occupies half the texture width
- Removed `inputLeft`/`inputRight` dual image inputs, replaced with single `inputImage`

### spout-publish tool (`tools/spout-publish/`)
- **Design decision:** ffmpeg handles camera capture + hstack (it already knows how), tiny Rust tool handles Spout publish (the one thing ffmpeg can't do). Unix composability.
- Pipeline: `ffmpeg ... -f rawvideo -pix_fmt rgba pipe:1 | spout-publish --name StereoRig -w W -h H`
- SpoutLibrary uses C++ vtable interface (COM-style), not flat C API. Wrote `csrc/spout_bridge.cpp` as a thin C wrapper, Rust calls through FFI.
- Runtime dependency: SpoutLibrary.dll + SpoutLibrary.lib (from Spout2 SDK releases)
- Build dependency: `SPOUT_SDK_DIR` env var or `SpoutSDK/` dir next to Cargo.toml
- `stereo-cam.bat` — one-click wrapper: reads camera names + resolution from `stereo-cam.conf`, runs the ffmpeg|spout-publish pipeline
- Invert flag set in SendImage (ffmpeg rawvideo is top-down, Spout expects bottom-up)

### flow-inject (`shaders/flow_inject.fs`)
- 2-pass ISF: persistent float FBO for field accumulation + output pass
- Reads depth via central-difference gradient → velocity (RG)
- Edge detection via gradient magnitude → density (A)
- Raw depth → scalar (B)
- Decays velocity and density each frame, accumulates new injection on top

### flow-euler (`shaders/flow_euler.fs`)
- 3-pass ISF with ping-pong persistent float textures (stateA ↔ stateB)
- Originally designed as 4-pass with same TARGET written twice — ISF doesn't support that, restructured to ping-pong
- Pass 0: inject + velocity update (pressure gradient, vorticity confinement, viscous diffusion) → stateB
- Pass 1: semi-Lagrangian advection + decay → stateA
- Pass 2: output stateA
- ISF `IMG_NORM_PIXEL` is a macro — can't pass sampler2D as function param. Used `sampleA()`/`sampleB()` wrapper functions instead
- Boundary modes: wrap (fract), reflect (triangle wave), absorb (zero outside)

### CI fix: multi-ISF build
- Previous CI built all ISF plugins in a loop but they all produce `ffgl_isf.dll` — only the last one survived to packaging
- Fixed: copy each ISF DLL to `isf-out/` immediately after building
- Updated `list-plugins.py` package format to source ISF plugins from `isf-out/`
- Applied fix to both `build.yml` and `release.yml`

### Registered in plugins.json
- `stereo_depth` (ISF, `stereo_depth.fs`)
- `flow_inject` (ISF, `shaders/flow_inject.fs`)
- `flow_euler` (ISF, `shaders/flow_euler.fs`)
