# Euler Soup tuning — devlog

Renders are reproducible: `features/euler-soup-tuning/cases.sh [aspect|churn|cells|flowscale|seed]`
writes `renders/<case>*.png` via `tools/isf_preview.py` (rows = param sets, columns = seconds @ 60 fps).
Param names below are the ISF `NAME`s Resolume shows (`stir` = "Turbulence", `dt` = "Sim Rate").

## 2026-09-29 — horizontal bands, edge seams

- **Horizontal bands were aspect ratio** (`aspect`). Velocity was applied in UV units, so at 16:9 a
  velocity moved 1.78x more pixels in x than y; the fluid's self-feedback grew that into horizontal
  slabs. Before-fix 16:9 tears horizontally, square tears isotropically; after the fix 16:9 matches
  square. Fix: displacements scale x by H/W (velocity in frame-heights). Commit a303ef8.
- **Edge seams were the host sampler.** ffgl-rs bound pass targets with glium's default Mirror wrap,
  so `fract()`-wrapped reads blended an edge with itself. Now Repeat (ffgl-rs d108d3e); Reflect and
  Absorb clamp to edge texel centres in the shader. The harness wraps with Repeat, so it can't show
  the before case.
- **Host checks that came back clean:** `isf_FragNormCoord` is pixel-centred and `res` is the real
  surface size; `FLOAT` targets are F32, so negative velocities survive.
- **Pressure didn't help creases.** Tried one Jacobi step per frame and divergence damping; neither
  changed the image visibly. The creases are fold-overs from long straight backtraces (they shrink
  with `flowScale`), not compression.
- **Still open:** a horizontal seam through the centre of the spiral appears even in the square,
  correctly-wrapped harness, so it's in the solver.

## 2026-09-30 — churn, cells

- **Churn** (`churn`). 0→1: backtrace in 1–6 substeps. Expected smoother; got more churn — the truer
  self-advection keeps more energy, so the flow folds into more, finer creases. 12 substeps looked
  the same as 6. 1→2: damping 0.985→0.995 and curl ×1→×2, giving wilder shards. `curlStrength` and
  `stir` both at 3 blow up to noise even at churn 0 — an existing limit, not churn's.
- **Cells** (`cells`). Live patch: `stir` 0, `curlStrength` 3, `viscosity` 1, `dt` 1.76.
  - The velocity field is pixel-scale noise at every churn (`cells_velocity`): at 3, curl
    confinement amplifies grid-scale swirl faster than `viscosity` 1 smooths it.
  - With `stir` 0 nothing in the shader seeds motion; with velocity cleared to zero the soup stays
    still. In the live patch the motion was left over from earlier in the session, when `stir` was
    above 0: `curlStrength` 3 keeps an existing field going indefinitely after the forcing stops (I
    first blamed uncleared GPU memory; wrong). The harness stands in for that history with
    `--seed=velA:1`. So the look depends on the patch's history: reloading the patch loses it.
  - Cells are in the dye, not the velocity. Probable mechanism: backtracing in substeps through
    noise pulls neighbouring paths onto the same points, so whole patches share one dye source and
    come out as flat, contiguous cells. More substeps, more merging, bigger cells. Churn 0 (one
    straight hop) gives grain. The mechanism fits the renders but isn't proven.
- **flowScale** (`flowscale`). Live patch, sweeping `flowScale`: 0 smooth, 0.004 small cells, 0.018
  clear cells, 0.05 noisy cells, 0.1+ noise. `flowScale` is the backtrace length. Zero means no
  motion; a few pixels means paths merge into cells; long hops across uncorrelated noise send
  neighbouring pixels to unrelated sources, giving noise. At `churn` 0 there are no cells at any
  `flowScale`, only grain, which supports the path-merging explanation.

- **Seed and Reset** (`seed`). `seed` adds per-pixel, per-frame velocity noise. From a clean start
  the live patch sits still at `seed` 0; any `seed` ≥ 0.02 brings the cells back, and values up to
  0.3 look much the same, since curl 3 amplifies the noise to the same level either way. `reset` (a
  held event) zeroes velocity and refills the dye from the input; the soup snaps back and regrows
  within seconds.
- **ffgl-rs hygiene.** Pass targets are now cleared on create and resize (their storage is
  undefined). The Repeat sampler exposed a leak: glium binds sampler objects to units and never
  unbinds them, and a bound sampler overrides filter/wrap for whatever the host or the next plugin
  samples there. The GL-state restore now unbinds samplers on units 0–15.
- **Transform 3D "offset" went away** after this batch. Transform 3D doesn't use ffgl-isf and reads
  only at exact, clamped texel centres, so neither the sampler change nor the leak can alter its
  own output. It sits in a Varispeed loop with Euler Soup, so the likely cause is Euler Soup's old
  drift and edge seams compounding each lap. Unconfirmed; bypassing Euler Soup would tell.

## Ideas not yet built

- Churn as a cell-size control, perhaps with more substeps once motion is seeded.
- A cell-edge output that draws where neighbouring paths split apart (stained glass, crackle).
