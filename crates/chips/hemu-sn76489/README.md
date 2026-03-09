# hemu-sn76489

Standalone emulation core for the **Texas Instruments SN76489** Programmable Sound Generator (PSG).

Used in the Sega Master System, Game Gear, ColecoVision, Sega SG-1000, and many other home computers and arcade boards.

## Features

- 3 tone channels (square wave, 10-bit frequency)
- 1 noise channel (white/periodic noise, LFSR)
- 4-bit volume control per channel
- NTSC/PAL clock support
- Sega variant (SN76496, 16-bit LFSR)

## Usage

```rust
use hemu_sn76489::{sn76489::Sn76489Adapter, TimingMode};

let mut psg = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_579_545.0);
psg.write_register(0, 0x9F); // channel 0 volume = 0 (silent)
let sample = psg.clock();    // generate one audio sample
```

## References

- [SMS Power SN76489 documentation](https://www.smspower.org/Development/SN76489)
- [SMS Technical Reference (Brett Ewins)](https://www.smspower.org/Development/SMSTechnicalManual)
