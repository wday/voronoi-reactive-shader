#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_scale;
uniform float u_rotation;
uniform float u_swirl;
uniform float u_mirror;
uniform float u_translate_x;
uniform float u_translate_y;
uniform vec2 u_uv_scale;
uniform vec2 u_texel;   // 1/hardware_size, per axis

// ---------------------------------------------------------------------------
// Catmull-Rom bicubic sampling.
//
// Why: this plugin is used INSIDE feedback loops, where its filter is applied
// once per lap. A zoom tunnel at 99.9% scale needs ~693 laps to halve the image,
// and across most of a 1080p frame the per-lap displacement is under half a
// texel — so a bilinear tap sits permanently in its blurriest regime while the
// content barely moves. Bilinear's response at Nyquist for a fractional shift t
// is |1-2t|; raised to the power of several hundred laps that annihilates all
// fine detail, which is what made gentle zooms blurrier than aggressive ones at
// equal magnification. Catmull-Rom's response is far flatter, so the per-lap
// loss is much smaller and the tunnel survives an order of magnitude deeper.
//
// At a whole-texel offset the weights are exactly (0,1,0,0), so an identity
// transform still returns the source texel bit-exactly.
// ---------------------------------------------------------------------------

// Catmull-Rom basis for taps at -1, 0, +1, +2. Sums to 1 for any f.
vec4 crWeights(float f) {
    float f2 = f * f;
    float f3 = f2 * f;
    return vec4(-0.5 * f3 +       f2 - 0.5 * f,
                 1.5 * f3 - 2.5 * f2           + 1.0,
                -1.5 * f3 + 2.0 * f2 + 0.5 * f,
                 0.5 * f3 - 0.5 * f2);
}

// 16-tap Catmull-Rom. `lo`/`hi` bound the content region (outer texel CENTRES);
// every tap is clamped into it, so the kernel — which reaches 2 texels past the
// sample point — can never pull in the black NPOT hardware padding. Clamping
// replicates the edge texel, which is the right behaviour at a content edge and
// is masked to black by inBounds outside the frame anyway.
vec4 sampleCubic(vec2 uv, vec2 lo, vec2 hi) {
    vec2 texSize = vec2(textureSize(u_input, 0));
    vec2 p = uv * texSize - 0.5;      // continuous texel index
    vec2 i0 = floor(p);
    vec2 f = p - i0;
    vec4 wx = crWeights(f.x);
    vec4 wy = crWeights(f.y);

    vec4 acc = vec4(0.0);
    for (int j = 0; j < 4; ++j) {
        for (int i = 0; i < 4; ++i) {
            vec2 tap = (i0 + vec2(float(i) - 1.0, float(j) - 1.0) + 0.5) / texSize;
            acc += texture(u_input, clamp(tap, lo, hi)) * wx[i] * wy[j];
        }
    }
    return acc;
}

void main() {
    // Scale, swirl, rotate around center
    vec2 centered = v_uv - 0.5;
    centered *= u_scale;

    // Swirl: angular displacement proportional to distance from center
    if (u_swirl != 0.0) {
        float r = length(centered);
        float angle = u_swirl * r;
        float cs = cos(angle);
        float ss = sin(angle);
        centered = vec2(centered.x * cs - centered.y * ss,
                        centered.x * ss + centered.y * cs);
    }

    // Rotation
    float c = cos(u_rotation);
    float s = sin(u_rotation);
    vec2 rotated = vec2(centered.x * c - centered.y * s,
                        centered.x * s + centered.y * c);

    vec2 transformed_uv = rotated + 0.5 + vec2(u_translate_x, u_translate_y);

    // Mirror or soft-clip at edges
    float inBounds = 1.0;
    if (u_mirror > 0.5) {
        // Kaleidoscope fold, done in TEXCOORD space and reflected about the
        // outer texel *centers* (half a texel inside each content edge) rather
        // than the content edge itself. Folding about the literal edge put the
        // mirror axis on the content->padding boundary, so bilinear sampling
        // there blended the last real column with padding (a dark seam), and it
        // also doubled the edge column. Reflecting between the two outer centers
        // keeps every sample >= half a texel inside the content region: no
        // padding bleed, no doubled column. Bypasses the shared sample below.
        vec2 half_t = 0.5 * u_texel;
        vec2 span   = u_uv_scale - u_texel;               // outer-center to outer-center
        vec2 y      = mod(transformed_uv * u_uv_scale - half_t, 2.0 * span);
        vec2 folded = half_t + (span - abs(y - span));    // apex on real texel centers
        out_color = sampleCubic(folded, half_t, u_uv_scale - half_t);
        return;
    } else {
        // Soft clip: fade to black only OUTSIDE the frame. The fade band lives
        // in [-edge, 0], so any transformed_uv inside [0,1] — including the
        // outermost pixel rows/cols (whose centers sit at 0.5/res, never at a
        // literal 0 or 1) — stays at full brightness. The old form faded within
        // [0,edge]/[1-edge,1], darkening the edge rows; in a feedback loop that
        // compounded into black bars.
        float edge = 0.005;
        vec2 d = min(transformed_uv, 1.0 - transformed_uv); // >=0 inside, <0 outside
        inBounds = smoothstep(-edge, 0.0, d.x) * smoothstep(-edge, 0.0, d.y);
    }

    // Keep every sample at least half a texel inside the content, so the filter
    // never blends the last real row/col with the black NPOT hardware padding (a
    // 1px dark seam, visible on translate_y + scale-up). Out-of-frame uv is
    // clamped here but masked to black by inBounds, so the clamp is invisible.
    //
    // CLAMP, not rescale. This previously read
    //     sample_uv = 0.5*u_texel + transformed_uv * (u_uv_scale - u_texel)
    // which squeezed [0,1] onto the outer-texel-CENTRE span — N-1 texels instead
    // of N. That put every pixel at a fractional texel offset even with no
    // transform at all, so an identity Scale/Rotation still resampled the whole
    // frame: measured 0.56 of the source's high-frequency energy surviving a
    // single pass, and a ~0.05%/lap zoom-out nobody asked for. Inside a feedback
    // loop that compounds — it was the dominant reason deep zoom tunnels blurred
    // out. Clamping instead is exactly identity for in-range uv (verified
    // bit-exact) and is strictly safer at the edges than the rescale was.
    // The mirror path above never had this: its fold is additive, so it was
    // already identity.
    vec2 sample_uv = clamp(transformed_uv * u_uv_scale, 0.5 * u_texel, u_uv_scale - 0.5 * u_texel);
    vec4 color = sampleCubic(sample_uv, 0.5 * u_texel, u_uv_scale - 0.5 * u_texel) * inBounds;
    out_color = color;
}
