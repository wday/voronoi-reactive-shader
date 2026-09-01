# Fractal Voronoi — plan

Remaining work. Delete steps as they land.

## 1. Live validation (user, in Resolume)
- Confirm `barPhase` sync lands on the beat at each period. Multi-bar periods
  align to sync engagement, not to an absolute bar — check whether that reads
  as wrong in practice.
- Judge Coastline Bias against real footage: it deforms the partition strongly
  but is not a watershed. Decide whether it earns its knob.
- Judge whether Drift is worth keeping in Fractal mode given FV-LINKSTABLE
  holds the topology still.

## 2. Tune defaults against real footage
Current defaults are set from synthetic sources only. Density, Depth, Layer Mix
and Edge Width all want a pass against a real still through the harness.

## 3. Measure GPU cost
Depth 6 is ~54 site evaluations plus 2 root traces per pixel. Never measured on
target hardware; if it is too slow, cap Depth rather than cutting the search.
