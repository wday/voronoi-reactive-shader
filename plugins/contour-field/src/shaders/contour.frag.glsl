#version 150
// Contour Field — Finding Ground P1.
//
// A GENERATOR: draws the iso-contour lines of a procedural height field, read as
// a topographic map / valley system. Ignores any input image entirely (out_color
// is synthesized from uv + time + params). This is the "ground" layer — the
// terrain whose line-logic P2 (rivers) and P3 (parcels) will later share.
//
// Aesthetic target (Gallardo, Finding Ground): the extracted line-logic of
// valleys, carried with a HAND IMPERFECTION — lines wobble in position and
// weight and occasionally break, so the map never reads as sterile CAD.
//
// Harness notes: the shader viewer discovers `uniform float`s as adjustable
// controls (0.5 = FFGL-neutral midpoint unless overridden by contour.defaults.json);
// u_texel_size is fed as (1/w, 1/h); u_time is left static in the harness but
// driven live by the Resolume plugin. No #include — the terrain block below is
// inlined for now and will be factored into a shared terrain.glsl (concatenated
// by the Rust plugin) when P2 Dendritic reuses it.
//
// Hash / value-noise: Dave Hoskins style (sine-free), matching the repo house style.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;     // ignored (generator) — declared so the harness binds cleanly
uniform vec2  u_texel_size;    // (1/w, 1/h); aspect = y/x

uniform float u_scale;         // terrain zoom (world frequency)
uniform float u_warp;          // domain-warp amount → organic, meandering valleys
uniform float u_contours;      // contour density (how many iso-bands across the range)
uniform float u_line_weight;   // line thickness
uniform float u_elevation;     // scrub the elevation origin (which heights get lines)
uniform float u_jitter;        // hand-imperfection: line wobble + weight variation
uniform float u_breakup;       // incompleteness: erase random line segments (pencil lift)
uniform float u_warmth;        // palette: cool grey-ink ↔ warm earth-ink on paper
uniform float u_time;          // animation phase (static in harness, live in Resolume)

const float TAU = 6.28318530718;

// --- Hash (Dave Hoskins, no sine) ---
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

// --- value noise + fbm (the shared terrain, inlined) ---
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);            // smoothstep interpolant
    float a = hash1(i + vec2(0.0, 0.0));
    float b = hash1(i + vec2(1.0, 0.0));
    float c = hash1(i + vec2(0.0, 1.0));
    float d = hash1(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    float freq = 1.0;
    for (int i = 0; i < 5; i++) {
        sum += amp * vnoise(p * freq);
        freq *= 2.0;
        amp *= 0.5;
    }
    return sum;                                   // ~[0,1]
}

// Height field: fbm domain-warped by a second fbm → sinuous ridges and valleys
// instead of round blobs. This is the terrain P2/P3 will eventually share.
float height(vec2 p) {
    vec2 q = vec2(fbm(p + vec2(0.0, 0.0)),
                  fbm(p + vec2(5.2, 1.3)));
    float w = mix(0.0, 1.4, u_warp);
    return fbm(p + w * q);
}

void main() {
    // Square up the domain so terrain isn't stretched by frame aspect.
    float aspect = (u_texel_size.x > 0.0) ? (u_texel_size.y / u_texel_size.x) : 1.0;
    vec2 uv = vec2(v_uv.x * aspect, v_uv.y);

    // World position. u_scale 0.5 → ~4 octaves of terrain across the frame.
    // Scale about the frame CENTRE, not the (0,0) corner, so modulating Scale
    // zooms symmetrically in place instead of smearing the terrain diagonally
    // out of the top-left. (Drift is a separate constant translation.)
    float zoom = mix(1.5, 9.0, u_scale);
    vec2 center = vec2(aspect, 1.0) * 0.5;
    vec2 p = (uv - center) * zoom + vec2(0.13, 0.47) * u_time;   // slow geological drift

    // Hand-imperfection: displace the sample point by SMOOTH low-frequency noise
    // so contours meander like a drawn line. (A floored-cell hash here instead
    // makes discontinuous height jumps that clump into ragged black speckle —
    // the noise must be continuous to read as wobble rather than damage.)
    vec2 jit = (vec2(vnoise(p * 2.5 + 11.3), vnoise(p * 2.5 + 37.1)) - 0.5) * (u_jitter * 0.18);
    float h = height(p + jit);

    // Elevation scrub shifts which heights land on a contour line.
    h += (u_elevation - 0.5) * 0.5;

    // --- Iso-contour extraction ---
    // Bands at multiples of `interval`; distance to the nearest band edge, made
    // resolution-independent with fwidth so line weight is stable at any zoom.
    float bands    = mix(4.0, 32.0, u_contours);
    float interval = 1.0 / bands;
    float f  = h / interval;
    float d  = abs(fract(f) - 0.5);               // 0 at a line, 0.5 between lines
    float aa = fwidth(f) + 1e-5;                  // per-pixel band-space gradient

    // Line weight (with a touch of per-line jitter in the weight itself — pencil
    // pressure). Capped so neighbouring contours can never fatten into each other
    // and fill solid black in steep zones; the cap is what keeps this line-work
    // rather than marbling.
    float wobble = 1.0 + (hash1(vec2(floor(f), 3.1)) - 0.5) * u_jitter * 0.5;
    float lw = min(mix(0.02, 0.20, u_line_weight) * wobble, 0.18);
    float line = 1.0 - smoothstep(lw - aa, lw + aa, d);

    // Resolution guard: where terrain is steep, bands pack sub-pixel and the
    // naive line fills solid black. Fade ink out as the band-space gradient (aa)
    // approaches the half-band limit, so lines are drawn only where the contour
    // logic is legible — delicate line-work, not marbled fills. Very Gallardo.
    line *= 1.0 - smoothstep(0.30, 0.50, aa);

    // Incompleteness: lift the pencil on random segments along each contour.
    // Keyed by (band index, position along the contour) so gaps travel the line.
    float seg = hash1(vec2(floor(f), floor(length(p) * 6.0 + h * 20.0)));
    float ink = line * step(u_breakup * 0.9, seg);

    // --- Palette: ink on paper ---
    vec3 paper = mix(vec3(0.90, 0.89, 0.86), vec3(0.93, 0.88, 0.80), u_warmth);
    vec3 inkc  = mix(vec3(0.10, 0.11, 0.13), vec3(0.16, 0.10, 0.06), u_warmth);
    // Faint elevation tint on the paper so bands read as a subtle relief.
    float tint = 0.06 * (h - 0.5);
    paper -= tint;

    vec3 col = mix(paper, inkc, clamp(ink, 0.0, 1.0));
    out_color = vec4(col, 1.0);
}
