# Development Log — Normalized Convolution Image Input

## 2026-03-06 — Feature inception

### Context
Idea surfaced during discussion about deconvolution. Recalled Knutsson & Westin 1993 normalized convolution algorithm — random sparse sampling + Gaussian blur + normalization to reconstruct continuous fields from incomplete data.

### Connection to voronoi shader
Current shader uses grid-aligned hash points (Worley noise). NC could replace this with:
- Truly irregular point placement (no grid artifacts)
- Soft/blended cell boundaries instead of hard F1/F2 edges
- Spatially varying density via certainty field
- GPU-friendly splat-blur-slice pipeline

### Decisions
- Downloaded reference paper to `docs/references/knutsson-westin-normalized-convolution-1993.pdf`
- Created feature directory with draft requirements
- Requirements still have significant open questions — need to clarify point source, output interpretation, and whether this adds image input capability

### Next
- Resolve open questions in requirements before planning implementation
