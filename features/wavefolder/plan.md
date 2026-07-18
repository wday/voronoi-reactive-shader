# Wavefolder — Implementation Plan

## Phase 1 — ISF shader (this phase)

1. **`shaders/wavefolder.fs`** — single-pass ISF v2 Fx.
   - Header block: DESCRIPTION, CREDIT (wday), CATEGORIES ["FX"], 6 INPUTS.
   - `foldTri/foldSin/foldWrap` (vec3, component-wise) + `foldShape(x, s)` morph.
   - Fold loop: fixed max 8, break at `float(i) >= stages`, fractional last-stage
     blend via `clamp(stages - i, 0, 1)`.
   - `IMG_THIS_PIXEL(inputImage)`, preserve source alpha, clamp output.
2. **ISF smoke test** — `features/wavefolder/isf_fold_test.py`. NOTE: `harness.py`
   and `explore.py` only load FFGL plugin GLSL (`plugins/*.frag.glsl`), **not** ISF
   `.fs` files — so ISF is verified by compiling `wavefolder.fs` through a moderngl
   ISF shim (`IMG_THIS_PIXEL`/`gl_FragColor`/`isf_FragNormCoord`). DONE 2026-07-12,
   all asserts green.
3. **Live check** — drop into Resolume / an ISF editor over a hot source; dial
   presets by eye. (The evolve/gallery tooling can't drive ISF; if we want it,
   that's a reason to do the Phase 2 plugin-GLSL port.)

## Phase 2 — FFGL port (deferred, only if it earns a live slot)

- Wrap as `wavefolder` Rust plugin on the pluglib pattern (stateless → no FBO/state).
- 16-byte plugin name, unique 4-char ID, register in `plugins.json`.
- `make build PLUGIN=wavefolder && make deploy PLUGIN=wavefolder`.

## Open questions / possible extensions

- Add a per-stage gain ramp (currently uniform Drive)? Keep simple for now.
- Luminance-preserving mode as a second Shape-adjacent knob later?
- Feed a Voronoi/flow field into Bias for spatially-varying fold (fractal warp)?
