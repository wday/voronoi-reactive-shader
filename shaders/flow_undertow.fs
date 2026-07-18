/*{
  "DESCRIPTION": "Undertow — stateless in-loop advector. Drop inside a host feedback loop (source → tap → Undertow → write): it reconstructs a velocity field from the loop image's own luminance gradient each frame and advects that same image along it. No buffers — the loop's visible image is the only memory. The source, blended into the loop upstream, becomes the forcing function implicitly: its injected density is a gradient is velocity. Swirl rotates the gradient (90deg = circulate around blobs = vortices; 0deg = drain along it); Ambient Drift adds divergence-free curl-noise so still regions keep moving.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX", "Flow"],
  "INPUTS": [
    { "NAME": "inputImage", "TYPE": "image", "LABEL": "Input" },

    { "NAME": "velScale",    "TYPE": "float", "LABEL": "Current",       "DEFAULT": 0.3,  "MIN": 0.0,  "MAX": 2.0 },
    { "NAME": "rotAngle",    "TYPE": "float", "LABEL": "Swirl",         "DEFAULT": 0.75, "MIN": 0.0,  "MAX": 1.0 },
    { "NAME": "dt",          "TYPE": "float", "LABEL": "Flow Rate",     "DEFAULT": 0.5,  "MIN": 0.01, "MAX": 2.0 },

    { "NAME": "ambient",     "TYPE": "float", "LABEL": "Ambient Drift", "DEFAULT": 0.1,  "MIN": 0.0,  "MAX": 1.0 },
    { "NAME": "noiseScale",  "TYPE": "float", "LABEL": "Drift Scale",   "DEFAULT": 3.0,  "MIN": 0.5,  "MAX": 10.0 },
    { "NAME": "noiseDrift",  "TYPE": "float", "LABEL": "Drift Speed",   "DEFAULT": 0.15, "MIN": 0.0,  "MAX": 1.0 },

    { "NAME": "diffusion",   "TYPE": "float", "LABEL": "Viscosity",     "DEFAULT": 0.1,  "MIN": 0.0,  "MAX": 1.0 },
    { "NAME": "persistence", "TYPE": "float", "LABEL": "Persistence",   "DEFAULT": 1.0,  "MIN": 0.9,  "MAX": 1.0 },

    { "NAME": "boundaryMode","TYPE": "long",  "LABEL": "Boundary", "DEFAULT": 0, "VALUES": [0, 1, 2], "LABELS": ["Wrap", "Reflect", "Absorb"] }
  ]
}*/

// -----------------------------------------------------------------------
// Undertow — stateless fluid advection as an insert fx.
//
// Unlike flow_euler (owned velA/velB/dyeA/dyeB buffers), this is a single
// pass reading only `inputImage`. It occupies the f (forcing) + -(v.grad)v
// (self-advection) terms of Navier-Stokes; viscosity = blur, damping =
// decay, incompressibility skipped (rotated-gradient + curl velocity are
// ~divergence-free). The density field lives in the host loop; velocity is
// its derivative, re-derived every frame.
// -----------------------------------------------------------------------

float luma(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

bool applyBoundary(inout vec2 uv) {
    if (boundaryMode == 0) {
        uv = fract(uv);
    } else if (boundaryMode == 1) {
        uv = 1.0 - abs(mod(uv, 2.0) - 1.0);
    } else {
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0)
            return false;
    }
    return true;
}

// IMG_NORM_PIXEL needs a literal sampler name -> one helper.
vec4 sampleInput(vec2 uv) {
    if (!applyBoundary(uv)) return vec4(0.0);
    return IMG_NORM_PIXEL(inputImage, uv);
}

vec2 rot(vec2 v, float turns) {
    float a = turns * 6.28318530718;
    float s = sin(a), c = cos(a);
    return vec2(c * v.x - s * v.y, s * v.x + c * v.y);
}

// --- divergence-free curl noise (ambient drift) -----------------------
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash(i), b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0)), d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
// v = ( dpsi/dy, -dpsi/dx ) of scalar potential psi -> zero divergence.
vec2 curlNoise(vec2 p) {
    float e = 0.1;
    float ny = vnoise(p + vec2(0.0, e)) - vnoise(p - vec2(0.0, e));
    float nx = vnoise(p + vec2(e, 0.0)) - vnoise(p - vec2(e, 0.0));
    return vec2(ny, -nx) / (2.0 * e);
}

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 px = 1.0 / RENDERSIZE;

    // --- 1. velocity from the loop image's own density gradient --------
    // central difference of luminance (same form as flow_euler's turbulence)
    float lR = luma(sampleInput(uv + vec2(px.x, 0.0)).rgb);
    float lL = luma(sampleInput(uv - vec2(px.x, 0.0)).rgb);
    float lU = luma(sampleInput(uv + vec2(0.0, px.y)).rgb);
    float lD = luma(sampleInput(uv - vec2(0.0, px.y)).rgb);
    vec2 grad = vec2(lR - lL, lU - lD) * 0.5;

    // Swirl rotates the gradient: 0.25 turn = perpendicular = circulate
    // around bright blobs; 0.0 = flow straight down the gradient (drain).
    vec2 vel = rot(grad, rotAngle) * velScale;

    // --- 2. ambient divergence-free drift so still regions stay alive --
    vel += curlNoise(uv * noiseScale + vec2(TIME * noiseDrift)) * ambient;

    // --- 3. semi-Lagrangian advection of the SAME image ---------------
    // reference displacement scale (40px at full Current) -> live-tunable
    vec2 offsetPx = vel * dt * 40.0;
    vec3 c = sampleInput(uv - offsetPx * px).rgb;

    // --- 4. viscosity: mix in a 3x3 blur of the pre-advection image ----
    if (diffusion > 0.0) {
        vec3 b = vec3(0.0);
        b += sampleInput(uv + vec2(-px.x, -px.y)).rgb;
        b += sampleInput(uv + vec2( 0.0,  -px.y)).rgb;
        b += sampleInput(uv + vec2( px.x, -px.y)).rgb;
        b += sampleInput(uv + vec2(-px.x,  0.0 )).rgb;
        b += sampleInput(uv).rgb;
        b += sampleInput(uv + vec2( px.x,  0.0 )).rgb;
        b += sampleInput(uv + vec2(-px.x,  px.y)).rgb;
        b += sampleInput(uv + vec2( 0.0,   px.y)).rgb;
        b += sampleInput(uv + vec2( px.x,  px.y)).rgb;
        c = mix(c, b / 9.0, diffusion);
    }

    // --- 5. dissipation (default 1.0: host feedback opacity already decays)
    c *= persistence;

    gl_FragColor = vec4(c, 1.0);
}
