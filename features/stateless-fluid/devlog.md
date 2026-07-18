# Stateless Fluid — devlog

## 2026-07-10 — feature framed, artifacts created

Started from a conversation about generating fluid dynamics as **insert fx inside a
Resolume feedback loop** (`source → tap → fx → fx → write`), with the input source
acting as the forcing function.

Key decisions (from user):
- **Fresh insert-fx**, not an extension of `flow_euler`. The flow family is all
  owned-buffer; this fills the stateless-in-host-loop gap.
- **Stateless velocity** — reconstructed from the loop image's own luminance
  gradient each frame; the visible loop image is the only memory.
- **Surface: ISF in Resolume** (test live), not the harness.

Design insight recorded in requirements: "source is the forcing function" holds
*implicitly* — source is blended into the loop upstream, so its injected density
shows up as a gradient → velocity → force, unified with self-advection from old
density. Occupies the `f` + `-(v·∇)v` terms of Navier-Stokes; projection skipped
(rotated-gradient/curl velocity is ~divergence-free), viscosity=blur, damping=decay.

Named **"Undertow"** (plugin id `*Und`, unique vs `*Flo` Flow Inject / `*Eul` Euler
Soup — the `'*'+display_name[0..3]` gotcha).

Next: Step 1 — write `shaders/flow_undertow.fs` (single-pass ISF).

## 2026-07-10 (later) — Steps 1–2 done: shader written + validated headlessly

- Wrote `shaders/flow_undertow.fs` (single-pass, no PERSISTENT buffers, reads only
  `inputImage`). Velocity = `rot(∇luma, Swirl) * Current` + divergence-free curl-noise
  Ambient Drift (`TIME`-driven, stateless); semi-Lagrangian advect of the same image;
  3×3 blur mixed by Viscosity; `*= Persistence` (default 1.0 — host loop already decays).
  Advection displacement scaled to ~40px at full Current for a live-tunable range.
- Registered in `plugins.json` as `flow_undertow` / display **"Undertow"** / id `*Und`
  (verified unique vs `*Flo`, `*Eul`, `*Vor`, `*Ste`).
- **Static validation**: ISF JSON parses, single-pass, braces/parens balanced, every
  uniform declared+used.
- **Headless GLSL compile+link**: OK on llvmpipe (moderngl, ISF macros stubbed).
- **Headless feedback-loop sim** (256², 90 laps, moving blob source injected each lap
  to mimic `source → loop`): all frames finite, values bounded [0, 0.67] (no blow-up),
  steady frame-to-frame delta ~0.0044 (transporting, not static/exploding). Saved PNGs
  show the source advected into a coherent curving trail — mechanism confirmed.
- Observation: with a single dominant blob it reads "comet-smear" more than "vortex"
  (expected — stateless = no momentum). Swirl + Ambient Drift + Current are the levers
  toward eddies; final feel is a live-tuning job.

## 2026-07-10 (later still) — Step 3 done: built + deployed

- `make build PLUGIN=flow_undertow` → clean (only vendored ffgl-core warnings), 13.9s.
- Confirmed ISF baked into DLL: `Undertow` ×3, `Ambient Drift` ×2, `velScale` ×2,
  description string present (env!() rerun trap avoided — new plugin, fresh embed).
- `make deploy PLUGIN=flow_undertow` → copied to
  `Documents/Resolume Avenue/Extra Effects/`.

Next: Step 4 — live validation in Resolume. Open Resolume, build a feedback loop over a
moving source, drop **Undertow** in, tune Current / Swirl / Ambient Drift. Watch for
melt-vs-swirl; if it melts, Ambient Drift is the first lever, then the deferred 1-frame
velocity buffer (scope change — confirm first).
