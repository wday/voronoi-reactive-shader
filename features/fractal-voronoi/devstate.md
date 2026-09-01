# Fractal Voronoi — development state

## Built

`plugins/voronoi-fractal/` — FFGL plugin `VrFr`, "Voronoi Fractal ", 21 params.
Registered in `plugins/Cargo.toml` and `plugins.json`. Cross-builds clean to
`voronoi_fractal.dll` (945 KB).

- `clock.rs` — drift clock, free-run and bar-locked (FV-DRIFT / FV-SYNC).
- `params.rs` — 21 normalised params with mapped accessors.
- `voronoi.rs` — FFGL instance; reads `host_beat.barPhase`.
- `shader.rs` — GL plumbing, generated in the `parcel-subdivision` idiom.
- `shaders/voronoi.frag.glsl` — both modes; `voronoi.defaults.json` for the harness.

## Verified

- **Clock logic** — 10/10 unit tests pass (run standalone with `rustc --test`;
  see "Testing" below). Covers free-run, per-period cycle counts, bar-wrap
  counting, jitter rejection, sync ignoring Drift Speed, and phase-snap.
- **Shader compiles and renders** headless via moderngl at `#version 330`.
  Every key in `voronoi.defaults.json` maps to a live uniform; no dead keys.
- **The hierarchy works** — Depth visibly adds boundary detail with the
  diminishing returns the construction predicts (mean frame delta 13.4 → 10.2
  → 7.1 for depth 2 → 4 → 6).
- **±1 parent search is sufficient** — bit-identical to ±2 at every Layer
  Spread in 1.5..4.0 and Depth in 2..6. FV-BOUND holds.
- **Coastline Bias is not inert** — 57.6% of pixels change by >8/255 between
  bias 0 and 1.

## Established facts

- FFGL supplies `bpm` and `barPhase` (0..1 per bar) via `SetBeatInfo`
  (`vendor/ffgl-rs/ffgl-core/src/inputs.rs:34`). There is **no bar index** — the
  bar counter in `clock.rs` exists for that reason.
- ISF in Resolume has no tempo uniform. This is why the port exists.
- `ffgl-core` does **not** build on Linux: its `build.rs` `cfg_if` has macOS and
  Windows branches only, so `cargo test -p voronoi-fractal` fails natively.
  Build via `make build PLUGIN=voronoi_fractal` (Windows cargo through WSL).
- The harness globs `plugins/**/*.frag.glsl`, auto-feeds only `u_input` and
  `u_texel_size`, expands `//#include`, and upgrades `#version 150` → 330.
  `numpy` was added to the harness project for the verification scripts.
- Unique ID `VrFr`. Taken: ChDp CtrF DLMd DlyT DlyW DnNw DtLm LgFb MrTx PrcL
  SlpG SlwL Tx3D VdLp VsRd VsWr.

## Testing

`clock.rs` has no ffgl-core dependency, so it can be tested natively:

    cp plugins/voronoi-fractal/src/clock.rs /tmp/clock_test.rs
    rustc --test /tmp/clock_test.rs -o /tmp/clock_test && /tmp/clock_test

## Invariants

- **FV-SITEONLY** — image coupling never varies the *metric* per fragment. The
  domain warp is safe because it displaces the query point continuously; a
  per-fragment change to cell size would let adjacent pixels disagree about a
  shared ancestor and tear the coastline. This is why `densityMult` is confined
  to Layered mode.
- **FV-LINKSTABLE** — parent links come from undrifted site positions. Drifted
  links make partitions pop between hues.
- **FV-BOUND** — jitter confined to `[0.1,0.9]` is what makes ±1 sufficient.
  Widening the jitter requires widening the search to ±2.
- `shaders/voronoi_reactive.fs` is not modified by this feature.

## Unverified

- **Everything about how it looks.** No live Resolume check, and no run against
  real footage — all verification so far is against synthetic sources.
- Whether `barPhase` sync actually lands on the beat in Resolume. The clock
  logic is tested; the host's phase reporting is not.
- Whether Coastline Bias reads as "input structure" visually. It is a strong
  deformation but provably not a watershed (see FV-COAST); whether that is the
  wanted effect is a judgement to make on real footage.
- Whether undrifted links (FV-LINKSTABLE) leave enough visible motion in
  Fractal mode to justify the Drift knobs there.
- Real GPU cost. Never measured on the target hardware.
- Multi-bar sync periods (2 and 4 bars) align to whenever sync was engaged, not
  to an absolute bar. FFGL exposes no bar index, so this cannot be fixed
  plugin-side; unclear whether it matters in practice.
