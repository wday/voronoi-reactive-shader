//! Headless-GL harness for the v3 delay line.
//!
//! Brings up a surfaceless EGL/llvmpipe context and drives the REAL pieces:
//!   - `delay-core`'s ring (`dc_frame_tick` / `dc_tex` / `dc_frame_index` /
//!     `dc_buf_size`) on an actual GL texture array,
//!   - the REAL plugin shaders — `write.frag` (delay-write) and `output.frag`
//!     (pluglib) — pulled in with `include_str!` so there is zero shader drift
//!     from what ships,
//!   - `delay_dsp::Ring` for the Tap read slot.
//!
//! [`Passes::record`] mirrors `delay-write::draw_write` (a pure `tape = Send*input`
//! overwrite — no buffer read, no blend) and [`Passes::tap_read`] mirrors
//! `delay-tap::draw_tap`. Frame ordering (Tap read BEFORE Write advance) is the
//! caller's responsibility, matching the Tap-first stacking rule (COMPOSE-ORDER).

use gl::types::*;
use khronos_egl as egl;

use delay_dsp::Ring;
use pluglib::{OutputProgram, QuadGeometry, ShaderProgram};

// The real shaders, verbatim from the sibling crates (no reimplementation).
static FS_WRITE: &str = include_str!("../../delay-write/src/shaders/write.frag.glsl");

/// Owns the EGL objects so the context stays current for the whole test process.
pub struct Headless {
    _egl: egl::DynamicInstance<egl::EGL1_5>,
}

/// Create a surfaceless EGL context on mesa/llvmpipe and load GL. Call once.
pub fn headless() -> Headless {
    unsafe {
        let lib = libloading::Library::new("libEGL.so.1").expect("load libEGL");
        let inst =
            egl::DynamicInstance::<egl::EGL1_5>::load_required_from(lib).expect("egl instance");

        const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;
        let dpy = inst
            .get_platform_display(
                PLATFORM_SURFACELESS_MESA,
                egl::DEFAULT_DISPLAY,
                &[egl::ATTRIB_NONE],
            )
            .or_else(|_| inst.get_display(egl::DEFAULT_DISPLAY).ok_or(egl::Error::BadDisplay))
            .expect("get display");
        inst.initialize(dpy).expect("initialize");
        inst.bind_api(egl::OPENGL_API).expect("bind opengl");

        let cfg_attribs = [
            egl::SURFACE_TYPE, egl::PBUFFER_BIT,
            egl::RENDERABLE_TYPE, egl::OPENGL_BIT,
            egl::RED_SIZE, 8, egl::GREEN_SIZE, 8, egl::BLUE_SIZE, 8, egl::ALPHA_SIZE, 8,
            egl::NONE,
        ];
        let config = inst
            .choose_first_config(dpy, &cfg_attribs)
            .expect("choose_config")
            .expect("a matching config");

        let ctx_attribs =
            [egl::CONTEXT_MAJOR_VERSION, 3, egl::CONTEXT_MINOR_VERSION, 2, egl::NONE];
        let ctx = inst.create_context(dpy, config, None, &ctx_attribs).expect("context");
        let pb_attribs = [egl::WIDTH, 16, egl::HEIGHT, 16, egl::NONE];
        let surf = inst.create_pbuffer_surface(dpy, config, &pb_attribs).expect("pbuffer");
        inst.make_current(dpy, Some(surf), Some(surf), Some(ctx)).expect("make_current");

        gl::load_with(|s| {
            inst.get_proc_address(s)
                .map(|p| p as *const std::os::raw::c_void)
                .unwrap_or(std::ptr::null())
        });

        Headless { _egl: inst }
    }
}

/// An RGBA8 texture filled with a solid color (used as node input). Leaks the GL
/// name — fine for a short-lived test process.
pub fn solid_input_tex(w: u32, h: u32, rgba: [u8; 4]) -> GLuint {
    let pixels: Vec<u8> = std::iter::repeat(rgba).take((w * h) as usize).flatten().collect();
    unsafe {
        let mut tex = 0;
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA8 as i32, w as i32, h as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE, pixels.as_ptr().cast(),
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
        gl::BindTexture(gl::TEXTURE_2D, 0);
        tex
    }
}

/// The GL passes, built once from the real shaders.
pub struct Passes {
    write: ShaderProgram,
    output: OutputProgram,
    quad: QuadGeometry,
    record_fbo: GLuint,
    readback_fbo: GLuint,
    readback_tex: GLuint,
    loc_write_input: GLint,
    loc_write_uv: GLint,
    loc_write_send: GLint,
}

impl Passes {
    pub fn new() -> Self {
        let write = ShaderProgram::new(FS_WRITE);
        let output = OutputProgram::new();
        let quad = QuadGeometry::new();
        // Attribute locations are identical across the programs (shared VS); set
        // the quad's attribs up against each so any of them can draw it.
        quad.setup_attrs(write.program);
        output.setup_quad(&quad);

        let loc_write_input = write.uniform_loc("u_input");
        let loc_write_uv = write.uniform_loc("u_uv_scale");
        let loc_write_send = write.uniform_loc("u_send");

        let (record_fbo, readback_fbo, readback_tex);
        unsafe {
            let mut fbos = [0u32; 2];
            gl::GenFramebuffers(2, fbos.as_mut_ptr());
            record_fbo = fbos[0];
            readback_fbo = fbos[1];
            let mut t = 0;
            gl::GenTextures(1, &mut t);
            readback_tex = t;
        }

        Self {
            write, output, quad,
            record_fbo, readback_fbo, readback_tex,
            loc_write_input, loc_write_uv, loc_write_send,
        }
    }

