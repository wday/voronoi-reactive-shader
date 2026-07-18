#version 150
// Parcel Subdivision — Finding Ground P3.
//
// A GENERATOR and the hard-geometric counterweight to P1/P2: the measured,
// divided ground. A regular survey grid (townships / graticule) over the SAME
// terrain, each township recursively split into land parcels (binary space
// partition). The LOWLANDS subdivide deeper — contested valley land, "conflicts
// of use" — so the parcels register with P2's rivers and P1's valleys.
// Ignores any input image (generator).
//
// Harness contract: v150, v_uv/out_color, u_texel_size fed, u_time static in
// harness / live in Resolume.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;     // ignored (generator)
uniform vec2  u_texel_size;

uniform float u_scale;         // zoom (shares P1/P2 terrain space)
uniform float u_warp;          // terrain domain-warp (read by height())
uniform float u_depth;         // max subdivision levels
uniform float u_regularity;    // 0 → regular half-splits, 1 → irregular parcels
uniform float u_border;        // parcel line weight
uniform float u_inset;         // gap between parcels
uniform float u_jitter;        // hand-imperfection: border wobble
uniform float u_warmth;        // palette
uniform float u_time;          // drift

const int MAXD = 8;

// Shared terrain (hash / vnoise / fbm / height) — single source of truth.
//#include "../../../finding-ground-common/terrain.glsl"

void main() {
    float aspect = (u_texel_size.x > 0.0) ? (u_texel_size.y / u_texel_size.x) : 1.0;
    vec2 uv = vec2(v_uv.x * aspect, v_uv.y);

    // Same centre-anchored, zoomed terrain space as P1/P2.
    float zoom = mix(1.5, 9.0, u_scale);
    vec2 center = vec2(aspect, 1.0) * 0.5;
    vec2 p = (uv - center) * zoom + vec2(0.13, 0.47) * u_time;

    // Survey townships = integer cells of world space; f is position within one.
    vec2 cellId = floor(p);
    vec2 f      = fract(p);

    // Terrain at the township centre → deeper subdivision in the lowlands.
    float hc = height(cellId + 0.5);
    float terrainDepth = mix(1.0, 0.35, smoothstep(0.35, 0.65, hc)); // low→1, high→0.35
    int depth = int(floor(mix(1.0, float(MAXD), u_depth) * terrainDepth + 0.5));

    // Hand-imperfection: wobble the sample so borders meander a touch.
    vec2 wob = (vec2(vnoise((cellId + f) * 3.0 + 11.3),
                     vnoise((cellId + f) * 3.0 + 37.1)) - 0.5) * (u_jitter * 0.03);
    vec2 fp = f + wob;

    // Binary space partition of the township [0,1]² down to the parcel holding fp.
    vec2 lo = vec2(0.0), hi = vec2(1.0);
    float seed = hash1(cellId + 0.123);
    for (int i = 0; i < MAXD; i++) {
        if (i >= depth) break;
        vec2 sz = hi - lo;
        bool splitX = sz.x > sz.y;                          // split the longer axis
        float t = mix(0.5, 0.25 + 0.5 * hash1(lo + hi + vec2(seed, float(i))), u_regularity);
        if (splitX) {
            float s = mix(lo.x, hi.x, t);
            if (fp.x < s) hi.x = s; else lo.x = s;
        } else {
            float s = mix(lo.y, hi.y, t);
            if (fp.y < s) hi.y = s; else lo.y = s;
        }
    }

    // Inset the parcel → a gap between neighbours. Signed distance to the parcel
    // edge: >0 inside, <0 in the gap.
    float inset = u_inset * 0.18 * min(hi.x - lo.x, hi.y - lo.y);
    vec2 ilo = lo + inset, ihi = hi - inset;
    float dpar = min(min(fp.x - ilo.x, ihi.x - fp.x),
                     min(fp.y - ilo.y, ihi.y - fp.y));

    float aa = (fwidth(fp.x) + fwidth(fp.y)) * 0.5 + 1e-5;
    float bw = mix(0.004, 0.03, u_border);
    // Parcel outline: ink in the thin band just inside the edge (dpar in [0,bw]);
    // the gap (dpar<0) stays paper.
    float parcel = (1.0 - smoothstep(bw, bw + aa, dpar)) * step(-aa, dpar);

    // Township graticule: the heaviest lines, on the integer world grid.
    float gdist = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    float grat  = 1.0 - smoothstep(bw * 1.6, bw * 1.6 + aa, gdist);

    float ink = clamp(max(parcel, grat), 0.0, 1.0);

    // Palette: ink on paper, faint elevation tint so parcels sit on the land.
    vec3 paper = mix(vec3(0.90, 0.89, 0.86), vec3(0.93, 0.88, 0.80), u_warmth);
    vec3 inkc  = mix(vec3(0.10, 0.11, 0.13), vec3(0.16, 0.10, 0.06), u_warmth);
    paper -= 0.05 * (hc - 0.5);
    vec3 col = mix(paper, inkc, ink);
    out_color = vec4(col, 1.0);
}
