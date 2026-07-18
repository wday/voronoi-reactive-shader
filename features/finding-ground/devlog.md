# Finding Ground — Devlog

## 2026-07-18 — Kickoff: research + spec

**Research (grounding the aesthetic).** Web-researched Frances Gallardo's _Finding Ground_
(MECA/MECANISMOS, _Localidad Alterna III_). Core gesture: **intervenes on satellite images
of rivers and valleys, extracting the embedded codes from lines/patterns and reinterpreting
that logic as visual language** — a downward turn from her atmospheric work (hurricane
_calado_ cut-paper from NOAA imagery, embroidered storm tracks, nanoscale _Aerosoles_ dust)
to the land, "conflicts of use," connecting to the Earth. Signature: intentionally
**imperfect** line-work (leaves the pencil marks in).

**Interpretation confirmed by user** ("nailed it"): procedural terrain-line generators
destabilized live by the organic/feedback toolkit; "finding ground" = stable land-forms
precipitating from churn.

**Decisions locked:**
- Aesthetic leaning: **hybrid** (hard primitives eroding into organic flow).
- Primitives are **generators** (drawn from nothing), implemented **as effects that ignore
  input** for v1.
- **All three primitives in scope**: P1 Contour Field, P2 Dendritic Network, P3 Parcel
  Subdivision. Build order P1 → P2 → P3 → composition.
- Session scope: requirements + plan (both written) → straight into building, no fresh
  session.

**Recommendations pending user confirm (Stage 0):**
- Medium: **Rust FFGL plugin-GLSL** (to unlock the `explore.py`/`gallery.py` tuning loop),
  over ISF.
- **Shared height field** so contours/rivers/parcels describe one terrain.

**Open `_CLARIFY_`s in requirements:** palette (default ink-on-paper/earth mono), abstract-
vs-literal (default abstracted-map), shared-vs-independent terrain, FFGL source vs effect,
real-data image-input stretch mode.

**Artifacts:** `requirements.md`, `plan.md` written. Next: confirm Stage 0, then Stage 1
(P1 Contour Field + shared `terrain.glsl`).

## 2026-07-18 — Stage 1 (P1 Contour Field): shader landed + verified

**Proceeding on the Stage 0 recommendations** (Rust plugin-GLSL medium · generator-as-
effect · ink-on-paper palette) pending any user veto.

**Wrote** `plugins/contour-field/src/shaders/contour.frag.glsl` + `contour.defaults.json`.
Fbm value-noise height, domain-warped → sinuous valleys; iso-contour extraction with
`fwidth` AA; params: scale, warp, contours, line_weight, elevation, jitter, breakup,
warmth, time (9). Generator — ignores `u_input`. Matches harness contract (v150, v_uv/
out_color, u_texel_size auto-fed, defaults json).

**terrain.glsl deferred:** harness has no `#include`, so the fbm/height block is inlined
in the frag for now. Will factor into a shared `terrain.glsl` (concatenated by the Rust
plugin) when P2 Dendritic reuses it — that's when sharing earns its keep.

**Verified headless** via a standalone moderngl render script (scratchpad
`render_contour.py`, llvmpipe) → PNG, eyeballed. Compiles clean; all 9 uniforms bind.

**Tuning found (via 4 headless iterations):**
- Black-mush fills came from **line weight × weight-jitter wobble** fattening neighbouring
  contours until they merge — fixed with a hard `lw` cap (0.18 band-space) + softer wobble
  + lower default weight. Cap is what keeps it line-work not marbling.
- Ragged black **speckle** came from a **blocky floored-cell** jitter (`floor(p*40)`)
  making discontinuous height jumps → replaced with **smooth low-freq vnoise** displacement
  → organic hand meander.
- A `fwidth`-based resolution guard is in but wasn't the main lever; kept as insurance.

**State:** default look is a legible hand-drawn topo map (ink on warm paper).

### Rust FFGL scaffold — BUILT + DEPLOYED (per user "straight to rust, I'll test in Resolume")

Cloned the slipgrid pattern into `plugins/contour-field/` (Cargo.toml, lib.rs, contour.rs,
shader.rs, params.rs, shaders/fullscreen.vert.glsl + contour.frag.glsl). Registered in
`plugins/Cargo.toml` workspace + `plugins.json` (`contour_field` / crate `contour-field`).

- **Plugin id `CtrF`**, name `"Contour Field   "` (16 bytes), `PluginType::Effect`.
- **9 params** (all Standard 0..1, passed straight to the shader which does the mix()
  mapping): Scale, Warp, Contours, Line Weight, Elevation, Jitter, Break Up, Warmth, Drift.
  Defaults mirror `contour.defaults.json`.
- **Generator specifics vs slipgrid:** (1) `draw()` renders even with **no input**
  (slipgrid bails to black); (2) resolution/aspect from the **GL viewport** (`GetIntegerv
  VIEWPORT`) so it's input-independent; (3) internal **drift phase accumulator** →
  `u_time` (Drift knob, 0 = frozen), no host-time dependency; (4) host GL state saved/
  restored (scissor/blend/depth) like the other plugins.
- **Built clean** (MSVC, 4.67s; only ffgl-core bindgen warnings) → **deployed** to
  `Documents/Resolume Avenue/Extra Effects/contour_field.dll`.

**Effect-not-Source caveat (v1):** it registers as an Effect, so in Resolume drop it on a
layer/clip (a solid or any clip); it ignores that input and replaces output with the
contour field. A truly empty layer won't run the effect chain — that's the deferred
FFGL-source decision from requirements §7.

