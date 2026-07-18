# Stateless Fluid — implementation plan

## Step 1 — Write the ISF `shaders/flow_undertow.fs`
- Single pass, no PERSISTENT/FLOAT. Inputs per requirements param table.
- Helpers: `luma()`, `sobelGrad()` (∇luma via 3×3), `rot()` (2×2 rotation),
  `curlNoise()` (curl of value-noise, `TIME`-driven), `applyBoundary()`
  (copy convention from `flow_euler.fs`), `blur3x3()`.
- Body: velocity = rot(∇luma, angle)*velScale + ambient curl-noise; semi-Lagrangian
  fetch `inputImage` at `uv - v*dt`; mix in blur by `diffusion`; `*= persistence`.
- DESCRIPTION string: explain it's a stateless in-loop advector (bump on every edit
  — `ISF_NAME` env!() dep gotcha from flow_euler devlog).

## Step 2 — Static validation
- JSON parse + bracket balance (reuse whatever check flow_euler used).
- Confirm exactly one pass, single `inputImage`, sampler name matches.
- Optional cheap sanity: adapt into `tools/shader-harness` feedback loop to eyeball
  currents before the slow Resolume round-trip (harness uses `u_input`/`u_feedback`,
  ISF uses `inputImage`/`IMG_NORM_PIXEL` — small shim). Skip if not worth it.

## Step 3 — Register + build
- Add to `plugins.json`: `{ name: flow_undertow, display_name: "Undertow",
  type: isf, shader: shaders/flow_undertow.fs, dll: flow_undertow.dll }`.
  Verify `display_name[0..3]` = `Und` is unique vs `*Flo`/`*Eul`.
- `make build PLUGIN=flow_undertow` (Windows cargo from WSL, per build-notes).
- Grep the built DLL for a unique ISF token ("Undertow"/"Ambient Drift") to confirm
  the shader actually baked in (env!() rerun trap).

## Step 4 — Deploy + live validation
- Close Resolume (DLL lock), `make deploy PLUGIN=flow_undertow`.
- In Resolume: build `source → feedback → Undertow` loop over a moving source.
  Check: currents form; `Swirl` toggles vortex vs drain; `Ambient Drift` keeps
  empty areas moving; no blow-out (tune `persistence`/host feedback opacity);
  `Boundary` behaves.

## Step 5 — Tune defaults + devlog
- Lock in default param values from live feel. Record decisions in devlog.
- Note host-feedback-opacity vs in-fx `persistence` interaction findings.

## Sequencing / risk
- Steps 1–2 are safe now (WSL, no Resolume). Steps 3–4 gated on Resolume being
  closed. Do 1–2, hand off 3–4 when the user can free the DLL.
- Biggest unknown: whether pure self-derived velocity (no momentum) reads as
  "fluid" or "melt". Mitigation: the `Swirl` rotation + `Ambient Drift` curl-noise
  are specifically there to add divergence-free circulation the plain gradient
  lacks. If it still melts, next lever is a light owned 1-frame buffer (breaks
  stateless — would be a deliberate scope change, ask first).
