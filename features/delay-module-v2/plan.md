# Delay Module v2 — Implementation Plan

## Overview

Simplify the delay-line module from Send/Receive to Read/Write. Read is trivial (read oldest buffer frame, output it). Write owns all timing and buffer management. Mixing is delegated to Resolume's native blend/opacity controls.

## Ring buffer sizing and read position

The ring buffer uses `loop_length + 1` slots to avoid read/write aliasing. Write writes to `write_pos`, Read reads from `(write_pos + 1) % buf_size` — the oldest slot. This gives exactly `loop_length` frames of delay.

Trace with loop_length=30, buf_size=31:

| Frame | write_pos | Read: (wp+1)%31 | Read content age | Write writes to |
|-------|-----------|-----------------|------------------|-----------------|
| 0     | 0         | 1 (black)       | —                | buffer[0]       |
| 1     | 1         | 2 (black)       | —                | buffer[1]       |
| ...   | ...       | ...             | ...              | ...             |
| 30    | 30        | 0               | 30 frames old    | buffer[30]      |
| 31    | 0         | 1               | 30 frames old    | buffer[0]       |

After the buffer fills (frame 30+), Read consistently outputs content that is exactly 30 frames old.

## 1. Registry changes (`registry.rs`)

Buffer allocation uses `loop_length + 1` slots instead of `loop_length`:
- `begin_frame_write`: wraps `write_pos` at `loop_length + 1` (buf_size), not `loop_length`
- `read_channel` return value: add `buf_size` so Read can compute `(write_pos + 1) % buf_size`
- `BUFFER_DEPTH` remains 240 as the max — buf_size is clamped to `min(loop_length + 1, BUFFER_DEPTH)`
- Texture array allocation already uses `BUFFER_DEPTH` layers, so no reallocation needed

Updated `read_channel` signature:
```rust
/// Returns (texture_array, write_pos, buf_size, width, height) or None.
pub fn read_channel(channel: usize) -> Option<(GLuint, u32, u32, u32, u32)>
```

Changed: `loop_length` → `buf_size` in the return tuple. Read uses `buf_size` to compute read position. Write doesn't call `read_channel`.

## 2. Params restructure (`params.rs`)

Reduce from 8 to 7 params. Remove Passthrough (index 6), shift Decay to index 6.

| Index | Name         | Type    | Read uses | Write uses |
|-------|--------------|---------|-----------|------------|
| 0     | Mode         | Option  | yes       | yes        |
| 1     | Channel      | Option  | yes       | yes        |
| 2     | Sync Mode    | Option  | —         | yes        |
| 3     | Subdivision  | Option  | —         | yes        |
| 4     | Delay Ms     | Integer | —         | yes        |
| 5     | Delay Frames | Integer | —         | yes        |
| 6     | Decay        | Standard| —         | yes        |

Changes:
- `NUM_PARAMS`: 8 → 7
- Remove `PARAM_PASSTHROUGH`
- `PARAM_DECAY`: 7 → 6
- Rename `Mode::Send` → `Mode::Write`, `Mode::Receive` → `Mode::Read`
- Rename option labels in PARAM_INFOS: "Receive" → "Read", "Send" → "Write"
- Remove `passthrough()` getter
- Update `DelayParams::new()` defaults array

## 3. Read shader (`shaders/read_output.frag.glsl`) — **New**

```glsl
#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2DArray u_buffer;
uniform float u_layer;

void main() {
    out_color = texture(u_buffer, vec3(v_uv, u_layer));
}
```

Minimal: sample buffer layer, output it. No scaling, no mixing.

## 4. Shader restructure (`shader.rs`)

Remove programs that are no longer needed:
- Remove `receive` program (replaced by `read_output`)
- Remove `send_output` program (Write outputs buffer content, not passthrough)

Add:
- `read_output` program with uniforms `u_buffer`, `u_layer`
- `read_output_pass(buffer_tex, layer)` method

