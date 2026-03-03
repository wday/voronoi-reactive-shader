# Voronoi Reactive Shader — Development Log

## Checkpoint: v0.1 — Working FFGL Plugin (2026-03-02)

First successful build. The shader compiles, validates, and deploys as an FFGL `.bundle` for Resolume on macOS (arm64).

### What Was Built

```
voronoi-reactive-shader/
├── shaders/voronoi_reactive.fs   ← ISF shader (single file, ~260 lines)
├── ffgl-rs/                      ← git submodule: wday/ffgl-rs (build toolchain)
├── scripts/
│   ├── deploy.sh                 ← validate + build + install
│   └── preview.sh                ← launch browser preview with hot reload
├── preview/                      ← browser-based ISF preview harness (v0.2)
│   ├── package.json
│   ├── server.js
│   └── index.html
└── DEVELOPMENT_LOG.md
```

**Plugin output:**
- `~/Library/Graphics/FreeFrame Plug-Ins/voronoi_reactive.bundle`
- `~/Documents/Resolume Arena/Extra Effects/voronoi_reactive.bundle`

### Key Architectural Decisions

**1. ISF over raw FFGL**
The shader is pure ISF (JSON header + GLSL fragment). ffgl-rs embeds the ISF source at compile time via `include_str!` and generates the Rust FFGL plugin around it. We never write Rust — the shader file is the only source.

**2. Hash without sine (Dave Hoskins)**
Chose `fract(dot(...) * largeConstants)` over `fract(sin(...) * 43758.5453)`. The sin-based hash produces different results across GPUs due to precision differences in transcendental functions. The Hoskins hash uses only multiply/add/fract — deterministic everywhere.

**3. Float-only GLSL (no uint/bitwise)**
The `glsl` crate v7.0.0 used by ffgl-rs for validation may not handle all GLSL 1.40 features. We stayed with float types exclusively to avoid parser surprises. This also means the "integer hash" from the original plan became a float-arithmetic hash — same stability benefits, just different implementation.

**4. Drift amplitude budget**
Voronoi seed points check a 3×3 neighborhood. Drift must stay under ~0.5 cell widths from center or nearest-seed detection breaks. Both drift modes (circular and chaotic) are capped at 0.35 cell widths. This is baked into the constants, not controlled by `driftSpeed` — speed only controls temporal rate.

**5. Contrast as linear stretch**
`(color - 0.5) * contrast + 0.5` — simple, no gamma curves. At contrast=1.0, identity. Values >1 push away from midpoint (more dramatic edges), <1 flatten toward gray. Chosen over pow-based gamma because it's more intuitive for a VJ control surface.

**6. Layer compositing**
Weighted additive blend, not alpha compositing. Layer 0 always has weight 1.0. Deeper layers contribute `pow(layerMix, layerIndex)`. This gives smooth fade-in/fade-out of multi-scale detail with a single knob.

### Build Pipeline

```bash
# Full cycle: validate → compile → deploy
./scripts/deploy.sh

# Debug build (faster compile, includes symbols)
./scripts/deploy.sh --debug
```

Under the hood:
1. `validate_isf.sh` — parses ISF JSON, compiles to GLSL 1.40, validates with glsl crate
2. `deploy_isf.sh` — sets `ISF_SOURCE` env var, runs `cargo build --release -p ffgl-isf`
3. `deploy_bundle.sh` — copies `.dylib` into macOS `.bundle` structure in both FFGL plugin dirs

Critical env var: `export SDKROOT="$(xcrun --show-sdk-path)"` — required for bindgen to find macOS system headers. The `Xcode 16.0.app` path has a space that breaks `BINDGEN_EXTRA_CLANG_ARGS`; `SDKROOT` is the reliable alternative.

### Parameter Surface (13 params)

| # | Parameter | Range | Default | Notes |
|---|-----------|-------|---------|-------|
| 1 | density | 2–30 | 8 | Base cell count of first layer |
| 2 | layerSpread | 1.5–4.0 | 2.0 | Scale multiplier between layers |
| 3 | layerMix | 0–1 | 0.5 | How much deeper layers contribute |
| 4 | driftSpeed | 0–3 | 0.5 | Temporal rate of seed animation |
| 5 | driftChaos | 0–1 | 0.3 | 0=smooth orbits, 1=random walk |
| 6 | edgeWidth | 0–0.15 | 0.04 | Voronoi edge thickness |
| 7 | edgeGlow | 0–1 | 0.4 | Soft bloom around edges |
| 8 | warp | 0–1 | 0.0 | Spatial distortion (value noise) |
| 9 | colorShift | 0–1 | 0.0 | Hue rotation |
| 10 | colorSat | 0–1 | 0.7 | Saturation |
| 11 | brightness | 0–2 | 1.0 | Global brightness multiplier |
| 12 | contrast | 0–2 | 1.0 | Edge vs. cell brightness separation |
| 13 | tint | color | white | Global RGBA tint (4 knobs in Resolume) |

In Resolume, these appear as 16 knobs (tint splits into R/G/B/A) plus the auto-generated scale overlay.

### ISF/GLSL Patterns Worth Knowing

