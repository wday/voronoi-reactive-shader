# Delay Module v2 — Read-Before-Write with FX Insert

## Problem

The current delay-line module uses a Send/Receive split where Send writes input to the buffer and outputs passthrough, while Receive reads the buffer and outputs the delayed signal. This model doesn't support an fx insert point between read and write — you can't process the feedback signal through external effects before it gets written back to the buffer.

The read-before-write model (see `/mnt/d/assets/jzp15i/read-before-write-simple-model.png`) places the buffer read *before* the write so that Resolume effects in the chain can process the feedback signal.

## Goal

A two-instance delay module flow where:
1. **Read** outputs the current buffer content — no timing logic, no mixing
2. **Write** writes its input to the buffer, advances the write position, and outputs what it wrote

Send level and feedback level are controlled entirely by Resolume's native effect opacity and blend modes on the Read instance, not by plugin parameters.

## Resolume Composition Target

```
Layer (single chain):
  Source → [exposure] → Read(ch1, opacity=80%, Add) → [mirror-transform] → Write(ch1, decay=0.9) → Output
```

How it works:
- Exposure before Read controls how much live source enters the loop (send level)
- Read outputs buffer content; Resolume blends it with Read's input via opacity + blend mode (feedback level)
- FX chain processes the blended signal
- Write writes the processed result to the buffer and outputs it

## Signal Flow

```
Source → [gain/exposure] → Read ──→ [FX Insert] ──→ Write ──→ Output
                             │                         │
                     Resolume blends                   │
                     input + buffer read               ↓
                     via opacity/blend mode       Ring Buffer
                             ↑                         │
                             └─────────────────────────┘
```

- **Send level**: gain/exposure effect before Read, or Resolume layer opacity
- **Feedback level**: Resolume effect opacity + blend mode on Read
- **Decay**: Write parameter — how much previous buffer content survives

## Requirements

1. **Mode: Read** — looks up the channel buffer from the registry, outputs the oldest frame in the ring: `buffer[(write_pos + 1) % loop_length]`. No timing params, no mixing, no feedback param. Just channel selection and buffer output. If no buffer exists yet, outputs black.

2. **Mode: Write** — writes its input to `buffer[write_pos]`, advances write_pos via registry, and outputs the buffer content it just wrote. Timing params (sync mode, subdivision, ms, frames) control loop length. Decay controls previous-iteration survival.

3. **Buffer sizing**: the ring buffer is allocated with `loop_length + 1` slots. This extra slot ensures a full `loop_length` frames of delay — Read reads the oldest slot while Write writes to the current slot, and they never alias the same frame. With N+1 slots and `read_pos = (write_pos + 1) % (loop_length + 1)`, Read consistently outputs content that is exactly `loop_length` frames old.

4. **Decay parameter** (Write, 0.0–1.0, default 0.0): controls how much of the previous buffer content survives into the new write slot. 0.0 = clean overwrite. Same semantics as v1 overdub.

5. **Loop timing** (Write only): Write owns all timing — sync mode, subdivision, delay ms, delay frames. Write advances write_pos in the registry. Read gets `write_pos` and `loop_length` from the registry to compute its read position.

6. **Output semantics**: Write outputs what it wrote to the buffer. Read outputs the oldest buffer frame. Neither passes through live input — Resolume's host-level blending handles input/output mixing.

7. **Edge case: no Write active** — Read outputs the same frame indefinitely (frozen). Acceptable for v2.

## Parameters

### Read mode uses:
| Index | Name    | Type   | Default | Notes |
|-------|---------|--------|---------|-------|
| 0     | Mode    | Option | Read    | Read / Write |
| 1     | Channel | Option | 1       | Which buffer to read |

All other params are ignored in Read mode.

### Write mode uses:
| Index | Name         | Type    | Default | Notes |
|-------|--------------|---------|---------|-------|
| 0     | Mode         | Option  | Read    | Read / Write |
| 1     | Channel      | Option  | 1       | Which buffer to write |
| 2     | Sync Mode    | Option  | Subdiv  | Subdivision / Ms / Frames |
| 3     | Subdivision  | Option  | 1/4     | Beat subdivision selector |
| 4     | Delay Ms     | Integer | 500     | Millisecond delay |
| 5     | Delay Frames | Integer | 30      | Frame count delay |
| 6     | Decay        | Standard| 0.0     | Previous iteration survival |

Passthrough, Feedback, and Send Amount are all removed — handled by Resolume's native effect controls.

## Supersedes

This spec supersedes `features/overdub/` and the current Send/Receive model. The v1 decay/overdub behavior is preserved within Write mode's decay parameter.
