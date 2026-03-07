# Implementation Plan — Video Looper v1

## Crate setup

### 1. Create `ffgl-rs/video-looper/` crate
- New Rust library crate alongside `ffgl-isf`, `example-raw`, etc.
- Add to `ffgl-rs/Cargo.toml` workspace members
- Dependencies: `ffgl-core`, `gl`, `gl_loader`
- Cargo.toml: `crate-type = ["cdylib"]`

### 2. Add to plugin registry
```json
{
  "name": "video_looper",
  "type": "rust",
  "crate": "video-looper",
  "dll": "video_looper.dll"
}
```

## Plugin structure

### 3. `lib.rs` — entry point
```rust
ffgl_core::plugin_main!(SimpleFFGLHandler<VideoLooper>);
```

### 4. `looper.rs` — core struct
```rust
pub struct VideoLooper {
    ring_buffer: Option<RingBuffer>,  // None until first frame
    state: LooperState,               // Idle / Recording / Playing
    params: LooperParams,
    pbo: PboTransfer,
    passthrough_shader: PassthroughShader,  // render texture to screen
}
```

### 5. `ring_buffer.rs` — frame storage
```rust
pub struct RingBuffer {
    frames: Vec<Vec<u8>>,      // N frames, each width*height*4 bytes
    width: u32,
    height: u32,
    capacity: usize,           // max frames (fps * max_duration)
    write_head: usize,         // current write position
    frame_count: usize,        // frames recorded so far (up to capacity)
}
```

Methods:
- `new(width, height, max_duration_secs, fps) -> Self` — allocate
- `push_frame(data: &[u8])` — write to current slot, advance head
- `get_frame(index: usize) -> &[u8]` — read a frame
- `matches_resolution(width, height) -> bool` — detect resolution change
- `clear()` — reset heads without deallocating

### 6. `pbo.rs` — async GPU ↔ RAM transfer
```rust
pub struct PboTransfer {
    download_pbos: [GLuint; 2],  // double-buffered capture
    upload_pbos: [GLuint; 2],    // double-buffered playback
    current_download: usize,     // ping-pong index
    current_upload: usize,
    frame_size: usize,           // bytes per frame
}
```

Methods:
- `new() -> Self`
- `init(width, height)` — allocate PBO storage
- `begin_download(texture_id, width, height)` — async glReadPixels into PBO
- `finish_download() -> &[u8]` — map previous PBO, return pixel data
- `begin_upload(data: &[u8])` — copy frame data into PBO
- `finish_upload(texture_id, width, height)` — glTexSubImage2D from PBO

### 7. `params.rs` — parameter definitions
Indices:
- 0: `record` (bool as float, threshold 0.5)
- 1: `playback` (bool as float)
- 2: `loopBeats` (float, mapped to discrete: 1,2,4,8,16,32)
- 3: `playbackSpeed` (float -2.0 to 2.0)
- 4: `quality` (float 0.0 to 1.0)
- 5: `dryWet` (float 0.0 to 1.0)

Use `ffgl_core::parameters::ParamInfo` trait for each.

### 8. `shader.rs` — passthrough rendering
Minimal vertex+fragment shader to render a texture to the output framebuffer.
Also used for dry/wet blending (two texture inputs, mix uniform).

## State machine

```
Idle ──record=true──▶ Recording ──playback=true──▶ Playing
  ▲                                                   │
  └──────────────────playback=false───────────────────┘
```

- **Idle**: pass-through (input → output)
- **Recording**: capture frames to ring buffer, pass-through output
- **Playing**: read frames from ring buffer, blend with input via dryWet

## Draw loop (`SimpleFFGLInstance::draw`)

```
1. Get input texture from frame_data.textures[0]
2. Check resolution — if changed, flush + reallocate ring buffer
3. Read params, update state machine
4. If Recording:
   a. PBO download input texture → RAM
   b. Store in ring buffer
   c. Render input to output (pass-through)
5. If Playing:
   a. Calculate playback index from beat phase, speed, mode
   b. Read frame from ring buffer
   c. PBO upload frame → GPU texture
   d. Apply degradation (quality < 1.0): degrade frame in RAM, write back
   e. Blend loop texture with input texture via dryWet
   f. Render blended result to output
6. If Idle:
   a. Render input to output (pass-through)
```

## Beat-quantized timing

```rust
let beat_duration = 60.0 / bpm;  // seconds per beat
let loop_duration = loop_beats * beat_duration;
let loop_frames = (loop_duration * fps) as usize;

// Playback index from bar phase
let phase = data.host_beat.barPhase; // 0.0–1.0 within bar
let index = (phase * loop_frames as f32) as usize % frame_count;
```

Note: `barPhase` is 0–1 per bar (4 beats). For loop lengths != 4 beats, need to scale phase accordingly.

## Degradation (v1 — simple)

On each loop cycle (write_head wraps), apply to each frame in the buffer:
- `quality >= 1.0`: skip
- `quality < 1.0`: for each pixel, blend with neighbor pixels weighted by `(1.0 - quality)`. Simple box blur approximation — one pass, CPU-side.

This is the simplest degradation that gives visible results. More sophisticated effects (quantization, drift) can be added iteratively.

## Build and test

### 9. Build
```bash
make build PLUGIN=video_looper
```

### 10. Deploy and test in Resolume
- Apply as effect on a layer with video source
- Test: record → play → verify loop
- Test: sweep dryWet, playbackSpeed, quality
- Test: change resolution mid-session → verify buffer reset

## Files to create
```
ffgl-rs/video-looper/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── looper.rs
    ├── ring_buffer.rs
    ├── pbo.rs
    ├── params.rs
    └── shader.rs
```
