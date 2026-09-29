#version 150
// Slipgrid — tile permutation with edge-seeking gravity.
//
// Two modes, because they are genuinely different machines:
//
//   CONSERVE — a true bijection. Tiles are paired and SWAPPED, so every source
//     tile lands exactly once: nothing duplicated, nothing lost. Only a
//     conserved permutation can express "attraction", because attraction means
//     a finite supply of tiles piling up somewhere.
//
//   SMEAR — loose displacement. Each output tile pulls from a walked source
//     tile, so tiles duplicate and holes open up. Chaotic, glitchy, good in a
//     feedback loop. Edge Gravity only steers the walk here; it CANNOT accrete
//     (unconserved content leaks everywhere instead of piling up).
//
// Hash functions: Dave Hoskins (sine-free)
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform vec2 u_uv_scale;      // Width/HardwareWidth — NPOT input region
uniform vec2 u_texel;         // 1/hardware_size, per axis

uniform vec2  u_grid;         // (nx, ny) tiles
uniform float u_intensity;    // 0..1 — how much of the grid participates
uniform float u_locality;     // max displacement / swap distance, in tiles
uniform float u_edge_gravity; // -1..1 — bipolar; +1 draws bright tiles to edges
uniform int   u_iterations;   // rounds
uniform int   u_mode;         // 0 = Conserve (swap), 1 = Smear (displace)
uniform float u_seed;
uniform float u_dry_wet;

const int MAX_ITERATIONS = 8;
const float TAU = 6.28318530718;

