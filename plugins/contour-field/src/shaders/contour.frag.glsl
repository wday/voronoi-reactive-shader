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
// driven live by the Resolume plugin. The terrain (hash / noise / fbm / height)
// is pulled from the shared finding-ground-common/terrain.glsl via the //#include
// directive below (expanded by the harness/render tool and the Rust plugin).

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
uniform float u_invert;        // 0 dark-on-light, 1 light-on-dark
uniform float u_contrast;      // tone separation (0 soft, 0.5 neutral, 1 punchy)
uniform float u_drift_x;       // cyclic drift rate, X axis (0 = still)
uniform float u_drift_y;       // cyclic drift rate, Y axis (0 = still)
uniform float u_time;          // free-running clock (static in harness, live in Resolume)

const float TAU = 6.28318530718;

// Shared terrain (hash / vnoise / fbm / height) — single source of truth.
//#include "../../../finding-ground-common/terrain.glsl"

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
    vec2 p = (uv - center) * zoom + drift_offset(u_time, u_drift_x, u_drift_y);

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

    // Ink on paper — warmth / contrast / invert handled centrally in paint().
    float tint = 0.06 * (h - 0.5);   // faint elevation relief
    out_color = vec4(paint(ink, u_warmth, u_contrast, u_invert, tint), 1.0);
}