    /// One Write frame on `ch`. Mirrors `delay-write::draw_write`: `dc_frame_tick`
    /// (sizes the buffer and advances the channel's monotonic `frame_index` once
    /// per `frame_id`), write slot = `frame_index % buf_size`, then the pure
    /// write head overwrites `tape[wp] = send * input` (blend off — no buffer
    /// read).
    ///
    /// # Safety
    /// A current GL context (via [`headless`]) must exist.
    pub unsafe fn record(
        &self,
        ch: usize,
        loop_length: u32,
        input_tex: GLuint,
        w: u32,
        h: u32,
        send: f32,
        frame_id: u64,
    ) {
        delay_core::dc_frame_tick(ch, loop_length, w, h, frame_id);
        let tex = delay_core::dc_tex(ch);
        let bufsz = delay_core::dc_buf_size(ch);
        if tex == 0 || bufsz == 0 {
            return;
        }
        let wp = Ring::for_buffer(bufsz).write_slot(delay_core::dc_frame_index(ch));

        gl::BindFramebuffer(gl::FRAMEBUFFER, self.record_fbo);
        gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, tex, 0, wp as i32);
        gl::Viewport(0, 0, w as i32, h as i32);

        // Pure overwrite: slot = send * input (blend disabled, no read of `old`).
        gl::Disable(gl::BLEND);
        self.write.use_program();
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, input_tex);
        gl::Uniform1i(self.loc_write_input, 0);
        gl::Uniform2f(self.loc_write_uv, 1.0, 1.0);
        gl::Uniform1f(self.loc_write_send, send);
        self.quad.draw();
        gl::BindTexture(gl::TEXTURE_2D, 0);
    }

    /// Read the Tap output at the center pixel. Mirrors `delay-tap::draw_tap`:
    /// ticks the barrier read-only (zero dims), reads the oldest slot
    /// `(frame_index − loop_length) % buf_size` (via `delay_dsp::Ring`) and
    /// returns `clamp(dry*input + wet*tape, 0, 1)`. Wet is forced to 0 on an
    /// unallocated buffer (TAP-EMPTY-BUFFER).
    ///
    /// # Safety
    /// A current GL context must exist.
    pub unsafe fn tap_read(
        &self,
        ch: usize,
        input_tex: GLuint,
        w: u32,
        h: u32,
        dry: f32,
        wet: f32,
        gamma: f32,
        frame_id: u64,
    ) -> [u8; 4] {
        delay_core::dc_frame_tick(ch, 0, 0, 0, frame_id);
        let tex = delay_core::dc_tex(ch);
        let bufsz = delay_core::dc_buf_size(ch);
        let (read_slot, wet_eff) = if bufsz == 0 || tex == 0 {
            (0u32, 0.0)
        } else {
            (Ring::for_buffer(bufsz).read_slot(delay_core::dc_frame_index(ch)), wet)
        };
        self.render_output(input_tex, w, h, tex, read_slot as f32, dry, wet_eff, gamma)
    }

    /// Read a specific buffer layer directly (dry=0, wet=1) — used to inspect the
    /// record math independently of the Tap's `wp+1` read offset.
    ///
    /// # Safety
    /// A current GL context must exist.
    pub unsafe fn read_layer(&self, tex: GLuint, layer: u32, w: u32, h: u32) -> [u8; 4] {
        self.render_output(0, w, h, tex, layer as f32, 0.0, 1.0, 1.0)
    }

    /// Render the output shader into the readback FBO and sample the center pixel.
    unsafe fn render_output(
        &self,
        input_tex: GLuint,
        w: u32,
        h: u32,
        buffer_tex: GLuint,
        layer: f32,
        dry: f32,
        wet: f32,
        gamma: f32,
    ) -> [u8; 4] {
        // (Re)size the readback target to w x h.
        gl::BindTexture(gl::TEXTURE_2D, self.readback_tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA8 as i32, w as i32, h as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE, std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
        gl::BindTexture(gl::TEXTURE_2D, 0);

        gl::BindFramebuffer(gl::FRAMEBUFFER, self.readback_fbo);
        gl::FramebufferTexture2D(
            gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, self.readback_tex, 0,
        );
        gl::Viewport(0, 0, w as i32, h as i32);
        gl::ClearColor(0.0, 0.0, 0.0, 0.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        self.output.draw(&self.quad, input_tex, [1.0, 1.0], buffer_tex, layer, dry, wet, gamma);

        let mut px = [0u8; 4];
        gl::ReadPixels(
            (w / 2) as i32, (h / 2) as i32, 1, 1,
            gl::RGBA, gl::UNSIGNED_BYTE, px.as_mut_ptr().cast(),
        );
        px
    }
}

impl Default for Passes {
    fn default() -> Self {
        Self::new()
    }
}
