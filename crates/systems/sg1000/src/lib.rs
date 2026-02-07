//! SG-1000 emulator implementation
//!
//! This crate implements emulation of the Sega SG-1000 home console.
//!
//! # Architecture
//!
//! - **CPU**: Zilog Z80A @ 3.58 MHz
//! - **VDP**: Texas Instruments TMS9918A
//! - **PSG**: Texas Instruments SN76489
//! - **RAM**: 1 KB main RAM, 16 KB video RAM
//!
//! The SG-1000 shares very similar hardware with the ColecoVision,
//! using the same core components (Z80, TMS9918A VDP, SN76489 PSG).

mod bus;
mod debugger;
mod psg;
mod system;

pub use system::Sg1000System;

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn smoke_test_sg1000() {
        // Load the test ROM
        let rom = include_bytes!("../../../../test_roms/sg1000/test.sg");
        let mut system = Sg1000System::new();
        system.mount("Cartridge", rom).unwrap();

        system.reset();

        // Run for several frames to allow initialization
        for _ in 0..10 {
            let _ = system.step_frame();
        }

        // Get a frame
        let frame = system.step_frame().unwrap();

        // Verify frame dimensions (TMS9918A Graphics I mode)
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 192);

        // Verify we have actual pixel data
        assert_eq!(frame.pixels.len(), 256 * 192);

        // The VDP is rendering (even if just backdrop color)
        // This confirms basic system functionality is working
    }
}
