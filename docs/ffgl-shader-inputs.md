# FFGL Shader Inputs Reference

How textures and data flow from Resolume into your shader, traced through the
SDK source at `vendor/ffgl/`.

---

## Plugin types and what they receive

FFGL defines three plugin types ([FFGL.h:393–396](../vendor/ffgl/source/lib/ffgl/FFGL.h)):

| Type | Inputs | Use case |
|------|--------|----------|
| `FF_EFFECT` | 1 texture | Process incoming video |
| `FF_SOURCE` | 0 textures | Generate content |
| `FF_MIXER`  | 2+ textures | Blend/combine layers |

Set in constructor via `SetMinInputs(n)` / `SetMaxInputs(n)`
([FFGLPluginManager.h:232–239](../vendor/ffgl/source/lib/ffgl/FFGLPluginManager.h)).

A **blend shader** (fade, combine, crossfade) is an `FF_MIXER` with 2 inputs.

---

## The ProcessOpenGL call

Each frame, Resolume calls `ProcessOpenGL` with a
[ProcessOpenGLStruct](../vendor/ffgl/source/lib/ffgl/FFGL.h) (lines 558–569):

```c
typedef struct ProcessOpenGLStructTag {
    FFUInt32 numInputTextures;        // how many textures this frame
    FFGLTextureStruct** inputTextures; // array of pointers
    GLuint HostFBO;                   // host's bound FBO — restore after use
} ProcessOpenGLStruct;
```

**GL state on entry:**
- Host's FBO is bound (may be screen or offscreen capture)
- GL context is shared with Resolume — save/restore everything you touch
- Viewport may or may not match your needs — set it if rendering to your own FBO

---

## Texture struct

Each input texture is an [FFGLTextureStruct](../vendor/ffgl/source/lib/ffgl/FFGL.h) (lines 549–555):

```c
typedef struct FFGLTextureStructTag {
    FFUInt32 Width, Height;             // logical image size (content pixels)
    FFUInt32 HardwareWidth, HardwareHeight; // actual GPU texture allocation
    GLuint Handle;                      // GL texture name (from glGenTextures)
} FFGLTextureStruct;
```

### Width vs HardwareWidth

GPUs may allocate textures larger than the content (power-of-two padding,
alignment). The image sits in the top-left corner of the hardware texture.

```
+------------------+--------+
|                  |        |
|   Width x Height | padding|
|   (your image)   |        |
+------------------+        |
|       padding             |
+---------------------------+
   HardwareWidth x HardwareHeight
```

On modern GPUs with NPOT texture support, `Width == HardwareWidth` is common.
The newer Resolume FFGL SDK sets them equal explicitly
([FFGLFBO.cpp:97–98](../vendor/ffgl/source/lib/ffglex/FFGLFBO.cpp)).

### uv_scale / MaxUV

Standard fullscreen quad UVs go `[0, 1]` across the full hardware texture.
To sample only the valid image region:

```
uv_scale = [Width / HardwareWidth, Height / HardwareHeight]
```

The SDK provides a helper ([FFGLLib.h:63–75](../vendor/ffgl/source/lib/ffgl/FFGLLib.h)):

```c
inline FFGLTexCoords GetMaxGLTexCoords(FFGLTextureStruct t) {
    FFGLTexCoords texCoords;
    texCoords.s = (GLfloat)t.Width  / (GLfloat)t.HardwareWidth;
    texCoords.t = (GLfloat)t.Height / (GLfloat)t.HardwareHeight;
    return texCoords;
}
```

---

## Input textures — sampler2D binding

### Effect (1 input)

From [FFGLEffect.cpp:47–54](../vendor/ffgl/source/lib/ffglquickstart/FFGLEffect.cpp):

```cpp
ScopedSamplerActivation activateSampler0(0);           // GL_TEXTURE0
Scoped2DTextureBinding textureBinding0(tex->Handle);   // bind to unit 0
shader.Set("inputTexture", 0);                         // uniform sampler2D = unit 0
FFGLTexCoords maxCoords = GetMaxGLTexCoords(*tex);
shader.Set("maxUV", maxCoords.s, maxCoords.t);         // uv_scale
```

In the shader:

```glsl
uniform sampler2D inputTexture;
uniform vec2 maxUV;
// ...
vec4 color = texture(inputTexture, uv * maxUV);
```

### Mixer (2 inputs)

From [Add.cpp:106–128](../vendor/ffgl/source/plugins/Add/Add.cpp) and
[FFGLMixer.cpp:55–68](../vendor/ffgl/source/lib/ffglquickstart/FFGLMixer.cpp):

```cpp
// Input 0 → sampler unit 0
ScopedSamplerActivation activateSampler0(0);
Scoped2DTextureBinding textureBinding0(pGL->inputTextures[0]->Handle);
shader.Set("textureDest", 0);
FFGLTexCoords maxCoords = GetMaxGLTexCoords(*pGL->inputTextures[0]);
shader.Set("MaxUVDest", maxCoords.s, maxCoords.t);

// Input 1 → sampler unit 1
ScopedSamplerActivation activateSampler1(1);
Scoped2DTextureBinding textureBinding1(pGL->inputTextures[1]->Handle);
shader.Set("textureSrc", 1);
maxCoords = GetMaxGLTexCoords(*pGL->inputTextures[1]);
shader.Set("MaxUVSrc", maxCoords.s, maxCoords.t);
```

