// Shared procedural terrain for the Finding Ground generators (P1 Contour,
// P2 Dendritic, P3 Parcel). SINGLE SOURCE OF TRUTH — all three describe the
// SAME land so contours, rivers, and parcels register with one another.
//
// Pulled in by an include directive (see each frag shader), expanded at compile
// time: the shader harness + render tool resolve it relative to the including
// file; the Rust plugins string-replace it with this file (include_str!). The
// includer must declare the u_warp uniform that height() reads.
// (Avoid writing the literal hash-include token in comments — moderngl's naive
// include scanner is not comment-aware and would try to resolve it.)

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

// --- value noise + fbm ---
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash1(i + vec2(0.0, 0.0));
    float b = hash1(i + vec2(1.0, 0.0));
    float c = hash1(i + vec2(0.0, 1.0));
    float d = hash1(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
float fbm(vec2 p) {
    float sum = 0.0, amp = 0.5, freq = 1.0;
    for (int i = 0; i < 5; i++) { sum += amp * vnoise(p * freq); freq *= 2.0; amp *= 0.5; }
    return sum;
}

// Height field: fbm domain-warped by a second fbm → sinuous ridges and valleys.
// Reads u_warp from the including shader.
float height(vec2 p) {
    vec2 q = vec2(fbm(p + vec2(0.0, 0.0)),
                  fbm(p + vec2(5.2, 1.3)));
    float w = mix(0.0, 1.4, u_warp);
    return fbm(p + w * q);
}

// --- shared motion + paint (used by all three generators) ---

// Cyclic per-axis drift. dx/dy (0..1) set the oscillation RATE on each axis;
// the sample point traces a bounded Lissajous path, so the terrain keeps moving
// through fresh directions and never goes stale the way a single linear drift
// did. 0 on an axis freezes it. The Y phase is offset so equal rates don't make
// a pure diagonal.
vec2 drift_offset(float t, float dx, float dy) {
    const float AMP  = 2.5;   // world-space travel
    const float MAXW = 2.0;   // max angular rate (rad/sec)
    return AMP * vec2(sin(t * mix(0.0, MAXW, dx)),
                      sin(t * mix(0.0, MAXW, dy) + 1.7));
}

// Ink-on-paper compositing. coverage 0..1 = how much ink at this pixel.
//   warmth   — cool grey-ink ↔ warm earth-ink on paper
//   contrast — 0 soft/muddy, 0.5 neutral, 1 deep blacks + bright paper
//   invert   — 0 dark ink on light paper, 1 light lines on dark ground
//   tint     — faint signed relief added to the ground (e.g. elevation)
vec3 paint(float coverage, float warmth, float contrast, float invert, float tint) {
    vec3 paper = mix(vec3(0.90, 0.89, 0.86), vec3(0.93, 0.88, 0.80), warmth);
    vec3 inkc  = mix(vec3(0.10, 0.11, 0.13), vec3(0.16, 0.10, 0.06), warmth);

    // Push the two tones apart (or together) about their midpoint.
    float c = mix(0.6, 1.7, contrast);
    vec3 mid = (paper + inkc) * 0.5;
    paper = clamp(mix(mid, paper, c) - tint, 0.0, 1.0);
    inkc  = clamp(mix(mid, inkc, c), 0.0, 1.0);

    vec3 bg = mix(paper, inkc, invert);
    vec3 fg = mix(inkc, paper, invert);
    return mix(bg, fg, clamp(coverage, 0.0, 1.0));
}
