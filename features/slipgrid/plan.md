# Slipgrid — Implementation Plan

> **Status 2026-07-12: Phases 1–3 done, built and deployed.** The shader design
> changed substantially during Phase 1 (the gradient *walk* below was measured and
> rejected; Conserve/Smear modes replaced it). `requirements.md` and `devlog.md`
> are the current truth. Kept below as the original plan of record.
> Remaining: live validation in Resolume with real footage.

Template throughout: `plugins/channel-displace/` (self-contained stateless atom —
copies `QuadGeometry`/`ShaderProgram` rather than depending on `pluglib`, loads GLSL
with `include_str!`, writes straight to the host FBO).

## Phase 1 — Shader, previewed in the harness

Write the fragment shader **first** and look at it before any Rust exists. The
harness auto-discovers `plugins/**/*.frag.glsl`, so putting the file in its final
home immediately gets it explore/gallery/evolve coverage for free.

1. `plugins/slipgrid/src/shaders/slipgrid.frag.glsl`
   - `#version 150`, `in vec2 v_uv`, `out vec4 out_color`, uniforms `u_*`, sampler `u_input`.
   - Copy `hash1`/`hash2` (Dave Hoskins, sine-free) from `shaders/voronoi_reactive.fs:160-172`,
     keeping the credit line.
   - `luma()` with the repo constant `vec3(0.299, 0.587, 0.114)`.
   - `edge_energy(uv)` = magnitude of a 4-tap central-difference luma gradient.
   - `edge_gradient(uv)` = central difference **of** `edge_energy` → 4 evals, ~16 taps.
   - Walk loop exactly as specced in requirements §Behavior. Guard the whole gravity
     path behind `if (u_edge_gravity != 0.0)` so the zero case costs nothing.
   - `mix(original, slipped, u_dry_wet)` as the last line.
2. `plugins/slipgrid/src/shaders/slipgrid.defaults.json` — uniform defaults so the
   harness can preview it (the `mirror-transform/src/shaders/transform.defaults.json` pattern).
3. `plugins/slipgrid/src/shaders/fullscreen.vert.glsl` — copy verbatim from channel-displace.
4. **Look at it** in the harness on a high-contrast source. Confirm the gravity
   behavior actually reads before writing a line of Rust — this is the step that
   kills the feature cheaply if the idea doesn't survive contact with pixels.

## Phase 2 — Rust plugin

5. `plugins/slipgrid/Cargo.toml` — `crate-type = ["cdylib"]`, workspace deps
   `ffgl-core`, `gl`, `gl_loader`, `tracing`.
6. `plugins/slipgrid/src/params.rs` — `NUM_PARAMS = 8`, index consts, `LazyLock`
   array of `SimpleParamInfo`, `SlipgridParams` holding raw `[f32; 8]` in host 0..1
   space with accessors doing the range mapping from the requirements table
   (`grid_x()` → `1..64` rounded, `edge_gravity()` → `v * 2.0 - 1.0`, etc.).
7. `plugins/slipgrid/src/shader.rs` — `QuadGeometry`, `ShaderProgram`,
   `SlipgridShader` (caches every `loc_*` at construction, binds `TEXTURE0` → `u_input`),
   `compile_shader`/`link_program`. Copied from channel-displace.
8. `plugins/slipgrid/src/slipgrid.rs` — `SimpleFFGLInstance` impl. Lazy shader
   construction on first `draw` (GL context only exists then). Save/restore
   `SCISSOR_TEST`/`BLEND`/`DEPTH_TEST`. `plugin_info()` with the 16-byte name and `SlpG`.
9. `plugins/slipgrid/src/lib.rs` — four lines: `mod` decls + `plugin_main!`.
10. Pass `uv_scale` (`Width/HardwareWidth`) and `u_texel_size` (`1/HardwareWidth`)
    from `frame_data.textures[0]` — the `mirror-transform/src/transform.rs:32-42`
    convention. Required here, not optional: the shader samples neighbours.

## Phase 3 — Register, build, deploy

11. Add `"slipgrid"` to `members` in `plugins/Cargo.toml`.
12. Add to `plugins.json`:
    ```json
    { "name": "slipgrid", "type": "rust", "crate": "slipgrid", "dll": "slipgrid.dll" }
    ```
13. `make build PLUGIN=slipgrid && make deploy PLUGIN=slipgrid`
    (DLL is locked while Resolume is running — close it or remove the effect first).
14. Live-validate in Resolume against the acceptance list.

## Risks

- **Gravity field cost** — ~128 taps/pixel at max Iterations. The `EdgeGravity == 0`
  early-out covers the common case; if the max still stalls, coarsen the `E` sample
  offset before cutting the Iterations range.
- **Integer tile stepping** at high Locality with low grid counts can walk a tile
  clean off the grid and wrap it to the far side. That's intended (`mod`), but on a
  4×4 grid it will look less like "local slippage" and more like teleportation.
  If it reads badly, clamp `Locality` against the grid size rather than the fixed 16.
- **Harness scope** — only `plugins/*.frag.glsl` is globbed, so Phase 1 preview works,
  but the *param mapping* (host 0..1 → internal ranges) exists only in Rust and isn't
  exercised until Phase 2. Keep the defaults.json values in sync by hand.

## Deferred / possible extensions

- Tile rotation / flip on displacement (a `Tumble` knob).
- Bijective mode (mutual-pair involution) as a `Coherence` param, if the loose
  version's duplication turns out to be too mushy in practice.
- Feeding the gravity field from a *second* input rather than the image itself.
