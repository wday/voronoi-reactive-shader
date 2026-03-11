#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform float u_r_base;
uniform float u_sensitivity;
uniform int u_spatial_mode; // 0=off, 1=radial, 2=edge
uniform float u_dry_wet;
uniform vec2 u_texel_size;

// Sobel edge magnitude
float sobel_magnitude(vec2 uv) {
    float tl = dot(texture(u_input, uv + vec2(-1, -1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float tc = dot(texture(u_input, uv + vec2( 0, -1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float tr = dot(texture(u_input, uv + vec2( 1, -1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float ml = dot(texture(u_input, uv + vec2(-1,  0) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float mr = dot(texture(u_input, uv + vec2( 1,  0) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float bl = dot(texture(u_input, uv + vec2(-1,  1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float bc = dot(texture(u_input, uv + vec2( 0,  1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));
    float br = dot(texture(u_input, uv + vec2( 1,  1) * u_texel_size).rgb, vec3(0.299, 0.587, 0.114));

    float gx = -tl - 2.0*ml - bl + tr + 2.0*mr + br;
    float gy = -tl - 2.0*tc - tr + bl + 2.0*bc + br;

    return length(vec2(gx, gy));
}

void main() {
    vec4 original = texture(u_input, v_uv);

    // Compute spatial r modifier
    float r = u_r_base;
    if (u_spatial_mode == 1) {
        // Radial: distance from center pushes r toward 4.0
        float dist = length(v_uv - 0.5) * 2.0;
        r += dist * u_sensitivity * (4.0 - u_r_base);
    } else if (u_spatial_mode == 2) {
        // Edge: Sobel magnitude pushes r toward 4.0
        float edge = sobel_magnitude(v_uv);
        r += edge * u_sensitivity * (4.0 - u_r_base);
    }
    r = clamp(r, 0.0, 4.0);

    // Apply logistic map per-channel: x' = r * x * (1 - x)
    vec4 mapped;
    mapped.r = r * original.r * (1.0 - original.r);
    mapped.g = r * original.g * (1.0 - original.g);
    mapped.b = r * original.b * (1.0 - original.b);
    mapped.a = original.a;

    out_color = mix(original, mapped, u_dry_wet);
}
