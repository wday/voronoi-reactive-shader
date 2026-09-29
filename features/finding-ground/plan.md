# Finding Ground — Implementation Plan

Derived from `requirements.md` (2026-07-18). Build order: **P1 → P2 → P3 → composition**.

---

## Stage 0 — Lock the three gating decisions

These steer everything; recommend confirming before Stage 1.

1. **Medium — recommend Rust FFGL plugin-GLSL.**
   - *Why:* these are heavily generative/aesthetic-tuning-heavy, and the repo's
     `explore.py` → `gallery.py` → evolve loop globs **`plugins/*.frag.glsl`** — so a
     plugin-GLSL fragment shader unlocks parameter-space exploration and keeper-evolution
     for tuning the look, *and* runs in Resolume. ISF (`shaders/*.fs`) is faster to write
     but is **Resolume-only** to test (no ISF shim in the harness yet).
   - *Alt:* ISF-first if we want to sketch in Resolume before committing to Rust.
2. **Generator-as-effect vs FFGL source — recommend effect-ignoring-input for v1.**
   Draw from `uv+time+params`, ignore the input texture. Works on any layer today;
   revisit proper FFGL source support later.
3. **Shared height field — recommend yes.** A tiny shared GLSL include
   (`terrain.glsl`: fbm height + gradient) sampled by P1/P2/P3 so contours, rivers, and
   parcels describe one land. Establish it in Stage 1, reuse in 2–3.

---

## Stage 1 — P1 Contour Field  *(also lays the shared terrain foundation)*

1. `plugins/contour-field/src/shaders/terrain.glsl` — shared fbm height + gradient.
2. `…/shaders/contour.frag.glsl` — iso-lines via `fract(height/interval)` with
   derivative-based (`fwidth`) anti-aliased bands; elevation-offset scrub; jitter.
3. **Harness tune:** `explore.py` over the params, `gallery.py` review, evolve keepers.
4. **Rust scaffold:** clone a lean existing plugin (e.g. `slipgrid` / `slew-limiter`)
   for the FFGL boilerplate; wire params → uniforms; ignore input.
5. Pick 16-byte name + non-colliding display name; register in `plugins.json` + workspace.
6. Build (MSVC, Windows cargo from WSL) → deploy → **live Resolume check**.

## Stage 2 — P2 Dendritic Network

1. Prototype **both** river approaches in the harness against the shared terrain:
   (a) steepest-descent drainage accumulation, (b) SDF branch-growth — pick by look.
2. `…/shaders/dendritic.frag.glsl`; params (density, depth, channel weight, meander,
   growth, seed, jitter).
3. Rust scaffold + params + register; build → deploy → live check.
4. Verify it overlays coherently on P1 (rivers in the valleys).

## Stage 3 — P3 Parcel Subdivision

1. `…/shaders/parcel.frag.glsl` — recursive BSP / lattice + graticule over the shared
   space; regularity knob (grid ↔ irregular), border weight, gap/inset, jitter.
2. Rust scaffold + params + register; build → deploy → live check.
3. Verify the three-layer read (parcels over rivers over contours) holds up.

## Stage 4 — Composition assembly

1. Build the Resolume `.avc`: generator layers + existing effect chains
   (flow / voronoi / delay-tap+write / mirror / wavefolder) realizing the hybrid.
2. Tune the **"finding ground"** dynamic — hard structure precipitating from churn.
3. Inspect/verify with the `avc` skill; capture the patch recipe in the devlog.

## Stage 5 — Polish

- Shared hand-imperfection pass (jitter/incompleteness feel consistent across all three).
- Lock palette + motion (resolve §4 `_CLARIFY_`s from a live look).

---

## Testing & workflow

- **GLSL stages:** shader harness (`explore.py`/`gallery.py`) — the tuning engine.
- **Integration:** Resolume; deploy via `make build PLUGIN=… && make deploy PLUGIN=…`
  (close Resolume first — DLLs lock while running).
- **devlog.md:** update per stage with decisions + what the look actually did.

## Milestones

- **M1:** P1 Contour Field live in Resolume, harness-tunable, shared terrain established.
- **M2:** P2 + P3 live; three generators overlay coherently.
- **M3:** Composition assembled; "finding ground" dynamic reads; palette/motion locked.
