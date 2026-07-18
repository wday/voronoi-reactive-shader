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

## 2026-07-09 — flow-euler: Soup mode (playable colour advection/diffusion)

Motivation: flow-euler was being used standalone (not the depth→particle pipeline)
as a visual, but wasn't playable. Its output was the raw field texture
(RG=velocity, B=curl, A=density) so colour was dictated by physics, not
controllable; advection distance was tiny (`* 0.01`); no global flow direction;
and feedback tails were short. Goal: "flowy diffusion soup when feeding back
through a delay line."

### Added a `mode` switch — Soup (default) / Field
- **Field mode** = the original solver, preserved verbatim (for the Channel
  Displace velocity-as-warp trick). Non-default.
- **Soup mode** = advection/diffusion of the input's **RGB colour**:
  - State texture repurposed: RGB = colour soup, A = advected density (luma).
  - Velocity is *synthesised* each frame (no per-pixel velocity storage, which
    freed RGB for colour): `v = global(FlowAngle,FlowSpeed) + curl⟂grad(luma)`.
  - Pass 0: synth velocity → semi-Lagrangian advect colour → persistence decay →
    inject input. Pass 1: isotropic 4-tap diffusion (the "bleed"). Pass 2:
    HSV colour grade (hue/sat) + tint + gain, clamp to [0,1].

### Controls answering the three asks
- **Colour**: `Hue Shift`, `Saturation`, `Tint`, `Gain` — decoupled from physics.
- **Direction**: `Flow Angle` + `Flow Speed`; **Speed=0 → no offset → pure bleed**
  (the requested default). `Curl/Swirl` gives feature-following rotation.
- **Long tails in a feedback loop**: the range extender is *persistence* —
  `Persistence` (decay) near 1.0 lets advected colour survive many feedback
  frames, each nudged `FlowScale·v` further → long streaks. `Diffusion` trades
  against tail sharpness. Documented as the tail-length recipe.

### Status (v1 — bokeh soup)
- Shipped, but live feedback: looked like "soft gooey bokeh," not the watery
  field diffusion Field mode gets. Root cause: v1 Soup *synthesised* velocity
  from the colour gradient each frame (no momentum) and leaned on isotropic
  blur — blur without persistent currents = bokeh by definition. Superseded.

## 2026-07-09 (later) — flow-euler Soup rebuilt as stable-fluid + dye

The watery quality *requires* a persistent velocity field (momentum + vorticity);
RGBA can't hold 2-ch velocity + 3-ch colour, so v1 dropped velocity. Fixed by
going to the classic **fluid + dye** two-field architecture (4 persistent float
buffers, 5 passes):

- `velA`/`velB` (ping-pong): velocity field, RG=velocity B=curl. Self-advects
  (momentum), **vorticity confinement** (`Curl / Vorticity`), **viscosity**
  (Laplacian smoothing), and an image-gradient **Turbulence** force (`stir`) that
  lets the input stir the fluid → coherent eddies/currents.
- `dyeA`/`dyeB` (ping-pong): the input's **RGB colour** as dye, semi-Lagrangian
  advected *through* the velocity field (+ global Flow Angle/Speed drift), light
  Diffusion, Persistence decay. The dye is what you see; the fluid makes it swirl.
- Pass order: P0 velA→velB (vel update), P1 velB→velA (viscosity), P2 dyeA→dyeB
  (advect+inject), P3 dyeB→dyeA (diffuse), P4 grade+output. velA/dyeA hold state.
- **Field mode preserved** — runs its original 2-pass solver on velA/velB; dye
  passes are pass-through; output is the raw field (Channel Displace use intact).
- New param `stir` (Turbulence); `dt`→"Sim Rate" and `viscosity` now active in
  Soup too. v1's `Bleed`/bokeh path removed per user (replace, not keep).

### Migration note
- Persistent buffer names changed (stateA/stateB → velA/velB/dyeA/dyeB) and params
  reordered → **must remove + re-add the effect** in Resolume (cached instances
  carry stale buffers/param layout).

### Status
- ISF JSON/passes/sampler-names/bracket balance validated; builds clean (MSVC).
- **Deploy blocked** — Resolume holding `flow_euler.dll`. Close it, then
  `make deploy PLUGIN=flow_euler`.
- Not yet validated live.

## 2026-07-09 (later still) — params not showing in Resolume → renamed to "Euler Soup"

Live symptom: new params (Turbulence etc.) never appeared, even after full restart
+ fresh comp. Verified the deployed DLL *did* contain the new ISF (grep found
"Turbulence"/"velB"/"dyeA"). Root cause in `vendor/ffgl-rs/ffgl-isf/src/handler.rs`:
FFGL `unique_id = '*' + display_name[0..3]` — derived from the name, never from
shader content. So Resolume keys the plugin by a stable id and serves cached param
descriptors; editing the shader can't change what Resolume shows. Also a real
collision: "Flow Inject" and "Flow Euler" both → `*Flo`.

Fix: renamed flow_euler `display_name` "Flow Euler" → **"Euler Soup"** (id `*Eul`,
unique) so Resolume sees a brand-new plugin and scans fresh params. Cargo trap:
`ISF_NAME` is `env!()` with no rerun-if-env-changed, so also bumped the `.fs`
DESCRIPTION (include_str dep) to force the recompile; confirmed "Euler Soup" baked
into the DLL. Full writeup saved to memory `reference_ffgl_isf_plugin_id`.

- In Resolume the effect is now **"Euler Soup"** (old "Flow Euler" entry is dead).
- Deferred proper fix: hash-based unique_id in vendored ffgl-isf (avoided — would
  change every plugin's id and break existing comps).
