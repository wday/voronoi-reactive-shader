#version 150

// Shared node output: blend the node's own live input against a buffer layer.
//   Delay Write : u_wet = Thru↔Playback  (dry = live thru, wet = delayed playback)
//   Delay Tap   : u_wet = Buffer Mix     (dry = own input, wet = tapped buffer)
// Naming the control per behavior is deliberate; the mechanic is one shape.

in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;        // node input (live)
uniform vec2 u_uv_scale;          // Width/HardwareWidth, Height/HardwareHeight
uniform sampler2DArray u_buffer;  // shared ring buffer
uniform float u_layer;            // layer to read (playback / tap offset)
uniform float u_wet;              // 0 = live input, 1 = buffer

void main() {
    vec4 live = texture(u_input, v_uv * u_uv_scale);
    vec4 buf  = texture(u_buffer, vec3(v_uv, u_layer));
    out_color = mix(live, buf, u_wet);
}
