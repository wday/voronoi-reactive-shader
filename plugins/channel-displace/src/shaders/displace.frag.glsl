#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_amount;
uniform int u_pattern;  // 0=cyclic, 1=mutual
uniform float u_angle;
uniform float u_dry_wet;

void main() {
    vec4 original = texture(u_input, v_uv);
    vec2 dir = vec2(cos(u_angle), sin(u_angle)) * u_amount;

    float r_out, g_out, b_out;

    if (u_pattern == 0) {
        // Cyclic: R displaced by G, G by B, B by R
        vec2 uv_r = v_uv + dir * original.g;
        vec2 uv_g = v_uv + dir * original.b;
        vec2 uv_b = v_uv + dir * original.r;

        r_out = texture(u_input, uv_r).r;
        g_out = texture(u_input, uv_g).g;
        b_out = texture(u_input, uv_b).b;
    } else {
        // Mutual: each displaced by average of other two
        float avg_gb = (original.g + original.b) * 0.5;
        float avg_rb = (original.r + original.b) * 0.5;
        float avg_rg = (original.r + original.g) * 0.5;

        vec2 uv_r = v_uv + dir * avg_gb;
        vec2 uv_g = v_uv + dir * avg_rb;
        vec2 uv_b = v_uv + dir * avg_rg;

        r_out = texture(u_input, uv_r).r;
        g_out = texture(u_input, uv_g).g;
        b_out = texture(u_input, uv_b).b;
    }

    vec4 displaced = vec4(r_out, g_out, b_out, original.a);
    out_color = mix(original, displaced, u_dry_wet);
}
