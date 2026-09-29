#version 150

// Shared node output: two independent gains, NOT a crossfade.
//   perceptual (u_gamma == 1): out = clamp(dry*live + wet*buffer, 0, 1)
//   linear     (u_gamma  > 1): out = encode(clamp(dry*decode(live)
//                                                + wet*decode(buffer), 0, 1))
//
//   Delay Tap   : dry = Dry (live source into the chain)
//                 wet = Wet (delayed tape into the chain)
//   Delay Write : dry = 1, wet = 0, u_gamma = 1  (pure passthrough — the record
//                 head is visually transparent; you see what it is writing)
//
// Two gains instead of mix() so source-injection and loop-feedback are
// decoupled (hold Wet high, pulse Dry/Send independently). Clamp keeps additive
// bloom predictable regardless of the host FBO format.
//
// u_gamma selects the BLEND SPACE (Delay Tap "Blend Space" param). Host video is
// sRGB-encoded and the tape stores those encoded values (the Write is a straight
// store, unchanged), so:
//   * u_gamma == 1 -> blend the encoded values directly. The per-lap ×Wet reads
//     as a perceptually-even fade (a video-mixer look). Exact legacy behaviour.
//   * u_gamma  ~2.2 -> decode live AND the tape to ~linear light, blend + clamp
//     as real light, then re-encode for the host. The loop re-encodes every lap,
//     so the linear value round-trips through the sRGB tape (Send acts as ~Send^g
//     on the loop gain). Cleaner colour mixing + additive highlight bloom, at the
//     cost of the perceptual evenness. pow() is the whole cost — a cheap 2.2
//     stand-in for the sRGB curve (exact sRGB piecewise is deferred with float).
// Only .rgb is gamma-mapped; alpha blends linearly either way.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;        // node input (live source)
uniform vec2 u_uv_scale;          // Width/HardwareWidth, Height/HardwareHeight
uniform sampler2DArray u_buffer;  // shared ring buffer
uniform float u_layer;            // layer to read (full-delay tap)
uniform float u_dry;              // gain on live input
uniform float u_wet;              // gain on buffer
uniform float u_gamma;            // blend-space exponent (1.0 = perceptual)

// Clamp the sampled content half a texel inside the outer content texel centres,
// so bilinear never blends the last real row/col with the black NPOT padding
// (a 1px dark edge seam) into the live/dry signal. texel from textureSize.
vec4 sampleContent(vec2 uv) {
    vec2 texel = 1.0 / vec2(textureSize(u_input, 0));
    return texture(u_input, clamp(uv * u_uv_scale, 0.5 * texel, u_uv_scale - 0.5 * texel));
}

void main() {
    vec4 live = sampleContent(v_uv);
    vec4 buf  = texture(u_buffer, vec3(v_uv, u_layer));

    if (u_gamma == 1.0) {
        // Perceptual: blend the sRGB-encoded values directly (legacy, exact).
        out_color = clamp(u_dry * live + u_wet * buf, 0.0, 1.0);
    } else {
        // Linear light: decode both sources, blend + clamp as light, re-encode.
        vec3 l = pow(max(live.rgb, 0.0), vec3(u_gamma));
        vec3 b = pow(max(buf.rgb, 0.0), vec3(u_gamma));
        vec3 mixed = clamp(u_dry * l + u_wet * b, 0.0, 1.0);
        float a = clamp(u_dry * live.a + u_wet * buf.a, 0.0, 1.0);
        out_color = vec4(pow(mixed, vec3(1.0 / u_gamma)), a);
    }
}
