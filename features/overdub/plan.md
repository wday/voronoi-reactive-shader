# Overdub Implementation Plan

## 1. Frame detection in registry (`registry.rs`)

Add `last_advance_time: Instant` to `ChannelBuffer`. New function `begin_frame_write(ch, w, h) -> (tex, fbo, write_pos, is_first)`:
- If >2ms since last advance: advance `write_pos`, update timestamp, return `is_first=true`
- Otherwise: return current `write_pos`, `is_first=false`

2ms threshold: Resolume layers render sub-millisecond apart, frames are 8–16ms apart (120–60fps). Initialize timestamp to `Instant::now() - Duration::from_secs(1)` so first call always advances.

Remove `advance_write_pos()` call from `draw_send`.

## 2. Decay parameter (`params.rs`)

Add `PARAM_DECAY = 8`, `NUM_PARAMS` 8 → 9. Standard type, default 0.0 (backward compatible). Getter: `decay() -> f32`.

## 3. Fade shader (`shaders/fade.frag.glsl`)

New 12-line GLSL shader: reads `buffer[layer]`, scales by `u_decay`, outputs to write target.

## 4. Fade pass wiring (`shader.rs`)

Add `fade: ShaderProgram` with uniforms `u_buffer`, `u_layer`, `u_decay`. Add `fade_pass()` method following `read_pass` pattern.

## 5. Rewrite `draw_send` (`delay.rs`)

Three-case write to `buffer[write_pos]`:

| is_first | decay | Behavior |
|----------|-------|----------|
| true     | 0.0   | Clean overwrite via `write_pass` (seed / backward compat) |
| true     | >0.0  | `fade_pass(read_layer, decay)` then additive `write_pass` with `GL_BLEND(ONE, ONE)` |
| false    | any   | Additive `write_pass` with `GL_BLEND(ONE, ONE)` (subsequent map) |

Host output pass (zero_tap crossfade) is unchanged.

## 6. Verification

1. Single Send, decay=0.0 — identical to current behavior
2. Single Send, decay=0.95 — visible trails/echo accumulation
3. 3 Sends on same channel — write_pos advances once, not three times
4. 5-layer IFS composition — fractal convergence over beat subdivisions
5. `make build PLUGIN=delay_line_module && make deploy PLUGIN=delay_line_module`

## Files

| File | Change |
|------|--------|
| `plugins/delay-line-module/src/registry.rs` | `last_advance_time`, `begin_frame_write()` |
| `plugins/delay-line-module/src/params.rs` | `PARAM_DECAY`, bump `NUM_PARAMS` |
| `plugins/delay-line-module/src/shader.rs` | fade program + `fade_pass()` |
| `plugins/delay-line-module/src/delay.rs` | Rewrite `draw_send`, add decay to log |
| `plugins/delay-line-module/src/shaders/fade.frag.glsl` | **New** |
