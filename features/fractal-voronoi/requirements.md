# Fractal Voronoi — requirements

FFGL plugin `voronoi-fractal` (`VrFr`, "Voronoi Fractal "). A Rust/plugin-GLSL
port of `shaders/voronoi_reactive.fs` carrying two modes: **Layered** (the
existing 3-layer look) and **Fractal** (hierarchical jittered Voronoi after
Boris the Brave, <https://www.boristhebrave.com/2026/08/29/fractal-jittered-voronoi-partitions/>).

The ISF shader `shaders/voronoi_reactive.fs` is **not** modified and stays
registered in `plugins.json`. Existing sets keep working.

## Why a plugin and not an ISF mode

Two of the three goals are unreachable from ISF:

- **FV-BPM** — beat-locked drift needs host tempo. FFGL delivers `bpm` and
  `barPhase` via `SetBeatInfo` (`ffgl-core/src/inputs.rs`); ISF in Resolume
  exposes only `TIME`/`TIMEDELTA`/`FRAMEINDEX`.
- **FV-HARNESS** — `tools/shader-harness` globs `plugins/**/*.frag.glsl`. Plugin
  GLSL is testable as-is; ISF would need a shim that does not exist.

## Modes

**FV-MODE** — `Mode` selects Layered (< 0.5) or Fractal (>= 0.5).

### Layered

Port of the ISF behaviour: three independent Voronoi layers at
`density * layerSpread^i`, blended by `layerMix^i`, cell hue from the layer's own
cell ID. Layers have no structural relationship to each other.

Not bit-exact with the ISF original — drift is re-based (FV-DRIFT) and contrast
is re-ordered (FV-TONE). It is the same look, not the same numbers.

### Fractal

**FV-HIER** — Level 0 is a jittered grid at `density`. Level `i+1` is a grid at
`density * layerSpread^(i+1)`; each level-`i+1` site's **parent** is the
lowest-cost site at level `i`. `Depth` sets the number of levels (1..6).

**FV-ROOT** — A pixel's partition is found by taking the nearest site at the
finest level and tracing parents to level 0. The level-0 cell reached is the
**root**; its ID drives hue. A whole fractal partition therefore shares one hue,
and its boundary is resolved at the finest level's scale — that is the coastline.

**FV-EDGE** — Nearest (F1) and second-nearest (F2) finest-level sites are both
traced to root. Roots differ → the pixel is on a partition boundary, drawn with
`edgeDist = F2 - F1`. Roots match → interior; the fine-cell edge is drawn at
`layerMix` strength. So `layerMix` reads as "interior detail": 0 = coastlines
only, 1 = every cell edge visible.

**FV-BOUND** — Site jitter stays in `[0.1, 0.9]` of its cell, as in the ISF
original. Sites therefore never leave their cell, and a 3x3 neighbourhood is
sufficient for both nearest-site and parent search — so the search is ±1
(9 probes/level) and not the reference's ±2 (25), which it needs because its
jitter fills the whole square. Verified: ±1 and ±2 render bit-identically at
every Layer Spread in 1.5..4.0 and Depth in 2..6.

**FV-NOADAPT** — The article's adaptive early-out is deliberately **not**
implemented. It exists to decide when a fixed depth is deep enough to
approximate infinite recursion; here `Depth` is an artistic knob (boundary
crinkliness), so a fixed trace is correct by definition.

## Image coupling

**FV-COAST** — `Coastline Bias` bends the query domain along the image's
luminance gradient, before any grid lookup:

```
uv += vec2(dL/dx, dL/dy) * coastlineBias * imageInfluence * COAST_GAIN
```

Every level is viewed through the same warp, so the hierarchy bends coherently
and root boundaries deform with image structure. Sites are untouched, so
ancestry is unchanged and cannot tear.

What this does and does not do — measured, not asserted. It is a strong,
visible deformation: at full bias, 57.6% of pixels change by more than 8/255.
It is **not** a watershed: boundaries do not preferentially settle onto bright
ridges (enrichment ~1.0x, where 1.0x is chance).

Biasing the parent-link *cost* by the image was specified first and is
**refuted**. A chain's root is always a level-0 cell, and the level-0 sites are
fixed and image-independent, so a link bias only reassigns cells near ties and
the partition stays the level-0 Voronoi diagram. Enrichment saturated at 1.1x
across gains from 8 to 150, for both ±1 and ±2 search, and for both
midpoint-sampled and max-along-link penalties. Do not re-specify it: making
boundaries land on image structure requires the level-0 *sites* to move, which
is a different algorithm.

**FV-SITEONLY** — All image coupling in Fractal mode is evaluated **at site
positions**, never at the querying fragment. This is load-bearing: a per-pixel
metric warp lets adjacent pixels disagree about a shared ancestor and tears the
coastline. Consequence: the ISF's `densityMult` (per-fragment brightness →
grid scale, `voronoi_reactive.fs:232`) is **disabled in Fractal mode**. It
remains active in Layered mode, where there is no ancestry to break.

**FV-GATE** — `Image Influence` is the master gate for every image effect:
geometry (`densityMult`, `driftScale`, `coastBias`) and tone (`cellValue`,
`edgeBright`). At `Image Influence = 0` the plugin is a pure generator and
`Cert Contrast` / `Cert Brightness` / `Coastline Bias` are inert by design.

