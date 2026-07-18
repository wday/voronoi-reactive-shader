/*{
  "DESCRIPTION": "Wavefolder — stateless multi-stage intensity folder. Reflects/wraps over-range pixel values back into [0,1] instead of clipping, so blown-out highlights fold into contoured color bands rather than flat white. Per-channel RGB (neutral blowout breaks into color fringes). Each stage applies x <- fold(x*Drive + Bias); Stages sets the iteration count. Shape morphs the fold curve Triangle -> Sine -> Wrap. Built to tame blowout in uncontrollable live feedback (camera-in-a-room, hot projectors) while adding fractal/harmonic structure. No buffers.",
  "CREDIT": "wday",
  "ISFVSN": "2",
  "CATEGORIES": ["FX"],
  "INPUTS": [
    { "NAME": "inputImage", "TYPE": "image", "LABEL": "Input" },

    { "NAME": "drive",  "TYPE": "float", "LABEL": "Drive",  "DEFAULT": 1.8, "MIN": 1.0,  "MAX": 8.0 },
    { "NAME": "stages", "TYPE": "float", "LABEL": "Stages", "DEFAULT": 2.0, "MIN": 1.0,  "MAX": 8.0 },
    { "NAME": "shape",  "TYPE": "float", "LABEL": "Shape",  "DEFAULT": 0.0, "MIN": 0.0,  "MAX": 1.0 },
    { "NAME": "bias",   "TYPE": "float", "LABEL": "Bias",   "DEFAULT": 0.0, "MIN": -0.5, "MAX": 0.5 },
    { "NAME": "wet",    "TYPE": "float", "LABEL": "Mix",    "DEFAULT": 1.0, "MIN": 0.0,  "MAX": 1.0 }
  ]
}*/

// -----------------------------------------------------------------------
// Wavefolder — bounded nonlinearity for blowout control.
//
// A real out-of-the-box feedback loop (camera -> projector -> camera) can't
// be held under unity gain in ambient light: it clips to white. A wavefolder
// is the analog answer — fold the excess back instead of saturating. Cascade
// several folds and a smooth over-bright ramp becomes self-similar contour
// bands. Per-channel, so neutral blowout shatters into color.
//
// Fold curves (all map [0, inf) -> [0,1], applied component-wise):
//   Triangle  1 - |mod(x,2) - 1|      hard reflect, identity on [0,1]
//   Sine      0.5 - 0.5*cos(PI*x)     smooth Buchla fold
//   Wrap      fract(x)                hard modulo, glitchy
// -----------------------------------------------------------------------

#define PI 3.14159265358979

vec3 foldTri(vec3 x)  { return 1.0 - abs(mod(x, 2.0) - 1.0); }
vec3 foldSin(vec3 x)  { return 0.5 - 0.5 * cos(PI * x); }
vec3 foldWrap(vec3 x) { return fract(x); }

// Morph Triangle(0) -> Sine(0.5) -> Wrap(1) on a single knob.
vec3 foldShape(vec3 x, float s) {
    vec3 t  = foldTri(x);
    vec3 si = foldSin(x);
    vec3 w  = foldWrap(x);
    if (s < 0.5) return mix(t, si, s * 2.0);
    return mix(si, w, (s - 0.5) * 2.0);
}

void main() {
    vec4 src = IMG_THIS_PIXEL(inputImage);
    vec3 dry = src.rgb;

    // Multi-stage fold. Fixed max 8 (GLSL needs a constant bound); break past
    // the requested stage count. The stage straddling the fractional part of
    // `stages` is partially applied for smooth knob automation.
    vec3 c = dry;
    float st = clamp(stages, 0.0, 8.0);
    for (int i = 0; i < 8; i++) {
        if (float(i) >= st) break;
        float w = clamp(st - float(i), 0.0, 1.0);      // 1.0 for full stages, frac for the last
        vec3 folded = foldShape(c * drive + bias, shape);
        c = mix(c, folded, w);
    }

    vec3 outc = mix(dry, c, wet);
    gl_FragColor = vec4(clamp(outc, 0.0, 1.0), src.a);
}
