# Transform 3D — Development State

## Built

Crate `plugins/transform-3d`, plugin ID `Tx3D`, display name `Transform 3D`.
Registered in `plugins.json` and the `plugins/` workspace. Windows DLL builds
clean (no warnings from this crate) and is deployed to
`Documents/Resolume Avenue/Extra Effects/`.

11 params: Scale, Perspective, Rotate X, Rotate Y, Rotate Z, Anamorph Rot,
Swirl, Translate X, Translate Y, Edges (4-way option), Fold Tile.

Mirror Transform (`MrTx`) is untouched and still ships alongside.

## Verified, and how

Headless moderngl (EGL/llvmpipe under WSLg), fragment shader driven directly
with scalar uniforms. All assertions pass.

- **Identity is bit-exact** — max channel difference 0 against the source, in
  *all four* Edges modes, with rotations centred. This is the property that
  matters most: it is what stops the plugin from bleeding detail per lap inside a
  feedback loop, and it is why the perspective block can be left permanently in
  circuit.
- **Rigid Rotate Z is a true rotation** — `rot_z = +90°` on a square frame
  reproduces `np.rot90(k=+1)` with mean error 0.00.
- **Anamorph Rot is a distinct knob** — mean difference 111.9/255 against rigid
  Rotate Z at the same angle on 16:9. It is not a redundant duplicate.
- **Horizon** — at 85° tilt with d=2.1 the analytic horizon is image row 114.
  Above it: luma 0.0 (masked). Rows 116–122: luma 2.1 (faded). Rows 154–174:
  luma 111.9 (full). Sky is masked and the aliasing band is suppressed.
- **All four modes are visually distinct and artifact-free** under combined
  X/Y/Z + swirl + scale; no NaN frames, no all-black frames across a
  mode × tilt sweep.
- **Gutter masking** — Fold Tile above 1.0× leaves clean black gutters between
  mirrored copies rather than streaking the edge texel across them.

## Invariants that must not be broken

- **The identity chain must resample bit-exactly.** No rescaling of `[0,1]` onto
  the outer-texel-centre span — that squeeze is what made Mirror Transform's deep
  zooms blur, costing ~44% of high-frequency energy per pass. Clamp, never
  rescale.
- **`R = I` must yield the exact identity map.** The perspective block is always
  in circuit; if it perturbs the coordinate at zero rotation, every patch pays
  per lap. The focal length is pinned to the camera distance precisely so that
  the untilted plane projects 1:1 for any Perspective value.
- **Every Catmull-Rom tap stays ≥ half a texel inside the content region.** The
  kernel reaches 2 texels past the sample point; unclamped it pulls in black NPOT
  hardware padding as a 1px seam.
- **The horizon fade stays analytic in `t`, never derivative-based.** `t` is
  continuous across mirror-fold seams because the fold is a triangle wave;
  `dFdx` spikes there and would draw a dark line along every seam.
- **No early return before the horizon mask.** Validity is carried as a flag and
  multiplied in at the end so screen-space derivatives stay well-defined for
  every fragment. Branching on `u_edges` is safe — it is uniform across the draw.
- **Aspect comes from content size, not hardware size.** NPOT padding must not
  tilt the perspective.

## Unverified

Everything requiring Resolume or a GPU driver rather than llvmpipe: the effect
browser listing, param ordering and ranges as the host presents them, behaviour
in a live feedback loop, and every aesthetic judgement — the horizon fade
constant, the Perspective curve, and whether Mirror Box's dependence on Fold Tile
< 1.0 reads as a dead knob. See `plan.md`.
