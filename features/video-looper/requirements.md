# Video Looper (FFGL Plugin)

## Status: v1 scope locked

## Concept
A frame-level video looper implemented as a standalone FFGL plugin for Resolume. Captures input frames to a system RAM ring buffer with beat-quantized timing and controllable tape-style degradation. Designed to compose with other effects (including the Voronoi shader) via Resolume's effect chain, transforms, and masking.

## Architecture

### System RAM ring buffer
- Frames stored in system RAM, not GPU VRAM
- Ring buffer: `N = fps × max_loop_duration` frames
- Each frame: raw RGBA pixels (~8MB at 1080p)
- Memory budget examples (30fps):
  - 4 sec → ~960MB
  - 30 sec → ~7.2GB
  - 2 min → ~28.8GB
- Optional (future): LZ4 compression (~2:1 ratio, ~4 GB/s throughput) to extend duration

### Memory allocation
- **Allocate on first frame** — read resolution from input texture, allocate buffer
- **Resolution change** — detect via input texture dimensions each frame. On change: flush buffer, reallocate. Current loop is lost (acceptable — resolution changes are rare during performance)
- Max loop duration: 30 seconds (configurable at compile time for v1)

### GPU ↔ RAM transfer
- **Capture** (GPU → RAM): PBO double-buffering to avoid pipeline stalls
  - Frame N: `glReadPixels` into PBO A (async DMA)
  - Frame N+1: map PBO A for CPU read while capturing into PBO B
- **Playback** (RAM → GPU): PBO upload, same double-buffer pattern
  - `glTexSubImage2D` from mapped PBO, no stall
- PCI-e 3.0 x16 bandwidth: ~16 GB/s, single frame transfer < 1ms

### Plugin type
- Custom FFGL Rust plugin (not ISF — needs GL resource management)
- Based on `ffgl-rs/example-raw` scaffold
- Standalone in Resolume's effect chain — composable with transforms, masking, other effects
- Receives input texture from Resolume, outputs processed texture

## Degradation Model — "Tape Quality"

Inspired by audio tape loop degradation. A single `quality` parameter controls how much the loop degrades on each cycle. Degradation is **destructive on write** — the buffer itself degrades, so each play-through sounds/looks different, like tape wearing out.

### Quality parameter
- `quality = 1.0`: pristine, lossless re-recording
- `quality = 0.0`: aggressive degradation per cycle

### Degradation effects (applied per cycle on write-back)
Intensity scales with `(1.0 - quality)`:
- **Resolution loss**: downsample → upsample (progressively blurrier)
- **Color quantization**: reduce bit depth (posterization creep)
- **Spatial drift**: subtle per-frame offset/wobble (tape transport instability)
- **Bleed**: slight mix with temporally adjacent frames (head smear)

### Behavior over time
- At `quality = 1.0`: loop stays clean indefinitely
- At `quality = 0.5`: loop gradually softens and shifts over many cycles
- At `quality = 0.0`: loop rapidly decays into abstract color/texture within a few cycles
- The degradation accumulates — there is no "reset" short of re-recording

## Controls

### Timing
| Parameter | Description | Range | Default |
|-----------|-------------|-------|---------|
| `loopBeats` | Loop length in beats (quantized) | 1, 2, 4, 8, 16, 32 | 4 |
| `record` | Toggle recording | bool | true |
| `playback` | Toggle playback | bool | false |

Loop duration derived from Resolume's host BPM: `duration = loopBeats × (60 / BPM)`

### Playback
| Parameter | Description | Range | Default |
|-----------|-------------|-------|---------|
| `playbackSpeed` | Rate multiplier | -2.0–2.0 | 1.0 |
| `playbackMode` | Forward / Reverse / Ping-pong | enum | Forward |
| `scrub` | Manual position (overrides auto) | 0.0–1.0 | — |

### Degradation
| Parameter | Description | Range | Default |
|-----------|-------------|-------|---------|
| `quality` | Tape quality — 1.0 pristine, 0.0 aggressive decay | 0.0–1.0 | 1.0 |

### Output
| Parameter | Description | Range | Default |
|-----------|-------------|-------|---------|
| `dryWet` | Crossfade between live input and loop | 0.0–1.0 | 1.0 |

- Pass-through when not looping (`dryWet` irrelevant, input = output)
- During playback: `dryWet = 0.0` = live input, `1.0` = loop only

## v1 scope decisions
- **Overdub**: deferred to v2
- **Memory**: allocate on first frame, flush on resolution change
- **loopBeats change during playback**: truncate to new length (mod existing write index)
- **Visual feedback**: deferred to v2

## Future (v2+)
- Overdub mode with mix parameter
- LZ4 compression for extended duration
- Half-resolution recording option
- Visual feedback for loop position
- Beat-phase alignment (start recording on next beat)

## Dependencies
- `ffgl-rs` framework (Rust FFGL plugin scaffold)
- FFGL host info (BPM from Resolume)
- No ISF dependency — pure Rust + OpenGL

## References
- PBO streaming: [OpenGL wiki — Pixel Buffer Object](https://www.khronos.org/opengl/wiki/Pixel_Buffer_Object)
- LZ4: [lz4-rs crate](https://crates.io/crates/lz4)
- FFGL host info: BPM, beat phase available via `FF_GetInfo` / `ProcessOpenGL` host struct
