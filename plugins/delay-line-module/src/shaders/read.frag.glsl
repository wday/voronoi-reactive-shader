#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2DArray u_buffer;
uniform float u_layer;

void main() {
    out_color = texture(u_buffer, vec3(v_uv, u_layer));
}
