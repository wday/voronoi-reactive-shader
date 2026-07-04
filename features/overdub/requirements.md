# Delay-Line Overdub for IFS Accumulation

> **Status (2026-07-04): OPEN.** The delay line already ships the Read/Write
> model (commit `a39693f`) with a **decay crossfade** on Write
> (`decay*old + (1-decay)*input`). What is NOT built is this feature's core
> mechanism: **additive** accumulation (`GL_BLEND(ONE,ONE)`, `is_first`
> multi-Write) so several contractive-map Writes sum into one buffer slot per
> subdivision to form IFS attractors. Open design question below: whether that
> additive path lives plugin-side or is done entirely via Resolume blend modes
> on Read. The plan.md steps that assume the old Send/Receive model need
> re-basing onto the shipped Read/Write code before implementing.

## Problem

The delay-line module currently does clean overwrites on Send — `buffer[write_pos] = input`. This prevents accumulation, which is required for iterated function systems where multiple contractive maps must write transformed copies into a shared buffer each beat subdivision.

Without overdub, the buffer has no memory of previous iterations and fractal attractors cannot emerge.

## Goal

Beat-synced IFS iteration: each musical subdivision, N Send instances (each preceded by a spatial transform) read the previous iteration, apply their map, and additively accumulate into the next buffer slot. A decay parameter controls how much of the previous iteration survives.

## Resolume Composition Target

```
Layer 1 (output):   Tap(ch1, sync=1/4)
Layer 2 (map T1):   Tap(ch1) → mirror-transform → Send(ch1, decay=0.95)
Layer 3 (map T2):   Tap(ch1) → channel-displace → Send(ch1, decay=0.95)
Layer 4 (map T3):   Tap(ch1) → mirror-transform → Send(ch1, decay=0.95)
Layer 5 (seed):     Source → Send(ch1, decay=0.0)
```

Resolume renders bottom-to-top. The seed writes clean input, maps T1-T3 blend additively, output Tap displays the accumulated attractor.

## Requirements

1. **Decay parameter** (0.0–1.0, default 0.0): Controls how much of the previous iteration's buffer content survives into the new write slot. 0.0 = clean overwrite (backward compatible). >0 = previous content scaled by decay before additive blend.

2. **Multi-Send frame coordination**: Multiple Send instances on the same channel must advance `write_pos` exactly once per frame. First Send does the fade blit, subsequent Sends blend additively.

3. **Additive blending**: Non-first Sends on a channel within a frame use `GL_BLEND(ONE, ONE)` to accumulate into the buffer.

4. **Backward compatibility**: Existing setups with single Send and decay=0.0 must behave identically to current behavior.

## Constraints

- Only the first Send's decay value applies to the fade blit; subsequent Sends' decay is ignored.
- Fade reads `buffer[read_layer]`, writes `buffer[write_pos]` — different layers, no aliasing.
- Blend enable/disable must be scoped to buffer write pass; disabled before host output.
