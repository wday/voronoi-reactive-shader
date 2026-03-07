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
