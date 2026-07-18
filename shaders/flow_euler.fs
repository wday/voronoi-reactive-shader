/*{
  "DESCRIPTION": "Euler Soup — stable-fluid + dye soup, or raw Eulerian field. SOUP mode (default): a persistent velocity field (self-advecting, vorticity confinement, viscosity, image-driven turbulence) carries the input's RGB colour as dye — watery diffusion you can steer and grade. FIELD mode: original velocity/density solver (RG=velocity, B=curl, A=density) for Channel Displace / flow-lagrange.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX", "Flow"],
  "INPUTS": [
    { "NAME": "inputImage", "TYPE": "image", "LABEL": "Input" },
    { "NAME": "mode", "TYPE": "long", "LABEL": "Mode", "DEFAULT": 0, "VALUES": [0, 1], "LABELS": ["Soup", "Field"] },

    { "NAME": "flowAngle",   "TYPE": "float", "LABEL": "Flow Angle",   "DEFAULT": 0.0,  "MIN": 0.0, "MAX": 1.0 },
    { "NAME": "flowSpeed",   "TYPE": "float", "LABEL": "Flow Speed",   "DEFAULT": 0.0,  "MIN": 0.0, "MAX": 1.0 },
    { "NAME": "flowScale",   "TYPE": "float", "LABEL": "Flow Scale",   "DEFAULT": 0.05, "MIN": 0.0, "MAX": 0.2 },
    { "NAME": "curlStrength","TYPE": "float", "LABEL": "Curl / Vorticity", "DEFAULT": 0.6, "MIN": 0.0, "MAX": 3.0 },
    { "NAME": "stir",        "TYPE": "float", "LABEL": "Turbulence",   "DEFAULT": 0.6,  "MIN": 0.0, "MAX": 3.0 },
    { "NAME": "viscosity",   "TYPE": "float", "LABEL": "Viscosity",    "DEFAULT": 0.12, "MIN": 0.0, "MAX": 1.0 },
    { "NAME": "dt",          "TYPE": "float", "LABEL": "Sim Rate",     "DEFAULT": 0.5,  "MIN": 0.01,"MAX": 2.0 },

    { "NAME": "diffusion",   "TYPE": "float", "LABEL": "Diffusion",    "DEFAULT": 0.1,  "MIN": 0.0, "MAX": 1.0 },
    { "NAME": "decayRate",   "TYPE": "float", "LABEL": "Persistence",  "DEFAULT": 0.98, "MIN": 0.8, "MAX": 1.0 },
    { "NAME": "inputMix",    "TYPE": "float", "LABEL": "Input Mix",    "DEFAULT": 0.15, "MIN": 0.0, "MAX": 1.0 },

    { "NAME": "hueShift",    "TYPE": "float", "LABEL": "Hue Shift",    "DEFAULT": 0.0,  "MIN": 0.0, "MAX": 1.0 },
    { "NAME": "saturation",  "TYPE": "float", "LABEL": "Saturation",   "DEFAULT": 1.0,  "MIN": 0.0, "MAX": 2.0 },
    { "NAME": "gain",        "TYPE": "float", "LABEL": "Gain",         "DEFAULT": 1.0,  "MIN": 0.0, "MAX": 2.0 },
    { "NAME": "tint",        "TYPE": "color", "LABEL": "Tint",         "DEFAULT": [1.0, 1.0, 1.0, 1.0] },

    { "NAME": "boundaryMode","TYPE": "long",  "LABEL": "Boundary", "DEFAULT": 0, "VALUES": [0, 1, 2], "LABELS": ["Wrap", "Reflect", "Absorb"] }
  ],
  "PASSES": [
    { "TARGET": "velB", "PERSISTENT": true, "FLOAT": true, "DESCRIPTION": "Velocity update: reads velA → velB" },
    { "TARGET": "velA", "PERSISTENT": true, "FLOAT": true, "DESCRIPTION": "Velocity viscosity/advect: reads velB → velA" },
    { "TARGET": "dyeB", "PERSISTENT": true, "FLOAT": true, "DESCRIPTION": "Advect dye through velocity: reads dyeA + velA → dyeB" },
    { "TARGET": "dyeA", "PERSISTENT": true, "FLOAT": true, "DESCRIPTION": "Dye diffusion: reads dyeB → dyeA" },
    { "DESCRIPTION": "Output — colour-grade dye (Soup) or raw field (Field)" }
  ]
}*/

