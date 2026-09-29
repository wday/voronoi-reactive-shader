# Transform 3D — Requirements

## Status: v1 in build

## Concept

A sibling to Mirror Transform (`MrTx`) that adds true 3-axis rotation — X and Y
tilt through a perspective projection, plus an aspect-correct Z spin — on top of
the Catmull-Rom resampler that made deep feedback zooms survive.

Resolume's stock Transform has rotate X/Y/Z but samples bilinear, which is the
wrong filter for a transform living inside a feedback loop: at sub-texel per-lap
displacement bilinear sits permanently in its blurriest regime, and the loss
compounds once per lap. This plugin is that feature set on the Catmull-Rom tap.

### Why a new plugin rather than extending Mirror Transform

Resolume caches a plugin's parameter list against its plugin ID. Adding params to
`MrTx` requires a new ID and display name, which renames the effect in every
existing composition. Transform 3D is a strict superset shipped alongside;
`MrTx` and every patch using it are untouched.

Back-compat is exact, not approximate: with Rotate X/Y/Z at 0 the perspective
block is the identity map, and the remaining chain is `MrTx`'s chain verbatim.
Same knob values produce the same frame, bit for bit.

## Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Scale | Standard | 0.5×–2.0× | 0.5 (1.0×) | `2^(v*2-1)`, as MrTx |
| 1 | Perspective | Standard | d = 12.6 → 0.6 | 0.5 (d≈2.1) | Camera distance; low v ≈ orthographic |
| 2 | Rotate X | Standard | ±180° | 0.5 (0°) | Tilt about horizontal axis |
| 3 | Rotate Y | Standard | ±180° | 0.5 (0°) | Tilt about vertical axis |
| 4 | Rotate Z | Standard | ±180° | 0.5 (0°) | **Rigid** spin, aspect-correct |
| 5 | Anamorph Rot | Standard | ±180° | 0.5 (0°) | MrTx's uv-space rotation, kept verbatim |
| 6 | Swirl | Standard | ±2.0 rad | 0.5 (0) | As MrTx |
| 7 | Translate X | Standard | ±1.0 | 0.5 (0) | As MrTx |
| 8 | Translate Y | Standard | ±1.0 | 0.5 (0) | As MrTx |
| 9 | Edges | Option | 4-way | Soft Clip | See below |
| 10 | Fold Tile | Standard | 0.25×–4.0× | 0.5 (1.0×) | Mirror cell size; meaning is per-mode |

### Two Z rotations, on purpose

`Rotate Z` is a rigid rotation in aspect-corrected space and folds into the same
3×3 as X and Y, so X/Y/Z read as one coherent triple the way Resolume's do.

`Anamorph Rot` is MrTx's original rotation, applied in raw uv space. On a 16:9
frame that is a rotation conjugated by an anisotropic scale — rotation, shear and
a breathing scale all at once. It is not a bug and it is not redundant with
Rotate Z; it is a distinct, already-tuned look, so it stays as its own knob.

## Transform pipeline

Inverse map, destination pixel → source texel, in three blocks:

1. **3D block.** `q = (uv - 0.5) * (aspect, 1)`. Cast a ray from a camera at
   `(0,0,-d)` through `(q,0)`, intersect the z=0 plane rotated by
   `R = Ry(θy)·Rx(θx)·Rz(θz)`, express the hit in that plane's own 2-D
   coordinates. `R = I` ⇒ output is `q` exactly.
2. **2D block.** MrTx's chain, verbatim, in uv space: scale → swirl →
   anamorphic rotation → translate.
3. **Edges.** Soft clip, or one of three mirror folds, then the 16-tap
   Catmull-Rom sample with taps clamped to the content region.

`R` is built in the shader from the four scalar angle/distance uniforms rather
than passed as a matrix: the trig is free next to a 16-tap bicubic, and it keeps
the shader drivable from `tools/shader-harness` with scalar sliders and a
`.defaults.json`.

## Edges — the four modes

The fold always acts on a *source-side* coordinate. Folding the destination
coordinate is a no-op, since it is in frame by construction; what distinguishes
the mirror modes is which point in the chain the fold sits at.

| Mode | Fold site | Reading |
|---|---|---|
| Soft Clip | none | Out-of-frame fades to black. MrTx's Mirror=Off. |
| Mirror Plane | after the 2D block | One continuous mirror-tiled plane seen in perspective — a mirrored floor receding to a vanishing point. Swirl and scale run *across* cell boundaries. MrTx's Mirror=On, with perspective upstream. |
| Mirror Tile | between the 3D and 2D blocks | The tilted plane is tiled with complete copies of the transformed frame; swirl restarts inside every cell. "Kaleidoscope, then tilt." |
| Mirror Box | screen space, with parity | Cells smaller than the frame, each odd cell's tilt sign flipped. A chevron/folded-fan rather than a floor. |

Mirror Box's parity flip costs no extra uniform. Reflecting the coordinate on
both sides of the perspective block is conjugation by `diag(±1,±1,1)`, which is
exactly the mirrored rotation.

`Fold Tile` sets the period of whichever fold defines the mode: the source fold
for Mirror Plane, the plane fold for Mirror Tile, the screen fold for Mirror Box.
At 1.0× Mirror Plane is identical to MrTx's mirror.

## Horizon handling

Rotating about X or Y puts part of the plane behind the camera and pushes the
rest toward a horizon.

- **Behind the camera** (ray parameter `t <= 0`, or a degenerate edge-on plane):
  masked to black. Evaluated as a flag and multiplied in at the end, never as an
  early return, so screen-space derivatives stay well-defined.
- **Near the horizon** the source coordinate runs to infinity. With no mipmaps
  and a 16-tap cubic that moirés hard, so magnification fades to black over a
  resolution-derived range in `t`. The fade is analytic in `t` rather than
  derivative-based: `t` stays continuous across mirror-fold seams (the fold is a
  triangle wave) where `dFdx` would spike and draw dark lines along every seam.

## Constraints

- FFGL 2.1 effect, plugin ID `Tx3D`, display name `Transform 3D` (16 bytes, space-padded).
- Windows DLL cross-compiled from WSL; must save/restore host GL state.
- Single pass, stateless, no GPU buffers.
- Every tap clamped at least half a texel inside the content region, so the
  kernel can never pull in black NPOT hardware padding.
- Identity chain must resample bit-exactly — no rescale of `[0,1]` onto the
  outer-texel-centre span. This is the defect that made MrTx's deep zooms blur.
