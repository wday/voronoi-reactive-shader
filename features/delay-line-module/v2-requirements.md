# Delay Line Module v2 — Requirements

## Status: in progress

## Changes from v1

### 1. Tap mode (new)

Third mode alongside Send and Receive. Outputs the delayed frame directly from the shared buffer — no input mixing, no feedback. Ignores the plugin's input texture entirely.

**Use case**: Multi-tap echo clouds. Multiple Tap instances on separate Resolume layers, each at different delay times, with independent spatial transforms (position, scale, rotation, opacity). One Receive instance handles the feedback path; Tap instances are read-only observers of the buffer.

**Resolume setup**: Use a static black source clip on each Tap layer. Tap ignores input and outputs the delayed buffer frame. Resolume's layer blending composites the taps into the final output.

**Future**: Refactor Tap into a standalone Source plugin (`delay-line-tap`) using cross-DLL shared memory IPC. The draw logic stays identical — only the buffer lookup mechanism changes.

### 2. Sync mode (new)

Replaces the fixed subdivision-based delay with three selectable time modes:

| Mode | Delay parameter | Range | Notes |
|------|----------------|-------|-------|
| Subdivision | Musical division | 1/16 – 4 bars | Current behavior. Derived from host BPM + FPS estimate |
| Ms | Milliseconds | 0 – 5000ms | Free-running. Derived from ms value + FPS estimate |
| Frames | Frame count | 1 – 899 | Direct. No BPM or FPS dependency |

- Sync Mode is a new Option parameter visible to all modes (Send, Receive, Tap)
- The existing Subdivision param is reused when sync mode = Subdivision
- Two new params: Delay Ms, Delay Frames — always visible but only active for their sync mode
- FFGL doesn't support conditional param visibility, so all delay params are always shown

## Updated parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Mode | Option | Receive, Send, Tap | Receive | Switches plugin behavior |
| 1 | Channel | Option | 1, 2, 3, 4 | 1 | Pairs instances on same buffer |
| 2 | Sync Mode | Option | Subdivision, Ms, Frames | Subdivision | Selects delay time source |
| 3 | Subdivision | Option | 1/16 – 4 bars | 1/4 | Active when sync = Subdivision |
| 4 | Delay Ms | Standard | 0 – 5000 | 500 | Active when sync = Ms |
| 5 | Delay Frames | Standard | 1 – 899 | 30 | Active when sync = Frames |
| 6 | Feedback | Standard | 0.0 – 1.0 | 0.5 | Receive only |

## Unchanged

- Buffer architecture (GL_TEXTURE_2D_ARRAY, 900 frames, 4 channels)
- Registry (global static, single DLL)
- Shaders (write, read, receive — Tap reuses `read_pass`)
- Send behavior (write + output delayed)
- Receive behavior (mix input + feedback × delayed)
