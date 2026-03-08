# Delay Line Module — Implementation Plan

## Architecture

The simplest possible FFGL effect: a GPU texture array used as a ring buffer. Write the input frame to the current slot, output the frame from D slots ago. Two shader passes per frame (write + read), no CPU pixel work.

```
Per frame:
  1. Save host GL state
  2. Write: render input texture → buffer[write_pos] via FBO
  3. Read:  render buffer[write_pos - D] → host FBO
  4. Advance write_pos
  5. Restore host GL state
```

### Buffer design

- `GL_TEXTURE_2D_ARRAY` — one layer per frame slot
- 900 layers max (15s at 60fps)
- Single resolution (matches input)
- Write pointer increments by 1 each frame, wraps at 900
- Read pointer = `write_pos - D`, wrapped, clamped to `[1, 899]`

### Delay calculation

```
fps_estimate = lerp(fps_estimate, 1.0 / frame_delta, 0.05)
D = round((60.0 / bpm) * subdivision_beats * fps_estimate)
D = clamp(D, 1, buffer_size - 1)
```

Read pointer is derived fresh each frame from write_pos - D. No drift possible.

### Shaders

One vertex shader (fullscreen quad), two fragment shaders:

**Write shader** — copies sampler2D input to current FBO target (an array layer):
```glsl
uniform sampler2D u_input;
void main() { out_color = texture(u_input, v_uv); }
```

**Read shader** — samples from sampler2DArray at a specific layer:
```glsl
uniform sampler2DArray u_buffer;
uniform float u_layer;
void main() { out_color = texture(u_buffer, vec3(v_uv, u_layer)); }
```

## Files

```
plugins/delay-line-module/
├── Cargo.toml
└── src/
    ├── lib.rs        # entry point: plugin_main!(SimpleFFGLHandler<DelayLine>)
    ├── delay.rs      # DelayLine struct + SimpleFFGLInstance impl
    ├── params.rs     # 1 parameter: subdivision
    ├── shader.rs     # quad geometry, shader loading, GL helpers
    └── shaders/
        ├── fullscreen.vert.glsl   # shared vertex shader (fullscreen quad)
        ├── write.frag.glsl        # passthrough: sampler2D → FBO layer
        └── read.frag.glsl         # sample: sampler2DArray + layer → output
```

Shader files are embedded at compile time via `include_str!()` — syntax-highlightable in the editor, zero runtime file I/O.

## Steps

### Step 1: Scaffold crate

- Create `plugins/delay-line-module/` with Cargo.toml (cdylib, workspace deps: ffgl-core, gl, gl_loader, tracing)
- Add to `plugins/Cargo.toml` workspace members
- Add to `plugins.json` registry
- Stub `lib.rs` with `plugin_main!`

### Step 2: params.rs — subdivision parameter

- 1 parameter: Subdivision (Option type, 6 discrete values)
- Knob zones: 1/16, 1/8, 1/4, 1/2, 1 bar, 2 bars
- `subdivision_beats()` returns the beat multiplier (0.25, 0.5, 1.0, 2.0, 4.0, 8.0)
- Reuse the pattern from existing plugins (LazyLock<[SimpleParamInfo; 1]>)

### Step 3: shader.rs — quad + write/read shaders

- Port `QuadGeometry` from LTM dream (identical fullscreen quad)
- Port `ShaderProgram` helper and compile/link functions
- Write shader: passthrough sampler2D
- Read shader: sample sampler2DArray at u_layer
- Cache uniform locations at init

### Step 4: delay.rs — the plugin

**Struct fields:**
- `buffer_tex: GLuint` — GL_TEXTURE_2D_ARRAY handle
- `fbo: GLuint` — single FBO, rebind layer per frame
- `write_pos: u32` — current write position
- `buffer_depth: u32` — 900 (const)
- `width / height: u32` — current resolution
- `params: DelayParams`
- `shaders: Option<DelayShaders>` — lazy init
- `fps_estimate: f32` — EMA of measured fps
- `last_frame_time: Option<Instant>` — for delta measurement

**new():**
- Init GL loader, zero all fields, fps_estimate = 60.0

**draw():**
1. Lazy-init shaders
2. Get input texture, detect resolution change → (re)allocate texture array + FBO
3. Measure frame delta, update fps EMA
4. Compute D from BPM + subdivision + fps_estimate, clamp
5. Save host FBO/viewport/scissor/blend/depth
6. **Write pass**: bind FBO → array layer `write_pos`, render input texture with write shader
7. **Read pass**: bind host FBO, render array layer `(write_pos - D) % depth` with read shader
8. Advance `write_pos = (write_pos + 1) % depth`
9. Restore host GL state

**Drop:** delete texture array + FBO

### Step 5: Build and deploy

- `make build PLUGIN=delay_line_module`
- Deploy to Resolume, test with feedback source enabled
- Verify: subdivision changes delay length, echoes compound with stacked Resolume effects

## Reuse from existing plugins

| Component | Source | Reuse method |
|-----------|--------|-------------|
| QuadGeometry | LTM dream shader.rs | Copy (identical) |
| ShaderProgram + compile/link | LTM dream shader.rs | Copy (identical) |
| GL state save/restore pattern | LTM dream dream.rs | Copy pattern |
| Parameter framework | LTM dream params.rs | Simplified (1 param) |
| Texture array allocation | LTM dream pyramid.rs | Simplified (1 tier, no downsampling) |

## What's NOT included

- No PBO transfers (GPU-only, no system RAM)
- No degradation/blur
- No feedback logic (Resolume handles it)
- No dry/wet (Resolume handles it)
- No transforms (Resolume handles it)
- No composite/blend shader
- No MIDI output
- No multi-tier pyramid

## Estimated size

~250 lines total across 4 files. Compare: video-looper ~600 lines, LTM dream ~800 lines.
