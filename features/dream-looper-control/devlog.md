# Development Log — Dream Looper Control (M4L)

## 2026-03-08 — Initial scoping

### Context
Dream LTM plugin is working in Resolume with musical tap model (one tap per
pyramid tier at doubling delays). Want to control it from Ableton for:
- Automation in arrangement view
- Audio-reactive modulation
- Tempo sync via Ableton Link

### IPC decision: OSC via Resolume
Chose OSC over MIDI CC for float precision and self-documenting addresses.
Resolume has native OSC input — no custom listener needed in the FFGL plugin.
Max has trivial OSC output via [udpsend].

### Gen~ for audio analysis
Gen~ compiles per-sample DSP code to native machine code. Relevant for building
precise envelope followers and transient detectors that feed modulation values
to the OSC control layer. Not needed for phase 1 (manual controls only).

### Key insight: Ableton Link for tempo
Both apps support Link natively. Enabling it in both eliminates the need to
send BPM over OSC. Could eventually make Dream LTM's BPM param Link-aware
and remove it from the FFGL parameter list entirely.

### File format
M4L patches are JSON (.maxpat format). Can generate programmatically.
Decided to scaffold feature docs first, implement in a later session.
