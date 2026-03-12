# IFS Fractal Toolkit — Dev Log

## 2026-03-11 — Requirements and plan

Decided against building a monolithic IFS plugin. The existing modular architecture (delay-line + mirror-transform) already provides the primitives for IFS — each Resolume layer acts as one "copy" of the iterated function. Only missing piece: X/Y translation in mirror-transform.

Deliverables: translation params on mirror-transform, logistic-feedback plugin, channel-displace plugin, and documented example compositions.

## 2026-03-11 — Logistic feedback plugin added

Added logistic-feedback plugin to scope. The logistic map `x = r*x*(1-x)` applied per-channel gives bifurcation control over feedback dynamics with one knob (R). Spatial modulation of r via built-in Sobel edge detection creates structured chaos — stable flat regions, chaotic edges that dissolve through feedback iterations. Radial mode renders the bifurcation diagram as concentric rings. Plugin is stateless (no FBO) — the delay-line provides temporal iteration.

Added channel-displace plugin to scope. Cross-channel UV displacement — R sampled at offset driven by G, etc. Creates strange attractor dynamics through feedback by introducing variable coupling (the third ingredient of chaos alongside iteration and nonlinearity). Cyclic and mutual coupling patterns. Small displacement range (0–10% UV) compounds through feedback iterations.
