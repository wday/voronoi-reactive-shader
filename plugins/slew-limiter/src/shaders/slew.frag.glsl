#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform sampler2D u_previous;
uniform float u_rate;
uniform vec2 u_uv_scale;

// Clamp the sampled content half a texel inside the outer content texel centres,
// so bilinear never blends the last real row/col with the black NPOT padding
// (a 1px dark edge seam). texel from textureSize — no host-side uniform needed.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv * u_uv_scale, 0.5 * texel, u_uv_scale - 0.5 * texel));
}

void main() {
    vec4 input_col = sampleContent(v_uv);
    vec4 prev_col = texture(u_previous, v_uv);
    vec4 delta = clamp(input_col - prev_col, -u_rate, u_rate);
    out_color = prev_col + delta;
}
