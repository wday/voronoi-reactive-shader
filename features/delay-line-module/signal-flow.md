# Delay Line Module — Signal Flow

## Send: Self-Contained Recursive Overdub

This is the core feedback loop. A single Send instance on one Resolume layer
creates a wet-only echo that compounds through the FX chain every loop iteration.

```mermaid
sequenceDiagram
    participant R as Resolume Layer
    participant S as Send Plugin
    participant B as Ring Buffer [ch N]

    Note over R,B: Frame 0 (first frame — buffer empty)
    R->>S: input frame (live source)
    S->>B: write input → buf[0]
    B-->>S: read buf[0 - D] = black (unwritten)
    S->>R: output: black

    Note over R,B: Frame 1..D (filling up)
    R->>S: input frame
    S->>B: write input → buf[pos]
    B-->>S: read buf[pos - D] = still black
    S->>R: output: black

    Note over R,B: Frame D+1 (first echo appears)
    R->>S: input frame
    S->>B: write input → buf[pos]
    B-->>S: read buf[pos - D] = frame 1
    S->>R: output: frame 1 (delayed)

    Note over R,B: Frame D+2 (recursion begins)
    R->>S: FX(frame 1) — Resolume fed output back through FX chain
    S->>B: write FX(frame 1) → buf[pos]
    B-->>S: read buf[pos - D] = frame 2
    S->>R: output: frame 2 (delayed)

    Note over R,B: Steady state
    R->>S: FX(FX(FX(...))) — each pass through FX chain = one echo generation
    S->>B: write compound frame
    B-->>S: read D-frames-ago compound frame
    S->>R: output: accumulated echo
```

**Key insight**: The FX chain on the Resolume layer (rotate, scale, color, blur, etc.)
IS the feedback function. Every trip through the loop applies it once more. Decay comes
from layer opacity or color effects — the plugin itself has no feedback gain control in
Send mode.

## All Three Modes — Data Flow

```mermaid
flowchart LR
    subgraph SEND["Send Mode"]
        SI[Layer Input] --> SW{Write}
        SW --> BUF[(Ring Buffer\nchannel N)]
        BUF --> SR{Read\npos - D}
        SR --> SO[Layer Output\n= delayed frame]
    end

    subgraph RECEIVE["Receive Mode"]
        RI[Layer Input] --> MIX{Mix}
        BUF2[(Ring Buffer\nchannel N)] --> RR{Read\npos - D}
        RR --> |"× feedback"| MIX
        MIX --> |"clamp(input + fb×delayed)"| RO[Layer Output]
    end

    subgraph TAP["Tap Mode"]
        BUF3[(Ring Buffer\nchannel N)] --> TR{Read\npos - D}
        TR --> TO[Layer Output\n= delayed frame]
        TI[Layer Input] -.-> |ignored| X[ ]
    end

    style SEND fill:#1a1a2e,color:#ff3b00
    style RECEIVE fill:#1a1a2e,color:#00ff88
    style TAP fill:#1a1a2e,color:#8888ff
```

## Parameter Relevance by Mode

```
             Send    Receive    Tap
            ------  ---------  -----
Channel       ✓        ✓        ✓     which buffer to use
Sync Mode     ✓        ✓        ✓     how delay time is calculated
Subdivision   ✓        ✓        ✓     delay time (if sync=subdivision)
Delay Ms      ✓        ✓        ✓     delay time (if sync=ms)
Delay Frames  ✓        ✓        ✓     delay time (if sync=frames)
Feedback      -        ✓        -     mix amplitude of delayed signal
```

Time params are meaningful in ALL modes — they control the read position.
Send uses them because it outputs the delayed frame, not the input.

## Resolume Composition: Recursive Overdub

```mermaid
flowchart TB
    subgraph COMP["Resolume Composition (render order: bottom → top)"]
        direction TB

        subgraph L1["Layer 1 — Source (opacity 0%)"]
            CAM[Camera / Clip]
        end

        subgraph L2["Layer 2 — Feedback Loop (blend: Add)"]
            direction LR
            VR[VideoRouter → L1] --> RCV["Receive(ch1)\nfb=0.7"]
            RCV --> FX["FX Chain\n(rotate, blur, color)"]
            FX --> SND["Send(ch1)\n1/4 note"]
        end

        subgraph L3["Layer 3 — Dry (blend: Normal)"]
            VR2[VideoRouter → L1]
        end
    end

    SND -.-> |"output feeds back\nas next frame's\nlayer input"| NOTE["⚠ NOT how it works —\nSend output goes to\ncomposition, not back\nto layer input"]

    style NOTE fill:#440000,color:#ff6666
```

**Wait — where does the recursion actually happen?**

In the current code, Send writes its input and outputs the delayed frame.
But Resolume doesn't feed a layer's output back to its own input — layers
are one-pass top-to-bottom. The recursion happens purely through the **buffer**:

```mermaid
flowchart LR
    subgraph FRAME_N["Frame N"]
        IN1[Layer Input] -->|"= live source\n(same every frame)"| SEND1[Send]
        SEND1 -->|write| BUF1[(Buffer)]
        BUF1 -->|"read [pos-D]"| SEND1
        SEND1 -->|"output delayed"| COMP1[Composition]
    end

    subgraph FRAME_N_PLUS_D["Frame N+D"]
        IN2[Layer Input] -->|"= same live source"| SEND2[Send]
        SEND2 -->|"write (overwrites frame N)"| BUF2[(Buffer)]
        BUF2 -->|"read frame N"| SEND2
        SEND2 -->|output| COMP2[Composition]
    end
```

**With Send alone, there's no recursion** — each frame writes the same live source.
The buffer just holds D copies of the input. Output is always a D-frame-old copy of
the same source. No compound echoes.

**For actual recursive overdub, you need Receive → FX → Send on the same channel:**

```mermaid
flowchart LR
    subgraph FRAME["Each Frame (on one Resolume layer)"]
        direction LR
        IN[Layer Input\n= live source] --> RCV[Receive ch1]
        RCV -->|"input + fb × buf[pos-D]"| FX[FX Chain\nrotate, blur...]
        FX --> SND[Send ch1]
        SND -->|"write FX output\nto buf[pos]"| BUF[(Ring Buffer)]
        BUF -->|"read buf[pos-D]"| RCV
        SND -->|"output buf[pos-D]"| OUT[Layer Output]
    end
```

This is the magic: **Receive mixes live + delayed, FX transforms it, Send writes
the compound result back.** Each generation accumulates another pass of the FX chain.
The buffer contents evolve over time — they're not just copies of the source.
