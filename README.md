# voronoi-reactive-shader

FFGL plugin suite for Resolume — real-time fragment shaders for live visual performance. Started as a reactive Voronoi surface, grew into a modular toolkit of feedback transforms, temporal effects, and chaos-theory primitives.

**[Interactive math walkthrough](https://wday.github.io/voronoi-reactive-shader/)** — theory content from `THEORY.md` with embedded WebGL demos.

## Plugins

| Plugin | Type | What it does |
|--------|------|-------------|
| **Voronoi Reactive** | ISF | 3-layer Voronoi with animated seeds, edge glow, spatial warp, HSV coloring |
| **Mirror Transform** | Rust | Kaleidoscope edge folding with scale, rotation, swirl, translation |
| **Logistic Feedback** | Rust | Per-channel logistic map with Sobel edge detection, radial mode |
| **Channel Displace** | Rust | Cross-channel UV displacement for strange attractor dynamics |
| **Delay Line** | Rust | Video delay with send/receive feedback routing, tap sync |
| **Video Looper** | Rust | Frame buffer delay with PBO async transfers, decay blend |
| **Dream LTM** | Rust | Tiered temporal pyramid — 4 tiers, zero CPU, recency-weighted composite |

All Rust plugins build as Windows DLLs via FFGL. See `plugins.json` for the registry.

## Build

Requires WSL2 with Windows-side Rust toolchain. See `WINDOWS_ENVIRONMENT.md` for setup.

```
make list                           # show registered plugins
make build                          # build all plugins
make build PLUGIN=mirror_transform  # build one
make deploy                         # copy DLLs to Resolume
```

## Shader Test Harness

Offline renderer for exploring shader feedback without Resolume. Lives in `tools/shader-harness/`.

```
cd tools/shader-harness

# Interactive viewer — tweak uniforms live, feedback loop
uv run python harness.py --shaders transform

# Parameter space explorer — generate batches of feedback images
uv run python explore.py                                    # 40 random images
uv run python explore.py --keepers keepers.json             # evolve from favorites
uv run python explore.py --keepers keepers.json --strategy deep   # refine a vein
uv run python explore.py --keepers keepers.json --strategy bold   # break out

# Browser gallery — browse, flag keepers, evolve
uv run python gallery.py

# High-res render — re-render keepers at publication resolution
uv run python explore.py --keepers keepers.json --render hi     # 2560x1920
uv run python explore.py --keepers keepers.json --render max    # 3840x2880
uv run python explore.py --keepers keepers.json --render ultra  # 7680x5760
```

Evolution workflow: explore → gallery → flag keepers → evolve → repeat. Output is organized by generation (`gen_000/`, `gen_001/`, ...) with lineage tracking.

## Structure

```
shaders/          ISF fragment shaders
plugins/          Rust plugin workspace (7 crates)
vendor/ffgl-rs/   FFGL build pipeline (git submodule)
vendor/spout2/    Spout2 SDK for GPU texture sharing (git submodule)
tools/            Shader test harness, stereo teapot, spout-publish
preview/          Browser ISF preview
features/         Feature specs, plans, devlogs
docs/             GitHub Pages + reference papers
scripts/          Build, deploy, utility scripts
```

## Credits

See [CREDITS.md](CREDITS.md) for full attribution.
