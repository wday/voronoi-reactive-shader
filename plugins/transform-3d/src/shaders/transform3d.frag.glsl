#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;

// --- 3D block ---
uniform float u_rot_x;      // radians, tilt about the horizontal axis
uniform float u_rot_y;      // radians, tilt about the vertical axis
uniform float u_rot_z;      // radians, RIGID spin (aspect-corrected)
uniform float u_persp;      // camera distance d, world units; large => orthographic
uniform float u_aspect;     // content width / height

// --- 2D block (Mirror Transform's chain, verbatim) ---
uniform float u_scale;
uniform float u_swirl;
uniform float u_anamorph;   // radians, rotation in RAW uv space
uniform float u_translate_x;
uniform float u_translate_y;

// --- edges ---
uniform float u_edges;      // 0 soft clip | 1 mirror plane | 2 mirror tile | 3 mirror box
uniform float u_fold;       // mirror cell size, in frames

uniform vec2 u_uv_scale;    // content_size / hardware_size, per axis
uniform vec2 u_texel;       // 1/hardware_size, per axis

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

// ---------------------------------------------------------------------------
// Mirror folding.
//
// A continuous triangle wave: value is C0 across every cell boundary, only the
// derivative flips sign. That continuity is what lets the horizon fade below be
// analytic in t — a derivative-based fade would spike at each seam and draw a
// dark line along it.
// ---------------------------------------------------------------------------

// Reflect x into the cell [-p/2, +p/2] centred on the origin.
vec2 foldCell(vec2 x, vec2 p) {
    return (p - abs(mod(x + 0.5 * p, 2.0 * p) - p)) - 0.5 * p;
}

// +1 for even cells, -1 for odd (reflected) ones. Matches foldCell's parity.
vec2 cellSign(vec2 x, vec2 p) {
    return 1.0 - 2.0 * mod(floor((x + 0.5 * p) / p), 2.0);
}

// ---------------------------------------------------------------------------
// 3D block: inverse perspective map.
//
// Camera at (0,0,-d) looking down +z, screen plane at z=0 with focal length d,
// so that an untilted plane projects 1:1 and R = I is exactly the identity map.
// The source plane is z=0 rotated by R; its normal is therefore R's third
// column, and its own 2-D coordinates are the first two columns' projections.
//
// Returns the hit in plane coordinates. `ok` is false when the ray misses on the
// visible side — past the horizon, behind the camera, or an edge-on plane. It is
// returned as a flag and multiplied in at the very end rather than taken as an
// early return, so screen-space derivatives stay well-defined for every
// fragment in the quad.
// ---------------------------------------------------------------------------
mat3 rotMatrix(float ax, float ay, float az) {
    float cx = cos(ax), sx = sin(ax);
    float cy = cos(ay), sy = sin(ay);
    float cz = cos(az), sz = sin(az);
    // Column-major, as GLSL's mat3 constructor takes them.
    mat3 rx = mat3(1.0, 0.0, 0.0,   0.0,  cx,  sx,   0.0, -sx,  cx);
    mat3 ry = mat3( cy, 0.0, -sy,   0.0, 1.0, 0.0,    sy, 0.0,  cy);
    mat3 rz = mat3( cz,  sz, 0.0,   -sz,  cz, 0.0,   0.0, 0.0, 1.0);
    // Spin the card in its own plane, then tilt it. This is the ordering that
    // makes X/Y/Z read as one coherent triple rather than a camera roll.
    return ry * rx * rz;
}

vec2 unproject(vec2 q, mat3 r, float d, out bool ok, out float t) {
    vec3 n = r[2];                                  // plane normal = R * z_hat
    float denom = dot(n.xy, q) + d * n.z;
    ok = abs(denom) > 1e-6;
    t = ok ? (d * n.z) / denom : 0.0;
    ok = ok && t > 0.0;
    vec3 p = vec3(t * q, d * (t - 1.0));            // ray point at parameter t
    return vec2(dot(r[0], p), dot(r[1], p));
}

