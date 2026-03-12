# Camera Feedback — Devlog

## 2026-03-11 — Scoping
- Defined 4-phase plan: single camera → projector registration → GPU stereo depth → physical optics
- Key constraint: 30fps rolling shutter webcams, no CPU-side depth (GPU only)
- Rolling shutter artifacts and inter-camera sync drift treated as aesthetic features
- Phase 1 is self-contained and immediately useful for the synth meetup table installation
