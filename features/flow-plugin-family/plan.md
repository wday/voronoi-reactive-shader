# Flow Plugin Family — Implementation Plan

## Build Order

### Phase 1: `flow-inject` (ISF)

Single-pass ISF shader. Straightforward — reads depth, writes field texture.

1. Create `shaders/flow_inject.fs` with ISF header
2. Implement depth gradient → velocity (RG) via `dFdx`/`dFdy`
3. Implement edge detection → density (A) via gradient magnitude
4. Implement depth value → scalar (B)
5. Add parameters: `injectVelocity`, `injectDensity`, `edgeThreshold`, `decayRate`
6. Test with shader harness using `stereo_depth.fs` output (or static greyscale image as mock depth)
7. Register in `plugins.json`, verify build
8. Test in Resolume with stereo-depth upstream

### Phase 2: `flow-euler` (ISF)

4-pass ISF with two persistent textures. Core simulation.

1. Create `shaders/flow_euler.fs` with ISF header, declare persistent textures
2. Pass 0 — Inject: blend input field into velocity state
3. Pass 1 — Velocity update: 4-neighbor pressure gradient, curl, divergence
4. Pass 2 — Advect: semi-Lagrangian backward trace with bilinear sample
5. Pass 3 — Decay + output: density decay, emit field texture
6. Add parameters: `dt`, `viscosity`, `decayRate`, `curlStrength`, `boundaryMode`
7. Test standalone with shader harness (mouse-driven injection for quick iteration)
8. Test in chain: flow-inject → flow-euler
9. Test output as Channel Displace source
10. Register in `plugins.json`, verify build

### Phase 3: `flow-lagrange` (Rust FFGL)

Vertex pipeline particle system. Build after Euler works so there's a field to test against.

1. Create `plugins/flow-lagrange/` crate, add to workspace
2. Implement position texture (512x512 RGBA float) — init with random positions
3. Pass 0 (fragment): particle integration — sample velocity field, Euler step, write back
4. Pass 1 (vertex): GL_POINTS draw from position texture
5. Point sprite fragment shader: age/speed → color/alpha, additive blend
6. Add parameters: `particleCount`, `pointSize`, `speedScale`, `drag`, `spawnMode`, `colorMode`
7. Test with flow-euler output
8. Test full chain: stereo-depth → flow-inject → flow-euler → flow-lagrange
9. Register in `plugins.json`, verify build + deploy

### Phase 4: Integration & polish

1. Move `stereo_depth.fs` into `shaders/` for consistency
2. Test all composition paths documented in requirements
3. Resolve open questions (particle lifecycle, texture resolution, format)
4. Update CREDITS.md if new techniques referenced

## Notes

- ISF plugins (phases 1–2) can be previewed in shader harness before Resolume testing
- flow-lagrange needs GL_POINTS which requires Rust FFGL — ISF cannot do this
- Field texture convention must be documented in each plugin's ISF DESCRIPTION
- All ISF plugins should work at the host's native resolution for the field texture
