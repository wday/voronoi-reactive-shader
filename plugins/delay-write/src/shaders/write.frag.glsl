#version 150

// Write the node input into the ring-buffer layer bound as the render target.
// u_uv_scale corrects for hardware texture padding (Width/HardwareWidth, ...).
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform vec2 u_uv_scale;

void main() {
    out_color = texture(u_input, v_uv * u_uv_scale);
}
