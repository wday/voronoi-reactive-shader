#version 150

// Varispeed Read: fetch four ring-buffer layers around the fractional read
// position, interpolate them with a Catmull-Rom cubic by u_frac (VS-INTERP), then
// blend with the live source using two independent gains in the selected colour
// space (same
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
// Four consecutive frames around the read position. u_frac interpolates between
// u_layer0 and u_layer1; the outer two only shape the curve. At u_frac == 0 the
// cubic returns u_layer0 exactly, so integer reads (Rate +/-1x, +/-2x, no Warp)
// are unchanged from the old linear mix().
//
// Deliberately four scalars rather than a `float[4]`: uniform_loc returns -1 on a
// name miss and glUniform*(-1, ...) is a silent no-op, so an array-name mismatch
// on some driver would freeze the read on layer 0 with no error at all.
uniform float u_layer_prev;
uniform float u_layer0;
uniform float u_layer1;
uniform float u_layer_next;
uniform float u_frac;             // interp weight u_layer0 -> u_layer1
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

// Catmull-Rom through p1..p2 at t, with p0/p3 as the surrounding tangents. Chosen
// over linear because it is smoother between frames AND slightly sharper: linear
// mix() is a pure two-tap average, so at fractional rates it low-passed the loop a
// little on every lap. The cubic's mild overshoot cancels some of that.
//
// Overshoot is clamped: the tape stores encoded [0,1] values, and out-of-range
// results would otherwise be fed straight back round the loop.
vec4 catmull(vec4 p0, vec4 p1, vec4 p2, vec4 p3, float t) {
    float t2 = t * t;
    float t3 = t2 * t;
    vec4 v = 0.5 * ((2.0 * p1)
                  + (-p0 + p2) * t
                  + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                  + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
    return clamp(v, 0.0, 1.0);
}

void main() {
    vec4 live = sampleContent(v_uv);
    vec4 p0 = texture(u_buffer, vec3(v_uv, u_layer_prev));
    vec4 p1 = texture(u_buffer, vec3(v_uv, u_layer0));
    vec4 p2 = texture(u_buffer, vec3(v_uv, u_layer1));
    vec4 p3 = texture(u_buffer, vec3(v_uv, u_layer_next));
    vec4 loopc = catmull(p0, p1, p2, p3, u_frac);

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
