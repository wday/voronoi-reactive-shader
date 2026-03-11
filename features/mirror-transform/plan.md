# Mirror Transform — Implementation Plan

## Status: v1 complete

## Architecture

Single-pass stateless FFGL effect. No GPU buffers, no temporal state.

### Source files (`plugins/mirror-transform/src/`)

| File | Purpose |
|------|---------|
| `lib.rs` | Crate entry — `plugin_main!` macro |
| `transform.rs` | `MirrorTransform` — instance lifecycle, GL state save/restore, draw dispatch |
| `params.rs` | 4 params (Scale, Rotation, Swirl, Mirror) with FFGL descriptors + mapped getters |
| `shader.rs` | `TransformShader` — compiles fullscreen quad + transform fragment, sets uniforms, draws |
| `shaders/fullscreen.vert.glsl` | Passthrough vertex shader |
| `shaders/transform.frag.glsl` | Scale → swirl → rotate → edge-handle → sample |

### Design decisions

1. **Lazy shader init** — shader compiled on first `draw()`, not in `new()`, to ensure GL context is ready.
2. **Exponential scale** — `2^(v*2-1)` gives 0.5×–2.0× with 1.0× at midpoint. Linear felt wrong for zoom.
3. **Mirror via mod fold** — `1 - abs(mod(uv, 2) - 1)` gives seamless kaleidoscope without extra passes.
4. **Soft-clip fallback** — when mirror is off, edges fade to black via smoothstep over 0.5% border.
5. **Stateless** — no FBO, no texture allocation. Renders directly into host FBO.