Keep:
- `write` program (writes input texture to buffer target)
- `fade` program (decay blend for Write)

Updated `DelayShaders` struct has 3 programs: `write`, `fade`, `read_output`.

## 5. Read mode (`delay.rs` — replace `draw_receive`)

New `draw_read`:
```
1. channel = params.channel()
2. buf_info = registry::read_channel(channel)
3. If None → output black (clear host FBO or draw black quad)
4. (buf_tex, write_pos, buf_size, _, _) = buf_info
5. read_pos = (write_pos + 1) % buf_size
6. Bind host FBO, set viewport
7. read_output_pass(buf_tex, read_pos as f32)
```

No timing logic. Read computes read position from write_pos and buf_size, both provided by the registry.

## 6. Write mode (`delay.rs` — replace `draw_send`)

New `draw_write`:
```
1. channel = params.channel()
2. loop_length = delay_frames(bpm, buffer_depth)
3. (buf_tex, fbo, write_pos) = registry::begin_frame_write(channel, loop_length, w, h)
4. Bind buffer[write_pos] as render target
5. If decay > 0: fade_pass(buf_tex, write_pos, decay)
6. Write input to buffer[write_pos]:
   - If decay > 0: additive blend (GL_BLEND with CONSTANT_COLOR)
   - If decay = 0: clean overwrite (no blend)
7. Bind host FBO, set viewport
8. buf_size = loop_length + 1
9. read_output_pass(buf_tex, write_pos as f32)  — output what was written
```

Key change from v1 Send: step 9 outputs the buffer content instead of passthrough of live input.

## 7. Match block update (`delay.rs` — `draw()`)

```rust
match self.params.mode() {
    Mode::Read => self.draw_read(data, input_tex, host_fbo, host_viewport),
    Mode::Write => self.draw_write(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport),
}
```

Note: Read no longer needs `uv_scale` — buffer textures are exact-size, no hardware padding.

## 8. Clean up dead files

Remove:
- `shaders/receive.frag.glsl`
- `shaders/send_output.frag.glsl`
- `shaders/blend.frag.glsl` (incomplete, never used)
- `shaders/read.frag.glsl` (if unused)

## Files changed

| File | Change |
|------|--------|
| `src/registry.rs` | `buf_size = loop_length + 1`, return buf_size from read_channel |
| `src/params.rs` | 7 params, rename modes, remove passthrough |
| `src/delay.rs` | `draw_read` / `draw_write`, remove old methods |
| `src/shader.rs` | Remove receive/send_output, add read_output |
| `src/shaders/read_output.frag.glsl` | **New** — buffer[layer] output |
| `src/shaders/receive.frag.glsl` | **Remove** |
| `src/shaders/send_output.frag.glsl` | **Remove** |
| `src/shaders/blend.frag.glsl` | **Remove** |
| `src/shaders/read.frag.glsl` | **Remove** (if unused) |

## Verification

1. **Write only, decay=0** — clean delay, input overwrites buffer, output = input
2. **Write only, decay=0.9** — trails/echo accumulation
3. **Read alone, no Write** — outputs black (no buffer), or frozen frame if Write was active then removed
4. **Read → Write chain** — Read outputs content from loop_length frames ago, Resolume blends with source, Write writes blend
5. **Read(Add, 80%) → mirror-transform → Write(decay=0.9)** — feedback loop with spatial transform and controlled feedback
6. **Exposure → Read → Write** — send level controlled by upstream gain
7. **Timing check**: with loop_length=30 at 30fps, Read outputs content from exactly 1 second ago
8. `make build PLUGIN=delay_line_module && make deploy PLUGIN=delay_line_module`

## Implementation order

1. `registry.rs` — buf_size logic, updated return type
2. `params.rs` — restructure (compile check)
3. `shaders/read_output.frag.glsl` — new shader
4. `shader.rs` — swap programs
5. `delay.rs` — rewrite draw methods
6. Delete dead shader files
7. Build and deploy
8. Test Read → Write chain in Resolume
