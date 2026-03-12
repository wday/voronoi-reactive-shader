/*{
  "DESCRIPTION": "Eulerian fluid simulation — velocity/density solver with semi-Lagrangian advection on ping-pong persistent textures. Input: field texture from flow-inject (or any RGBA with RG=velocity, A=density). Output: evolved field texture (same convention). Route output to flow-lagrange for particles, or to Channel Displace for fluid warp.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX", "Flow"],
  "INPUTS": [
    {
      "NAME": "inputImage",
      "TYPE": "image",
      "LABEL": "Field Input"
    },
    {
      "NAME": "dt",
      "TYPE": "float",
      "LABEL": "Timestep",
      "DEFAULT": 0.5,
      "MIN": 0.01,
      "MAX": 2.0
    },
    {
      "NAME": "viscosity",
      "TYPE": "float",
      "LABEL": "Viscosity",
      "DEFAULT": 0.1,
      "MIN": 0.0,
      "MAX": 1.0
    },
    {
      "NAME": "decayRate",
      "TYPE": "float",
      "LABEL": "Decay",
      "DEFAULT": 0.98,
      "MIN": 0.8,
      "MAX": 1.0
    },
    {
      "NAME": "curlStrength",
      "TYPE": "float",
      "LABEL": "Curl",
      "DEFAULT": 0.0,
      "MIN": 0.0,
      "MAX": 2.0
    },
    {
      "NAME": "boundaryMode",
      "TYPE": "long",
      "LABEL": "Boundary",
      "DEFAULT": 0,
      "VALUES": [0, 1, 2],
      "LABELS": ["Wrap", "Reflect", "Absorb"]
    },
    {
      "NAME": "inputMix",
      "TYPE": "float",
      "LABEL": "Input Mix",
      "DEFAULT": 0.1,
      "MIN": 0.0,
      "MAX": 1.0
    }
  ],
  "PASSES": [
    {
      "TARGET": "stateB",
      "PERSISTENT": true,
      "FLOAT": true,
      "DESCRIPTION": "Inject + velocity update: reads stateA, writes stateB"
    },
    {
      "TARGET": "stateA",
      "PERSISTENT": true,
      "FLOAT": true,
      "DESCRIPTION": "Advect + decay: reads stateB, writes stateA (ping-pong)"
    },
    {
      "DESCRIPTION": "Output pass — emit field texture"
    }
  ]
}*/

// -----------------------------------------------------------------------
// Flow Euler — Eulerian fluid simulation
//
// Field texture convention:
//   R = velocity X
//   G = velocity Y
//   B = curl magnitude
//   A = density
//
// Ping-pong architecture:
//   Pass 0: Read stateA → inject + velocity update → write stateB
//   Pass 1: Read stateB → advect + decay → write stateA
//   Pass 2: Output stateA
// -----------------------------------------------------------------------

// Apply boundary mode to UV coordinates.
// Returns false for absorb mode when UV is out of bounds.
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

// Macros can't take sampler2D params, so we use these helpers
// that call IMG_NORM_PIXEL on the specific named texture.

vec4 sampleA(vec2 uv) {
    if (!applyBoundary(uv)) return vec4(0.0);
    return IMG_NORM_PIXEL(stateA, uv);
}

vec4 sampleB(vec2 uv) {
    if (!applyBoundary(uv)) return vec4(0.0);
    return IMG_NORM_PIXEL(stateB, uv);
}

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 px = 1.0 / RENDERSIZE;

    // ------------------------------------------------------------------
    // PASS 0 — Inject external input + velocity update → stateB
    //
    // Read from stateA (previous frame's result). Blend in external
    // field, compute pressure gradient and curl, apply viscous diffusion.
    // ------------------------------------------------------------------
    if (PASSINDEX == 0) {

        vec4 s = sampleA(uv);
        vec4 input_field = IMG_NORM_PIXEL(inputImage, uv);

        // Blend external injection
        s.rg = mix(s.rg, input_field.rg, inputMix);
        s.a  = mix(s.a,  input_field.a,  inputMix);

        // 4-neighbour samples from stateA
        vec4 sR = sampleA(uv + vec2(px.x, 0.0));
        vec4 sL = sampleA(uv - vec2(px.x, 0.0));
        vec4 sU = sampleA(uv + vec2(0.0, px.y));
        vec4 sD = sampleA(uv - vec2(0.0, px.y));

        // Pressure gradient — density as proxy for pressure
        vec2 pressureGrad = vec2(sR.a - sL.a, sU.a - sD.a) * 0.5;

        // Curl (vorticity) — dVy/dx - dVx/dy
        float curl = ((sR.g - sL.g) - (sU.r - sD.r)) * 0.5;

        // Vorticity confinement — use stored curl magnitude (B channel)
        // to push velocity perpendicular to curl gradient
        vec2 curlForce = vec2(0.0);
        if (curlStrength > 0.0) {
            float cR = abs(sR.b);
            float cL = abs(sL.b);
            float cU = abs(sU.b);
            float cD = abs(sD.b);
            vec2 curlGrad = vec2(cR - cL, cU - cD);
            float curlGradLen = length(curlGrad);
            if (curlGradLen > 1e-5) {
                curlGrad /= curlGradLen;
                curlForce = vec2(curlGrad.y, -curlGrad.x) * curl * curlStrength;
            }
        }

        // Viscous diffusion — Laplacian of velocity
        vec2 laplacian = (sR.rg + sL.rg + sU.rg + sD.rg) * 0.25 - s.rg;

        // Apply forces
        s.rg += (-pressureGrad + curlForce + laplacian * viscosity) * dt;

        // Store curl magnitude for vorticity confinement feedback + output
        s.b = curl;

        gl_FragColor = s;

    // ------------------------------------------------------------------
    // PASS 1 — Semi-Lagrangian advection + decay → stateA
    //
    // Read from stateB (velocity-updated). Trace each cell backwards
    // through the velocity field, sample with bilinear interpolation.
    // Apply decay. Write to stateA for next frame.
    // ------------------------------------------------------------------
    } else if (PASSINDEX == 1) {

        vec4 s = sampleB(uv);

        // Trace backwards: where did the fluid at this cell come from?
        // Scale velocity from field-space to UV-space
        vec2 sourceUV = uv - s.rg * dt * 0.01;

        // Sample advected state from stateB with boundary handling
        vec4 advected = sampleB(sourceUV);

        // Decay
        advected.rg *= decayRate;
        advected.a  *= decayRate;

        gl_FragColor = advected;

    // ------------------------------------------------------------------
    // PASS 2 — Output
    // ------------------------------------------------------------------
    } else {

        gl_FragColor = IMG_NORM_PIXEL(stateA, uv);

    }
}
