#version 150
// Voronoi Fractal — two modes over one jittered-Voronoi core.
//
//   Layered  — three independent Voronoi layers blended by layerMix^i. A port
//              of shaders/voronoi_reactive.fs.
//   Fractal  — a hierarchy: each level's sites have a PARENT one level up, and
//              a pixel's colour comes from the level-0 cell its ancestry
//              reaches. Partition boundaries are resolved at the finest level's
//              scale, so they are fractal coastlines rather than polygon edges.
//              After Boris the Brave, "Fractal Jittered Voronoi Partitions".
//
// The parent link is biased by the input image (FV-COAST), so coastlines settle
// onto bright ridges — a watershed of the source.
//
// Harness contract: v150, v_uv/out_color, u_texel_size fed, u_anim_time static
// in the harness / driven by the host transport in Resolume.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform vec2  u_texel_size;
// Content fraction of the NPOT-padded input: Width/HardwareWidth. Resolume hands
// us a texture whose real content occupies only [0,u_uv_scale]; sampling raw
// [0,1] reads padding and shifts the source against the generated pattern.
uniform vec2  u_uv_scale;

uniform float u_fractal;          // 0 = Layered, 1 = Fractal
uniform float u_density;          // base grid scale
uniform float u_layer_spread;     // per-level scale ratio
uniform float u_layer_mix;        // layer blend / interior detail
uniform float u_depth;            // fractal levels, 1..6
uniform float u_coastline;        // image bias on the parent link
uniform float u_drift_chaos;      // circular <-> random walk
uniform float u_warp;             // spatial domain warp
uniform float u_edge_width;
uniform float u_edge_glow;
uniform float u_color_shift;
uniform float u_color_sat;
uniform float u_image_influence;  // master image gate
uniform float u_nc_kernel;
uniform float u_cert_contrast;    // gamma on source luminance
uniform float u_cert_brightness;  // tonal swing from the image
uniform float u_fill_level;       // cell interior level; 0 = black ground
uniform float u_brightness;
uniform float u_contrast;
uniform float u_image_blend;
uniform float u_anim_time;        // drift cycle count (1.0 = one full cycle)
uniform float u_beat_locked;      // 1 = seeds share one rate (bar-locked)

const float TAU = 6.28318530718;
const int   MAX_LEVELS = 6;
// Parent-search radius in cells. The reference implementation argues ±2 is
// required when jitter fills the whole square; our jitter is confined to
// [0.1,0.9] (FV-BOUND), so ±1 may suffice. Settled by measurement, not assumption.
#define PARENT_R 1
// FV-COAST domain-warp strength, in aspect-scaled uv per unit luminance gradient.
const float COAST_GAIN = 0.6;

float g_aspect;
vec2  g_center;
// Smooth local certainty at this pixel, written by the mode paths.
float g_cert = 0.0;


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

// Decorrelate a level's grid from every other level's.
vec2 level_salt(float level) {
    return vec2(level * 71.7, level * 37.3);
}


// --- Smooth value noise for the spatial warp ---

vec2 valueNoise2(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);

    vec2 a = hash2(i) - 0.5;
    vec2 b = hash2(i + vec2(1.0, 0.0)) - 0.5;
    vec2 c = hash2(i + vec2(0.0, 1.0)) - 0.5;
    vec2 d = hash2(i + vec2(1.0, 1.0)) - 0.5;

    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}


// --- HSV ---

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}


// --- Image certainty ---

// Map frame uv onto the padded texture's content, staying half a texel inside
// the outer content texel centres so LINEAR filtering never blends the black
// padding in (the recurring NPOT edge seam).
vec2 content_uv(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return 0.5 * texel + clamp(uv, 0.0, 1.0) * (u_uv_scale - texel);
}

// `uv` is aspect-scaled space; undo the stretch before sampling.
float imageCertaintyRaw(vec2 uv) {
    vec3 c = texture(u_input, content_uv(vec2(uv.x / g_aspect, uv.y))).rgb;
    return dot(c, vec3(0.299, 0.587, 0.114));
}

float imageCertainty(vec2 uv) {
    return pow(clamp(imageCertaintyRaw(uv), 0.0, 1.0), u_cert_contrast);
}

// Regional brightness: small cross-blur so the density field responds to
// structure, not pixel noise (keeps the grid rescale from shimmering).
float imageCertaintyBlur(vec2 uv, float r) {
    float s = imageCertaintyRaw(uv);
    s += imageCertaintyRaw(uv + vec2(r, 0.0));
    s += imageCertaintyRaw(uv - vec2(r, 0.0));
    s += imageCertaintyRaw(uv + vec2(0.0, r));
    s += imageCertaintyRaw(uv - vec2(0.0, r));
    return s / 5.0;
}


