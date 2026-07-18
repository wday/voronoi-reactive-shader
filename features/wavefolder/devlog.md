# Wavefolder — Devlog

## 2026-07-12 — Feature set up + Phase 1 shader landed

- Motivation from live use: out-of-the-box camera feedback in ambient light
  blows out to white; need a bounded nonlinearity to fold intensity back and
  add fractal contouring instead of clipping.
- Locked 4 design decisions with user:
  - **ISF-first** (prototype in shader-harness, FFGL port deferred to Phase 2)
  - **Per-channel RGB** fold (blowout → color fringes)
  - **Selectable curve** — Triangle → Sine → Wrap morph on `shape`
  - **Iteration-knob** cascade — `x ← fold(x·Drive + Bias)` × `stages`
- Wrote `features/wavefolder/{requirements,plan}.md`.
- Wrote `shaders/wavefolder.fs`:
  - 6 params (Drive, Stages, Shape, Bias, Mix + inputImage).
  - Fold curves as component-wise vec3 helpers; `foldShape` morphs on one knob.
  - Fold loop: fixed max 8 (GLSL constant bound), break at `i >= stages`,
    fractional last stage via `clamp(stages - i, 0, 1)` for smooth automation.
  - **Naming gotcha:** dry/wet param is `wet` (NAME can't be `mix` — GLSL builtin).
  - Triangle is identity on [0,1] → quiet regions pass clean, only highlights
    fold (the actual blowout-control behavior). Sine adds soft contrast below
    the fold point too.

### Verification (2026-07-12)
- **Discovery:** `harness.py` / `explore.py` only load FFGL plugin GLSL
  (`plugins/*.frag.glsl`, `#version 150→330`, `out_color`) — they do **not**
  understand ISF `.fs` files (no `IMG_THIS_PIXEL`/`gl_FragColor` shim). So the
  evolve/gallery workflow is plugin-GLSL-only. ISF prototyping needs a different
  harness or Resolume/an ISF editor.
- Wrote `isf_fold_test.py`: compiles `wavefolder.fs` through a moderngl ISF shim
  and asserts fold behavior. All green:
  - compiles as valid GLSL ✓
  - Wet=0 → exact passthrough (0.0 error) ✓
  - Drive=1 triangle → identity below fold point (quiet passes clean) ✓
  - Drive=4 → output bounded [0, 0.996], 63 direction reversals (contour bands,
    never clips to white) ✓
  - Wrap mode bounded ✓
  - neutral grey stays neutral; colored input keeps channels distinct ✓

### Resolume build (2026-07-12)
- Registered in `plugins.json` as `type=isf`, `display_name` "Wavefolder"
  (→ unique_id `*Wav`, no collision with `*Vor/*Ste/*Flo/*Eul/*Und`),
  `shader: shaders/wavefolder.fs`, `dll: wavefolder.dll`.
- `make build PLUGIN=wavefolder` → `ffgl-isf` wrapped the `.fs` into
  `wavefolder.dll` (clean; only pre-existing vendored ffgl-core warnings). This
  is the standard ISF-ships-as-DLL path (same as voronoi/flow), NOT the Phase-2
  Rust rewrite. Build being green = 2nd confirmation the ISF compiles.
- `make deploy PLUGIN=wavefolder` → copied to
  `.../Documents/Resolume Avenue/Extra Effects/wavefolder.dll` (3.46 MB).

### TODO next
- [ ] Live check in Resolume over a hot source; dial presets. Appears as
      "Wavefolder" in Extra Effects (restart Resolume if it was open during deploy).
- [ ] Optional Phase-2 Rust FFGL port only if we want the evolve/gallery tooling.
