# Finding Ground — Requirements

**Status:** drafted 2026-07-18 · **Branch:** TBD (off `main`)
**Type:** composition + new geometric-primitive **generator** plugins

---

## 1. Concept & inspiration

A live composition inspired by **Frances Gallardo's _Finding Ground_** (installation,
_Localidad Alterna III / MECANISMOS_, MECA Art Fair, San Juan).

Gallardo's operative gesture in _Finding Ground_: she **intervenes on satellite images
of rivers and valleys, extracting the embedded codes from their lines and patterns and
reinterpreting that logic as visual language** — a deliberate turn *downward* from her
atmospheric work (hurricanes as NOAA cut-paper _calado_, embroidered storm tracks,
nanoscale dust) to the **land**: rivers, valleys, watersheds, parcels, and "conflicts
of use." Her line-work is intentionally **imperfect** — she leaves the pencil marks in,
"because hurricanes are not perfect, but they can be beautiful."

**Our translation:** procedural **terrain-line Sources** ("ground-structure") that are
then **destabilized live** by the existing organic/feedback toolkit ("the storm passing
back over the land"). "Finding ground" = stable land-forms *precipitating* out of the
churn. The plugin *is* the concept: each generator encodes a terrain feature as drawable
primitives — extracting line-logic and reinterpreting it as visual language.

**Aesthetic leaning (confirmed):** **hybrid** — hard geometric primitives eroding into
organic flow, not pure CAD and not pure noise.

---

## 2. Deliverables

1. **Three new generator plugins** (geometric primitives, drawn from nothing):
   - **P1 — Contour Field** — iso-lines of an animated height field → *valleys / ground*
   - **P2 — Dendritic Network** — recursive branching line-tree → *rivers / watersheds*
   - **P3 — Parcel Subdivision** — recursive BSP / lattice / graticule → *land use / conflicts of use*
2. **A Resolume composition** chaining the generators with existing effects into the
   finished piece (see §5). Deliverable = the `.avc` + a short patch recipe.

All three priorities are **in scope this feature** (build order in `plan.md`).

---

## 3. The generator primitives

Shared traits across all three:
- **Drawn procedurally** from `uv + time + params` — ignore any input image (generators).
- **Hand-imperfection** param family: line **jitter**, **incompleteness**, visible
  construction — so primitives never read as sterile CAD (Gallardo's pencil marks).
- **Shared coordinate space** so they can overlay coherently (rivers sit in valleys sit
  within parcels).

### Design principle — one terrain, three readings (recommended, see `_CLARIFY_`)
All three can sample **one shared procedural height field** so contours, rivers, and
parcels describe the *same* land: contours = iso-lines of it, rivers = its drainage
(steepest descent), parcels = the human grid laid over it. Conceptually faithful to
"extract line-logic from terrain" and visually unifying.
`_CLARIFY_` adopt the shared-height-field coherence, or keep the three fully independent?

### P1 — Contour Field
- **Draws:** anti-aliased iso-contour bands of an fbm height field (topographic map).
- **Draft params:** Scale/zoom · Contour interval (count) · Line weight · Elevation
  offset (scrub bands) · Warp/animation speed · Jitter · Palette/mode.

### P2 — Dendritic Network
- **Draws:** recursive branching line network (main channel + tributaries).
- **Approach (pick in plan):** (a) drainage/steepest-descent accumulation over the
  shared height field — coherent with P1; or (b) SDF of a procedurally grown branch tree.
- **Draft params:** Branch density · Recursion depth · Channel-vs-tributary weight ·
  Meander/jitter · Growth/animate · Source seed.

### P3 — Parcel Subdivision
- **Draws:** recursive space subdivision (BSP splits / lattice) + survey graticule.
- **Draft params:** Subdivision depth · Regularity (regular grid ↔ irregular parcels) ·
  Border weight · Gap/inset · Reshuffle/animate · Jitter.

---

## 4. Aesthetic constraints

- **Motion:** slow **geological drift** underneath, plus live-reactive destabilization.
- **Hand-imperfection: required** (shared jitter/incompleteness). Non-negotiable trait.
- `_CLARIFY_` **Palette** — default proposal: restrained **ink-on-paper / earth
  monochrome** to honor the cut-paper origin; the repo's gallery accent (`#ff3b00`) is
  an alt. Confirm direction.
- `_CLARIFY_` **Abstract vs literal** — default: **reads as map/terrain but abstracted**
  (mirrors Gallardo abstracting satellite imagery), not a literal GIS render.

---

## 5. Hybrid destabilization (existing plugins)

The organic half of the hybrid comes free from the shipped toolkit — the composition
chains `generator → …`:
- **flow_euler / flow_undertow** — rivers animate, silt, drift.
- **voronoi_reactive** — organic parcel/cell counter-field.
- **delay_tap / delay_write** — feedback accumulation, ghost terrain, sediment layering.
- **mirror_transform** — fold/kaleidoscope symmetry of the land.
- **wavefolder** — tame feedback blowout / band the intensity (contour-like itself).

---

## 6. Non-goals

- **No real GIS/DEM data import** — terrain is procedural. `_CLARIFY_` stretch: allow a
  real satellite/DEM *image input* to drive the generators (would make them effects, not
  pure generators — deferred; conflicts with the generators-from-nothing choice).
- Not audio-reactive (unless later desired).
- Not a single mega-plugin — three focused atoms + composition (matches repo philosophy).

---

## 7. Technical constraints

- Target **Resolume** via the existing FFGL/ISF pipeline.
- **Generator-as-effect** for v1: implement as an effect that **ignores its input** (draw
  over / replace), so it works on any layer without needing confirmed FFGL *source*
  support. `_CLARIFY_`/investigate proper FFGL source support in `ffgl-rs`.
- Must **save/restore host GL state** (shared context with Resolume).
- Plugin internal **name = exactly 16 bytes**; mind the **unique_id = '*'+name[0..3]**
  collision gotcha (see reference memory) — pick display names that don't collide.
- Register in **`plugins.json`** + the **Cargo workspace**; build/deploy per repo flow.

---

## 8. Open decisions (resolve during plan/build)

- [ ] Medium: **Rust FFGL plugin-GLSL** vs **ISF** (see `plan.md` Stage 0 — recommend Rust
      plugin-GLSL to unlock the `explore.py`/`gallery.py` generative-tuning loop).
- [ ] Shared height field vs independent generators (§3).
- [ ] Palette + abstract/literal (§4).
- [ ] FFGL source vs effect-ignoring-input (§7).
- [ ] Real-data image input as a stretch mode (§6).