// --- Site positions ---
//
// FV-BOUND: jitter is confined to [0.1,0.9] of the cell, so a site never leaves
// its own cell. That is what makes a 3x3 search sufficient everywhere.

vec2 site_local(float level, vec2 cell) {
    return hash2(cell + level_salt(level)) * 0.8 + 0.1;
}

// Site position in the level's own grid coordinates.
vec2 site_grid(float level, vec2 cell) {
    return cell + site_local(level, cell);
}

// Site position in level-0 grid space, the common frame across levels.
vec2 site_l0(float level, vec2 cell) {
    return site_grid(level, cell) / pow(u_layer_spread, level);
}

// Level-0 grid space -> aspect-scaled uv.
vec2 l0_to_uv(vec2 p0) {
    return p0 / u_density + g_center;
}


// FV-COAST: bend the query domain along the image's luminance gradient before
// any grid lookup. Every level is viewed through the same warp, so the whole
// hierarchy bends coherently and root boundaries trace image structure. Sites
// themselves are untouched, so ancestry is unchanged and cannot tear
// (FV-SITEONLY holds trivially).
vec2 coast_warp(vec2 uv, float amount) {
    if (amount < 0.001) {
        return uv;
    }
    // Central differences on a wide stencil: we want regional structure, not
    // pixel noise.
    float e = 0.02;
    float gx = imageCertaintyRaw(uv + vec2(e, 0.0)) - imageCertaintyRaw(uv - vec2(e, 0.0));
    float gy = imageCertaintyRaw(uv + vec2(0.0, e)) - imageCertaintyRaw(uv - vec2(0.0, e));
    return uv + vec2(gx, gy) * amount * COAST_GAIN;
}


// --- Drift ---
//
// FV-SYNC: when bar-locked every seed shares one rate (the field lands together
// on the beat) and keeps only a random start phase. Free-running seeds keep the
// per-seed rate randomisation, which is what makes the motion organic.

vec2 drift_for(vec2 cellPos, float level, float cert) {
    vec2 seedHash = hash2(cellPos + level_salt(level));

    float rate = mix(0.3 + seedHash.y * 0.7, 1.0, u_beat_locked);
    float angle = TAU * seedHash.x + TAU * u_anim_time * rate;
    vec2 circular = 0.35 * vec2(cos(angle), sin(angle));

    float step = floor(u_anim_time);
    float b = fract(u_anim_time);
    b = b * b * (3.0 - 2.0 * b);
    vec2 rA = hash2(cellPos + vec2(step * 17.3, step * 7.1)) - 0.5;
    vec2 rB = hash2(cellPos + vec2((step + 1.0) * 17.3, (step + 1.0) * 7.1)) - 0.5;
    vec2 chaotic = mix(rA, rB, b) * 0.7;

    // High certainty anchors a seed; low certainty lets it wander.
    float driftScale = 1.0 - cert * 0.8 * u_image_influence;
    return mix(circular, chaotic, u_drift_chaos) * driftScale;
}


// Normalised-convolution certainty over the 3x3 seed neighbourhood.
//
// The kernel radius is in CELL units, matching the distances it weighs. It used
// to be multiplied by the grid scale, which at Fractal depth 4 (scale = density
// * spread^4) made the radius ~14 cells — far wider than the 9 seeds available,
// so every seed got equal weight and the certainty field was a flat blur. A
// 0.5-cell floor then blocked any sharpening. Small radius = certainty snaps to
// the voronoi cell that owns the pixel.
//
// Weights are taken relative to the nearest seed so the exponential cannot
// underflow to zero at small radii; the common factor cancels in the ratio.//
// FV-NCALIGN: the distance fed to the certainty field must be the DRIFTED one —
// the same distance that decides which cell a pixel belongs to. Weighing by the
// undrifted distance quantises the certainty field to a different Voronoi
// diagram than the one being drawn, offset by the drift (a static 0.35 cells
// even at Drift Speed 0). The image then reads through softly, out of register
// with the visible cell boundaries, no matter how sharp the kernel is.

float nc_certainty(float certs[9], float d2s[9], float dmin2) {
    float kr = max(u_nc_kernel, 0.02);
    float inv = 1.0 / (2.0 * kr * kr);
    float wsum = 0.0;
    float csum = 0.0;
    for (int k = 0; k < 9; k++) {
        float w = exp(-(d2s[k] - dmin2) * inv);
        csum += certs[k] * w;
        wsum += w;
    }
    return wsum > 1e-6 ? csum / wsum : 0.0;
}


// ===================== Layered mode =====================

