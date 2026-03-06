# Implementation Plan — Normalized Convolution Image Input

## Steps

### 1. Add new ISF parameters
In `shaders/voronoi_reactive.fs` metadata, add:
- `sampleDensity` (float 0–1, default 0.5)
- `imageInfluence` (float 0–1, default 0.5)
- `kernelRadius` (float 0.01–0.5, default 0.1)

### 2. Add image sampling helper
Create a function that samples the input image at a given UV coordinate and returns luminance as certainty:
```glsl
float imageCertainty(vec2 uv) {
    vec4 col = IMG_NORM_PIXEL(inputImage, uv);
    return dot(col.rgb, vec3(0.299, 0.587, 0.114)); // luminance
}
```

### 3. Modify voronoiLayer to accept image influence
Change `voronoiLayer` signature to pass through the image influence parameters. Inside the 3x3 neighbor loop:

a. **Sample image at seed position** — convert cell-space seed position back to normalized UV, sample luminance → certainty `c`

b. **Modulate seed offset** — high certainty pulls the seed toward image features (reduce drift amplitude), low certainty lets noise dominate:
```glsl
float c = imageCertainty(seedUV);
vec2 drift = mix(noiseDrift, anchoredDrift, c * imageInfluence);
```

c. **Modulate effective distance** — bias the distance metric by certainty so high-certainty regions produce smaller/tighter cells:
```glsl
float effectiveDist = dist / (1.0 + c * imageInfluence * sampleDensity);
```

### 4. Normalized convolution of certainty
For smoother results, approximate NC in the seed loop: accumulate weighted certainty from neighboring seeds using the applicability function (Gaussian falloff based on `kernelRadius`):
```glsl
float weight = exp(-0.5 * (dist * dist) / (kernelRadius * kernelRadius));
certSum += c * weight;
weightSum += weight;
```
Then `localCertainty = certSum / max(weightSum, 0.001)` gives a smooth density estimate at each pixel.

### 5. Use localCertainty to shape the Voronoi output
- Modulate `edgeWidth` — thinner edges in high-certainty regions (more detail)
- Modulate cell color saturation or brightness by certainty (optional, subtle)

### 6. Test
- With `imageInfluence = 0`: output identical to current shader
- With webcam input + `imageInfluence > 0`: cells should cluster around bright/contrasty regions
- Sweep parameters to verify smooth transitions

### 7. Update devlog and commit

## Risk
- Performance: adding 9 texture samples per layer (one per neighbor cell) × 3 layers = 27 extra texture lookups. Should be fine for modern GPUs but worth profiling.
- The "right feel" for certainty → seed modulation will likely need tuning. Start with the simplest modulation and iterate.
