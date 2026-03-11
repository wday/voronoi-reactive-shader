# Mirror Transform — Dev Log

## 2026-03-11 — v1 implementation

Extracted the scale/rotate/swirl transform from the dream looper's ingest shader into a standalone FFGL effect. The dream looper applies these transforms internally on ingest, but the delay-line-module has no spatial transforms — chaining this effect between Send/Receive gives the same compound feedback transforms.

### What shipped
- 4 params: Scale (exponential), Rotation, Swirl, Mirror (on/off)
- Fragment shader: center → scale → swirl → rotate → uncenter → edge handle → sample
- Mirror mode: kaleidoscope fold via `1 - abs(mod(uv, 2) - 1)`
- Off mode: soft-clip fade to black at edges
- GL state save/restore (scissor, blend, depth)
- Registered in `plugins.json`

### Notes
- No FBO needed — single pass, stateless, renders into host FBO
- Shader compiled lazily on first draw to avoid GL context issues
- Plugin ID: `MrTx`