// -----------------------------------------------------------------------
// Flow Euler
//
// SOUP mode (default) — stable-fluid + dye.
//   velA/velB (ping-pong): velocity field. RG = velocity, B = curl.
//     Self-advects (momentum), vorticity confinement (Curl), viscosity,
//     and an image-gradient "Turbulence" force that stirs the fluid.
//   dyeA/dyeB (ping-pong): RGB dye = the input colour, carried along the
//     velocity field's currents. This is what you see — watery, not blurry,
//     because the velocity field has memory and forms coherent eddies.
//
// FIELD mode — original Eulerian solver on velA/velB (RG=vel, B=curl,
//   A=density). Dye passes are pass-through; output is the raw field.
//
// Passes: P0 velA→velB, P1 velB→velA, P2 dyeA→velA-advected→dyeB,
//   P3 dyeB→dyeA, P4 output. After a frame velA & dyeA hold current state.
// -----------------------------------------------------------------------

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

// IMG_NORM_PIXEL is a macro — needs a literal sampler name, so one helper each.
vec4 sampleVelA(vec2 uv) { if (!applyBoundary(uv)) return vec4(0.0); return IMG_NORM_PIXEL(velA, uv); }
vec4 sampleVelB(vec2 uv) { if (!applyBoundary(uv)) return vec4(0.0); return IMG_NORM_PIXEL(velB, uv); }
vec4 sampleDyeA(vec2 uv) { if (!applyBoundary(uv)) return vec4(0.0); return IMG_NORM_PIXEL(dyeA, uv); }
vec4 sampleDyeB(vec2 uv) { if (!applyBoundary(uv)) return vec4(0.0); return IMG_NORM_PIXEL(dyeB, uv); }

