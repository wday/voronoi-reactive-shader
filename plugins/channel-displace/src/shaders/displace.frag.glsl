#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_amount;
uniform int u_pattern;  // 0=cyclic, 1=mutual
uniform float u_angle;
uniform float u_dry_wet;

// Keep every sample half a texel inside the texture edge so filtering never blends
// with the black border (a dark edge fringe, and a thin seam where a displaced
// channel samples off-frame — clamps to edge-extend instead of black). texel from
// textureSize — no host uniform needed.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv, 0.5 * texel, 1.0 - 0.5 * texel));
}

// Catmull-Rom basis for taps at -1, 0, +1, +2. Sums to 1 for any f.
vec4 crWeights(float f) {
    float f2 = f * f;
    float f3 = f2 * f;
    return vec4(-0.5 * f3 +       f2 - 0.5 * f,
                 1.5 * f3 - 2.5 * f2           + 1.0,
                -1.5 * f3 + 2.0 * f2 + 0.5 * f,
                 0.5 * f3 - 0.5 * f2);
}

// 16-tap Catmull-Rom, every tap clamped inside the texture edge (the 4x4 kernel
// reaches 2 texels past the sample point, so the half-texel inset above is not
// enough on its own).
//
// Why cubic here: the displacement is content-driven and freely sub-pixel, so a
// bilinear tap sits at a fractional offset essentially always. Used inside a
// feedback loop that filter is applied once per lap and compounds — bilinear's
// Nyquist response for a fractional shift t is |1-2t|, which over hundreds of
// laps annihilates fine detail. Catmull-Rom's response is far flatter. At a
// whole-texel offset the weights are exactly (0,1,0,0), so a zero displacement
// still returns the source texel bit-exactly.
vec4 sampleCubic(vec2 uv) {
    vec2 texSize = vec2(textureSize(u_input, 0));
    vec2 texel = 1.0 / texSize;
    vec2 lo = 0.5 * texel;
    vec2 hi = 1.0 - 0.5 * texel;

    vec2 p = uv * texSize - 0.5;      // continuous texel index
    vec2 i0 = floor(p);
    vec2 f = p - i0;
    vec4 wx = crWeights(f.x);
    vec4 wy = crWeights(f.y);

    vec4 acc = vec4(0.0);
    for (int j = 0; j < 4; ++j) {
        for (int i = 0; i < 4; ++i) {
            vec2 tap = (i0 + vec2(float(i) - 1.0, float(j) - 1.0) + 0.5) * texel;
            acc += texture(u_input, clamp(tap, lo, hi)) * wx[i] * wy[j];
        }
    }
    return acc;
}

void main() {
    vec4 original = sampleContent(v_uv);
    vec2 dir = vec2(cos(u_angle), sin(u_angle)) * u_amount;

    float r_out, g_out, b_out;

    if (u_pattern == 0) {
        // Cyclic: R displaced by G, G by B, B by R
        vec2 uv_r = v_uv + dir * original.g;
        vec2 uv_g = v_uv + dir * original.b;
        vec2 uv_b = v_uv + dir * original.r;

        r_out = sampleCubic(uv_r).r;
        g_out = sampleCubic(uv_g).g;
        b_out = sampleCubic(uv_b).b;
    } else {
        // Mutual: each displaced by average of other two
        float avg_gb = (original.g + original.b) * 0.5;
        float avg_rb = (original.r + original.b) * 0.5;
        float avg_rg = (original.r + original.g) * 0.5;

        vec2 uv_r = v_uv + dir * avg_gb;
        vec2 uv_g = v_uv + dir * avg_rb;
        vec2 uv_b = v_uv + dir * avg_rg;

        r_out = sampleCubic(uv_r).r;
        g_out = sampleCubic(uv_g).g;
        b_out = sampleCubic(uv_b).b;
    }

    vec4 displaced = vec4(r_out, g_out, b_out, original.a);
    out_color = mix(original, displaced, u_dry_wet);
}
