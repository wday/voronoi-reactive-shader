/*{
  "DESCRIPTION": "Stereo depth from side-by-side image pair. Input is a single texture with left camera in the left half, right camera in the right half (hstacked). Outputs greyscale depth via block-matching disparity. Fast over accurate — for VJ.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX", "Depth"],
  "INPUTS": [
    {
      "NAME": "inputImage",
      "TYPE": "image",
      "LABEL": "Stereo Pair (Side-by-Side)"
    },
    {
      "NAME": "hShift",
      "TYPE": "float",
      "LABEL": "H Align",
      "DEFAULT": 0.0,
      "MIN": -0.3,
      "MAX": 0.3
    },
    {
      "NAME": "vShift",
      "TYPE": "float",
      "LABEL": "V Align",
      "DEFAULT": 0.0,
      "MIN": -0.15,
      "MAX": 0.15
    },
    {
      "NAME": "searchRange",
      "TYPE": "float",
      "LABEL": "Search Range",
      "DEFAULT": 0.2,
      "MIN": 0.01,
      "MAX": 0.5
    },
    {
      "NAME": "searchSteps",
      "TYPE": "float",
      "LABEL": "Search Steps",
      "DEFAULT": 16.0,
      "MIN": 4.0,
      "MAX": 32.0
    },
    {
      "NAME": "blockRadius",
      "TYPE": "float",
      "LABEL": "Block Radius",
      "DEFAULT": 2.0,
      "MIN": 1.0,
      "MAX": 6.0
    },
    {
      "NAME": "temporalBlend",
      "TYPE": "float",
      "LABEL": "Temporal Blend",
      "DEFAULT": 0.8,
      "MIN": 0.0,
      "MAX": 0.98
    },
    {
      "NAME": "depthGamma",
      "TYPE": "float",
      "LABEL": "Depth Gamma",
      "DEFAULT": 1.0,
      "MIN": 0.2,
      "MAX": 4.0
    },
    {
      "NAME": "invertDepth",
      "TYPE": "bool",
      "LABEL": "Invert Depth",
      "DEFAULT": false
    },
    {
      "NAME": "swapCameras",
      "TYPE": "bool",
      "LABEL": "Swap L/R",
      "DEFAULT": false
    }
  ],
  "PASSES": [
    {
      "TARGET": "rawDisp",
      "WIDTH": "$WIDTH/4",
      "HEIGHT": "$HEIGHT/4",
      "DESCRIPTION": "Block-match disparity at 1/4 res."
    },
    {
      "TARGET": "smoothed",
      "PERSISTENT": true,
      "WIDTH": "$WIDTH/4",
      "HEIGHT": "$HEIGHT/4",
      "DESCRIPTION": "EMA temporal blend — persistent FBO."
    },
    {
      "DESCRIPTION": "Final upscale to output res."
    }
  ]
}*/

// -----------------------------------------------------------------------
// Side-by-side sampling helpers
//
// Input is a single hstacked texture: [LEFT | RIGHT]
// Left half:  u ∈ [0.0, 0.5)  →  sample with u' = u * 0.5
// Right half: u ∈ [0.5, 1.0)  →  sample with u' = 0.5 + u * 0.5
// -----------------------------------------------------------------------

vec4 sampleLeft(vec2 uv) {
    vec2 sbs = vec2(uv.x * 0.5, uv.y);
    return IMG_NORM_PIXEL(inputImage, sbs);
}

vec4 sampleRight(vec2 uv) {
    vec2 sbs = vec2(0.5 + uv.x * 0.5, uv.y);
    return IMG_NORM_PIXEL(inputImage, sbs);
}

float luma(vec4 c) {
    return dot(c.rgb, vec3(0.299, 0.587, 0.114));
}