// Returns vec4(F1, F2, cellID.x, cellID.y) and writes g_cert.
vec4 voronoiLayer(vec2 uv, float scale, float level) {
    float localBright = imageCertaintyBlur(uv, 0.012);

    // Dark areas -> tighter cells, bright areas -> larger cells. Per-fragment,
    // which is fine here: Layered mode has no ancestry to tear (FV-SITEONLY).
    float densityMult = mix(1.0, mix(1.8, 0.5, localBright), u_image_influence);
    float modScale = scale * densityMult;

    // Scale about the frame centre, not the (0,0) corner.
    vec2 p = (uv - g_center) * modScale;
    vec2 cell = floor(p);
    vec2 localP = fract(p);

    float f1 = 10.0;
    float f2 = 10.0;
    vec2 nearestCell = vec2(0.0);

    float certs[9];
    float d2s[9];
    float dmin2 = 1e9;
    int idx = 0;

    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 neighbor = vec2(float(i), float(j));
            vec2 cellPos = cell + neighbor;
            vec2 seedBase = site_local(level, cellPos);

            vec2 seedUV = (cellPos + seedBase) / modScale + g_center;
            float cert = imageCertainty(seedUV);

            vec2 point0 = neighbor + seedBase;
            vec2 point = point0 + drift_for(cellPos, level, cert);
            float dist = length(point - localP);

            // FV-NCALIGN: same distance as the cell assignment below.
            certs[idx] = cert;
            d2s[idx] = dist * dist;
            dmin2 = min(dmin2, d2s[idx]);
            idx++;

            if (dist < f1) {
                f2 = f1;
                f1 = dist;
                nearestCell = cellPos;
            } else if (dist < f2) {
                f2 = dist;
            }
        }
    }

    g_cert = nc_certainty(certs, d2s, dmin2);
    return vec4(f1, f2, nearestCell);
}


// ===================== Fractal mode =====================

// Parent links use plain distance. Biasing this cost by the image was tried and
// measured a failure: see FV-COAST in requirements.md. A chain's root is always
// a level-0 cell, and the level-0 sites are fixed and image-independent, so
// biasing links only reassigns cells near ties — the partition stays the level-0
// Voronoi diagram. Enrichment saturated at 1.1x for gains from 8 to 150 and for
// both +/-1 and +/-2 search. The image moves the domain instead (coast_warp).
float link_cost(vec2 childL0, vec2 parentL0) {
    return length(parentL0 - childL0);
}

// The level-(level-1) cell owning the given level-`level` cell.
//
// FV-LINKSTABLE: computed from UNDRIFTED site positions. Drifted links would
// flip as sites cross tie-lines and whole partitions would pop between hues.
vec2 parent_cell(float level, vec2 cell) {
    vec2 childL0 = site_l0(level, cell);

    float parentLevel = level - 1.0;
    float parentScale = pow(u_layer_spread, parentLevel);
    vec2 pc = floor(childL0 * parentScale);

    float best = 1e9;
    vec2 bestCell = pc;

    for (int j = -PARENT_R; j <= PARENT_R; j++) {
        for (int i = -PARENT_R; i <= PARENT_R; i++) {
            vec2 cand = pc + vec2(float(i), float(j));
            float c = link_cost(childL0, site_l0(parentLevel, cand));
            if (c < best) {
                best = c;
                bestCell = cand;
            }
        }
    }
    return bestCell;
}

// FV-ROOT: walk a finest-level cell up to the level-0 cell it belongs to.
vec2 root_cell(int levels, vec2 cell) {
    for (int l = MAX_LEVELS; l >= 1; l--) {
        if (l > levels) continue;
        cell = parent_cell(float(l), cell);
    }
    return cell;
}

// Returns vec4(F1, F2, .zw unused); writes the two nearest finest cells and
// g_cert through the out params.
vec4 fractalNearest(vec2 uv, float level, out vec2 nearest, out vec2 second) {
    float scale = u_density * pow(u_layer_spread, level);

    vec2 p = (uv - g_center) * scale;
    vec2 cell = floor(p);
    vec2 localP = fract(p);

    float f1 = 10.0;
    float f2 = 10.0;
    nearest = cell;
    second = cell;

    float certs[9];
    float d2s[9];
    float dmin2 = 1e9;
    int idx = 0;

    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 neighbor = vec2(float(i), float(j));
            vec2 cellPos = cell + neighbor;
            vec2 seedBase = site_local(level, cellPos);

            // FV-SITEONLY: certainty read at the site, never at the fragment.
            vec2 seedUV = (cellPos + seedBase) / scale + g_center;
            float cert = imageCertainty(seedUV);

            vec2 point0 = neighbor + seedBase;
            vec2 point = point0 + drift_for(cellPos, level, cert);
            float dist = length(point - localP);

            // FV-NCALIGN: same distance as the cell assignment below.
            certs[idx] = cert;
            d2s[idx] = dist * dist;
            dmin2 = min(dmin2, d2s[idx]);
            idx++;

            if (dist < f1) {
                f2 = f1;
                second = nearest;
                f1 = dist;
                nearest = cellPos;
            } else if (dist < f2) {
                f2 = dist;
                second = cellPos;
            }
        }
    }

    g_cert = nc_certainty(certs, d2s, dmin2);
    return vec4(f1, f2, 0.0, 0.0);
}


