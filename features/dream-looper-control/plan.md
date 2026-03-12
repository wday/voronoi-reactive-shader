# Implementation Plan — Dream Looper Control (M4L)

## Phase 1: Minimal control surface
1. Determine Resolume's OSC address scheme for FFGL plugin params
2. Generate M4L patch with live.dial controls for core params (dry, wet, taps, feedback, subdivision)
3. Wire dials → OSC messages → udpsend
4. Test round-trip: Ableton dial → Resolume param change → visual result
5. Enable Ableton Link in both apps, verify tempo sync

## Phase 2: Spatial controls + automation
1. Add rotation, scale, swirl, hue shift controls
2. Verify all controls are automatable in Ableton's arrangement view
3. Test preset save/recall

## Phase 3: Gen~ audio analysis
1. Build envelope follower in Gen~ codebox
2. Build transient detector in Gen~ codebox
3. Wire analysis outputs to modulation amounts (scalable per-target)
4. Add modulation depth knobs (how much analysis affects each param)
5. Test with live audio material

## Phase 4: Polish
1. UI layout refinement
2. Parameter grouping / tabs if needed
3. Documentation / preset library
4. Consider RNBO export for standalone use

## Not yet planned
- Bidirectional OSC (Resolume → Max)
- Spectral analysis (FFT-based)
- Multi-layer control (controlling multiple Resolume layers)
