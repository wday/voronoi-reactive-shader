# Camera Feedback — Requirements

## Hardware
- 2x cheap USB webcams, 30fps, rolling shutter
- Projector (output surface)
- 3D-printed stereo mount for camera alignment
- Optional: fresnel lenses, physical objects in the projection beam

## Core loop
Projector → physical surface → cameras → shader chain → projector. The physical world (surface texture, lens distortion, objects, ambient light) becomes an unwritable transform in the feedback chain.

## Constraints
- 30fps rolling shutter — sync between cameras is approximate. Frame-to-frame skew and inter-camera phase drift are features, not bugs. Rolling shutter artifacts (wobble, partial exposure) feed into the aesthetic.
- No CPU-side depth computation — memory shuffle cost is too high. Any stereo depth extraction must be GPU-side (shader pass).
- Latency budget: camera capture → texture upload → shader chain → display should stay under 2 frames (~66ms) to keep feedback responsive.

## Requirements

### Phase 1: Single camera as source
1. Webcam capture feeds into the shader chain as `u_source` (replacing procedural test patterns)
2. Camera frame uploaded as texture each frame (GPU upload path)
3. Works in both harness.py (interactive) and as a Resolume plugin input
4. Source injection rate (`source_mix`) controls how much live camera bleeds into feedback

### Phase 2: Projector-camera feedback
1. Project shader output onto physical surface, camera films the result
2. Projector-camera registration: project calibration pattern, compute homography (one-time setup), unwarp camera feed to match projected frame coordinates
3. Switchable modes:
   - **Injection**: camera mixed into feedback via `source_mix` (current model — physical world tints the evolution)
   - **Replacement**: camera frame replaces feedback buffer entirely each frame (physical world IS the previous frame — true optical feedback)

### Phase 3: Stereo depth as control signal
1. Two cameras → GPU-side disparity/depth map (shader pass, block-matching or SAD)
2. Depth map available as a uniform texture to any shader in the chain
3. Depth drives shader parameters spatially: e.g. depth modulates scale, swirl, iteration count per-pixel
4. Stereo baseline: narrow (~4-6cm) for table-distance projection

### Phase 4: Physical optics
1. Fresnel lens between projector and surface — optical zoom/distortion compounds with shader transforms each feedback iteration
2. Fresnel between surface and camera — warps the capture, different aesthetic (input-side distortion)
3. Objects in the beam: occlusion and refraction create voids the shader fills. Removing an object lets pattern snap back
4. Material exploration: foil, water, fabric, frosted glass — each surface has a different optical transfer function

## Non-goals (for now)
- Real-time stereo depth at full resolution — coarse depth (quarter-res) is fine as a control signal
- Camera sync — accept phase drift, design around it
- Automated calibration — manual/semi-manual calibration grid is fine for v1