**Next:** user live-checks P1 in Resolume (milestone M1). Then Stage 2 (P2 Dendritic
Network — factor the shared `terrain.glsl` here so rivers follow this terrain's drainage).
Uncommitted on `feature/finding-ground`.

### Live feedback #1 — centre-anchor the Scale (2026-07-18)

User: modulating Scale slid the terrain diagonally (it zoomed about the (0,0) corner).
Fixed → scale about the frame centre: `p = (uv - center) * zoom` with
`center = vec2(aspect,1.0)*0.5`. Verified headless at scale 0.30 vs 0.70 — the central
motif stays put and zooms in place instead of sliding out of the corner. Rebuilt+deployed
`contour_field.dll`.

Same class of bug as voronoi's earlier fix. **voronoi source was already centre-anchored**
(`shaders/voronoi_reactive.fs:238`, seed back-projection re-adds centre at :260) — no code
change needed; redeployed `voronoi_reactive.dll` so the running copy matches. Captured the
pattern as a project memory so P3 Parcel (and any future generator with a zoom knob) does
it from the start.

**P1 committed** as two commits on `feature/finding-ground` (`docs:` spec, `feat:` plugin).

## 2026-07-18 — Stage 2 (P2 Dendritic Network): shader + plugin, BUILT + DEPLOYED

Rivers/watersheds over the **same terrain field as P1** (so they run down its valleys).

**Approach — two tries (headless-render loop, 4 iterations):**
1. *Drainage convergence (D8, single step)* — for each pixel, fraction of a neighbour
   ring whose downhill flow points inward. **Failed:** single-step convergence only marks
   local pits → speckled dots, not connected channels.
2. *Hessian valley-line extraction* (kept): a thalweg is where the surface curves UP across
   the valley (large positive principal curvature of the Hessian) AND we're at the bottom
   of that cross-section (slope along the across-valley eigenvector ≈ 0). Gives **connected,
   branching** lines. Then: probe curvature at a **coarse** radius (major valleys, not every
   fbm wrinkle), **gate to the lowlands** (`1-smoothstep(h)`) so only low terrain carries
   rivers, downstream **Hierarchy** widens with depth, and a final **harden** smoothstep
   kills grey partial-coverage haze → crisp ink line-work. Reads as a dendritic drainage
   network / river delta.

**Plugin** `plugins/dendritic-network/` (`DnNw`, name `"Dendritic Net   "`, crate
`dendritic-network`), cloned from contour-field. 9 params: Scale, Warp, **Density**
(threshold: high→main rivers only), **Thickness**, **Hierarchy** (lowland gating), Jitter,
Break Up, Warmth, Drift. Same generator plumbing (renders w/o input, viewport resolution,
drift phase, centre-anchored scale). Registered + built (MSVC, 1.17s) + deployed.

**Shared terrain still inlined+duplicated** (contour + dendritic each carry the fbm/height
block, delimited by a KEEP-IN-SYNC marker) — harness has no `#include`. True factoring into
a Rust-concatenated `terrain.glsl` still deferred; two copies is the current cost. When the
terrain block changes, update both.

**Starting look is a decent-but-tunable drainage network** (some chunkiness/fragmentation
remains — live taste-tuning territory, same as P1). Uncommitted on `feature/finding-ground`.
**Next:** user live-checks P2, then Stage 3 (P3 Parcel Subdivision).

**P2 committed** (`feat: … P2 Dendritic Network (DnNw)`).

## 2026-07-18 — Shared terrain factor + Stage 3 (P3 Parcel Subdivision)

### Factored the shared terrain (finally, not a 3rd copy)
- `plugins/finding-ground-common/terrain.glsl` — canonical hash/vnoise/fbm/height,
  **single source of truth**. Reads the includer's `u_warp`.
- **Include mechanism** (harness had no `#include`): a plain-comment directive
  `//#include "<relpath>"`, expanded by (a) the harness `load_shader_source`
  (`_expand_includes`, relative to the including file, recursive), (b) my headless
  render tool, and (c) the Rust plugins (`include_str!(terrain) + FS.replace(directive)`).
  Backward-compatible; shaders without the directive are untouched.
- **Gotcha:** moderngl has its own naive `#include` scanner that is NOT comment-aware —
  it tripped on the literal `#include "…"` token sitting inside terrain.glsl's *comment*.
  Fix: never write the literal quoted include token in comments.
- **Migrated P1 + P2** onto the include and **verified pixel-identical** headless (matched
  params → `getbbox() is None`), so the shipped plugins didn't regress. Rebuilt + redeployed.

### P3 Parcel Subdivision — BUILT + DEPLOYED
Generator `plugins/parcel-subdivision/` (`PrcL`, name `"Parcel Subdiv   "`, crate
`parcel-subdivision`). The hard-geometric counterweight: survey **townships** (integer
grid / graticule) over the shared terrain, each recursively **BSP-split into parcels**;
the **lowlands subdivide deeper** (`terrainDepth` from `height()` at township centre) so
parcels register with P2's rivers / P1's valleys — "conflicts of use". 9 params: Scale,
Warp, **Depth**, **Regularity** (regular half-splits ↔ irregular), **Border**, **Inset**
(parcel gap), Jitter, Warmth, Drift. Same generator plumbing + centre-anchored scale.
**Strongest first render of the three** — reads immediately as a cadastral map. Built
(MSVC ~1.2s) + deployed.

**Milestone M2 reached** (P1+P2+P3 live, overlaying coherently via one terrain).
Uncommitted parts: the factor + P3. **Next:** user live-checks P3; then Stage 4 —
compose the `.avc` (generators + flow/voronoi/delay/mirror/wavefolder hybrid chains).
