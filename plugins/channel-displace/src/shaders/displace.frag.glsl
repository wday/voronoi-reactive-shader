#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_amount;
uniform int u_pattern;  // 0=cyclic, 1=mutual
uniform float u_angle;
uniform float u_dry_wet;

// Keep every sample half a texel inside the texture edge so bilinear never blends
// with the black border (a dark edge fringe, and a thin seam where a displaced
// channel samples off-frame — clamps to edge-extend instead of black). texel from
// textureSize — no host uniform needed.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv, 0.5 * texel, 1.0 - 0.5 * texel));
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

        r_out = sampleContent(uv_r).r;
        g_out = sampleContent(uv_g).g;
        b_out = sampleContent(uv_b).b;
    } else {
        // Mutual: each displaced by average of other two
        float avg_gb = (original.g + original.b) * 0.5;
        float avg_rb = (original.r + original.b) * 0.5;
        float avg_rg = (original.r + original.g) * 0.5;

        vec2 uv_r = v_uv + dir * avg_gb;
        vec2 uv_g = v_uv + dir * avg_rb;
        vec2 uv_b = v_uv + dir * avg_rg;

        r_out = sampleContent(uv_r).r;
        g_out = sampleContent(uv_g).g;
        b_out = sampleContent(uv_b).b;
    }

    vec4 displaced = vec4(r_out, g_out, b_out, original.a);
    out_color = mix(original, displaced, u_dry_wet);
}
