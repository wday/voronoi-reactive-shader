# OpenGL Concepts for the Video Looper

Reference for GPU/GL concepts used in this plugin. Assumes familiarity with
C pointers and memory, but not with modern GPU programming.

---

## 1. PBO Async Timing — Why Double-Buffering Avoids Stalls

### The problem

The GPU and CPU run **in parallel on separate hardware**. When the CPU calls
a GL function, it usually just drops a command into a queue — the GPU executes
it later. This is fast because the CPU doesn't wait.

But `glMapBuffer` is different: it needs a CPU-accessible pointer to data the
**GPU is producing**. If the GPU hasn't finished the DMA transfer yet, the CPU
blocks (stalls) until the data is ready. At 60fps you have ~16ms per frame.
A stall can eat 5-10ms and tank your framerate.

### The solution: double-buffering

```
Frame 0:  begin_download → PBO[0]    (GPU starts DMA into PBO[0])
          CPU does other work         (no waiting!)

Frame 1:  begin_download → PBO[1]    (GPU starts DMA into PBO[1])
          finish_download ← PBO[0]   (GPU finished PBO[0] long ago — no stall)

Frame 2:  begin_download → PBO[0]    (reuse PBO[0], GPU starts DMA)
          finish_download ← PBO[1]   (PBO[1] finished during frame 1 — no stall)
```

We always **read from the PBO we wrote two frames ago**. By then the GPU has
definitely finished the transfer. The cost: 2 frames of latency (~33ms at 60fps).
For a video looper this is invisible.

### Why "orphaning" helps uploads

When uploading (RAM → GPU), we call `glBufferData(NULL)` before `glMapBuffer`.
This tells the driver: "I don't need the old buffer contents — allocate a fresh
one." The driver can then let us write to the new buffer immediately while the
GPU is still reading from the old one. Without orphaning, the driver would have
to stall until the GPU finishes reading.

### In our code

- `pbo.rs:begin_download()` — queues an async GPU→PBO transfer
- `pbo.rs:finish_download()` — maps the OTHER PBO (from previous frame), memcpy out
- `pbo.rs:upload_to_texture()` — orphan + map + memcpy in + glTexSubImage2D
- `current_download` flips 0↔1 each frame to alternate PBOs

---

## 2. FBOs — Rendering to a Texture Instead of the Screen

### Default rendering

Normally, `glDrawArrays` renders to the **screen** (the default framebuffer).
The pixels go directly to the monitor. The framebuffer is just a big 2D array
of pixels that the GPU writes into.

### Rendering to a texture

An FBO (Framebuffer Object) lets you redirect rendering to a **texture** instead.
It's the same draw calls, same shaders, same everything — just a different
destination for the pixels.

```
Without FBO:  DrawArrays() → pixels go to screen
With FBO:     BindFramebuffer(my_fbo)
              DrawArrays() → pixels go to my_texture (attached to the FBO)
              BindFramebuffer(0) → back to screen
```

The texture then contains the rendered image, and you can:
- Sample it in another shader (composition/post-processing)
- Read it back to CPU via glReadPixels/PBO
- Use it as input for another render pass

### In our code

We use this in two places:

**blend_fbo** (looper.rs, step 3):
  We render `mix(input, delayed, decay)` into `blend_texture` via `blend_fbo`.
  This is the decay blend — the result gets downloaded to the ring buffer.
  We can't render this to the screen because it's an intermediate result.

**Resolume's FBO** (looper.rs, step 5):
  Resolume itself uses an FBO — effects don't render to the screen directly.
  Resolume renders its composition into internal FBOs, composites layers, then
  sends the final result to the screen. That's why we save/restore the host FBO:

```c
// Pseudocode of what Resolume does:
glBindFramebuffer(resolume_layer_fbo);
for each effect in chain:
    effect->draw();   // ← our plugin runs here, must render to THIS fbo
// later, Resolume composites all layer FBOs to the screen
```

  If we bind FBO 0 (the window), Resolume never sees our output → black screen.
  This was the bug we fixed with `glGetIntegerv(GL_FRAMEBUFFER_BINDING)`.

### Mental model

Think of FBOs as "file redirection" in a shell:
- `program` → output goes to terminal (screen)
- `program > file.txt` → output goes to file (texture)

The program (shader) doesn't know or care where its output goes.

---

## 3. GL State Leaking Between Plugins

### The shared GL context

