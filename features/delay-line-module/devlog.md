# Delay Line Module — Development Log

## 2026-03-08: Initial implementation

### Design evolution

Started from observing that the video-looper and LTM dream plugins were reimplementing
Resolume's native capabilities (blend modes, transforms, blur, color effects). Asked: what
if we just build the one thing Resolume can't do — a beat-synced frame delay — and let
Resolume handle everything else?

**v1: Pure delay line** — single plugin, one parameter (subdivision). Designed for use with
Resolume's built-in Feedback Source at composition level. Worked perfectly for single-layer
setups.

**v2: Send/Receive** — discovered that Feedback Source only taps composition output, so
multi-layer setups bleed other content into the feedback path. Refactored to send/receive
pair using a shared GPU buffer registry. Single DLL, mode parameter switches behavior.
Resolume effects between Receive and Send compound through the feedback loop.

**v3: Approach B output** — realized that Send passing through its input meant the wet
layer showed the live signal (rotated by transforms in the chain). Changed Send to output
the delayed frame instead. First visible echo is always transformed — matches audio delay
behavior where wet output is pure echo, never dry.

### Key technical decisions

- **GPU texture array** over system RAM + PBO: eliminates the lag seen in video-looper v1
- **Global static registry**: works because all FFGL instances in same DLL share address space
- **FPS via EMA**: `fps += 0.05 * (instant - fps)`. No drift because read pointer is derived
  (not accumulated) each frame
- **Additive receive mix**: `input + feedback × delayed`. High feedback can blow out — managed
  by user setting feedback < 1.0 and controlling layer opacity
- **include_str!() for shaders**: GLSL in separate .glsl files for syntax highlighting,
  embedded at compile time

### Resolume layer setup (tested working)

```
Layer 1 (bottom): Cam + conditioning, opacity 0%
Layer 2 (blend: Add): VideoRouter→L1, Receive(ch1) → Transform(rotate) → Send(ch1)
Layer 3 (top): VideoRouter→L1 (dry)
```

Video Router "Input Opacity" defaults to off, so 0% opacity Layer 1 is still routable.
Routing from below = zero latency.

### VRAM usage

RTX 5070 Ti Laptop, 12GB. One active channel at 1080p (900 frames × 8MB) = ~7.2GB.
Leaves ~3.3GB for Resolume + OS. Multi-channel at full buffer size would exceed VRAM.
Future optimization: allocate based on actual max subdivision, not fixed 900 frames.
