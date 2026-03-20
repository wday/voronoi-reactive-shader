#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform sampler2DArray u_buffer;
uniform float u_layer;
uniform float u_zero_tap;
uniform vec2 u_uv_scale;

void main() {
    vec4 live = texture(u_input, v_uv * u_uv_scale);
    vec4 delayed = texture(u_buffer, vec3(v_uv, u_layer));
    out_color = mix(delayed, live, u_zero_tap);
}