// 9-point ring SAD: center + 4 cardinal + 4 diagonal.
// Covers a disk of radius r without a nested loop —
// constant 9 samples regardless of blockRadius value.
float ringSAD(vec2 uvL, vec2 uvR, vec2 px, float r) {
    float s = 0.0;

    s += abs(luma(sampleLeft(uvL)) - luma(sampleRight(uvR)));

    vec2 ax = vec2(r, 0.0) * px;
    vec2 ay = vec2(0.0, r) * px;
    s += abs(luma(sampleLeft(uvL + ax)) - luma(sampleRight(uvR + ax)));
    s += abs(luma(sampleLeft(uvL - ax)) - luma(sampleRight(uvR - ax)));
    s += abs(luma(sampleLeft(uvL + ay)) - luma(sampleRight(uvR + ay)));
    s += abs(luma(sampleLeft(uvL - ay)) - luma(sampleRight(uvR - ay)));

    vec2 ad = vec2(r * 0.707) * px;
    s += abs(luma(sampleLeft(uvL + ad))                  - luma(sampleRight(uvR + ad)));
    s += abs(luma(sampleLeft(uvL + vec2(-ad.x,  ad.y))) - luma(sampleRight(uvR + vec2(-ad.x,  ad.y))));
    s += abs(luma(sampleLeft(uvL + vec2( ad.x, -ad.y))) - luma(sampleRight(uvR + vec2( ad.x, -ad.y))));
    s += abs(luma(sampleLeft(uvL - ad))                  - luma(sampleRight(uvR - ad)));

    return s / 9.0;
}

// -----------------------------------------------------------------------
// Main
// -----------------------------------------------------------------------

void main() {
    vec2 uv = isf_FragNormCoord;

    // ----------------------------------------------------------------
    // PASS 0 — Disparity search at 1/4 resolution
    // ----------------------------------------------------------------
    if (PASSINDEX == 0) {

        // Resolve swap
        vec2 uvL = uv;
        vec2 uvR_base = uv + vec2(hShift, vShift);
        if (swapCameras) {
            uvL = uv + vec2(hShift, vShift);
            uvR_base = uv;
        }

        // Texel size in the logical camera space (each half is its own image)
        // Width of each camera = half the input texture width
        vec2 px = vec2(2.0, 1.0) / IMG_SIZE(inputImage);
        float r   = blockRadius;
        int   steps = int(clamp(searchSteps, 4.0, 32.0));

        float bestSAD  = 1e6;
        float bestDisp = 0.0;

        for (int i = 0; i <= steps; i++) {
            float t    = float(i) / float(steps);
            float disp = t * searchRange;

            vec2 uvR = uvR_base - vec2(disp, 0.0);

            float s = ringSAD(uvL, uvR, px, r);
            if (s < bestSAD) {
                bestSAD  = s;
                bestDisp = disp;
            }
        }

        float depth = bestDisp / searchRange;
        depth = pow(clamp(depth, 0.0, 1.0), 1.0 / max(depthGamma, 0.01));
        if (invertDepth) depth = 1.0 - depth;

        float confidence = 1.0 - clamp(bestSAD * 6.0, 0.0, 1.0);

        gl_FragColor = vec4(vec3(depth), confidence);

    // ----------------------------------------------------------------
    // PASS 1 — Confidence-weighted temporal EMA (persistent FBO)
    // ----------------------------------------------------------------
    } else if (PASSINDEX == 1) {

        vec4 current  = IMG_NORM_PIXEL(rawDisp,  uv);
        vec4 previous = IMG_NORM_PIXEL(smoothed, uv);

        float baseAlpha = 1.0 - temporalBlend;
        float alpha = baseAlpha * current.a;
        alpha = clamp(alpha, baseAlpha * 0.05, 1.0);

        float depth = mix(previous.r, current.r, alpha);

        gl_FragColor = vec4(vec3(depth), 1.0);

    // ----------------------------------------------------------------
    // PASS 2 — Upscale & output
    // ----------------------------------------------------------------
    } else {

        float d = IMG_NORM_PIXEL(smoothed, uv).r;
        gl_FragColor = vec4(vec3(d), 1.0);

    }
}
