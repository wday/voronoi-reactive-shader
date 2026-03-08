#version 150

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;

void main() {
    out_color = texture(u_input, v_uv);
}
