# Delay Line Module

A modular beat-synced frame delay for Resolume. Add it twice — one in **Send** mode, one in **Receive** mode — with native Resolume effects between them to create feedback loops where transforms compound through each echo.

## How it works

The plugin closes a feedback loop through a shared GPU buffer, bypassing Resolume's composition-level Feedback Source. This means independent delay loops per layer, no Arena required.

**Send** writes the current frame to the buffer and outputs the delayed frame.
**Receive** reads the delayed frame from the buffer and mixes it into the live signal.

```
┌─────────────────────────────────────────────────────────┐
│  Effect chain (single layer):                           │
│                                                         │
│  Webcam ──► Receive(ch1) ──► Transform ──► Send(ch1)    │
│                 │                             │         │
│                 │         Shared GPU Buffer    │         │
│                 │        ┌───────────────┐    │         │
│                 ◄────────┤ read [pos-D]  │◄───┘ write   │
│            mix delayed   └───────────────┘   [pos]      │
│            + feedback                                   │
│                                                         │
│  Layer output = delayed frame (echo only, transformed)  │
└─────────────────────────────────────────────────────────┘
```

## Resolume layer setup

Three layers for clean dry/wet separation:

```
┌──────────────────────────────────────────────────────┐
│                                                      │
│  Layer 3 (top): DRY                                  │
│    Source: Video Router → Layer 1                     │
│    Blend: Normal                                     │
│                                                      │
│  Layer 2: WET (echo + feedback)                      │
│    Source: Video Router → Layer 1                     │
│    Effects:                                          │
│      1. Delay Line [Receive, ch 1, feedback 0.5-0.8] │
│      2. Transform [rotate 137.5°, scale 62%]         │
│      3. Blur [subtle, optional]                      │
│      4. Delay Line [Send, ch 1]                      │
│    Blend: Add or Screen                              │
│                                                      │
│  Layer 1 (bottom): SOURCE (hidden)                   │
│    Source: Webcam                                     │
│    Effects: [any input conditioning]                  │
│    Opacity: 0% (still routable — Video Router        │
│    "Input Opacity" is off by default)                │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Why 3 layers?** Layer 1 conditions the input once. Layers 2 and 3 both tap it via Video Router — no duplicated effect chains. Layer 1 at 0% opacity is invisible to the composition but still readable by Video Router.

## Parameters

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Mode** | Receive / Send | Switches plugin behavior |
| **Channel** | 1 / 2 / 3 / 4 | Pairs Send with Receive instances |
| **Subdivision** | 1/16, 1/8, 1/4, 1/2, 1 bar, 2 bars, 4 bars | Musical delay length (BPM from host) |
| **Feedback** | 0.0 – 1.0 | Echo intensity (Receive only) |

## Multi-tap delay

For multiple delay taps at different rates, duplicate the wet layer with a different channel and subdivision:

```
Layer 4: Dry (Video Router → L1)
Layer 3: Wet tap 2 — Receive(ch2, 1/2 note) → FX → Send(ch2), blend: Add
Layer 2: Wet tap 1 — Receive(ch1, 1/4 note) → FX → Send(ch1), blend: Add
Layer 1: Source (webcam, 0% opacity)
```

Each channel is an independent feedback loop with its own buffer.

## Build

```
make build PLUGIN=delay_line_module
make deploy PLUGIN=delay_line_module
```

## VRAM usage

Each active channel allocates a 900-frame GPU texture array at the input resolution:
- 1080p: ~7.2 GB per channel
- 720p: ~3.2 GB per channel
- Typical use (1/4 note @ 120bpm = 30 frames): ~240 MB

Channels are allocated lazily on first Send write. Unused channels cost nothing.
