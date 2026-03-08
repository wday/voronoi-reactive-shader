# Delay Line Module — Requirements

## Status: v1 scope locked

## Concept

A send/receive pair FFGL plugin that creates feedback delay loops within Resolume's effect chain, without requiring composition-level Feedback Source or Arena routing. Native Resolume effects (transform, blur, hue shift) placed between Receive and Send compound through the feedback loop automatically.

### Why send/receive?

Resolume's built-in Feedback Source only taps the composition output. This means:
- Other layers bleed into the feedback path
- No per-layer or per-group isolation
- Video Router can't self-reference within a frame (no temporal buffer)

The send/receive design closes the feedback loop through a **shared GPU buffer**, decoupled from Resolume's routing. This works in Avenue, no Arena needed.

### Resolume signal flow

```
Layer: "Spiral Echo"
┌─────────────────────────────────────────────┐
│                                             │
│  Source: Webcam                              │
│  Effects chain (top to bottom):              │
│    1. Delay Line [Receive, ch 1]            │
│       → mixes in delayed buffer at feedback │
│    2. Transform (rotate 137.5°, scale 0.62) │
│    3. Blur (subtle)                          │
│    4. Delay Line [Send, ch 1]               │
│       → writes result to buffer, passes thru│
│                                             │
└─────────────────────────────────────────────┘
```

Per-frame signal flow:
1. **Receive** reads buffer[channel] from D frames ago, mixes with live input at feedback amount
2. Native Resolume effects transform the mixed signal (rotation, blur, etc.)
3. **Send** writes the transformed result to buffer[channel], outputs unchanged

Each echo compounds the transforms. The spiral/zoom/drift emerges naturally.

### Multi-tap / tiered delay

Duplicate the layer with different channel + subdivision settings. Resolume's layer mixer handles blending. No multi-tap logic needed in the plugin.

## Plugin design

**Single DLL, mode parameter.** All instances within Resolume share the same global buffer registry (same DLL = same statics). A Mode parameter switches between Send and Receive behavior.

### Shared buffer registry

Global static keyed by channel (1–4). Each channel holds:
- `GL_TEXTURE_2D_ARRAY` — GPU ring buffer (900 layers)
- Write pointer — advanced by Send each frame
- Resolution — set by Send, Receive adapts via GL bilinear scaling

Allocated on first Send write. Persists until last reference drops.

## Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Mode | Option | Send, Receive | Receive | Switches plugin behavior |
| 1 | Channel | Option | 1, 2, 3, 4 | 1 | Pairs Send with Receive |
| 2 | Subdivision | Option | 1/16, 1/8, 1/4, 1/2, 1 bar, 2 bars, 4 bars | 1/4 | Receive only: delay length |
| 3 | Feedback | Standard | 0.0–1.0 | 0.5 | Receive only: echo intensity |

- **BPM**: from host beat info
- **FPS**: measured via EMA of frame deltas (Receive only)

## Send behavior

1. Get/create shared buffer for channel
2. If resolution changed, reallocate texture array
3. Render input texture → buffer[write_pos] (FBO write)
4. Render input texture → host FBO (passthrough)
5. Advance write_pos

Send is transparent — it outputs exactly what it receives. The buffer write is a side effect.

## Receive behavior

1. Look up shared buffer for channel (if no Send yet, pass through)
2. Compute D = delay in frames from BPM + subdivision + fps_estimate
3. Read buffer[(write_pos - D) % depth]
4. Output: input + feedback × delayed_frame (additive mix, clamped)

## Memory / GPU

- **Storage**: GPU texture array per channel (GL_TEXTURE_2D_ARRAY)
- **Max buffer**: 900 frames per channel (15s at 60fps)
- **VRAM per channel**: ~7.2GB max at 1080p, ~240MB typical (1/4 note @ 120bpm)
- **Max channels**: 4 (max 4 independent feedback loops)
- **Allocation**: lazy on first Send, reallocate on resolution change

## Constraints

- FFGL 2.1 effect plugin
- Windows DLL target (cross-compiled from WSL)
- Must save/restore host GL state (FBO, viewport, blend, scissor, depth)
- Single-threaded GL: all instances called from Resolume's render thread

## Out of scope (v1)

- Tap tempo / manual BPM override
- Ping-pong or reverse playback
- Frame interpolation for non-integer delay lengths
- Freeze / hold
- MIDI output
- Any visual processing (blur, color, spatial transform)
- More than 4 channels