The SDK applies uv_scale in the **vertex shader**, not the fragment shader:

```glsl
// vertex shader (Add.cpp:18–33)
uniform vec2 MaxUVDest;
uniform vec2 MaxUVSrc;
out vec2 uvDest;
out vec2 uvSrc;

void main() {
    gl_Position = vPosition;
    uvDest = vUV * MaxUVDest;  // scale applied here, interpolated to fragments
    uvSrc  = vUV * MaxUVSrc;
}
```

Each input gets its own MaxUV because the two textures may have different
hardware sizes (e.g. different resolution sources feeding the mixer).

---

## Masks

FFGL has **no dedicated mask type**. In Resolume, masks are regular input
textures — the plugin receives them as `inputTextures[N]` like any other input.
A plugin that wants a mask declares `SetMaxInputs(2)` (or more) and interprets
one input as mask semantically.

---

## Feedback / previous frame

FFGL provides **no built-in previous-frame access**. Feedback is always
plugin-managed:

1. Allocate your own FBO via [FFGLFBO](../vendor/ffgl/source/lib/ffglex/FFGLFBO.h) (lines 67–110)
2. Each frame: render to your FBO, then read it back next frame as a `sampler2D`
3. **Critical:** save/restore the host FBO (`ProcessOpenGLStruct.HostFBO`)

This is what your plugins do — slew-limiter stores `u_previous` in a
plugin-owned FBO, delay-line uses a texture array buffer. The host never
provides the previous output automatically.

When sampling your own FBOs: do **not** apply uv_scale. Your FBOs are allocated
at the exact size you chose — `Width == HardwareWidth` by construction. Only
host-provided input textures may have the padding mismatch.

---

## Default uniforms

The quickstart framework sends these automatically via
[SendDefaultParams](../vendor/ffgl/source/lib/ffglquickstart/FFGLPlugin.cpp) (line 168–176):

```glsl
uniform vec2 resolution;   // viewport width, height (pixels)
uniform float time;         // seconds since plugin instantiation
uniform float deltaTime;    // seconds since last frame
uniform int frame;          // frame counter
uniform float bpm;          // host BPM
uniform float phase;        // bar phase (0.0–1.0)
```

Declared in the auto-generated shader prefix
([FFGLPlugin.h:212–221](../vendor/ffgl/source/lib/ffglquickstart/FFGLPlugin.h)).

In the Rust bindings, the equivalent data lives in
[FFGLData](../vendor/ffgl-rs/ffgl-core/src/inputs.rs) (lines 29–62):
`viewport`, `host_time`, `host_beat` (bpm + barPhase). Your plugins send
these manually as uniforms.

---

## Host data via FFGLData (Rust bindings)

From [inputs.rs](../vendor/ffgl-rs/ffgl-core/src/inputs.rs):

```rust
pub struct GLInput<'a> {
    pub textures: &'a [FFGLTextureStruct],  // input texture array
    pub host: u32,                          // host FBO handle
}

pub struct FFGLData {
    pub viewport: FFGLViewportStruct,       // {x, y, width, height}
    pub host_time: SystemTime,              // from FF_SET_TIME
    pub host_beat: SetBeatinfoStruct,       // {bpm, barPhase}
}
```

---

## Summary: what a shader can access

| Source | Uniform type | Where it comes from | uv_scale needed? |
|--------|-------------|---------------------|-------------------|
| Input texture 0 | `sampler2D` | Host via `inputTextures[0]` | **Yes** |
| Input texture 1 | `sampler2D` | Host via `inputTextures[1]` | **Yes** (separate MaxUV) |
| Plugin-owned FBO | `sampler2D` | Your own allocation | **No** |
| Resolution | `vec2` | Viewport struct | N/A |
| Time | `float` | Host or plugin clock | N/A |
| Delta time | `float` | Frame-to-frame diff | N/A |
| BPM | `float` | Host beat info | N/A |
| Bar phase | `float` | Host beat info | N/A |
| Frame count | `int` | Plugin counter | N/A |
| User params | `float`/`bool`/`vec3`/`vec4` | Plugin params | N/A |

---

## For your blend shader

A combined fade/combine shader is an `FF_MIXER` with 2 inputs. The minimum
skeleton:

1. `SetMinInputs(2); SetMaxInputs(2);` in constructor
2. Bind both inputs to sampler units 0 and 1
3. Compute `MaxUV` per-input (they may differ)
4. Shader receives both as `sampler2D`, blends with your math
5. Output goes to host FBO (already bound on entry)

See [Add.cpp](../vendor/ffgl/source/plugins/Add/Add.cpp) for the complete
reference implementation — it's the simplest possible mixer.
