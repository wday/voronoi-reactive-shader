# Development Log — Video Looper

## 2026-03-07 — Feature inception

### Context
Arose from speculation about buffering input frames for loop playback. Initial thought was GPU VRAM, but memory limits make it impractical beyond ~4 seconds at 1080p. System RAM approach removes the storage constraint — 30+ seconds easily, minutes if needed.

### Architecture decision
- System RAM ring buffer with PBO async transfer (no GPU stalls)
- Custom FFGL Rust plugin, not ISF (needs GL resource management)
- Based on `ffgl-rs/example-raw` scaffold

### Open questions at inception
- Decay model, trigger model, standalone vs integrated, output blending

## 2026-03-07 — Controls and degradation model

### Decisions
- **Degradation**: single `quality` parameter (1.0 pristine → 0.0 aggressive). Destructive on write — buffer itself degrades per cycle like tape wearing out. Effects: resolution loss, color quantization, spatial drift, temporal bleed.
- **Beat quantization**: loop duration in beats (1/2/4/8/16/32), derived from Resolume host BPM. Essential for performance.
- **Standalone plugin**: composes with Resolume's effect chain, transforms, masking. Can chain Voronoi after looper. More flexible than integration.
- **Accumulation/trails**: dropped as a concept — it's just trails, Resolume already does this. Tape degradation is the novel thing.
- **Output**: `dryWet` crossfade between live input and loop playback.

### Remaining open questions
- Overdub mode and mix parameter
- Memory management: pre-allocate vs dynamic, resolution control
- State behavior when loopBeats changes mid-playback

## 2026-03-07 — v1 scope locked, plan written

### Decisions
- Memory: allocate on first frame, flush + reallocate on resolution change
- Overdub: deferred to v2
- loopBeats change during playback: truncate to new length
- Degradation v1: simple CPU-side box blur per cycle, gated by quality param

### Research
- Studied `ffgl-rs` plugin architecture: `SimpleFFGLInstance` trait
  - `draw()` receives `FFGLData` (host_beat with bpm/barPhase) and `GLInput` (input textures)
  - Parameters via `get_param`/`set_param` with `ParamInfo` trait
  - `plugin_main!` macro for entry point
- Input textures provide Handle/Width/Height — resolution detection is trivial
- `example-raw` provides scaffold template

### Architecture
- 6 source files: lib, looper, ring_buffer, pbo, params, shader
- State machine: Idle → Recording → Playing
- PBO double-buffering for async GPU↔RAM transfer
- Beat-quantized playback via host_beat.barPhase

### Infrastructure
- Plugin registry (`plugins.json`) and multi-plugin Makefile created
- `scripts/build-plugin.sh` handles both ISF and Rust plugin types
- GitHub Actions workflows for CI (build.yml) and releases (release.yml)

### Next
- Implement in fresh context — start with crate setup and ring buffer
