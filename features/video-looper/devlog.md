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
- ~~Implement in fresh context — start with crate setup and ring buffer~~

## 2026-03-07 — v1 implementation

### Created `ffgl-rs/video-looper/` crate
All 6 source files implemented per plan:

- **`lib.rs`** — entry point with `plugin_main!`
- **`looper.rs`** — core `VideoLooper` struct implementing `SimpleFFGLInstance`
  - State machine: Idle / Recording / Playing
  - Lazy buffer allocation on first frame, reallocates on resolution change
  - Beat-synced playback index from `host_beat.barPhase` and BPM
  - Cycle-based degradation trigger
- **`ring_buffer.rs`** — system RAM frame storage
  - Pre-allocates 30s * 30fps = 900 frames on init
  - `degrade()`: CPU-side 3x3 box blur blended by `(1.0 - quality)` strength
- **`pbo.rs`** — double-buffered PBO async transfer
  - Download: FBO-based `glReadPixels` into PBO, map on next frame
  - Upload: orphan + map + `glTexSubImage2D` from PBO
- **`params.rs`** — 6 parameters (record, playback, loopBeats, speed, quality, dryWet)
  - Knob-to-value mapping for beats (discrete 1/2/4/8/16/32) and speed (-2..2)
- **`shader.rs`** — passthrough/mix shader for output blending

### Plugin registration
- Added to workspace `Cargo.toml` members
- Added to `plugins.json` as `video_looper` (type: rust, crate: video-looper)
- Build: `make build PLUGIN=video_looper`

### Known limitations / next steps
- ~~Cannot `cargo check` from WSL~~ — works via cmd.exe, same as build script
- PBO double-buffering: first frame after recording starts will be empty (pipeline priming)
- Degradation is basic box blur — plan has room for quantization, drift, bleed later

## 2026-03-07 — Architecture rework: delay line with feedback

### Record/play → always-on delay line
Replaced the record/play state machine with a continuously-running video delay
line with feedback. Inspired by Frippertronics tape loop model.

**Old**: record toggle → capture N frames → play toggle → playback
**New**: always recording, `decay` controls feedback, `dry/wet` controls output

Parameters reduced from 6 to 4: loopBeats, decay, quality, dry/wet.
Record, playback, and speed dropped (speed deferred to v2).

### Performance optimization (3 iterations)
1. **Reusable temp buffers** — eliminated per-frame heap allocations (~16MB/frame)
2. **Cached FBO** — stopped creating/deleting FBO every frame in PBO download
3. **GPU-side decay blend** — moved the per-pixel lerp from CPU to a shader pass
   via blend FBO. CPU now does zero per-pixel math during normal operation.
   Only remaining CPU cost: `degrade()` box blur on cycle wrap (still causes stutter).

### Bug fix: black screen after GPU blend migration
Resolume renders effects into its own FBO, not the default framebuffer (0).
We were binding FBO 0 after the blend pass, so the final output rendered to
the window — which Resolume never reads. Fix: save host FBO with
`glGetIntegerv(GL_FRAMEBUFFER_BINDING)`, restore before final output render.

### Documentation and code annotation
- Created `architecture.md` — Mermaid data flow diagram, memory layout, GL resource table
- Created `gl-concepts.md` — PBO timing, FBOs, GL state leaking, `unsafe` in Rust
- All source files annotated with C/Python parallels for Rust-isms
- Established `_CLARIFY_` comment pattern for async inline Q&A

### Current status
- Plugin builds, deploys, runs in Resolume
- Delay line with feedback works (decay, dry/wet functional)
- Still slightly laggy — likely PBO sync stalls or host interaction
- Degradation (quality) untested post-GPU-blend migration
- Next: test FBO save/restore fix, investigate remaining lag, `_CLARIFY_` pass

## 2026-03-07 — Repo restructure: plugins separated from ffgl-rs

### Problem
video-looper crate lived inside the `ffgl-rs` submodule, conflating our plugin
code with the upstream FFGL SDK/bindings. This made it impossible to pull
upstream changes cleanly and meant our creative work lived in someone else's
repo history.

### Changes
- Created `plugins/` workspace at project root with its own `Cargo.toml`
- Moved `video-looper` crate from `ffgl-rs/video-looper/` to `plugins/video-looper/`
- `ffgl-core` referenced as path dependency: `../ffgl-rs/ffgl-core`
- `ffgl-rs` submodule is now clean/unmodified — no workspace member changes needed
  (video-looper was never committed to the submodule)
- Updated `scripts/build-plugin.sh`: Rust plugins now build from `plugins/` dir
- Added `tracing` dependency for frame timing instrumentation

### Tracing instrumentation
Added per-step timing to `draw()` in `looper.rs`:
- Measures: download, upload, blend, output, degrade (each in microseconds)
- Logs every 60 frames (~2s) or whenever degradation fires
- Output goes to Resolume's log (`%LOCALAPPDATA%\Resolume Avenue\Resolume Avenue log.txt`)
- Will reveal whether stutter is PBO sync, GPU blend, or CPU degrade

### Structure
```
plugins/
├── Cargo.toml          # workspace root
└── video-looper/
    ├── Cargo.toml      # depends on ffgl-core via ../ffgl-rs/ffgl-core
    └── src/            # 6 source files (unchanged logic)
```
