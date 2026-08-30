#version 150

// Varispeed Read: fetch two ring-buffer layers (the fractional read's bracketing
// frames), linearly interpolate them by u_frac (VS-INTERP), then blend with the
// live source using two independent gains in the selected colour space (same
// u_gamma logic as the delay's shared output pass):
//   perceptual (u_gamma == 1): out = clamp(dry*live + wet*loop, 0, 1)
//   linear     (u_gamma  > 1): out = encode(clamp(dry*decode(live)
//                                              + wet*decode(loop), 0, 1))
// The buffer is a full-res texture array, so sampling with normalised uv is a
// 1:1 fetch (no spatial upscale); mix() still gives the temporal interpolation.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;        // live source
uniform vec2 u_uv_scale;          // Width/HardwareWidth, ...
uniform sampler2DArray u_buffer;  // shared ring buffer
uniform float u_layer0;           // bracketing frames (fractional read)
uniform float u_layer1;
uniform float u_frac;             // interp weight layer0 -> layer1
uniform float u_dry;
uniform float u_wet;
uniform float u_gamma;            // blend space (1.0 = perceptual)

// Clamp the sampled content half a texel inside the outer content texel centres,
// so bilinear never blends the last real row/col with the black NPOT padding
// (a 1px dark edge seam). texel from textureSize — no host-side uniform needed.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv * u_uv_scale, 0.5 * texel, u_uv_scale - 0.5 * texel));
}

void main() {
    vec4 live = sampleContent(v_uv);
    vec4 b0 = texture(u_buffer, vec3(v_uv, u_layer0));
    vec4 b1 = texture(u_buffer, vec3(v_uv, u_layer1));
    vec4 loopc = mix(b0, b1, u_frac);

    if (u_gamma == 1.0) {
        out_color = clamp(u_dry * live + u_wet * loopc, 0.0, 1.0);
    } else {
        vec3 l = pow(max(live.rgb, 0.0), vec3(u_gamma));
        vec3 b = pow(max(loopc.rgb, 0.0), vec3(u_gamma));
        vec3 mixed = clamp(u_dry * l + u_wet * b, 0.0, 1.0);
        float a = clamp(u_dry * live.a + u_wet * loopc.a, 0.0, 1.0);
        out_color = vec4(pow(mixed, vec3(1.0 / u_gamma)), a);
    }
}
