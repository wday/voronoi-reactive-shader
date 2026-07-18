# Wavefolder — Requirements

## Goal

A stateless multi-stage **wavefolder** atom that reflects/wraps pixel intensity
back into range instead of clipping. Primary job: **tame blowout** in
uncontrollable live environments (camera feedback in ambient light, hot
projectors), where a real feedback loop can't be kept in range. Instead of
white-clipping, over-range values *fold* back down — which is also where the
fun **fractal / harmonic contouring** comes from for free.

## Motivation

In the box you can keep a feedback loop's gain under unity. Outside the box
(camera → projector → camera in a room with ambient light) you can't — the loop
blows out to white. A wavefolder is the classic analog answer: a bounded
nonlinearity that folds excess signal back rather than saturating. Cascading
several folds turns a smooth over-bright ramp into self-similar contour bands —
harmonically rich, controllably chaotic.

## Design decisions (locked 2026-07-12)

| Decision | Choice | Rationale |
|---|---|---|
| **Form factor** | ISF shader first | Prototype look fast in `shader-harness` (explore/gallery/evolve); port to FFGL later if it earns a live slot. Stateless → trivial FFGL port. |
| **Fold domain** | Per-channel RGB | Blown highlights fold into *shifting color fringes* — the most "fractal implication". Hue not preserved (intentional). |
| **Fold curve** | Selectable (param) | Morph **Triangle → Sine → Wrap** on one knob for live exploration. |
| **Stage control** | Iteration knob | Single fold applied N times in a loop; `Drive` re-injects gain each pass. Cheap, uniform, smoothly automatable (fractional N). |

## Behavior

- Signal path per stage: `x ← fold(x · Drive + Bias)`, repeated `Stages` times.
- `fold()` maps `[0, ∞) → [0, 1]` by reflection/wrapping. Below the fold point
  (Drive·x ≤ 1) triangle mode is ~identity, so quiet regions pass clean and only
  highlights fold — exactly the blowout-control behavior.
- Per-channel: R/G/B fold independently, so neutral blowout breaks into color.
- Dry/Wet so it can be dialed in live.

## Fold curves

- **Triangle** — `1 − |mod(x,2) − 1|`. Hard reflect, sharp contour lines. Classic wavefolder. Identity on [0,1].
- **Sine** — `½ − ½·cos(π·x)`. Smooth Buchla-style fold, rounded bands, soft contrast even below the fold point.
- **Wrap** — `fract(x)`. Hard modulo, discontinuous, most aggressive/glitchy, strongest self-similarity under cascade.

## Parameters

| Name | Label | Type | Min | Max | Default | Notes |
|---|---|---|---|---|---|---|
| `inputImage` | Input | image | — | — | — | Fx input |
| `drive` | Drive | float | 1.0 | 8.0 | 1.8 | Pre-fold gain; higher = more folding, lower effective blowout ceiling |
| `stages` | Stages | float | 1.0 | 8.0 | 2.0 | Fold iterations (fractional = smooth blend of last stage) |
| `shape` | Shape | float | 0.0 | 1.0 | 0.0 | 0 Triangle · 0.5 Sine · 1 Wrap |
| `bias` | Bias | float | −0.5 | 0.5 | 0.0 | Offset before fold → asymmetric folds / DC shift |
| `wet` | Mix | float | 0.0 | 1.0 | 1.0 | Dry/Wet (can't be named `mix` — GLSL builtin) |

## Non-goals (this atom)

- No feedback buffer / persistent state (stateless by design; that's what makes
  it safe to drop anywhere in a stack).
- No luminance-preserving mode (deferred; per-channel is the chosen character).
- No FFGL packaging yet (Phase 2 if it earns a live slot).

## Acceptance

- Loads in `shader-harness` and folds a test gradient / bright source without clipping.
- Drive=1.8, Stages=2, Shape=0 on a blown-white region produces contoured color
  bands rather than flat white.
- Wet=0 is bit-identical passthrough.
