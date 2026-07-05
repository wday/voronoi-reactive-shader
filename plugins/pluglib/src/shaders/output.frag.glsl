#version 150

// Shared node output: two independent gains, NOT a crossfade.
//   out = clamp(dry*live + wet*buffer, 0, 1)
//
//   Delay Tap   : dry = Dry (live source into the chain)
//                 wet = Wet (delayed tape into the chain)
//   Delay Write : dry = 1, wet = 0  (pure passthrough — the record head is
//                 visually transparent; you see what it is writing)
//
// Two gains instead of mix() so source-injection and loop-feedback are
// decoupled (hold Wet high, pulse Dry/Send independently). Clamp keeps additive
// bloom predictable regardless of the host FBO format.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;        // node input (live source)
uniform vec2 u_uv_scale;          // Width/HardwareWidth, Height/HardwareHeight
uniform sampler2DArray u_buffer;  // shared ring buffer
uniform float u_layer;            // layer to read (full-delay tap)
uniform float u_dry;              // gain on live input
uniform float u_wet;              // gain on buffer

void main() {
    vec4 live = texture(u_input, v_uv * u_uv_scale);
    vec4 buf  = texture(u_buffer, vec3(v_uv, u_layer));
    out_color = clamp(u_dry * live + u_wet * buf, 0.0, 1.0);
}
