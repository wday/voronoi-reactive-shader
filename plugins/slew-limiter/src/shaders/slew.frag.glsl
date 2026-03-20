#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform sampler2D u_previous;
uniform float u_rate;
uniform vec2 u_uv_scale;

void main() {
    vec4 input_col = texture(u_input, v_uv * u_uv_scale);
    vec4 prev_col = texture(u_previous, v_uv);
    vec4 delta = clamp(input_col - prev_col, -u_rate, u_rate);
    out_color = prev_col + delta;
}
