#version 150

// Write head: record `Send * input` into the ring-buffer layer bound as the
// render target. Pure overwrite (blend disabled) — the write head does NOT read
// the buffer; all feedback/mixing lives in the Tap read head. u_uv_scale
// corrects for hardware texture padding (Width/HardwareWidth, ...).
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform vec2 u_uv_scale;
uniform float u_send;

// Clamp the sampled content half a texel inside the outer content texel centres,
// so bilinear never blends the last real row/col with the black NPOT padding
// (a 1px dark edge seam) into the tape. texel from textureSize.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv * u_uv_scale, 0.5 * texel, u_uv_scale - 0.5 * texel));
}

void main() {
    out_color = u_send * sampleContent(v_uv);
}
