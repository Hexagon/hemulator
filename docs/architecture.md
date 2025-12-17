# Architecture (brief)

This document sketches the planned architecture:

- `core` defines `System` trait, `Frame`, and common utilities.
- Each `systems/*` crate implements a `System` for a console and contains the CPU, PPU, APU/MMU, and cartridge logic.
- Frontends consume `System` implementations to run headless or interactive emulation.

Timing model: cycle-accurate core logic is preferred; initial focus is on correctness tests (CPU instruction vectors), then PPU timing and audio.

Save-states: JSON for early debugging; upgrade to versioned binary formats later.
