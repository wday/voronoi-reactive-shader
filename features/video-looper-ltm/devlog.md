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

## 2026-03-08 — Tuning fixes

### Screen blend (diagonal lines fix)
Replaced `max(live, prev)` with screen blend `live + prev - live * prev` in
ingest shader. The per-channel `max()` created hard selection boundaries that
compounded through feedback as visible diagonal lines.

### GL state save/restore (gap fix)
Resolume (or other effects) can leave GL_SCISSOR_TEST, GL_BLEND, GL_DEPTH_TEST
enabled. These leaked into our FBO rendering passes, clipping/corrupting the
stored frames. Now save and restore all three around the ingest/downsample passes.

### Texture wrapping (edge repetition fix)
Changed texture arrays from CLAMP_TO_EDGE to CLAMP_TO_BORDER. Edge pixels were
being repeated when rotation/swirl mapped UVs outside [0,1]. Also softened the
inBounds check from hard `step()` to `smoothstep()` so edges fade over a few
pixels instead of creating hard lines that compound through feedback.

## 2026-03-08 — Musical tap model redesign

### Problem with original composite
The multi-tap-per-tier composite with trail_length/trail_opacity/tier_weights was
not musically meaningful. Taps were spread across raw frame depth, not synced to
tempo. Parameters were overloaded (trail_length controlled both tier count and
temporal depth).

### New model: one tap per tier at doubling delays
Each tier provides one echo at a musically-timed offset:
- Tap 1 (T0, full res): 1× subdivision — sharp echo
- Tap 2 (T1, half res): 2× subdivision — soft echo
- Tap 3 (T2, quarter res): 4× subdivision — dreamy echo
- Tap 4 (T3, eighth res): 8× subdivision — deep memory

Resolution loss at longer delays IS the aesthetic. This plays to the pyramid's
natural strengths instead of fighting them.

### Parameter changes
Removed: Trail Length, Trail Opacity, Weight T0-T3 (6 params)
Added: Dry, Wet, Tap 1-4 (6 params, same total count)
- Dry/Wet: independent level controls for live and echo mix
- Tap 1-4: per-tier echo levels, 0-2 range (overdrivable), decaying defaults
- Composite shader reduced to 4 single-tap samples + weighted sum
