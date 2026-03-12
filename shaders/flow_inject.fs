/*{
  "DESCRIPTION": "Writes scene information into a flow field texture. Reads a depth/greyscale source and outputs: RG = velocity (depth gradient), B = scalar (depth value), A = density (edge magnitude). Place upstream of flow-euler or flow-lagrange.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX", "Flow"],
  "INPUTS": [
    {
      "NAME": "inputImage",
      "TYPE": "image",
      "LABEL": "Depth / Source"
    },
    {
      "NAME": "injectVelocity",
      "TYPE": "float",
      "LABEL": "Velocity Scale",
      "DEFAULT": 1.0,
      "MIN": 0.0,
      "MAX": 5.0
    },
    {
      "NAME": "injectDensity",
      "TYPE": "float",
      "LABEL": "Density Scale",
      "DEFAULT": 1.0,
      "MIN": 0.0,
      "MAX": 5.0
    },
    {
      "NAME": "edgeThreshold",
      "TYPE": "float",
      "LABEL": "Edge Threshold",
      "DEFAULT": 0.02,
      "MIN": 0.0,
      "MAX": 0.2
    },
    {
      "NAME": "decayRate",
      "TYPE": "float",
      "LABEL": "Decay Rate",
      "DEFAULT": 0.95,
      "MIN": 0.0,
      "MAX": 1.0
    }
  ],
  "PASSES": [
    {
      "TARGET": "fieldOut",
      "PERSISTENT": true,
      "FLOAT": true,
      "DESCRIPTION": "Persistent field texture — accumulates injected velocity/density with decay."
    },
    {
      "DESCRIPTION": "Output pass — emit field texture at full res."
    }
  ]
}*/

// -----------------------------------------------------------------------
// Flow Inject — depth → field texture
//
// Field texture convention (shared across flow plugins):
//   R = velocity X
//   G = velocity Y
//   B = scalar (depth / pressure / curl — context-dependent)
//   A = density / weight
// -----------------------------------------------------------------------

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 px = 1.0 / RENDERSIZE;

    // ------------------------------------------------------------------
    // PASS 0 — Inject into persistent field texture
    // ------------------------------------------------------------------
    if (PASSINDEX == 0) {

        // Sample depth at this pixel and neighbours for gradient
        float d  = IMG_NORM_PIXEL(inputImage, uv).r;
        float dR = IMG_NORM_PIXEL(inputImage, uv + vec2(px.x, 0.0)).r;
        float dL = IMG_NORM_PIXEL(inputImage, uv - vec2(px.x, 0.0)).r;
        float dU = IMG_NORM_PIXEL(inputImage, uv + vec2(0.0, px.y)).r;
        float dD = IMG_NORM_PIXEL(inputImage, uv - vec2(0.0, px.y)).r;

        // Central difference gradient — particles flow along depth surfaces
        vec2 grad = vec2(dR - dL, dU - dD) * 0.5;

        // Gradient magnitude → edge detection for density injection
        float edgeMag = length(grad);
        float density = smoothstep(edgeThreshold, edgeThreshold + 0.05, edgeMag)
                      * injectDensity;

        // Scale gradient → velocity
        vec2 velocity = grad * injectVelocity;

        // Read previous field state and decay
        vec4 prev = IMG_NORM_PIXEL(fieldOut, uv);
        prev.rg *= decayRate;
        prev.a  *= decayRate;

        // Accumulate new injection on top of decayed state
        vec4 field;
        field.r = prev.r + velocity.x;
        field.g = prev.g + velocity.y;
        field.b = d;                    // scalar = raw depth, no decay
        field.a = prev.a + density;

        gl_FragColor = field;

    // ------------------------------------------------------------------
    // PASS 1 — Output
    // ------------------------------------------------------------------
    } else {

        gl_FragColor = IMG_NORM_PIXEL(fieldOut, uv);

    }
}
