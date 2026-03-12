# Dream Looper Control — Max for Live Device

## Project
Max for Live audio effect that controls the Dream LTM FFGL plugin in Resolume
via OSC, with optional audio-reactive modulation via Gen~.

## Goals
- Control Dream LTM parameters from Ableton's session view
- Sync tempo via Ableton Link (both apps see same BPM/phase)
- Audio analysis drives visual parameters reactively
- Automatable in Ableton — all controls are Live parameters

## Architecture

```
Ableton Live (audio + control)
├── Max for Live Device (audio effect slot)
│   ├── UI: live.dial / live.slider for key params
│   ├── Gen~: audio analysis (envelope, transients, spectral)
│   ├── Mapping: analysis → param modulation
│   └── OSC out: udpsend to Resolume
│
├── Ableton Link: tempo + phase sync
│   (shared with Resolume — no explicit BPM message needed)
│
Resolume Avenue
├── OSC input (port TBD, e.g. 7000)
│   └── /composition/layers/N/effects/DreamLTM/params/...
└── Dream LTM FFGL plugin
```

## IPC: OSC via Resolume's Native Input

Resolume exposes every parameter at an OSC address. The Max device sends
float values to these addresses. No custom OSC listener in the FFGL plugin
needed — Resolume handles the routing.

**Advantages over MIDI CC:**
- Float precision (vs 7-bit 0-127)
- Human-readable addresses (self-documenting)
- No CC number assignment / mapping step

**OSC address format (Resolume):**
```
/composition/layers/{layer}/effects/{effect_index}/params/{param_name}
```
Exact paths TBD — need to check Resolume's OSC naming for FFGL params.

## Parameters to Expose

### Phase 1 — Core controls
| M4L Control | Dream LTM Param | Notes |
|-------------|-----------------|-------|
| Dry | Dry (0-2) | Live signal level |
| Wet | Wet (0-2) | Echo mix level |
| Tap 1-4 | Tap 1-4 (0-2) | Per-tier echo levels |
| Feedback | Feedback (0-1) | Decay per echo |
| Subdivision | Subdivision | Discrete: 1/16, 1/8, 1/4, 1/2, 1 bar |

### Phase 2 — Spatial controls
| M4L Control | Dream LTM Param | Notes |
|-------------|-----------------|-------|
| Rotation | Rotation (±180°) | Could be audio-modulated |
| Scale | Scale (0.5×-2×) | |
| Swirl | Swirl (±2 rad) | |
| Hue Shift | Hue Shift (±180°) | Good candidate for spectral mapping |

### Phase 3 — Audio-reactive modulation
| Analysis Source | Modulation Target | Idea |
|----------------|-------------------|------|
| Envelope follower | Wet or Feedback | Echoes swell with energy |
| Transient / kick detect | Tap 1 pulse | Sharp echo on downbeat |
| Spectral centroid | Hue Shift | Bright sounds = color rotation |
| RMS level | Scale | Loud = zoom, quiet = settle |
| Beat phase | Rotation | Rotate per beat position |

## Gen~ Audio Analysis Components

### Envelope Follower
Sample-accurate asymmetric smoothing (fast attack, slow release).
Drives macro modulation — "how loud is it right now?"

### Transient Detector
Derivative of envelope — spikes on note onsets.
Drives impulse modulation — "a hit just happened."

### Spectral Analysis (stretch goal)
Spectral centroid or band energy via FFT.
Drives timbral mapping — "what kind of sound is playing?"

All Gen~ components output control signals (0-1 floats) that get scaled
and mapped to OSC parameter ranges in the Max patcher.

## Tempo Sync

**Ableton Link** handles BPM + beat phase sync between Ableton and Resolume.
Both apps support Link natively — just enable in preferences.

This means:
- No BPM parameter needs to be sent over OSC
- Dream LTM's BPM knob could be removed or made Link-aware
- Beat phase is available for modulation timing

## Open Questions

- [ ] Exact Resolume OSC address format for FFGL plugin params
- [ ] OSC port number (Resolume default? custom?)
- [ ] Should the M4L device be an audio effect (has audio input for analysis)
      or a MIDI effect (lighter, control-only)?
- [ ] Bidirectional: does Resolume send OSC back? (e.g., visual feedback in M4L UI)
- [ ] Link phase: can Resolume expose beat phase to sync modulation timing?
- [ ] Max patch format: .amxd generated as JSON vs hand-built in Max editor

## File Format

Max for Live patches (.amxd) are JSON internally (.maxpat format).
Can be generated programmatically — no GUI editor required for creation.
Layout/positioning may need manual adjustment after generation.
