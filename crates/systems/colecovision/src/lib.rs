//! ColecoVision emulator implementation
//!
//! This crate implements emulation of the ColecoVision home console.
//!
//! # Architecture
//!
//! - **CPU**: Zilog Z80A @ 3.58 MHz
//! - **VDP**: Texas Instruments TMS9918A
//! - **PSG**: Texas Instruments SN76489
//! - **RAM**: 1 KB main RAM, 16 KB video RAM
//! - **BIOS**: 8 KB system ROM
//!
//! For detailed implementation information, see the README.md

mod bus;
mod debugger;
mod psg;
mod system;

pub use system::ColecoVisionSystem;

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn test_system_creation() {
        let system = ColecoVisionSystem::new();
        assert_eq!(system.get_total_cycles(), 0);
    }

    #[test]
    fn test_system_reset() {
        let mut system = ColecoVisionSystem::new();

        // Load dummy BIOS and cartridge
        let bios = vec![0; 0x2000];
        let cart = vec![0; 0x8000];
        system.load_bios(bios);
        system.load_cartridge(cart);

        // Run some cycles
        let _ = system.step_frame();
        assert!(system.get_total_cycles() > 0);

        // Reset should clear cycles
        system.reset();
        assert_eq!(system.get_total_cycles(), 0);
    }

    #[test]
    #[ignore] // TODO: Fix sprite collision test after TMS9918A refactor
    fn test_vdp_sprite_collision_detection() {
        use emu_core::tms9918a::Tms9918a;

        let mut vdp = Tms9918a::new();

        // Enable display and sprites
        vdp.write_control(0x00); // Set address low byte
        vdp.write_control(0x81); // Write to register 1
        vdp.write_control(0x40); // Enable display (bit 6)
        vdp.write_control(0x81); // Write to register 1

        // Set up sprite attribute table at 0x3F00
        vdp.write_control(0x7E); // Address low (0x7E << 7 = 0x3F00)
        vdp.write_control(0x85); // Register 5: sprite attribute table

        // Set up sprite pattern table at 0x0000
        vdp.write_control(0x00); // Address low
        vdp.write_control(0x86); // Register 6: sprite pattern table

        // Write sprite attribute for sprite 0 (Y=50, X=50)
        vdp.write_control(0x00); // Address low
        vdp.write_control(0x7F); // Address high | write mode
        vdp.write_data(51); // Y position (offset by 1)
        vdp.write_data(50); // X position
        vdp.write_data(0); // Pattern number
        vdp.write_data(1); // Color 1

        // Write sprite attribute for sprite 1 (overlapping, Y=50, X=52)
        vdp.write_data(51); // Y position
        vdp.write_data(52); // X position (overlaps with sprite 0)
        vdp.write_data(0); // Pattern number
        vdp.write_data(2); // Color 2

        // Write end marker for sprite list
        vdp.write_data(0xD0);

        // Write pattern data (simple pattern with pixels set)
        vdp.write_control(0x00); // Address low
        vdp.write_control(0x40); // Address high | write mode
        for _ in 0..8 {
            vdp.write_data(0xFF); // All pixels on
        }

        // Render the frame to detect sprite collision
        vdp.render_frame();

        // Read status to check collision flag
        let status = vdp.read_status();
        // Bit 5 should be set for sprite collision
        assert_ne!(status & 0x20, 0, "Sprite collision should be detected");
    }

    #[test]
    #[ignore] // TODO: Fix sprite overflow test after TMS9918A refactor
    fn test_vdp_sprite_overflow() {
        use emu_core::tms9918a::Tms9918a;

        let mut vdp = Tms9918a::new();

        // Enable display
        vdp.write_control(0x40);
        vdp.write_control(0x81);

        // Set up sprite attribute table at 0x3F00
        vdp.write_control(0x7E); // 0x7E << 7 = 0x3F00
        vdp.write_control(0x85);

        // Set up sprite pattern table
        vdp.write_control(0x00);
        vdp.write_control(0x86);

        // Write to attribute table
        vdp.write_control(0x00);
        vdp.write_control(0x7F);

        // Create 5 sprites on the same scanline (Y=50, adjusted for offset)
        // This should trigger sprite overflow (max 4 per line)
        for i in 0..5 {
            vdp.write_data(51); // Y position (50 + 1 offset)
            vdp.write_data(i * 20); // X position (spread out to avoid collision)
            vdp.write_data(0); // Pattern
            vdp.write_data(1); // Color
        }

        // End marker
        vdp.write_data(0xD0);

        // Write pattern data
        vdp.write_control(0x00);
        vdp.write_control(0x40);
        for _ in 0..8 {
            vdp.write_data(0xFF);
        }

        // Clear any previous status
        let _ = vdp.read_status();

        // Render the frame to detect sprite overflow
        vdp.render_frame();

        // Check status
        let status = vdp.read_status();
        // Bit 6 should be set for sprite overflow
        assert_ne!(
            status & 0x40,
            0,
            "Sprite overflow should be detected with 5 sprites on line 50"
        );
    }

    #[test]
    fn test_vdp_address_register_wrapping() {
        use emu_core::tms9918a::Tms9918a;

        let mut vdp = Tms9918a::new();

        // Set address to near end of VRAM (0x3FFE)
        vdp.write_control(0xFE);
        vdp.write_control(0x7F);

        // Write data - should wrap around
        vdp.write_data(0x42);
        vdp.write_data(0x43);
        vdp.write_data(0x44);

        // Read back from wrapped addresses
        vdp.write_control(0xFE);
        vdp.write_control(0x3F);
        assert_eq!(vdp.read_data(), 0x42);

        vdp.write_control(0xFF);
        vdp.write_control(0x3F);
        assert_eq!(vdp.read_data(), 0x43);

        vdp.write_control(0x00);
        vdp.write_control(0x00);
        assert_eq!(vdp.read_data(), 0x44);
    }

    #[test]
    fn test_save_state_roundtrip() {
        let mut system1 = ColecoVisionSystem::new();

        // Load dummy data
        system1.load_bios(vec![0; 0x2000]);
        system1.load_cartridge(vec![0; 0x8000]);

        // Run some cycles
        let _ = system1.step_frame();

        // Save state
        let state = system1.save_state();

        // Create new system and load state
        let mut system2 = ColecoVisionSystem::new();
        system2.load_bios(vec![0; 0x2000]);
        system2.load_cartridge(vec![0; 0x8000]);
        system2.load_state(&state).unwrap();

        // Verify cycles match
        assert_eq!(system1.get_total_cycles(), system2.get_total_cycles());
    }
}
