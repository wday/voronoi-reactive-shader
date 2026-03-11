# Mirror Transform — Requirements

## Status: v1 complete

## Concept

A standalone FFGL effect that applies spatial transforms (scale, rotation, swirl) around the frame center, with kaleidoscope mirror or soft-clip edge handling. Extracted from the video-looper-ltm-dream ingest shader's transform pipeline.

### Why standalone?

The delay-line-module handles temporal delay but has no spatial transforms. Resolume's native Transform effect doesn't offer kaleidoscope edge folding — out-of-bounds content is either clamped or shows black. This effect fills that gap: when content is scaled down through feedback iterations, the mirror fold fills the frame with reflections instead of black borders.

### Use case

Chain with delay-line-module in Resolume's effect stack:
- Between Receive and Send: transforms compound through feedback
- On Tap layers: one-shot spatial transform of delayed echoes
- Standalone: kaleidoscope/swirl effect on any source

## Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Scale | Standard | 0.0–1.0 → 0.5×–2.0× | 0.5 (1.0×) | Exponential: 2^(v*2-1) |
| 1 | Rotation | Standard | 0.0–1.0 → -180°–+180° | 0.5 (0°) | Per-frame rotation |
| 2 | Swirl | Standard | 0.0–1.0 → -2.0–+2.0 rad | 0.5 (0) | Angular displacement ∝ distance from center |
| 3 | Mirror | Option | Off / On | Off | Kaleidoscope fold vs soft-clip edges |

## Shader

Single-pass fragment shader. Transform pipeline:
1. Center UV at (0.5, 0.5)
2. Apply scale (multiply centered UV)
3. Apply swirl (angular displacement proportional to radius)
4. Apply rotation
5. Uncenter UV + apply edge handling (mirror fold or soft clip)
6. Sample input at transformed UV

## Constraints

- FFGL 2.1 effect plugin
- Windows DLL target (cross-compiled from WSL)
- Must save/restore host GL state
- No GPU buffers — single pass, stateless
