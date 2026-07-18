#version 150
// Dendritic Network — Finding Ground P2.
//
// A GENERATOR: draws the river/watershed network of the SAME procedural terrain
// P1 Contour draws — so rivers run down its valleys. The channel network is
// found by drainage convergence (D8-style hydrology): a point is a channel where
// the downhill flow of the surrounding terrain funnels INTO it. Convergence
// branches naturally → a dendritic tree, main channels thickening downstream.
// Ignores any input image (generator).
//
// Aesthetic (Gallardo, Finding Ground): the extracted line-logic of rivers,
// carried with hand imperfection (meander, weight variation, breaks).
//
// Harness notes: same contract as contour.frag.glsl (v150, v_uv/out_color,
// u_texel_size fed, u_time static in harness / live in Resolume). The terrain is
// pulled from the shared finding-ground-common/terrain.glsl (//#include below).

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;     // ignored (generator)
uniform vec2  u_texel_size;    // (1/w, 1/h)

uniform float u_scale;         // terrain zoom (shares P1's field)
uniform float u_warp;          // domain-warp amount
uniform float u_density;       // channel threshold: high → only main rivers, low → many tributaries
uniform float u_thickness;     // base line weight
uniform float u_hierarchy;     // how much lower elevation fattens the main channels
uniform float u_jitter;        // hand-imperfection: meander
uniform float u_breakup;       // incompleteness: lift the pen on segments
uniform float u_warmth;        // palette: cool ink ↔ warm earth-ink
uniform float u_time;          // drift (static in harness, live in Resolume)

const float TAU = 6.28318530718;

// Shared terrain (hash / vnoise / fbm / height) — single source of truth.
//#include "../../../finding-ground-common/terrain.glsl"

// Valley-line (thalweg) extraction from the terrain's Hessian. A river runs
// along the valley floor, where the surface curves UP across the valley (large
// positive principal curvature) AND we sit at the bottom of that cross-section
// (slope along the across-valley direction ≈ 0). Both together isolate a thin,
// CONNECTED line — unlike single-step convergence, which only marks pits.
// `density` lowers the curvature bar so more tributaries qualify.
float valley_line(vec2 p, float e, float density) {
    float h0  = height(p);
    float hpx = height(p + vec2(e, 0.0)), hmx = height(p - vec2(e, 0.0));
    float hpy = height(p + vec2(0.0, e)), hmy = height(p - vec2(0.0, e));
    float hpp = height(p + vec2(e, e)),   hpm = height(p + vec2(e, -e));
    float hmp = height(p + vec2(-e, e)),  hmm = height(p + vec2(-e, -e));

    // Hessian (÷e² → curvature) and gradient (÷e → slope).
    float inv = 1.0 / (e * e);
    float hxx = (hpx - 2.0 * h0 + hmx) * inv;
    float hyy = (hpy - 2.0 * h0 + hmy) * inv;
    float hxy = (hpp - hpm - hmp + hmm) * 0.25 * inv;
    vec2  g   = vec2(hpx - hmx, hpy - hmy) * (0.5 / e);

    // Largest eigenvalue of the Hessian = max curvature (across the valley).
    float tr   = hxx + hyy;
    float det  = hxx * hyy - hxy * hxy;
    float disc = sqrt(max(tr * tr * 0.25 - det, 0.0));
    float lmax = tr * 0.5 + disc;
    vec2  evec = normalize(vec2(hxy, lmax - hxx) + vec2(1e-7));  // across-valley axis
    float across_slope = abs(dot(g, evec));

    float cmin    = mix(9.0, 3.0, density);                 // curvature threshold (high → sparse)
    float concave = smoothstep(cmin, cmin + 2.0, lmax);     // concave-up across
    float floorln = 1.0 - smoothstep(0.10, 0.45, across_slope); // thin: only the exact thalweg
    return concave * floorln;
}

void main() {
    float aspect = (u_texel_size.x > 0.0) ? (u_texel_size.y / u_texel_size.x) : 1.0;
    vec2 uv = vec2(v_uv.x * aspect, v_uv.y);

    // Same terrain + centre-anchored zoom as P1, so rivers register with contours.
    float zoom = mix(1.5, 9.0, u_scale);
    vec2 center = vec2(aspect, 1.0) * 0.5;
    vec2 p = (uv - center) * zoom + vec2(0.13, 0.47) * u_time;

    // Hand meander: smooth low-freq displacement of the sample point.
    vec2 jit = (vec2(vnoise(p * 2.5 + 11.3), vnoise(p * 2.5 + 37.1)) - 0.5) * (u_jitter * 0.15);
    vec2 pp = p + jit;

    float h    = height(pp);
    float e    = 0.05;                     // coarse curvature probe → major valleys, not every wrinkle
    float chan = valley_line(pp, e, u_density);

    // Rivers collect in the LOW terrain — gate the network to the lowlands, and
    // let lower elevation carry the network deeper (downstream hierarchy).
    float lowland = 1.0 - smoothstep(0.42, 0.60, h);
    float hier    = mix(1.0, lowland, u_hierarchy);
    float ink     = clamp(chan * mix(0.7, 1.3, u_thickness) * hier, 0.0, 1.0);
    // Harden to crisp ink-on-paper: kill the grey partial-coverage haze so the
    // network reads as drawn line-work, not a smudge.
    ink = smoothstep(0.18, 0.5, ink);

    // Incompleteness: break the pen along the channel.
    float seg = hash1(floor(pp * 8.0) + floor(h * 20.0));
    ink *= step(u_breakup * 0.9, seg);

    vec3 paper = mix(vec3(0.90, 0.89, 0.86), vec3(0.93, 0.88, 0.80), u_warmth);
    vec3 inkc  = mix(vec3(0.10, 0.11, 0.13), vec3(0.16, 0.10, 0.06), u_warmth);
    vec3 col   = mix(paper, inkc, ink);
    out_color = vec4(col, 1.0);
}