Resolume (and most plugin hosts) runs **all plugins in one GL context**.
A GL context is like a process — it owns all the GL objects (textures, buffers,
programs, FBOs) and has a big bank of **current state**:

```
Current state (global, mutable):
  - bound texture unit 0:  <some texture ID>
  - bound texture unit 1:  <some texture ID>
  - bound VAO:             <some VAO ID>
  - bound program:         <some program ID>
  - bound framebuffer:     <some FBO ID>
  - viewport:              (x, y, width, height)
  - blend mode, depth test, etc.
```

When Resolume calls `effect_A->draw()` then `effect_B->draw()`, they share
this state. If effect A leaves texture unit 1 bound to its texture, effect B
might accidentally sample A's texture instead of its own.

### Our cleanup

That's why `draw_mix()` unbinds everything after rendering:

```c
// After drawing:
gl::BindVertexArray(0);          // unbind our VAO
gl::BindTexture(GL_TEXTURE_2D, 0); // unbind textures from unit 0 and 1
gl::UseProgram(0);               // unbind our shader program
```

And why we save/restore the host's FBO and viewport — those are state too.

### Rules of thumb for plugin GL code

1. **Restore what you change.** Especially framebuffer, viewport, blend state.
2. **Unbind what you bind.** Textures, VAOs, programs.
3. **Don't assume initial state.** The previous plugin may have left anything bound.
4. **Never delete objects another plugin might reference.** (Not an issue for us —
   we only use our own objects.)

This is the GPU equivalent of "don't leave global variables dirty in a shared library."

---

## 4. `unsafe` in Rust — Why We Need It and What It Means

### What `unsafe` does

Rust's compiler proves at compile time that your code has:
- No dangling pointers (use-after-free)
- No data races (two threads writing same memory)
- No buffer overflows (array bounds checked)
- No null pointer dereferences

The `unsafe` keyword says: **"the compiler can't verify this section — I the
programmer am taking responsibility for correctness."** It's not "dangerous" —
it's "unverified." Like `volatile` in C marking "the compiler shouldn't optimize
this", `unsafe` marks "the compiler shouldn't verify this."

### Why every GL call is unsafe

The GL API is a C API that manages GPU-side resources. Rust can't verify:

1. **Use-after-delete**: If you `DeleteTexture(tex)` then `BindTexture(tex)`,
   that's a bug. Rust can't track GPU object lifetimes — they live on the
   driver side, not in Rust's memory model.

2. **Thread safety**: GL contexts are single-threaded. Calling GL from two
   threads is undefined behavior. Rust's type system can't enforce "only call
   from the thread that owns the GL context."

3. **State validity**: GL functions depend on what's currently bound. Calling
   `glDrawArrays` with no VAO bound, or `glReadPixels` with no FBO bound,
   is undefined. Rust can't model GL's state machine.

4. **Raw pointers from glMapBuffer**: The pointer is valid only until
   `glUnmapBuffer`. Rust can't enforce this lifetime.

### What this means in practice

- `unsafe { }` blocks are a **code review flag** — "scrutinize this section for
  GPU-side correctness since the compiler can't help."
- If the plugin crashes or renders garbage, the bug is almost certainly inside
  an `unsafe` block (wrong texture ID, forgot to bind something, etc).
- The rest of the Rust code (ring buffer, params, state logic) is fully verified
  by the compiler. Bugs there would be caught at compile time.

### The tradeoff

In C, ALL code is implicitly unsafe. In Rust, only the `unsafe` blocks are.
Our plugin is maybe 30% `unsafe` (the GL calls) and 70% safe (buffer management,
parameter logic, state machine). The safe 70% is provably bug-free at compile time.
That's the value proposition — you get C-level control where you need it (GPU
interop) and compiler guarantees everywhere else.

---

## Quick Reference: GL Object Types in This Plugin

| Object    | Created with        | What it holds                              | Analogy            |
|-----------|--------------------|--------------------------------------------|---------------------|
| Texture   | glGenTextures      | 2D pixel array on GPU                      | An image file       |
| Buffer    | glGenBuffers       | Raw bytes on GPU (vertices or pixel data)  | malloc on GPU       |
| VAO       | glGenVertexArrays  | Vertex format description (schema)         | A struct definition  |
| FBO       | glGenFramebuffers  | Render target (points to a texture)        | stdout redirection   |
| Program   | glCreateProgram    | Linked vertex+fragment shader executable   | A compiled binary    |
