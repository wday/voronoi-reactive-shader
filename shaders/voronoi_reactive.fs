/*{
    "ISFVSN": "2",
    "DESCRIPTION": "Reactive layered Voronoi — 3-layer multi-scale Voronoi with animated seeds, edge glow, spatial warp, and HSV coloring. Designed as a control surface for external audio reactivity.",
    "CREDIT": "wday | Hash functions: Dave Hoskins | Voronoi: Worley/Quilez | HSV: Sam Hocevar",
    "CATEGORIES": [
        "Fx",
        "Generator"
    ],
    "INPUTS": [
        {
            "NAME": "density",
            "LABEL": "Density",
            "TYPE": "float",
            "MIN": 2.0,
            "MAX": 30.0,
            "DEFAULT": 8.0
        },
        {
            "NAME": "layerSpread",
            "LABEL": "Layer Spread",
            "TYPE": "float",
            "MIN": 1.5,
            "MAX": 4.0,
            "DEFAULT": 2.0
        },
        {
            "NAME": "layerMix",
            "LABEL": "Layer Mix",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.5
        },
        {
            "NAME": "driftSpeed",
            "LABEL": "Drift Speed",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 3.0,
            "DEFAULT": 0.5
        },
        {
            "NAME": "driftChaos",
            "LABEL": "Drift Chaos",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.3
        },
        {
            "NAME": "edgeWidth",
            "LABEL": "Edge Width",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 0.15,
            "DEFAULT": 0.04
        },
        {
            "NAME": "edgeGlow",
            "LABEL": "Edge Glow",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.4
        },
        {
            "NAME": "warp",
            "LABEL": "Warp",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.0
        },
        {
            "NAME": "colorShift",
            "LABEL": "Color Shift",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.0
        },
        {
            "NAME": "colorSat",
            "LABEL": "Color Sat",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.7
        },
        {
            "NAME": "brightness",
            "LABEL": "Brightness",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 2.0,
            "DEFAULT": 1.0
        },
        {
            "NAME": "contrast",
            "LABEL": "Contrast",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 2.0,
            "DEFAULT": 1.0
        },
        {
            "NAME": "tint",
            "LABEL": "Tint",
            "TYPE": "color",
            "DEFAULT": [1.0, 1.0, 1.0, 1.0]
        },
        {
            "NAME": "inputImage",
            "TYPE": "image"
        },
        {
            "NAME": "blendAmount",
            "LABEL": "Image Blend",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.0
        },
        {
            "NAME": "imageInfluence",
            "LABEL": "Image Influence",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.0
        },
        {
            "NAME": "kernelRadius",
            "LABEL": "NC Kernel",
            "TYPE": "float",
            "MIN": 0.01,
            "MAX": 0.5,
            "DEFAULT": 0.1
        },
        {
            "NAME": "certContrast",
            "LABEL": "Cert Contrast",
            "TYPE": "float",
            "MIN": 0.1,
            "MAX": 5.0,
            "DEFAULT": 1.0
        },
        {
            "NAME": "certBrightness",
            "LABEL": "Cert Brightness",
            "TYPE": "float",
            "MIN": 0.0,
            "MAX": 1.0,
            "DEFAULT": 0.0
        }
    ]
}*/


// --- Hash functions (Dave Hoskins, no sine) ---

float hash1(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 hash2(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}


// --- Smooth value noise for spatial warp ---

vec2 valueNoise2(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);

    vec2 a = hash2(i) - 0.5;
    vec2 b = hash2(i + vec2(1.0, 0.0)) - 0.5;
    vec2 c = hash2(i + vec2(0.0, 1.0)) - 0.5;
    vec2 d = hash2(i + vec2(1.0, 1.0)) - 0.5;

    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}


// --- HSV to RGB ---

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}


// --- NC output: smooth local certainty (set by voronoiLayer) ---
float ncLocalCertainty = 0.0;

// --- Image certainty: luminance as importance ---

float imageCertainty(vec2 seedUV) {
    vec4 col = IMG_NORM_PIXEL(inputImage, clamp(seedUV, 0.0, 1.0));
    return dot(col.rgb, vec3(0.299, 0.587, 0.114));
}


// --- Voronoi for a single layer ---
// Returns vec4(F1, F2, cellID.x, cellID.y)