// --- Shared shading ---

vec3 shade(float hue, float edgeDist, float edgeScale) {
    // FV-TONEGATE: fills and edges share one gate, so Cert Brightness moves both.
    float toneGate = u_cert_brightness * u_image_influence;

    // Fill Level is the interior/background level. It must be a knob, not the
    // old hardcoded 0.55: the Contrast stretch pivots at 0.5, so a fill sitting
    // at 0.55 is pinned to the pivot and Contrast can never drive it to black.
    float cellValue = mix(u_fill_level, g_cert, toneGate);
    vec3 cellRGB = hsv2rgb(vec3(hue, u_color_sat, cellValue));

    float w = max(u_edge_width, 0.001);
    float edgeFactor = 1.0 - smoothstep(0.0, w, edgeDist);
    float glowRange = max(w * (1.0 + u_edge_glow * 4.0), 0.001);
    float glowFactor = (1.0 - smoothstep(0.0, glowRange, edgeDist)) * u_edge_glow;
    // FV-EDGEGATE: edge presence scales with local certainty as Image Influence
    // comes up, so edges fade out over dark ground and survive only on lit
    // subjects. With Fill Level at 0 this isolates fractal outlines of whatever
    // the image actually contains. Cert Contrast sets how hard the cut is.
    float edgeGain = mix(1.0, g_cert, u_image_influence);
    float totalEdge = clamp(max(edgeFactor, glowFactor), 0.0, 1.0) * edgeScale * edgeGain;

    float edgeBright = mix(1.0, mix(0.15, 1.0, g_cert), toneGate);
    vec3 edgeRGB = hsv2rgb(vec3(hue, u_color_sat * 0.2, edgeBright));

    return mix(cellRGB, edgeRGB, totalEdge);
}


void main() {
    g_aspect = u_texel_size.y / u_texel_size.x;
    g_center = vec2(g_aspect, 1.0) * 0.5;

    vec2 uv = v_uv;
    uv.x *= g_aspect;

    if (u_warp > 0.001) {
        vec2 warpOffset = valueNoise2(uv * 3.0 + u_anim_time * 0.5);
        warpOffset += valueNoise2(uv * 7.0 - u_anim_time * 0.3) * 0.5;
        uv += warpOffset * u_warp * 0.25;
    }

    vec3 color;

    if (u_fractal >= 0.5) {
        int levels = int(clamp(u_depth, 1.0, float(MAX_LEVELS)));
        uv = coast_warp(uv, u_coastline * u_image_influence);

        vec2 nearest, second;
        vec4 v = fractalNearest(uv, float(levels), nearest, second);

        vec2 r1 = root_cell(levels, nearest);
        vec2 r2 = root_cell(levels, second);

        // FV-EDGE: a partition boundary is where the two nearest finest sites
        // trace to DIFFERENT roots. Interior cell edges are drawn at Layer Mix,
        // so the knob reads as "how much interior detail shows".
        bool boundary = r1.x != r2.x || r1.y != r2.y;
        float edgeScale = boundary ? 1.0 : u_layer_mix;

        float hue = fract(hash1(r1) + u_color_shift);
        color = shade(hue, v.y - v.x, edgeScale);
    } else {
        color = vec3(0.0);
        float totalWeight = 0.0;

        for (int layer = 0; layer < 3; layer++) {
            float fl = float(layer);
            float scale = u_density * pow(u_layer_spread, fl);
            float layerWeight = (layer == 0) ? 1.0 : pow(u_layer_mix, fl);

            vec4 vor = voronoiLayer(uv, scale, fl);
            float hue = fract(hash1(vor.zw) + u_color_shift + fl * 0.15);

            color += shade(hue, vor.y - vor.x, 1.0) * layerWeight;
            totalWeight += layerWeight;
        }
        color /= max(totalWeight, 0.001);
    }

    // FV-TONE: contrast once, on the composite — not per layer inside the loop.
    color = clamp((color - 0.5) * u_contrast + 0.5, 0.0, 1.0);
    color *= u_brightness;

    vec4 src = texture(u_input, content_uv(v_uv));
    color = mix(color, src.rgb, u_image_blend);

    out_color = vec4(color, 1.0);
}
