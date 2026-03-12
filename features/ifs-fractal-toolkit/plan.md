# IFS Fractal Toolkit — Plan

## Status: in progress

## Step 1: Add translation to mirror-transform

Files to modify:

- `plugins/mirror-transform/src/params.rs` — add Translate X/Y params (indices 4, 5), mapped getters
- `plugins/mirror-transform/src/shader.rs` — pass new uniforms to shader
- `plugins/mirror-transform/src/shaders/transform.frag.glsl` — add translation after rotate, before edge handling

Translation in the shader:
```glsl
// After rotate, before edge handling:
vec2 transformed_uv = rotated + 0.5 + vec2(u_translate_x, u_translate_y);
```

## Step 2: Build logistic-feedback plugin

New plugin: `plugins/logistic-feedback/`

Files to create:

- `Cargo.toml` — crate config, depends on ffgl-core
- `src/lib.rs` — crate entry, `plugin_main!` macro
- `src/params.rs` — 4 params (R, Sensitivity, Spatial Mode, Dry/Wet) with FFGL descriptors
- `src/shader.rs` — compile fullscreen quad + logistic fragment, set uniforms, draw
- `src/logistic.rs` — instance lifecycle, GL state save/restore, draw dispatch
- `src/shaders/fullscreen.vert.glsl` — passthrough vertex (can share with mirror-transform)
- `src/shaders/logistic.frag.glsl` — per-channel logistic map + Sobel edge detection + radial mode

Shader structure:
```glsl
// 1. Sample input
vec4 color = texture(u_input, v_uv);

// 2. Compute spatial r modifier
float r = u_r_base;
if (mode == EDGE) {
    float sobel = sobel_magnitude(u_input, v_uv, u_texel_size);
    r += sobel * u_sensitivity * (4.0 - u_r_base);
} else if (mode == RADIAL) {
    float dist = length(v_uv - 0.5) * 2.0;
    r += dist * u_sensitivity * (4.0 - u_r_base);
}
r = clamp(r, 0.0, 4.0);

// 3. Apply logistic map per-channel
color.r = r * color.r * (1.0 - color.r);
color.g = r * color.g * (1.0 - color.g);
color.b = r * color.b * (1.0 - color.b);

// 4. Mix with original
color = mix(original, color, u_dry_wet);
```

Register in `plugins.json` and `plugins/Cargo.toml`.

## Step 3: Build channel-displace plugin

New plugin: `plugins/channel-displace/`

Files to create:

- `Cargo.toml` — crate config, depends on ffgl-core
- `src/lib.rs` — crate entry, `plugin_main!` macro
- `src/params.rs` — 4 params (Amount, Pattern, Angle, Dry/Wet) with FFGL descriptors
- `src/shader.rs` — compile fullscreen quad + displace fragment, set uniforms, draw
- `src/displace.rs` — instance lifecycle, GL state save/restore, draw dispatch
- `src/shaders/fullscreen.vert.glsl` — passthrough vertex
- `src/shaders/displace.frag.glsl` — cross-channel UV displacement

Shader structure:
```glsl
// 1. Sample input at current UV
vec4 center = texture(u_input, v_uv);

// 2. Compute displacement direction
vec2 dir = vec2(cos(u_angle), sin(u_angle)) * u_amount;

// 3. Per-channel offset UV (cyclic: R←G, G←B, B←R)
vec2 uv_r = v_uv + dir * center.g;
vec2 uv_g = v_uv + dir * center.b;
vec2 uv_b = v_uv + dir * center.r;
// (mutual mode: each displaced by avg of other two)

// 4. Re-sample per channel
float r = texture(u_input, uv_r).r;
float g = texture(u_input, uv_g).g;
float b = texture(u_input, uv_b).b;

// 5. Mix with original
vec4 displaced = vec4(r, g, b, 1.0);
out_color = mix(center, displaced, u_dry_wet);
```

Register in `plugins.json` and `plugins/Cargo.toml`.

## Step 4: Example compositions

Document Resolume layer setups for classic IFS patterns. These are text recipes (param values and layer structure), not Resolume project files since those are binary and environment-specific.

- `example-compositions/cantor-dust.md`
- `example-compositions/sierpinski-triangle.md`
- `example-compositions/fractal-spiral.md`

## Step 5: Update feature docs

- Update `features/mirror-transform/requirements.md` with new translate params
- Update `features/mirror-transform/devlog.md` with translation addition
- Update `features/ifs-fractal-toolkit/devlog.md` as work progresses
