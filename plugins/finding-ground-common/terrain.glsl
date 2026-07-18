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
