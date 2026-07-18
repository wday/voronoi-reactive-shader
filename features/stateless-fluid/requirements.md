# Stateless Fluid — insert-fx advection inside the host feedback loop

## One-line
A single-pass ISF effect that turns *any* Resolume feedback loop into fluid, by
reconstructing a velocity field from the loop image each frame and advecting that
same image along it. No owned buffers — the loop's visible image is the only state.

## Context / how it differs from the existing flow family
The flow family is all **owned-buffer**:
- `flow_inject.fs` — reads a source, writes a flow-field texture (RG=velocity from
  gradient, B=scalar, A=density) for a downstream solver to consume.
- `flow_euler.fs` ("Euler Soup") — owned ping-pong velocity+dye stable-fluid sim
  (velA/velB/dyeA/dyeB, 5 passes, persistent float buffers).

This effect is the **stateless insert-fx** regime instead:
- Placed *inside the host's own feedback loop*: `source → tap → [this fx] → write`.
- Sees ONE input image (the tapped loop content). No PERSISTENT passes, no owned
  velocity/dye buffers.
- Velocity is **recomputed every frame** from the loop image's own luminance
  gradient. The image *is* the memory (density persists in the loop; velocity is
  derivative, re-derived each frame).

## The forcing model (why "source is the forcing function" still holds)
The effect only sees one combined image, but the **source is blended into the loop
upstream** (the `source →` at the head of the chain). So by the time this fx runs,
freshly-injected source content is already present in the image. The fx derives
velocity from the gradient of the *combined* image, so:
- Wherever the source injects new density → a gradient → velocity → **forcing** `f`.
- Wherever old feedback density sits → its gradient → **self-advection** `-(v·∇)v`.

Both couplings fall out of one gradient computation. Physically this occupies the
`f` (external force) + `-(v·∇)v` (self-advect) terms of Navier-Stokes; viscosity =
blur, damping = decay. Incompressibility/projection is **skipped** (approximated by
using a rotated-gradient / curl velocity that is divergence-free-ish, plus blur).

## Requirements

### R1 — Single-pass, stateless, single-input ISF
- No `PASSES` with `PERSISTENT`/`FLOAT`. Reads `inputImage` only.
- Deployable as a normal ISF FFGL plugin, droppable anywhere in a host feedback loop.

### R2 — Velocity from feedback density
- Compute `∇luma(inputImage)` via a small (Sobel/3×3) kernel.
- `v = rotate(∇luma, rotAngle) * velScale`.
  - `rotAngle ≈ 90°` → flow circulates *around* bright blobs (vortex/swirl look).
  - `rotAngle ≈ 0°` → flow runs along the gradient (sharpen/drain look).

### R3 — Semi-Lagrangian advection of the same image
- `c = texture(inputImage, uv - v * dt)`.

### R4 — Ambient drift (optional, keeps empty regions alive)
- Add a curl-noise term driven by `TIME` (stateless): `v += curlNoise(uv*scale +
  TIME*drift) * ambient`. `TIME` is allowed (not owned buffer state).

### R5 — Viscosity + dissipation
- Viscosity: small blur mixed in by `diffusion`.
- Dissipation: `c *= persistence` to stop the loop saturating. Default near 1.0 —
  the **host loop's own feedback opacity already decays**, so this must not
  double-decay by default. Document the interaction.

### R6 — Boundary handling
- Wrap / Reflect / Absorb, matching `flow_euler`'s `applyBoundary` convention.

### R7 — Plugin identity
- `display_name` first 3 chars must be unique vs `*Flo` (Flow Inject) and `*Eul`
  (Euler Soup). Proposed: **"Undertow"** → `*Und`. (Alternatives: Drift/Riptide/Eddy.)

## Parameters (draft)
| Param | Label | Default | Range | Role |
|---|---|---|---|---|
| velScale | Current | 0.3 | 0–2 | velocity magnitude (forcing gain) |
| rotAngle | Swirl | 0.75 | 0–1 (→0–360°) | gradient rotation; 0.25≈swirl |
| dt | Flow Rate | 0.5 | 0.01–2 | advection step |
| ambient | Ambient Drift | 0.1 | 0–1 | curl-noise floor |
| noiseScale | Drift Scale | 3.0 | 0.5–10 | curl-noise spatial freq |
| diffusion | Viscosity | 0.1 | 0–1 | blur mix |
| persistence | Persistence | 1.0 | 0.9–1.0 | in-fx dissipation (default off) |
| boundaryMode | Boundary | 0 | Wrap/Reflect/Absorb | edge behaviour |

## Non-goals / deferred
- **Owned velocity buffers / true momentum** — that's `flow_euler`'s job.
- **Separate source input (2-input ISF)** — cleaner explicit forcing, but Resolume's
  multi-image-input wiring is uncertain; deferred. Implicit-via-density is the v1.
- **Frame-difference ("touch") forcing** — needs the previous source frame = owned
  state; violates stateless. Deferred (would be a 1-frame-buffer variant).
- **Velocity packed into loop channels** — corrupts the visible RGB; deferred.

## Validation
- Static: ISF JSON parses, bracket balance, sampler names, single-pass.
- Live (chosen surface): build + deploy to Resolume, drop into a feedback loop over
  a moving source, confirm currents form, swirl knob changes vortex behaviour, and
  it doesn't blow out or freeze. (Deploy blocked while Resolume holds the DLL —
  close Resolume first, per flow_euler devlog.)
