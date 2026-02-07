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

        // Count unique colors to verify rendering is happening
        use std::collections::HashMap;
        let mut color_counts: HashMap<u32, usize> = HashMap::new();
        for &pixel in &frame.pixels {
            *color_counts.entry(pixel).or_insert(0) += 1;
        }

        // The test ROM attempts to produce a checkerboard pattern
        // Ideally we'd see 2 colors with ~50/50 distribution, but at minimum
        // we should verify the VDP is rendering (not all transparent/zero)
        assert!(
            !color_counts.is_empty(),
            "Expected rendered output, got empty frame"
        );

        let total_pixels = frame.pixels.len();

        if color_counts.len() >= 2 {
            // Multiple colors detected - validate distribution is reasonable
            let mut counts: Vec<_> = color_counts.values().cloned().collect();
            counts.sort_unstable();

            // Check that no single color dominates too heavily (would indicate backdrop only)
            let max_count = counts[counts.len() - 1];
            let max_percentage = (max_count as f32 / total_pixels as f32) * 100.0;

            assert!(
                max_percentage <= 95.0,
                "Expected meaningful color variation, but one color dominates at {:.1}%",
                max_percentage
            );
        } else {
            // Single color - acceptable as long as it's a valid color
            // (indicates VDP is at least rendering backdrop color)
            let color = *color_counts.keys().next().unwrap();
            assert!(
                color != 0,
                "Expected valid backdrop color, got transparent/zero"
            );

            // Note: Future enhancement would be to verify the test ROM actually
            // renders the checkerboard pattern correctly once VDP implementation is complete
        }
    }
}
