# Credits & Sources

This shader builds on foundational work from the real-time graphics community.
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

## HSV to RGB — Sam Hocevar

The branchless `vec4 K` conversion pattern.

- **Author:** Sam Hocevar
- **Stack Overflow answer (2013):** [stackoverflow.com/a/17897228](https://stackoverflow.com/a/17897228)
- **Blog post:** [lolengine.net — RGB to HSV in GLSL](http://lolengine.net/blog/2013/07/27/rgb-to-hsv-in-glsl)
- **License:** [WTFPL](http://www.wtfpl.net/) (essentially public domain)
- **glslify package:** [glsl-hsv2rgb](https://github.com/hughsk/glsl-hsv2rgb)

## Value Noise & Hermite Interpolation — Ken Perlin, Inigo Quilez

The smooth value noise used for spatial warp uses cubic Hermite interpolation
(`3t² − 2t³`, i.e. `smoothstep`), a technique dating back to Perlin's original
noise function.

- **Original paper:** Ken Perlin, "An Image Synthesizer", SIGGRAPH 1985 —
  [ACM Digital Library](https://dl.acm.org/doi/10.1145/325165.325247)
- **Improved noise (quintic curve):** Ken Perlin, "Improving Noise",
  SIGGRAPH 2002 — [PDF (NYU)](https://mrl.cs.nyu.edu/~perlin/paper445.pdf)
- **Inigo Quilez:** [Value Noise Derivatives](https://iquilezles.org/articles/morenoise/)
- **Tutorial:** [The Book of Shaders, Chapter 11: Noise](https://thebookofshaders.com/11/)

## ISF (Interactive Shader Format) — VIDVOX

The shader metadata format (the JSON header, built-in uniforms like `TIME`,
`RENDERSIZE`, `isf_FragNormCoord`).

- **Specification:** [isf.video](https://isf.video/) / [docs.isf.video](https://docs.isf.video/)
- **GitHub:** [ISF_Spec](https://github.com/mrRay/ISF_Spec) / [ISF-Files](https://github.com/Vidvox/ISF-Files)

---

## Licensing Notes

| Component | License | Commercial use? |
|---|---|---|
| Hash functions (Hoskins) | MIT | Yes |
| Voronoi algorithm | Mathematical technique (not copyrightable) | Yes |
| HSV conversion (Hocevar) | WTFPL | Yes |
| Value noise | Mathematical technique | Yes |
| ISF format | Open specification | Yes |

All components are permissively licensed or uncopyrightable mathematical
techniques. Attribution is maintained because it's the right thing to do.
