# Development Log — Video Looper LTM (Dream)

## 2026-03-07 — Feature inception

### Context
The hybrid CPU/GPU approach in `video-looper` works but has inherent stutter
from CPU-side operations (PBO readback, system RAM ring buffer, CPU blur on
cycle wrap). Rather than incrementally fixing, exploring a fundamentally
different architecture: all-GPU temporal pyramid.

### Design: Dream-Tiered Temporal Pyramid (DTTP)
See `requirements.md` for full spec. Key insight: equal VRAM per tier by
trading spatial resolution for temporal depth. Every frame flows through all
tiers via GPU downsample chain — no frame skipping, no CPU involvement.

### Approach
- New plugin `video-looper-ltm-dream` in `plugins/` workspace
- Parallel development with existing `video-looper` (hybrid approach)
- Can A/B test both plugins simultaneously in Resolume
- If DTTP succeeds, it becomes the primary direction; hybrid kept as fallback

## 2026-03-07 — First working build

### Scaffold and initial deploy
- 4 source files: dream.rs, pyramid.rs, shader.rs, params.rs
- Compiled and deployed to Resolume on first attempt
- FPS rock solid — confirmed all-GPU architecture eliminates stutter entirely
- Passthrough worked immediately, but no visible trails

### Fix: composite shader temporal reach
The composite shader was sampling only 1 frame back from each tier (offset 1.0).
At 60fps that's 16ms — visually invisible. Fixed by:
- 4 taps per tier, spread across each tier's depth proportional to trail_length param
- Exponentially weighted (recency bias: 1.0, 0.5, 0.33, 0.25)
- Trail Length knob controls both active tiers (1-4) and temporal reach (fraction of depth)
- At default 0.5: Tier 0 reaches ~0.5s back, Tier 3 reaches ~34s back

### Result
Fluid, beautiful motion trails. No stutter. DTTP architecture validated.