**FV-TONEGATE** — Cell fill and edge brightness use the **same** gate,
`certBrightness * imageInfluence`. (The ISF gates fills by
`certBrightness * imageInfluence` but edges by `imageInfluence` alone
(`voronoi_reactive.fs:348` vs `:357`), so Cert Brightness moves fills and never
edges. Fixed here.)

## Tone

**FV-FILL** — The cell interior level is the `Fill Level` knob, not a constant.
It must be a knob: `Contrast` stretches around a 0.5 pivot, so the old
hardcoded 0.55 sat essentially *on* the pivot and no amount of Contrast could
drive the ground to black. At `Fill Level = 0` with `Color Sat = 0` the ground
is fully black (measured: 97.4% of pixels at exactly 0), which is what makes
edge-only output possible.

**FV-EDGEGATE** — Edge presence is scaled by `mix(1.0, certainty, imageInfluence)`,
so as Image Influence rises, edges fade over dark ground and survive only where
the image is lit. Combined with `Fill Level = 0` this isolates fractal outlines
of the subject. Measured: ink falling on a lit figure rises from 20.6% (chance)
to 78.9% as Influence goes 0 → 1. `Cert Contrast` sets how hard the cut is.
Note the total amount of ink drops as the gate closes — compensate with Edge
Width or Edge Glow.

**FV-TONE** — `Contrast` is applied **once, to the composited image**, after the
layer average and before `Brightness`. (The ISF applies it per layer inside the
loop, with a `clamp(...,0,1)` that crushes each layer's headroom before the
blend, so raising Contrast flattens instead of punching —
`voronoi_reactive.fs:365`.)

**FV-NOTINT** — The ISF `tint` colour input is dropped. It is a flat
`color *= tint.rgb` and is better served by a downstream Resolume colour effect.

## Drift

**FV-DRIFT** — Drift is driven by `u_anim_time`, a **cycle count** (accumulating
float, 1.0 = one full drift cycle), not seconds. Circular drift advances one
orbit per cycle; chaotic drift steps once per cycle (`floor`/`fract`).

**FV-SYNC** — `Beat Sync` selects `Off | 4 bars | 2 bars | 1 bar | 1/2 | 1/4`.

- `Off` — free-run: `anim_time += driftSpeed / 60` per frame. Per-seed rate
  randomisation is **on** (`rate = 0.3 + hash*0.7`), giving organic,
  non-coherent motion. This is the ISF behaviour.
- Synced — `anim_time = bars_elapsed / period_bars`, where `bars_elapsed`
  comes from `barPhase` plus a bar counter maintained in Rust (FFGL supplies no
  bar index). Per-seed rate randomisation is **off**; seeds keep a random start
  phase but share one rate, so the whole field lands together on the beat.
  `Drift Speed` is inert while synced.

**FV-LINKSTABLE** — In Fractal mode, parent links are computed from
**undrifted** base site positions. Drift moves sites for rendering only. If links
were computed from drifted positions the ancestry would flip as sites cross
tie-lines and whole partitions would pop between hues. Boundaries wobble;
topology holds.

## Parameters

All FFGL params are normalised `0..1` and mapped in accessors, matching
`parcel-subdivision`. 22 params:

| # | Name | Range / meaning |
|---|---|---|
| 0 | Mode | Layered / Fractal |
| 1 | Density | 2..30 grid scale |
| 2 | Layer Spread | 1.5..4 per-level scale ratio |
| 3 | Layer Mix | layer blend (Layered) / interior detail (Fractal) |
| 4 | Depth | 1..6 levels (Fractal only) |
| 5 | Coastline Bias | 0..1 image bias on parent link (Fractal only) |
| 6 | Drift Speed | 0..3 free-run rate (Beat Sync = Off only) |
| 7 | Beat Sync | Off / 4 / 2 / 1 / 1÷2 / 1÷4 bars |
| 8 | Drift Chaos | 0..1 circular ↔ random-walk |
| 9 | Warp | 0..1 spatial domain warp |
| 10 | Edge Width | 0..0.15 |
| 11 | Edge Glow | 0..1 |
| 12 | Color Shift | 0..1 hue rotation |
| 13 | Color Sat | 0..1 |
| 14 | Image Influence | 0..1 master image gate |
| 15 | NC Kernel | 0.01..0.5 certainty blur radius |
| 16 | Cert Contrast | 0.1..5 gamma on source luminance |
| 17 | Cert Brightness | 0..1 tonal swing from the image |
| 18 | Fill Level | 0..1 cell interior / ground level; 0 = black |
| 19 | Brightness | 0..2 output gain |
| 20 | Contrast | 0..2 output tone stretch |
| 21 | Image Blend | 0..1 mix back to source |

Params 14–17 are adjacent so the image-mapping group reads as a group in the
Resolume panel.

## Harness contract

**FV-HARNESS-UNIFORMS** — `plugins/voronoi-fractal/src/shaders/voronoi.frag.glsl`
follows the house contract: `#version 150`, `v_uv` / `out_color`, `uniform
sampler2D u_input`, `u_texel_size` fed automatically by the harness. Companion
`voronoi.defaults.json` supplies every other uniform's default.

`u_anim_time` and `u_beat_locked` are plain uniforms — static in the harness
(scrub them by hand to tune a frame), live in Resolume. The harness does not
model a transport.

## Cost

9 probes/level × up to 6 levels ≈ 54 site evaluations, versus the ISF's 27
(9 × 3), plus 2 root traces per pixel. The parent search is pure arithmetic —
no texture fetches, since FV-COAST moved out of the link cost — and the domain
warp costs 4 fetches per pixel once. Not yet measured on the target GPU.