// --- Hash functions (Dave Hoskins, no sine) ---
float hash1(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 hash2(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// Sample content, clamped to stay half a texel inside the outer content texel
// centres. At the frame's far edge (uv->1) the naive uv*u_uv_scale reaches the
// content->padding boundary, where bilinear blends the last real row/col with the
// black hardware padding — a 1px dark seam at the top/bottom edge. Clamping keeps
// every sample on real content. (Same fix as mirror-transform's edges.)
vec4 sampleContent(vec2 uv) {
    vec2 s = clamp(uv * u_uv_scale, 0.5 * u_texel, u_uv_scale - 0.5 * u_texel);
    return texture(u_input, s);
}

float luma_at(vec2 uv) {
    return dot(sampleContent(uv).rgb, vec3(0.299, 0.587, 0.114));
}

float tile_luma(vec2 ti) {
    return luma_at((ti + 0.5) / u_grid);
}

// Edge energy, measured at TILE scale — the scale the tiles actually move in.
// Peaks on tiles that STRADDLE a contour; ~0 on tiles wholly inside or outside
// one. A pixel-scale stencil is useless here: probed a tile apart it reads zero
// almost everywhere and the permutation never sees the edge at all.
//
// This measures coarse CONTOURS, not texture busyness: structure finer than
// about half a tile falls between the taps and does not attract.
float edge_energy(vec2 ti) {
    vec2 uv = (ti + 0.5) / u_grid;
    vec2 h = 0.5 / u_grid;
    float gx = luma_at(uv + vec2(h.x, 0.0)) - luma_at(uv - vec2(h.x, 0.0));
    float gy = luma_at(uv + vec2(0.0, h.y)) - luma_at(uv - vec2(0.0, h.y));
    return length(vec2(gx, gy));
}

// Ascent direction on edge energy — only Smear needs this.
vec2 edge_gradient(vec2 ti) {
    vec2 uv = (ti + 0.5) / u_grid;
    vec2 h = 1.0 / u_grid;
    float ex = edge_energy(ti + vec2(1.0, 0.0)) - edge_energy(ti - vec2(1.0, 0.0));
    float ey = edge_energy(ti + vec2(0.0, 1.0)) - edge_energy(ti - vec2(0.0, 1.0));
    return vec2(ex, ey);
}

// ---------------------------------------------------------------- CONSERVE
// One round: pair the grid into dominoes and swap the pairs that improve the
// edge score. Pairing geometry is per-ROUND and global (axis, distance, phase),
// so a tile and its partner always agree on who they are paired with — mutual
// by construction, which is what makes this a true involution.
vec2 conserve_round(vec2 ti, float salt) {
    float ha = hash1(vec2(salt, u_seed));
    float hd = hash1(vec2(salt + 3.0, u_seed));
    float hp = hash1(vec2(salt + 7.0, u_seed));

    bool  use_x = ha < 0.5;
    float span  = use_x ? u_grid.x : u_grid.y;
    float d     = max(1.0, floor(hd * u_locality + 0.5));   // hop, in tiles
    float phase = floor(hp * 2.0 * d);

    float c = use_x ? ti.x : ti.y;

    // Blocks of 2d along the axis: the low half pairs UP, the high half pairs DOWN.
    float k    = mod(c - phase, 2.0 * d);
    float step = (k < d) ? d : -d;

    vec2 p = ti;
    if (use_x) p.x = mod(ti.x + step, u_grid.x);
    else       p.y = mod(ti.y + step, u_grid.y);

    // Wrapping breaks the pairing when the axis is not a multiple of 2d. Verify
    // mutuality; leftovers just sit out the round.
    float cp = use_x ? p.x : p.y;
    float kp = mod(cp - phase, 2.0 * d);
    float step_back = (kp < d) ? d : -d;
    if (abs(mod(cp + step_back - c, span)) > 0.5) return ti;

    // Both gates are keyed on the PAIR (order-independent), so the two tiles
    // reach the same verdict without communicating.
    vec2 lo = min(ti, p), hi = max(ti, p);
    if (hash1(lo + hi * 3.7 + u_seed + salt) >= u_intensity) return ti;

    // Acceptance: does the swap put the brighter tile on the edgier one?
    // Symmetric in (ti, p): negating both differences leaves the product fixed.
    float dL = tile_luma(p) - tile_luma(ti);
    float dE = edge_energy(ti) - edge_energy(p);
    float gain = dL * dE * sign(u_edge_gravity);

    // gravity 0 -> accept everything (plain random shuffle)
    // gravity 1 -> accept only edge-favourable swaps (pure sorting)
    float g = abs(u_edge_gravity);
    bool accept = (gain > 0.0) || (hash1(hi + lo * 3.7 + u_seed + salt + 91.0) > g);
    return accept ? p : ti;
}

// ------------------------------------------------------------------- SMEAR
// One hop of a random walk, optionally bent toward the edge field.
vec2 smear_hop(vec2 ti, float salt) {
    vec2 r = hash2(ti + u_seed + salt);
    float angle = r.x * TAU;
    vec2 dir = vec2(cos(angle), sin(angle));

    if (u_edge_gravity != 0.0) {
        vec2 ge = edge_gradient(ti);
        float mag = length(ge);
        if (mag > 1e-5) {
            vec2 g = (ge / mag) * sign(u_edge_gravity);
            vec2 blended = mix(dir, g, abs(u_edge_gravity));
            float bl = length(blended);
            dir = bl > 1e-5 ? blended / bl : g;   // mix can cancel; fall back to g
        }
    }
    return mod(ti + floor(dir * u_locality * r.y + 0.5), u_grid);
}

void main() {
    vec4 original = sampleContent(v_uv);

    vec2 tf    = v_uv * u_grid;
    vec2 ti    = floor(tf);   // home tile
    vec2 local = fract(tf);   // position within the tile — carried through untouched,
                              // so a displaced tile is a pixel-exact copy of its source

    if (u_mode == 0) {
        for (int i = 0; i < MAX_ITERATIONS; i++) {
            if (i >= u_iterations) break;
            ti = conserve_round(ti, float(i) * 17.0);
        }
    } else {
        // Intensity gates which tiles move at all; in Conserve it gates the pairs.
        if (hash1(ti + u_seed) < u_intensity) {
            for (int i = 0; i < MAX_ITERATIONS; i++) {
                if (i >= u_iterations) break;
                ti = smear_hop(ti, float(i) * 17.0);
            }
        }
    }

    // Snap the tile hop to a WHOLE number of content texels. The hop is
    // (ti_new - ti_home)/u_grid, which is only an integer texel count when the
    // grid divides the frame — grid 8 into 1920 is 240px exactly, grid 7 is
    // 274.3px, and then every hop lands at a fractional texel offset and bilinear
    // resamples the tile. That made the "pixel-exact copy" above false for most
    // grid values, and inside a feedback loop the resample compounds once per lap
    // into real blur. Snapping restores an exact copy for ANY grid: a whole-texel
    // fetch has zero loss, which beats any interpolation filter. Costs at most
    // half a texel of tile-boundary placement, which is invisible.
    vec2 content_texel = u_texel / u_uv_scale;              // one source texel, in uv
    vec2 hop = (ti - floor(tf)) / u_grid;                   // tile hop, in uv
    hop = round(hop / content_texel) * content_texel;       // -> whole texels
    vec2 src = v_uv + hop;
    vec4 slipped = sampleContent(src);

    out_color = mix(original, slipped, u_dry_wet);
}
