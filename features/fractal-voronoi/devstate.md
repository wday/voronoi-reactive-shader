# Fractal Voronoi — development state

## Built

`plugins/voronoi-fractal/` — FFGL plugin `VrF2`, "Voronoi Fract v2", 22 params,
Layered and Fractal modes. Registered in `plugins/Cargo.toml` and `plugins.json`.
Cross-builds clean and is deployed to Resolume.

- `clock.rs` — drift clock, free-run and bar-locked (FV-DRIFT / FV-SYNC).
- `params.rs` — 22 normalised params with mapped accessors.
- `voronoi.rs` — FFGL instance; reads `host_beat.barPhase`; logs input vs
  hardware texture dims once per instance.
- `shader.rs` — GL plumbing in the `parcel-subdivision` idiom.
- `shaders/voronoi.frag.glsl` — both modes; `voronoi.defaults.json` for the harness.

Live in a Resolume comp and playable.

## Verified

- **Clock** — 10/10 unit tests (see Testing below).
- **Shader compiles and renders** headless at `#version 330`; every
  `voronoi.defaults.json` key maps to a live uniform, no dead keys.
- **Hierarchy works** — Depth adds boundary detail with the diminishing returns
  the construction predicts (mean frame delta 13.4 → 10.2 → 7.1 for depth
  2 → 4 → 6).
- **±1 parent search suffices** — bit-identical to ±2 at every Layer Spread in
  1.5..4.0 and Depth in 2..6.
- **NPOT padding is real on this rig** — logged live: 1920x1080 content in a
  1920x1088 texture, `uv_scale 1.00000,0.99265`, i.e. **vertical padding only**.
  `content_uv()` corrects it exactly (see FV-NPOT).
- **NC Kernel controls plate flatness** — in-cell gradient 0.0000 at 0.02
  (perfectly flat plates, full 0-255 range), 0.43 at 0.40, 0.20 at 1.00 with the
  range compressed to 90-255. Low = sharp cell-quantised plates.
- **FV-NCALIGN changes the picture** — 23-29% of pixels move by >8/255 with
  edges off, mean delta 29/255 at a sharp kernel.
- **Fill Level reaches true black** — 97.4% of the frame exactly 0 at Fill
  Level 0 (the old hardcoded 0.55 pinned the frame minimum at 140/255).
- **Edge gating isolates the subject** — ink on a lit figure rises from 20.6%
  (chance) to 78.9% as Image Influence goes 0 → 1.
- **Coastline warp magnitude** — `displacement_px = gradient * amount^2 * 0.15 * H`;
  full range ~162 px, ~9 px at bias 0.24. The gradient stencil is isotropic
  (±22 px on both axes at 1080p).

## Established facts

- FFGL supplies `bpm` and `barPhase` (0..1 per bar) via `SetBeatInfo`
  (`vendor/ffgl-rs/ffgl-core/src/inputs.rs:34`). **No bar index** — hence the
  bar counter in `clock.rs`, and hence multi-bar periods align to sync
  engagement rather than to an absolute bar.
- ISF in Resolume has no tempo uniform. This is why the port exists.
- `ffgl-core` does **not** build on Linux (its `build.rs` `cfg_if` has macOS and
  Windows branches only). Build via `make build PLUGIN=voronoi_fractal`.
- **Resolume caches param descriptors against `unique_id`.** Adding param 22
  required bumping `VrFr` → `VrF2`. Settle the param list before a live load, or
  budget an id bump per layout change.
- Resolume's REST API is at `http://<windows-host>:8088/api/v1/composition`.
  From WSL2 use the gateway IP from `ip route show default`, not localhost.
  Effects are at `layers[N]/video/effects[M]`, `params` keyed by name.
- Resolume's log is at
  `/mnt/c/Users/<user>/AppData/Local/Resolume Avenue/Resolume Avenue log.txt`.

## Testing

`clock.rs` has no ffgl-core dependency, so it tests natively:

    cp plugins/voronoi-fractal/src/clock.rs /tmp/clock_test.rs
    rustc --test /tmp/clock_test.rs -o /tmp/clock_test && /tmp/clock_test

Shader verification is headless moderngl through the harness loader; `numpy` was
added to the harness project for it.

## Invariants

- **FV-SITEONLY** — image coupling never varies the *metric* per fragment. The
  domain warp is safe because it displaces the query point continuously; a
  per-fragment change to cell size would let adjacent pixels disagree about a
  shared ancestor and tear the coastline. Hence `densityMult` is Layered-only.
- **FV-LINKSTABLE** — parent links come from undrifted site positions. Drifted
  links make partitions pop between hues.
- **FV-NCALIGN** — certainty weights and cell assignment must use the *same*
  distance.
- **FV-BOUND** — jitter confined to `[0.1,0.9]` is what makes ±1 sufficient.
- `COAST_GAIN` is in frame-heights. Any change must be checked in pixels.
- `shaders/voronoi_reactive.fs` is not modified by this feature.

## Unverified

- **Whether any source/pattern misregistration remains** after the Coastline
  rescale. The warp accounted for the 20-30 px reported; NPOT padding accounts
  for ~8 px more. If a residual persists at Coastline Bias 0, the next suspect
  is the viewport-derived aspect — `g_aspect` comes from `glGetIntegerv(GL_VIEWPORT)`
  and would be wrong if Resolume renders the effect at the padded 1088 height.
  A checkerboard source is the instrument that exposes this.
- Small top/bottom edge deviation of 6-10/255 against an edge-uniform source.
  Near the noise floor of that metric at ~3 cells vertically; cause unknown, and
  it is **not** off-frame seed clamping (clamp vs mirror differ by 0.00) and not
  the padding alone.
- Whether `barPhase` sync lands on the beat in Resolume. Clock logic is tested;
  the host's phase reporting is not.
- Whether Coastline Bias reads as "input structure" visually. It is a strong
  deformation but provably not a watershed.
- Whether undrifted links leave enough visible motion in Fractal mode to justify
  the Drift knobs there.
- Real GPU cost. Never measured on target hardware.
