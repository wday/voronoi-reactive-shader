#version 150

// Varispeed Write: record `Send * input` into the ring-buffer layer bound as the
// render target. Pure overwrite (blend disabled) — the write head does NOT read
// the buffer. u_uv_scale corrects for hardware texture padding
// (Width/HardwareWidth, ...). The layer is stored at reduced (half) resolution, so
// this is a full-frame downsample into the smaller viewport.
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform vec2 u_uv_scale;
uniform float u_send;

void main() {
    out_color = u_send * texture(u_input, v_uv * u_uv_scale);
}