void main() {
    int mode = int(u_edges + 0.5);
    mat3 r = rotMatrix(u_rot_x, u_rot_y, u_rot_z);

    // Aspect-corrected, centred destination coordinate. Everything in the 3D
    // block lives here, where a rotation is an actual rotation.
    vec2 q = (v_uv - 0.5) * vec2(u_aspect, 1.0);
    vec2 sgn = vec2(1.0);

    if (mode == 3) {
        // Mirror Box: mirror walls standing in SCREEN space. Fold into a cell,
        // and reflect the tilt for odd cells — reflecting the coordinate on both
        // sides of the perspective block is conjugation by diag(±1,±1,1), which
        // is exactly the mirrored rotation, for the price of two multiplies.
        vec2 period = vec2(u_aspect, 1.0) * u_fold;
        sgn = cellSign(q, period);
        q = foldCell(q, period);
    }

    bool ok;
    float t;
    vec2 s = unproject(q, r, u_persp, ok, t);
    if (mode == 3) {
        s *= sgn;
    }

    if (mode == 2) {
        // Mirror Tile: the fold sits BETWEEN the blocks, so the tilted plane is
        // tiled with complete copies of the transformed frame and the 2D chain
        // restarts inside every cell — "kaleidoscope, then tilt". Contrast
        // Mirror Plane below, where the fold sits after the 2D block and the
        // swirl runs continuously across cell boundaries.
        vec2 period = vec2(u_aspect, 1.0) * u_fold;
        s = foldCell(s, period) / u_fold;
    }

    // Back to raw uv space for the 2D block. With R = I this is exactly
    // (v_uv - 0.5), so the whole chain below is Mirror Transform's, unchanged.
    vec2 centered = s / vec2(u_aspect, 1.0);

    // --- 2D block: scale, swirl, anamorphic rotation, translate ---
    centered *= u_scale;

    // Swirl: angular displacement proportional to distance from center
    if (u_swirl != 0.0) {
        float rad = length(centered);
        float angle = u_swirl * rad;
        float cs = cos(angle);
        float ss = sin(angle);
        centered = vec2(centered.x * cs - centered.y * ss,
                        centered.x * ss + centered.y * cs);
    }

    // Anamorphic rotation: in RAW uv space, so on a non-square frame this is a
    // rotation conjugated by an anisotropic scale — rotation, shear and a
    // breathing scale at once. Kept as its own knob because it is a distinct
    // (and already-tuned) look, not a broken Rotate Z.
    float c = cos(u_anamorph);
    float sn = sin(u_anamorph);
    vec2 rotated = vec2(centered.x * c - centered.y * sn,
                        centered.x * sn + centered.y * c);

    vec2 transformed_uv = rotated + 0.5 + vec2(u_translate_x, u_translate_y);

    // --- horizon fade ---
    //
    // Near the horizon the source coordinate runs to infinity. With no mipmaps
    // and a 16-tap cubic that moirés hard, so fade out once magnification gets
    // away from us. Analytic in the ray parameter t, not in dFdx: t is
    // continuous across mirror-fold seams (the fold is a triangle wave) where a
    // derivative-based measure would spike and draw a dark line along every
    // seam. t == 1 everywhere when the plane is untilted, so this is inert
    // unless Rotate X/Y is actually in use.
    float content_h = float(textureSize(u_input, 0).y) * u_uv_scale.y;
    float lim = sqrt(content_h / 32.0);
    float horizon = 1.0 - smoothstep(lim, 3.0 * lim, t);
    float inBounds = ok ? horizon : 0.0;

    vec2 lo = 0.5 * u_texel;
    vec2 hi = u_uv_scale - 0.5 * u_texel;

    if (mode >= 1) {
        // Kaleidoscope fold, done in TEXCOORD space and reflected about the
        // outer texel *centers* (half a texel inside each content edge) rather
        // than the content edge itself. Folding about the literal edge put the
        // mirror axis on the content->padding boundary, so sampling there
        // blended the last real column with padding (a dark seam), and it also
        // doubled the edge column. Reflecting between the two outer centers
        // keeps every sample >= half a texel inside the content region: no
        // padding bleed, no doubled column.
        //
        // Mode 1 (Mirror Plane) is the only mode whose defining fold this is, so
        // it is the only one that takes its period from Fold Tile; modes 2 and 3
        // have already folded upstream and just need the frame-period tiling to
        // fill out from.
        float period_frames = (mode == 1) ? u_fold : 1.0;
        vec2 span = (u_uv_scale - u_texel) * period_frames; // outer-center to outer-center
        vec2 y = mod(transformed_uv * u_uv_scale - lo, 2.0 * span);
        vec2 folded = lo + (span - abs(y - span));          // apex on real texel centers

        // A cell wider than the frame leaves gutters between the mirrored
        // copies. Mask those to black rather than letting the clamp below streak
        // the edge texel across them. Inert for period_frames <= 1, where the
        // fold lands inside the content region by construction.
        vec2 gutter = min(folded - lo, hi - folded);
        inBounds *= smoothstep(-u_texel.x, 0.0, gutter.x)
                  * smoothstep(-u_texel.y, 0.0, gutter.y);

        out_color = sampleCubic(clamp(folded, lo, hi), lo, hi) * inBounds;
        return;
    }

    // Soft clip: fade to black only OUTSIDE the frame. The fade band lives in
    // [-edge, 0], so any transformed_uv inside [0,1] — including the outermost
    // pixel rows/cols (whose centers sit at 0.5/res, never at a literal 0 or 1)
    // — stays at full brightness. The old form faded within [0,edge]/[1-edge,1],
    // darkening the edge rows; in a feedback loop that compounded into black
    // bars.
    float edge = 0.005;
    vec2 d = min(transformed_uv, 1.0 - transformed_uv); // >=0 inside, <0 outside
    inBounds *= smoothstep(-edge, 0.0, d.x) * smoothstep(-edge, 0.0, d.y);

    // Keep every sample at least half a texel inside the content, so the filter
    // never blends the last real row/col with the black NPOT hardware padding (a
    // 1px dark seam, visible on translate_y + scale-up). Out-of-frame uv is
    // clamped here but masked to black by inBounds, so the clamp is invisible.
    //
    // CLAMP, not rescale. Mirror Transform previously read
    //     sample_uv = 0.5*u_texel + transformed_uv * (u_uv_scale - u_texel)
    // which squeezed [0,1] onto the outer-texel-CENTRE span — N-1 texels instead
    // of N. That put every pixel at a fractional texel offset even with no
    // transform at all, so an identity Scale/Rotation still resampled the whole
    // frame: measured 0.56 of the source's high-frequency energy surviving a
    // single pass, and a ~0.05%/lap zoom-out nobody asked for. Inside a feedback
    // loop that compounds — it was the dominant reason deep zoom tunnels blurred
    // out. Clamping instead is exactly identity for in-range uv (verified
    // bit-exact) and is strictly safer at the edges than the rescale was.
    vec2 sample_uv = clamp(transformed_uv * u_uv_scale, lo, hi);
    out_color = sampleCubic(sample_uv, lo, hi) * inBounds;
}
