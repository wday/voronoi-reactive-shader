# Credits & Sources

This project builds on foundational work from the real-time graphics community.
Below are the techniques used and their origins, for anyone who wants to learn
more or trace the lineage.

---

## Hash Functions — Dave Hoskins

The "hash without sine" functions (`hash1`, `hash2`) avoid the
`fract(sin(dot(...)))` pattern, which produces machine-dependent results due to
floating-point precision variance across GPUs.

- **Author:** Dave Hoskins
- **Original shader:** [Hash without Sine (Shadertoy)](https://www.shadertoy.com/view/4djSRW)
- **License:** [MIT](https://opensource.org/licenses/MIT) (specified in shader source)
- **Academic survey:** ["Hash Functions for GPU Rendering" (JCGT, 2020)](https://jcgt.org/published/0009/03/02/paper.pdf)
  (run `./scripts/download-refs.sh` for a local copy)

## Voronoi / Cellular Noise — Steven Worley, Inigo Quilez

The F1/F2 distance computation with a 3×3 neighbor cell search is the standard
approach to cellular noise, introduced by Steven Worley and widely popularized
in GLSL by Inigo Quilez.

- **Original paper:** Steven Worley, "A Cellular Texture Basis Function",
  SIGGRAPH 1996 — [ACM Digital Library](https://dl.acm.org/doi/10.1145/237170.237267)
- **Inigo Quilez articles:**
  - [Voronoi Edges](https://iquilezles.org/articles/voronoilines/) — correct edge-distance computation
  - [Cellular Effects](https://iquilezles.org/articles/cellularffx/) — overview of cellular techniques
- **Reference implementations (Shadertoy):**
  - [Voronoi - distances](https://www.shadertoy.com/view/ldl3W8)
  - [Voronoi - basic](https://www.shadertoy.com/view/MslGD8)
- **Tutorial:** [The Book of Shaders, Chapter 12](https://thebookofshaders.com/12/)

## Fractal Jittered Voronoi Partitions — Boris the Brave

The `voronoi-fractal` plugin's Fractal mode implements a hierarchical jittered
Voronoi partition: each level subdivides the one above, every site is parented
to the nearest site one level up, and a query point's colour comes from the
level-0 cell its ancestry reaches. Boundaries are therefore resolved at the
finest level's scale — fractal coastlines rather than polygon edges.

- **Author:** Boris the Brave
- **Article:** ["Fractal Jittered Voronoi Partitions"](https://www.boristhebrave.com/2026/08/29/fractal-jittered-voronoi-partitions/), 29 August 2026
- **Reference implementation:** [Shadertoy sfKSDw](https://www.shadertoy.com/view/sfKSDw), by the same author
- **License:** the article states no explicit licence; the Shadertoy carries no
  licence header, so Shadertoy's default CC BY-NC-SA 3.0 should be assumed for
  *that source text*. The algorithm itself is a mathematical technique.

**Provenance of our implementation.** `voronoi.frag.glsl` was written from the
article's prose description; the reference shader was read afterwards, for
comparison only. No code was copied from it. Two points were checked against it
after the fact: the parent-search radius argument (it uses ±2 for full-square
jitter, and states why), and the choice to colour by root cell.

**Where we deliberately diverge:**

- **Boundary detection.** The reference re-queries `rootCell` at four
  neighbouring pixels and draws a binary edge. We compare the roots of the
  nearest and second-nearest sites instead, which yields a *distance* to the
  boundary and so supports antialiased edges and glow — necessary for a video
  effect, and cheaper than four extra full ancestry traces.
- **Search radius.** The reference uses ±2 (25 cells) because its jitter fills
  the whole square. Ours is confined to `[0.1,0.9]`, so we use ±1 (9 cells).
  Measured: ±1 and ±2 produce indistinguishable output at our jitter bound.
- **Level ratio.** The reference halves each level (`exp2(-level)`). Ours
  exposes the ratio as the `Layer Spread` knob.
- **Adaptive depth.** The reference's article describes an adaptive early-out
  for approximating infinite recursion. We omit it: `Depth` is an artistic knob
  here, so a fixed trace is correct by definition.

## HSV to RGB — Sam Hocevar

The branchless `vec4 K` conversion pattern.

- **Author:** Sam Hocevar
- **Stack Overflow answer (2013):** [stackoverflow.com/a/17897228](https://stackoverflow.com/a/17897228)
- **Blog post:** ~~[lolengine.net — RGB to HSV in GLSL](http://lolengine.net/blog/2013/07/27/rgb-to-hsv-in-glsl)~~ (site returns 403 as of March 2026)
- **License:** [WTFPL](http://www.wtfpl.net/) (essentially public domain)
- **glslify package:** [glsl-hsv2rgb](https://github.com/hughsk/glsl-hsv2rgb)

## Value Noise & Hermite Interpolation — Ken Perlin, Inigo Quilez

The smooth value noise used for spatial warp uses cubic Hermite interpolation
(`3t² − 2t³`, i.e. `smoothstep`), a technique dating back to Perlin's original
noise function.

- **Original paper:** Ken Perlin, "An Image Synthesizer", SIGGRAPH 1985 —
  [ACM Digital Library](https://dl.acm.org/doi/10.1145/325165.325247)
- **Improved noise (quintic curve):** Ken Perlin, "Improving Noise",
  SIGGRAPH 2002 — [PDF (NYU)](https://cs.nyu.edu/~perlin/paper445.pdf)
  (run `./scripts/download-refs.sh` for a local copy)
- **Inigo Quilez:** [Value Noise Derivatives](https://iquilezles.org/articles/morenoise/)
- **Tutorial:** [The Book of Shaders, Chapter 11: Noise](https://thebookofshaders.com/11/)

## Normalized Convolution — Hans Knutsson, Carl-Fredrik Westin

Image-driven Voronoi density uses normalized convolution to reconstruct a
smooth certainty field from sparse image samples. The certainty field biases
seed placement and cell size, producing a Voronoi tessellation that tracks
image structure.

- **Original paper:** Hans Knutsson & Carl-Fredrik Westin, "Normalized and
  Differential Convolution: Methods for Interpolation and Filtering of
  Incomplete and Uncertain Data", CVPR 1993, pp. 515–523 —
  [IEEE Xplore](https://ieeexplore.ieee.org/document/341081)
  (run `./scripts/download-refs.sh` for a local copy via CiteSeerX)
- **Key equations used:** Definition 2 (eq. 3), 0th-order interpolation (eq. 12)

## ISF (Interactive Shader Format) — VIDVOX

The shader metadata format (the JSON header, built-in uniforms like `TIME`,
`RENDERSIZE`, `isf_FragNormCoord`).

- **Specification:** [isf.video](https://isf.video/) / [docs.isf.video](https://docs.isf.video/)
- **GitHub:** [ISF_Spec](https://github.com/mrRay/ISF_Spec) / [ISF-Files](https://github.com/Vidvox/ISF-Files)

---

## Logistic Map — Robert May

The logistic-feedback plugin applies the discrete logistic map
`x(n+1) = r * x(n) * (1 - x(n))` per color channel. This is the canonical
example of a simple system exhibiting bifurcation and deterministic chaos.

- **Original paper:** Robert M. May, "Simple mathematical models with very
  complicated dynamics", Nature 261, pp. 459–467, 1976 —
  [doi:10.1038/261459a0](https://doi.org/10.1038/261459a0)
- **Reference:** Steven Strogatz, *Nonlinear Dynamics and Chaos*, Chapter 10

## FFGL — Resolume

The Free Frame GL plugin specification used by Resolume and other VJ software.

- **Specification:** [freeframe.org](http://freeframe.org/)
- **Rust bindings:** `ffgl-rs` (git submodule, see `vendor/ffgl-rs/`)

## Licensing Notes

| Component | License | Commercial use? |
|---|---|---|
| Hash functions (Hoskins) | MIT | Yes |
| Voronoi algorithm | Mathematical technique (not copyrightable) | Yes |
| Fractal jittered partition (Boris the Brave) | Algorithm: mathematical technique. Reference shader not copied. | Yes |
| HSV conversion (Hocevar) | WTFPL | Yes |
| Normalized convolution (Knutsson & Westin) | Mathematical technique | Yes |
| Value noise | Mathematical technique | Yes |
| Logistic map | Mathematical technique | Yes |
| ISF format | Open specification | Yes |
| FFGL | Open specification | Yes |

All components are permissively licensed or uncopyrightable mathematical
techniques.

---

## Acknowledgements

- **Jeff La** — Discussion leading to the application of Knutsson & Westin's
  normalized convolution for image-driven Voronoi density.