vec4 voronoiLayer(vec2 uv, float scale, float animTime, float aspect) {
    // Sample local image brightness at fragment position
    vec2 fragNormUV = vec2(uv.x / aspect, uv.y);
    float localBright = imageCertainty(fragNormUV);

    // Modulate density: bright areas → tighter cells, dark areas → larger cells
    // imageInfluence=0 → no modulation; imageInfluence=1 → 4:1 density ratio
    float densityMult = mix(1.0, 0.5 + 1.5 * localBright, imageInfluence);
    float modScale = scale * densityMult;

    vec2 p = uv * modScale;
    vec2 cell = floor(p);
    vec2 localP = fract(p);

    float f1 = 10.0;
    float f2 = 10.0;
    vec2 nearestCell = vec2(0.0);

    // NC accumulators: weighted certainty across neighbor seeds
    float certWeightedSum = 0.0;
    float certWeightSum = 0.0;

    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 neighbor = vec2(float(i), float(j));
            vec2 cellPos = cell + neighbor;

            // Seed position within cell: hash in [0.1, 0.9] for safe margin
            vec2 seedHash = hash2(cellPos);
            vec2 seedBase = seedHash * 0.8 + 0.1;

            // Sample image certainty at seed position
            vec2 seedWorldUV = (cellPos + seedBase) / modScale;
            vec2 seedNormUV = vec2(seedWorldUV.x / aspect, seedWorldUV.y);
            float rawCert = imageCertainty(seedNormUV);
            float cert = pow(rawCert, certContrast) * imageInfluence;

            // NC: accumulate certainty with Gaussian applicability
            vec2 point0 = neighbor + seedBase;
            float dist0 = length(point0 - localP);
            float kr = max(kernelRadius * modScale, 0.5);
            float ncWeight = exp(-0.5 * (dist0 * dist0) / (kr * kr));
            certWeightedSum += cert * ncWeight;
            certWeightSum += ncWeight;

            // Circular drift: smooth orbit around base
            float angle = 6.2832 * seedHash.x + animTime * (0.3 + seedHash.y * 0.7);
            vec2 circularDrift = 0.35 * vec2(cos(angle), sin(angle));

            // Chaotic drift: smoothly interpolated random walk
            float phase = floor(animTime * 0.3);
            float blend = fract(animTime * 0.3);
            blend = blend * blend * (3.0 - 2.0 * blend);
            vec2 rA = hash2(cellPos + vec2(phase * 17.3, phase * 7.1)) - 0.5;
            vec2 rB = hash2(cellPos + vec2((phase + 1.0) * 17.3, (phase + 1.0) * 7.1)) - 0.5;
            vec2 chaoticDrift = mix(rA, rB, blend) * 0.7;

            // Certainty modulates drift: high cert → anchored, low cert → free drift
            float driftScale = 1.0 - cert * 0.8;
            vec2 drift = mix(circularDrift, chaoticDrift, driftChaos) * driftScale;
            vec2 point = neighbor + seedBase + drift;

            // Raw distance — density variation handled by grid scale modulation
            float dist = length(point - localP);

            if (dist < f1) {
                f2 = f1;
                f1 = dist;
                nearestCell = cellPos;
            } else if (dist < f2) {
                f2 = dist;
            }
        }
    }

    // NC result: smooth local certainty at this pixel
    float localCert = certWeightSum > 0.001 ? certWeightedSum / certWeightSum : 0.0;
    ncLocalCertainty = localCert;

    return vec4(f1, f2, nearestCell);
}


// --- Main ---

void main() {
    vec2 uv = isf_FragNormCoord;
    float aspect = RENDERSIZE.x / RENDERSIZE.y;
    uv.x *= aspect;

    // Spatial warp
    if (warp > 0.001) {
        vec2 warpOffset = valueNoise2(uv * 3.0 + TIME * 0.08);
        warpOffset += valueNoise2(uv * 7.0 - TIME * 0.05) * 0.5;
        uv += warpOffset * warp * 0.25;
    }

    float animTime = TIME * driftSpeed;

    vec3 color = vec3(0.0);
    float totalWeight = 0.0;

    // 3 Voronoi layers at increasing scales
    for (int layer = 0; layer < 3; layer++) {
        float fl = float(layer);
        float scale = density * pow(layerSpread, fl);
        float layerWeight = pow(layerMix, fl);
        if (layer == 0) layerWeight = 1.0;

        vec4 vor = voronoiLayer(uv, scale, animTime + fl * 1.7, aspect);

        float edgeDist = vor.y - vor.x;

        // Cell color: hue from cell ID hash, value from certainty
        float hue = fract(hash1(vor.zw) + colorShift + fl * 0.15);
        float cellValue = mix(0.55, ncLocalCertainty, certBrightness);
        vec3 cellRGB = hsv2rgb(vec3(hue, colorSat, cellValue));

        // Edge detection: hard edge + soft glow
        float edgeFactor = 1.0 - smoothstep(0.0, max(edgeWidth, 0.001), edgeDist);
        float glowRange = edgeWidth * (1.0 + edgeGlow * 4.0);
        float glowFactor = (1.0 - smoothstep(0.0, max(glowRange, 0.001), edgeDist)) * edgeGlow;
        float totalEdge = clamp(max(edgeFactor, glowFactor), 0.0, 1.0);

        // Edge color: brightness tracks local image brightness
        // When imageInfluence=0, edges are full brightness; otherwise they dim in dark areas
        float normCert = imageInfluence > 0.001 ? ncLocalCertainty / imageInfluence : 1.0;
        float edgeBright = mix(1.0, mix(0.15, 1.0, normCert), imageInfluence);
        vec3 edgeRGB = hsv2rgb(vec3(hue, colorSat * 0.2, edgeBright));
        vec3 layerColor = mix(cellRGB, edgeRGB, totalEdge);

        // Contrast: stretch around midpoint
        layerColor = clamp((layerColor - 0.5) * contrast + 0.5, 0.0, 1.0);

        color += layerColor * layerWeight;
        totalWeight += layerWeight;
    }

    color /= max(totalWeight, 0.001);
    color *= brightness;
    color *= tint.rgb;

    vec4 inputColor = IMG_THIS_PIXEL(inputImage);
    color = mix(color, inputColor.rgb, blendAmount);

    gl_FragColor = vec4(color, tint.a);
}
