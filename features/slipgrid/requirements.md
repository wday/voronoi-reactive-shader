# Slipgrid — Requirements

## Goal

A stateless **tile permutation** atom. Break the frame into an `nx × ny` grid and
rearrange the tiles. The rearrangement can be bent by the image's own **edges**,
so the scramble stops being uniform noise and starts organising itself around the
structure already in the picture.

## Motivation

Tile shuffles are a stock glitch move and they read as *flat* — every tile is as
likely to move as every other, and the result is uniform hash. The interesting
version is a scramble that **knows where the picture is**. Slipgrid runs a cheap
edge detector internally and uses it to decide which rearrangements are worth
making: bright tiles get drawn onto contours, and the scramble accretes into a
rough, blocky sketch of the source's structure.

It's also the natural spatial complement to the existing feedback atoms: pointed
at a feedback loop, the accreted tiles become the next frame's edges, which
re-steer the next permutation.

## Design decisions (locked 2026-07-12)

| Decision | Choice | Rationale |
|---|---|---|
| **Form factor** | FFGL Rust plugin | Wanted live in Resolume directly. Follows the `channel-displace` self-contained atom layout. |
| **Time behavior** | Frozen, seeded | Permutation is a pure function of `(seed, params)`. No `TIME`, no state, no per-frame re-shuffle. Turning Seed *is* the animation, and it's automatable. |
| **Mapping** | **Two modes** | `Conserve` (bijective swap) and `Smear` (loose displacement). They are genuinely different machines — see below. |
| **Gravity field** | Tile-scale edge energy | `E = |∇luma|` sampled with a **half-tile** stencil. Peaks on tiles that *straddle* a contour, ~0 on tiles wholly inside or outside one. |
| **Passes** | Single | Multi-pass FBO ping-pong was prototyped and measured; it did **not** beat single-pass (110–117 vs 112 on the accretion metric). Not worth the FBOs. |

### The two modes, and why both exist

**Conserve** — tiles are paired and **swapped**, so the permutation is a true
bijection: every source tile lands exactly once, nothing duplicated, nothing lost.
This is the only mode in which Edge Gravity means anything. Attraction is a claim
about a *finite supply* of tiles piling up somewhere; a mapping that doesn't
conserve tiles cannot express it.

**Smear** — each output tile pulls from a walked source tile. Tiles duplicate,
holes open up. A chaotic glitch/smear look, good in a feedback loop. **Edge
Gravity steers the walk here but cannot accrete** — unconserved content leaks
across the frame instead of accumulating. Kept as an aesthetic option, not as a
second way of doing gravity.

This was established empirically, not assumed — see `devlog.md`. At full gravity
on the test image, Smear-style formulations score *below* the do-nothing baseline
on the accretion metric and collapse the frame to 9–18 distinct tiles out of 64.

## Behavior

### Conserve (bijective swap)

Per round (`Iterations` rounds total):

1. The round picks **one global axis, hop distance `d ≤ Locality`, and phase**,
   all hashed from `(round, Seed)`. These tile the grid into dominoes, so a tile
   and its partner always agree on who they're paired with — **mutual by
   construction**, which is what makes the round a true involution.
2. Wrapping can break the pairing when the axis length isn't a multiple of `2d`;
   mutuality is verified and leftovers sit out the round.
3. `Intensity` gates each pair (keyed on the pair, order-independent).
4. **Acceptance**: swap if it puts the brighter tile on the edgier one —
   `sign(ΔL · ΔE · gravity) > 0`. This expression is *symmetric* in the pair
   (negating both differences leaves the product fixed), so both tiles reach the
   same verdict without communicating. Gravity is never throttled by disagreement.
   - `gravity = 0` → accept everything (plain random shuffle)
   - `gravity = ±1` → accept only edge-favourable swaps (pure sorting)

### Smear (loose displacement)

Gated by `Intensity`, each tile takes `Iterations` random hops of up to `Locality`
tiles; Edge Gravity bends the hop direction toward `∇E`. Wraps at the borders.

### Both

Tile *contents* are never resampled or rotated — the within-tile coordinate is
carried through untouched, so a moved tile is a pixel-exact copy of its source.
That crispness is what makes it read as *permutation* rather than warp.
Final line is the house convention: `mix(original, slipped, Dry/Wet)`.

## Parameters

| # | Name | Type | Range (host 0..1 → internal) | Default | Notes |
|---|---|---|---|---|---|
| 0 | Grid X | Standard | 1 → 64 tiles | 8 | Integer. |
| 1 | Grid Y | Standard | 1 → 64 tiles | 8 | Independent of X (non-square tiles are wanted). |
| 2 | Intensity | Standard | 0 → 1 | 0.5 | Conserve: fraction of *pairs* that swap. Smear: fraction of *tiles* that move. |
| 3 | Locality | Standard | 0 → 16 tiles | 2 | Max swap distance / hop radius. |
| 4 | Edge Gravity | Standard | 0..1 → −1..+1 | 0 (centre) | Bipolar. +1 draws bright tiles onto contours; −1 drives them off. |
| 5 | Iterations | Standard | 1 → 8 rounds | 3 | Integer. |
| 6 | Mode | Option | Conserve / Smear | Conserve | See above. |
| 7 | Seed | Standard | 0 → 1024 | 0 | Re-rolls the permutation. The automation handle. |
| 8 | Dry/Wet | Standard | 0 → 1 | 1 | House convention. |

Plugin identity: `name = *b"Slipgrid        "` (16 bytes), `unique_id = *b"SlpG"`.

## Non-goals

- **No animation of its own.** No `TIME`, no per-frame reshuffle. Motion comes
  from automating Seed (or from what's upstream).
- **No tile rotation/flip.** Displacement only. Could be a later param.
- **No inter-frame state.** Stateless single pass, straight to the host FBO.

## Known limits

- **Edge energy sees contours, not texture.** The half-tile stencil measures
  coarse structure. Detail finer than ~half a tile (fine checkerboards, noise,
  hatching) reads as *flat* and will not attract. This is inherent to a 2-tap
  gradient at tile scale, and it bit during development — see `devlog.md`.
- **Rounds after the first decide on stale luminance.** A gather shader can't know
  where content moved to in a previous round, so rounds 2..N compare *source*
  luminance, not current. Measured against a correct multi-pass implementation,
  this costs nothing (112 vs 110–117), so it stands — but it means `Iterations`
  is not a true annealing schedule.

## Acceptance — all verified 2026-07-12 (see devlog)

- ✅ `Intensity = 0` or `Dry/Wet = 0` → output bit-identical to input, both modes.
- ✅ Conserve is a true bijection: 64/64 source tiles used, 0 lost, 0 duplicated.
- ✅ Conserve accretes: luminance on the shader's own edge tiles goes
  62.4 → 72.3 → **112.1** as gravity goes 0 → 0.5 → 1 (baseline 91.8, frame mean
  51.2). Gravity −1 → 57.0 (repels, as specced).
- ✅ Smear duplicates by design (14/64 tiles) — confirms the modes differ.
- ✅ Seed re-rolls the permutation (39.7% of pixels differ between seed 0 and 7).
- ✅ Non-square grids stay axis-aligned and pixel-exact.
- ✅ NPOT/non-square input handled via `u_uv_scale`.
- ⬜ **Live validation in Resolume** — built and deployed, not yet driven with real
  footage.
