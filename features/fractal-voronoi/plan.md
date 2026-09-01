# Fractal Voronoi — plan

Remaining work. Delete steps as they land.

## 1. Confirm registration is clean
Re-run a checkerboard source at Coastline Bias 0 and look for residual overlap.
If any remains, check `g_aspect`: it derives from the GL viewport and would be
wrong if Resolume renders at the padded 1088 height rather than 1080.

## 2. Tune defaults against real footage
Current defaults come from synthetic sources. Density, Depth, Layer Spread,
NC Kernel and Edge Width all want a pass against real stills. In particular the
shipped NC Kernel default should land in the sharp end of its new cell-unit
range, not the middle.

## 3. Validate beat sync live
Confirm each Beat Sync period lands on the beat. Multi-bar periods align to sync
engagement, not to an absolute bar — check whether that reads as wrong in play.

## 4. Measure GPU cost
Depth 6 is ~54 site evaluations plus 2 root traces per pixel. If too slow, cap
Depth rather than cutting the search (±1 is already proven minimal).

## 5. Decide Drift's fate in Fractal mode
FV-LINKSTABLE holds topology still, so drift only wobbles boundaries. Either
justify the knobs or drop them from Fractal mode.