float luma(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));
    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 px = 1.0 / RENDERSIZE;
    vec2 dx = vec2(px.x, 0.0);
    vec2 dy = vec2(0.0, px.y);

    // ==================================================================
    // SOUP MODE — stable-fluid + dye
    // ==================================================================
    if (mode == 0) {

        // -------------------------------------------------------------
        // PASS 0 — velocity update → velB
        // -------------------------------------------------------------
        if (PASSINDEX == 0) {

            vec4 vC = sampleVelA(uv);
            vec4 vR = sampleVelA(uv + dx);
            vec4 vL = sampleVelA(uv - dx);
            vec4 vU = sampleVelA(uv + dy);
            vec4 vD = sampleVelA(uv - dy);

            // Self-advection (momentum): carry velocity along itself.
            vec2 v = sampleVelA(uv - vC.rg * flowScale).rg;

            // Curl of the velocity field.
            float curl = ((vR.g - vL.g) - (vU.r - vD.r)) * 0.5;

            // Vorticity confinement — push velocity toward higher curl,
            // sustaining eddies instead of letting them diffuse away.
            vec2 curlForce = vec2(0.0);
            if (curlStrength > 0.0) {
                vec2 cg = vec2(abs(vR.b) - abs(vL.b), abs(vU.b) - abs(vD.b));
                float l = length(cg);
                if (l > 1e-5) {
                    cg /= l;
                    curlForce = vec2(cg.y, -cg.x) * curl * curlStrength;
                }
            }

            // Turbulence — the dye's luminance gradient stirs the fluid
            // (rotated 90° → rotational forcing, so the image seeds swirls).
            vec3 dR = sampleDyeA(uv + dx).rgb;
            vec3 dL = sampleDyeA(uv - dx).rgb;
            vec3 dU = sampleDyeA(uv + dy).rgb;
            vec3 dD = sampleDyeA(uv - dy).rgb;
            vec2 g = vec2(luma(dR) - luma(dL), luma(dU) - luma(dD)) * 0.5;
            vec2 stirForce = vec2(-g.y, g.x) * stir;

            v += (curlForce + stirForce) * dt;

            // Damp + clamp so the field stays energetic but bounded.
            v *= 0.985;
            v = clamp(v, -3.0, 3.0);

            gl_FragColor = vec4(v, curl, 1.0);

        // -------------------------------------------------------------
        // PASS 1 — velocity viscous diffusion → velA
        // -------------------------------------------------------------
        } else if (PASSINDEX == 1) {

            vec4 c = sampleVelB(uv);
            vec2 lap = (sampleVelB(uv + dx).rg + sampleVelB(uv - dx).rg
                      + sampleVelB(uv + dy).rg + sampleVelB(uv - dy).rg) * 0.25 - c.rg;
            vec2 v = c.rg + lap * viscosity;
            gl_FragColor = vec4(v, c.b, 1.0);

        // -------------------------------------------------------------
        // PASS 2 — advect dye through velocity + inject → dyeB
        // -------------------------------------------------------------
        } else if (PASSINDEX == 2) {

            vec2 vel = sampleVelA(uv).rg;
            float theta = flowAngle * 6.28318530718;
            vec2 global = vec2(cos(theta), sin(theta)) * flowSpeed;

            // Semi-Lagrangian: where did this dye come from?
            vec2 srcUV = uv - (vel + global) * flowScale;
            vec3 col = sampleDyeA(srcUV).rgb * decayRate;

            // Inject fresh input (or the delayed feedback signal).
            vec3 inp = IMG_NORM_PIXEL(inputImage, uv).rgb;
            col = mix(col, inp, inputMix);

            gl_FragColor = vec4(col, 1.0);

        // -------------------------------------------------------------
        // PASS 3 — dye diffusion → dyeA
        // -------------------------------------------------------------
        } else if (PASSINDEX == 3) {

            vec4 c = sampleDyeB(uv);
            vec4 blur = (sampleDyeB(uv + dx) + sampleDyeB(uv - dx)
                       + sampleDyeB(uv + dy) + sampleDyeB(uv - dy)) * 0.25;
            gl_FragColor = mix(c, blur, diffusion);

        // -------------------------------------------------------------
        // PASS 4 — colour grade + output
        // -------------------------------------------------------------
        } else {

            vec3 c = max(IMG_NORM_PIXEL(dyeA, uv).rgb, 0.0);
            vec3 hsv = rgb2hsv(c);
            hsv.x = fract(hsv.x + hueShift);
            hsv.y = clamp(hsv.y * saturation, 0.0, 1.0);
            c = hsv2rgb(hsv);
            c *= gain * tint.rgb;
            gl_FragColor = vec4(clamp(c, 0.0, 1.0), 1.0);

        }

    // ==================================================================
    // FIELD MODE — original Eulerian solver (velA/velB), dye pass-through
    // ==================================================================
    } else {

        // PASS 0 — inject + velocity update → velB
        if (PASSINDEX == 0) {

            vec4 s = sampleVelA(uv);
            vec4 input_field = IMG_NORM_PIXEL(inputImage, uv);
            s.rg = mix(s.rg, input_field.rg, inputMix);
            s.a  = mix(s.a,  input_field.a,  inputMix);

            vec4 sR = sampleVelA(uv + dx);
            vec4 sL = sampleVelA(uv - dx);
            vec4 sU = sampleVelA(uv + dy);
            vec4 sD = sampleVelA(uv - dy);

            vec2 pressureGrad = vec2(sR.a - sL.a, sU.a - sD.a) * 0.5;
            float curl = ((sR.g - sL.g) - (sU.r - sD.r)) * 0.5;

            vec2 curlForce = vec2(0.0);
            if (curlStrength > 0.0) {
                vec2 curlGrad = vec2(abs(sR.b) - abs(sL.b), abs(sU.b) - abs(sD.b));
                float curlGradLen = length(curlGrad);
                if (curlGradLen > 1e-5) {
                    curlGrad /= curlGradLen;
                    curlForce = vec2(curlGrad.y, -curlGrad.x) * curl * curlStrength;
                }
            }

            vec2 laplacian = (sR.rg + sL.rg + sU.rg + sD.rg) * 0.25 - s.rg;
            s.rg += (-pressureGrad + curlForce + laplacian * viscosity) * dt;
            s.b = curl;
            gl_FragColor = s;

        // PASS 1 — advect + decay → velA
        } else if (PASSINDEX == 1) {

            vec4 s = sampleVelB(uv);
            vec4 advected = sampleVelB(uv - s.rg * dt * 0.01);
            advected.rg *= decayRate;
            advected.a  *= decayRate;
            gl_FragColor = advected;

        // PASS 2/3 — dye pass-through (unused in Field mode)
        } else if (PASSINDEX == 2) {
            gl_FragColor = IMG_NORM_PIXEL(dyeA, uv);
        } else if (PASSINDEX == 3) {
            gl_FragColor = IMG_NORM_PIXEL(dyeB, uv);

        // PASS 4 — output raw field
        } else {
            gl_FragColor = IMG_NORM_PIXEL(velA, uv);
        }
    }
}
