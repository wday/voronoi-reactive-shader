#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_scale;
uniform float u_rotation;
uniform float u_swirl;
uniform float u_mirror;
uniform float u_translate_x;
uniform float u_translate_y;
uniform vec2 u_uv_scale;

void main() {
    // Scale, swirl, rotate around center
    vec2 centered = v_uv - 0.5;
    centered *= u_scale;

    // Swirl: angular displacement proportional to distance from center
    if (u_swirl != 0.0) {
        float r = length(centered);
        float angle = u_swirl * r;
        float cs = cos(angle);
        float ss = sin(angle);
        centered = vec2(centered.x * cs - centered.y * ss,
                        centered.x * ss + centered.y * cs);
    }

    // Rotation
    float c = cos(u_rotation);
    float s = sin(u_rotation);
    vec2 rotated = vec2(centered.x * c - centered.y * s,
                        centered.x * s + centered.y * c);

    vec2 transformed_uv = rotated + 0.5 + vec2(u_translate_x, u_translate_y);

    // Mirror or soft-clip at edges
    float inBounds = 1.0;
    if (u_mirror > 0.5) {
        // Kaleidoscope: fold UV back into 0..1 range
        transformed_uv = 1.0 - abs(mod(transformed_uv, 2.0) - 1.0);
    } else {
        // Soft clip: fade to black at edges
        float edge = 0.005;
        inBounds = smoothstep(0.0, edge, transformed_uv.x) * smoothstep(1.0, 1.0 - edge, transformed_uv.x)
                 * smoothstep(0.0, edge, transformed_uv.y) * smoothstep(1.0, 1.0 - edge, transformed_uv.y);
    }

    // Scale UVs to account for hardware texture padding
    vec4 color = texture(u_input, transformed_uv * u_uv_scale) * inBounds;
    out_color = color;
}
