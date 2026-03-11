# Delay Line Module v2 — Implementation Plan

## Step 1: Update params.rs

- Add `Mode::Tap` variant
- Add `PARAM_SYNC_MODE` (Option: Subdivision / Ms / Frames)
- Add `PARAM_DELAY_MS` (Standard, 0.0–1.0 mapped to 0–5000ms)
- Add `PARAM_DELAY_FRAMES` (Standard, 0.0–1.0 mapped to 1–899)
- Renumber: Mode(0), Channel(1), SyncMode(2), Subdivision(3), DelayMs(4), DelayFrames(5), Feedback(6)
- NUM_PARAMS = 7
- Add `sync_mode()`, `delay_ms()`, `delay_frames_raw()` accessors

## Step 2: Update delay.rs

- Refactor `delay_frames()` to branch on sync mode:
  - Subdivision: existing BPM-based calculation
  - Ms: `(delay_ms / 1000.0 * fps_estimate).round()`
  - Frames: direct value from param
- Add `draw_tap()` method: same as Receive's buffer lookup but calls `read_pass` instead of `receive_pass`. No feedback, no input mixing.
- Add `Mode::Tap` arm in `draw()` match

## Step 3: Test build

- Cross-compile, verify in Resolume
- Test all three sync modes on Send, Receive, and Tap
- Verify Tap outputs delayed frame with black source underneath