- **ISF builtins**: `RENDERSIZE` (vec2), `TIME` (float, seconds), `isf_FragNormCoord` (vec2, 0–1), `gl_FragColor` (output)
- **Param uniforms**: ISF params become `uniform float paramName;` — accessible from any function, not just main()
- **`#define` limitation**: glsl crate v7.0.0 only supports `#define` at global scope. Use `const float` inside functions.
- **No image inputs** = plugin type is "Source" (generator). Adding an `IMAGE` input would make it an "Effect".
- **ISF name**: truncated to 16 chars by `deploy_isf.sh` → `voronoi_reactive` (exactly 16)

### Shader Structure

```
hash1(vec2) → float           — cell ID hashing, noise
hash2(vec2) → vec2            — seed placement, drift
valueNoise2(vec2) → vec2      — smooth spatial warp (bilinear interpolation of hash)
hsv2rgb(vec3) → vec3          — color model conversion
voronoiLayer(uv, scale, time) → vec4(F1, F2, cellID)  — single-layer Voronoi
main()                        — aspect correction, warp, 3-layer loop, composite, tint
```

### Performance Budget
- 3 layers × 9 neighbor checks = 27 distance computations per fragment
- Single-pass, no texture lookups, no multi-pass
- Should be well within 60fps at 1080p on any discrete GPU

### Not Yet Tested
- [ ] Resolume load test (plugin appears, params show as knobs)
- [ ] Visual verification at runtime
- [ ] Parameter sweep (each param visibly affects output)
- [ ] Performance at 1080p/60fps
- [ ] Audio reactivity path (external automation of params)

### Possible Future Directions
- Add `IMAGE` input to make it an effect (Voronoi overlay on video)
- Expose per-layer drift speed/chaos (currently shared)
- Add a palette mode (discrete color palettes instead of continuous HSV)
- Multi-pass: render Voronoi to texture, then post-process (blur, feedback)
- Audio-reactive parameter mapping (separate audio-dynamics-extractor project)
- Preview harness: add image/video input support, MIDI controller mapping

---

## Checkpoint: v0.2 — Browser-Based ISF Preview Harness (2026-03-02)

Added a lightweight local preview that hot-reloads on save — eliminates the build→deploy→Resolume cycle during shader iteration.

### What Was Added

```
preview/
├── package.json       ← interactive-shader-format, lil-gui, ws
├── server.js          ← HTTP + WebSocket + fs.watch (~65 lines)
└── index.html         ← canvas + ISFRenderer + lil-gui + WS client (~147 lines)

scripts/
└── preview.sh         ← convenience launcher (auto-installs deps)
```

### How It Works

```bash
./scripts/preview.sh                     # default: voronoi_reactive.fs
./scripts/preview.sh path/to/other.fs    # any ISF shader
# → http://localhost:9000
```

1. **server.js** serves the page, shader source (`/shader`), and vendored libs (`/lib/*`). A WebSocket broadcasts `"reload"` when `fs.watch()` detects a shader file change (debounced 100ms).

2. **index.html** renders the shader via `interactiveShaderFormat.Renderer` (WebGL) and auto-generates a lil-gui panel from the ISF JSON header:
   - `float` → slider with min/max/step
   - `color` → color picker
   - `bool` → checkbox

3. **Hot reload** — on WebSocket `"reload"`: re-fetches shader, snapshots current slider values, calls `loadSource()`, restores param values for inputs that still exist, rebuilds GUI for any new/removed inputs. No page refresh needed.

4. **Error overlay** — if `renderer.valid` is false after `loadSource()`, a red overlay shows the compilation error at the bottom of the viewport. Last working frame stays visible. Overlay clears on next successful load.

5. **FPS counter** — subtle monospace readout in the bottom-right corner.

### Design Decisions

**No bundler** — The ISF library ships a UMD browser build (`dist/build.js`), lil-gui ships UMD too. Plain `<script>` tags, zero build step. A dev tool doesn't need Vite.

**Generic shader path** — Not hardcoded to `voronoi_reactive.fs`. Any ISF `.fs` can be previewed by passing its path as a CLI arg. Default is `../shaders/voronoi_reactive.fs` relative to the preview dir.

**Param preservation across reloads** — Slider positions survive hot reloads. This matters: you spend time dialing in parameters, and losing them on every save would negate the fast-iteration benefit.

**Node-only server** — Uses only builtins (`http`, `fs`, `path`) plus `ws`. No Express, no framework. Runs on Node 16+.

### Verified

- [x] All HTTP routes return 200 (/, /shader, /lib/isf.js, /lib/lil-gui.js, /lib/lil-gui.css)
- [x] Shader renders in browser with all 13 parameter sliders + color picker
- [x] Hot reload triggers on file save
- [x] Parameter values preserved across reloads
- [x] Error overlay appears on shader syntax errors, clears on fix

---

### Recovery Instructions

If future work gets tangled, return to this checkpoint:

```bash
# The shader source of truth:
shaders/voronoi_reactive.fs

# Preview (fast iteration):
./scripts/preview.sh

# Rebuild FFGL plugin from scratch:
git submodule update --init --recursive
./scripts/deploy.sh

# If Rust toolchain is missing:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

The ffgl-rs submodule pins a specific commit. If you need to update it, `cd ffgl-rs && git pull origin main`, then rebuild. The shader is independent of ffgl-rs version as long as the ISF pipeline is unchanged.
