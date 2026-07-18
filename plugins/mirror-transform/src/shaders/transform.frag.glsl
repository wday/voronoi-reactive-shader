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
        out_color = texture(u_input, folded);
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

    // Sample inside the content's outer texel CENTRES (half a texel in from each
    // edge), the same inset the mirror path uses. Sampling transformed_uv*u_uv_scale
    // directly reaches the content->padding boundary at transformed_uv→0/1, where
    // bilinear blends the last real row/col with the black hardware padding — a 1px
    // dark seam at the edge (visible on translate_y + scale-up). Out-of-frame uv is
    // clamped here but masked to black by inBounds, so the clamp is invisible.
    vec2 span = u_uv_scale - u_texel;                       // outer-centre to outer-centre
    vec2 sample_uv = 0.5 * u_texel + clamp(transformed_uv, 0.0, 1.0) * span;
    vec4 color = texture(u_input, sample_uv) * inBounds;
    out_color = color;
}
