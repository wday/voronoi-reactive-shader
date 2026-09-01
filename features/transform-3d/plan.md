# Transform 3D — Plan

Remaining work only.

## Live validation in Resolume

The DLL is built and deployed; nothing below has been seen on real hardware.

1. Confirm `Transform 3D` appears in the effect browser with all 11 params and
   that Edges shows four options. A stale `MrTx` cache cannot affect it — the ID
   is new — but Resolume must be restarted to pick up a newly dropped DLL.
2. Sanity-check back-compat by ear: park Transform 3D and Mirror Transform on the
   same clip with matching Scale / Swirl / Anamorph Rot / Translate and all three
   rotations at centre. They should be indistinguishable. Headless says
   bit-exact; this is only guarding against a param-index or range slip.
3. Drive Rotate X/Y under feedback and confirm the tunnel depth advantage over
   Resolume's stock Transform still holds with the perspective block in circuit.

## Tuning passes that need eyes

4. **Horizon fade constant.** `lim = sqrt(content_h / 32)` — fade over
   `t ∈ [lim, 3·lim]`. Verified to suppress the aliasing band, but the aesthetic
   choice of how hard to cut is untested at 1080p and in a feedback loop, where a
   soft black band may read better or worse than a hard one.
5. **Perspective curve.** `d = 0.6 + 12·(1-v)³`. Check the knob's useful range
   isn't bunched at one end of the fader.
6. **Mirror Box needs Fold Tile below 1.0 to show anything** — at 1.0× the screen
   cell is exactly the frame, so there are no walls in view and the mode collapses
   onto Mirror Plane. If that reads as a broken knob in practice, remap Fold Tile
   for mode 3 so the default lands at ~0.4×.

## Deferred

7. The Catmull-Rom sampler now exists in three copies (mirror-transform,
   transform-3d, and the other resamplers). Factor into `pluglib` if a fourth
   consumer shows up; not worth the churn for two.
