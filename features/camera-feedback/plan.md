# Camera Feedback — Plan

## Phase 1: Single camera source

### Step 1: Camera capture in harness.py
- Add `--camera N` flag to harness.py (device index)
- Use OpenCV (`cv2.VideoCapture`) for frame grab — it handles V4L2/UVC
- Each frame: `cap.read()` → flip/resize → `source_tex.write()` — single memcpy to GL texture
- Add `uv add opencv-python-headless` to pyproject.toml
- Graceful fallback if camera unavailable

### Step 2: Camera as ShaderChain source mode
- `ShaderChain.set_camera_source(device_index)` — opens capture, replaces `generate_test_pattern`
- `ShaderChain.update_source()` called each frame — grabs + uploads
- Existing `source_mix` controls injection rate (same knob, new source)

### Step 3: Test with mirror-transform
- Interactive harness: camera pointed at desk/hands, mirror-transform feedback
- Validate latency feels responsive at 30fps input
- Test with different `source_mix` values — low (ghost trail of hands), high (camera-dominant)

## Phase 2: Projector-camera registration

### Step 4: Calibration utility
- New script `tools/shader-harness/calibrate.py`
- Projects checkerboard/ArUco grid via fullscreen window
- Camera captures the projected grid
- OpenCV `findHomography` from detected corners
- Saves `calibration.json` (3x3 homography matrix)

### Step 5: Unwarp camera feed
- Add an optional calibration-aware unwarp pass (shader or CPU-side `cv2.warpPerspective`)
- GPU-side preferred: upload homography as uniform mat3, do the warp in a pre-pass shader
- Camera feed → unwarp → source_tex — now spatially registered with projector output

### Step 6: Feedback modes
- **Injection mode** (default): `out = mix(feedback, camera, source_mix)` — current model
- **Replacement mode**: `feedback = camera` each frame — the physical surface IS the frame buffer
- Toggle via key/param in interactive mode

## Phase 3: GPU stereo depth

### Step 7: Stereo capture
- Open both cameras (`--camera 0 --camera2 1`)
- Upload both as textures: `u_camera_left`, `u_camera_right`
- Accept that frames won't be perfectly synced — rolling shutter + USB scheduling means ~0-33ms phase drift between cameras

### Step 8: GPU disparity shader
- New fragment shader: `stereo_depth.frag.glsl`
- SAD (Sum of Absolute Differences) block matching at quarter resolution
- Search window: ~32 pixels horizontal (for narrow baseline at table distance)
- Output: single-channel disparity texture (`u_depth`)
- Run as first pass in chain, before mirror-transform

### Step 9: Depth-driven parameters
- Modify mirror-transform (or add a new spatial modulation shader) to read `u_depth`
- Depth modulates per-pixel: `u_scale * depth_factor`, `u_swirl * depth_factor`
- Close objects → more transform intensity → more visual activity around your hands
- Uniform `u_depth_influence` controls how much depth matters (0 = ignore, 1 = full modulation)

## Phase 4: Physical optics (experimentation, not code)
- 3D print fresnel lens mount that attaches to projector or camera mount
- Document interesting surface materials and their visual transfer functions
- Build a "table installation" configuration for the synth meetup:
  projector → table surface ← camera(s), objects placed by audience

## Dependencies
- `opencv-python-headless` for capture + calibration
- Existing moderngl/pygame stack for rendering
- No new Rust/plugin work — this is all harness-side initially

## Open questions
- Should camera input also work as a Resolume plugin (FFGL webcam capture)? Or keep it harness-only for now?
- Ideal stereo baseline for ~1m table projection distance? Start with ~5cm, adjust empirically.
- Worth doing temporal averaging on the depth map to reduce noise from rolling shutter desync? Probably — simple IIR filter in the shader.
