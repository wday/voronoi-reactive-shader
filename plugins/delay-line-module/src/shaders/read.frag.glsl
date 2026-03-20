#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2DArray u_buffer;
uniform float u_layer;
uniform vec2 u_uv_scale;

void main() {
    out_color = texture(u_buffer, vec3(v_uv * u_uv_scale, u_layer));
}
