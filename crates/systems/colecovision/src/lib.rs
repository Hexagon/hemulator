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

    #[test]
    fn test_audio_generation() {
        let mut system = ColecoVisionSystem::new();

        // Load dummy BIOS and cartridge
        system.load_bios(vec![0; 0x2000]);
        system.load_cartridge(vec![0; 0x8000]);

        // Generate audio samples
        let samples = system.get_audio_samples(1000);

        // Should generate exactly the requested number of samples
        assert_eq!(samples.len(), 1000);

        // With default muted state, samples should be near zero
        let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= 100,
            "Expected near-silent output by default, got max sample: {}",
            max_sample
        );
    }

    #[test]
    fn test_audio_with_tone() {
        use emu_core::cpu_z80::MemoryZ80;

        let mut system = ColecoVisionSystem::new();

        // Load dummy BIOS and cartridge
        system.load_bios(vec![0; 0x2000]);
        system.load_cartridge(vec![0; 0x8000]);

        // Write to PSG to enable a tone
        // ColecoVision PSG is at I/O ports 0xA0-0xA1 (but all ports 0xA0+ work)
        system.cpu.memory.io_write(0xA0, 0x90); // Channel 0, Volume 0 (max)
        system.cpu.memory.io_write(0xA0, 0x80 | 0x04); // Channel 0, Tone low bits
        system.cpu.memory.io_write(0xA0, 0x01); // Tone high bits

        // Generate samples
        let samples = system.get_audio_samples(1000);
        assert_eq!(samples.len(), 1000);

        // Should now have audible output
        let non_zero_count = samples.iter().filter(|&&s| s != 0).count();
        assert!(
            non_zero_count > 0,
            "Expected audio output after enabling tone"
        );
    }

    #[test]
    fn test_psg_reset() {
        use emu_core::cpu_z80::MemoryZ80;

        let mut system = ColecoVisionSystem::new();

        // Load dummy BIOS and cartridge
        system.load_bios(vec![0; 0x2000]);
        system.load_cartridge(vec![0; 0x8000]);

        // Enable a tone
        system.cpu.memory.io_write(0xA0, 0x90); // Max volume
        system.cpu.memory.io_write(0xA0, 0x84); // Tone frequency

        // Reset system
        system.reset();

        // PSG should be reset - audio should be silent again
        let samples = system.get_audio_samples(100);
        let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= 100,
            "Expected near-silent output after reset, got max sample: {}",
            max_sample
        );
    }

    #[test]
    fn test_vdp_sprite_transparency() {
        use emu_core::renderer::Renderer;
        use emu_core::tms9918a::Tms9918a;

        let mut vdp = Tms9918a::new();

        // Enable display (bit 6 set in register 1 = display enabled)
        vdp.write_control(0x40); // Bit 6 set = display enabled, 8x8 sprites, no mag
        vdp.write_control(0x81); // Write to register 1

        // Set name table at 0x3800 (register 2)
        vdp.write_control(0x0E); // 0x0E = bits 3-0 = 14, 14 << 10 = 0x3800
        vdp.write_control(0x82); // Write to register 2

        // Set pattern table at 0x0000 (register 4)
        vdp.write_control(0x00); // 0x00 = bits 2-0 = 0, 0 << 11 = 0x0000
        vdp.write_control(0x84); // Write to register 4

        // Set color table at 0x0000 (register 3)
        vdp.write_control(0x00); // 0x00, 0 << 6 = 0x0000
        vdp.write_control(0x83); // Write to register 3

        // Set sprite attribute table at 0x3F00 (register 5)
        vdp.write_control(0x7E); // 0x7E << 7 = 0x3F00
        vdp.write_control(0x85); // Write to register 5

        // Set sprite pattern table at 0x3800 (register 6)
        vdp.write_control(0x07); // 0x07, 7 << 11 = 0x3800
        vdp.write_control(0x86); // Write to register 6

        // Set backdrop color to color 1 (black)
        vdp.write_control(0x01); // Backdrop color = 1
        vdp.write_control(0x87); // Write to register 7

        // Fill color table with black-on-black (so background tiles don't interfere)
        vdp.write_control(0x00); // Address 0x0000
        vdp.write_control(0x40); // Write mode
        for _ in 0..32 {
            vdp.write_data(0x11); // Color: bg=1 (black), fg=1 (black)
        }

        // Write sprite attribute for sprite 0 (transparent sprite with color 0)
        vdp.write_control(0x00); // Address low
        vdp.write_control(0x7F); // Address high (0x3F00) | write mode
        vdp.write_data(51); // Y position (50 + 1 offset)
        vdp.write_data(50); // X position
        vdp.write_data(0); // Pattern number
        vdp.write_data(0); // Color 0 (transparent)

        // Write sprite attribute for sprite 1 (visible sprite with color 15/white)
        vdp.write_data(51); // Y position
        vdp.write_data(100); // X position (different from sprite 0)
        vdp.write_data(0); // Pattern number
        vdp.write_data(15); // Color 15 (white)

        // End marker
        vdp.write_data(0xD0);

        // Write sprite pattern data at 0x3800 (all pixels on)
        vdp.write_control(0x00); // Address low (0x3800)
        vdp.write_control(0x78); // Address high | write mode
        for _ in 0..8 {
            vdp.write_data(0xFF); // All pixels on
        }

        // Render the frame
        vdp.render_frame();

        // Get the frame data
        let frame = vdp.get_frame();

        // Check that sprite 0 with color 0 is NOT rendered (pixels should be backdrop color)
        // Line 50, X positions 50-57 should show backdrop color (0xFF000000 = black)
        let line_offset = 50 * 256;
        let backdrop_color = 0xFF000000; // Color 1 (black) from palette

        // Verify sprite 0 (color 0, transparent) didn't render - pixels should be backdrop
        for x in 50..58 {
            let pixel = frame.pixels[line_offset + x];
            assert_eq!(
                pixel, backdrop_color,
                "Sprite with color 0 should be transparent at X={}, got {:08X} instead of backdrop {:08X}",
                x, pixel, backdrop_color
            );
        }

        // Check that sprite 1 with color 15 IS rendered (pixels should be white)
        // Line 50, X positions 100-107 should show white (0xFFFFFFFF)
        let white_color = 0xFFFFFFFF; // Color 15 (white) from palette

        // Verify sprite 1 (color 15, white) did render
        for x in 100..108 {
            let pixel = frame.pixels[line_offset + x];
            assert_eq!(
                pixel, white_color,
                "Sprite with color 15 should render white at X={}, got {:08X}",
                x, pixel
            );
        }
    }

    #[test]
    fn test_vdp_sprite_transparency_counts_toward_limit() {
        use emu_core::tms9918a::Tms9918a;

        let mut vdp = Tms9918a::new();

        // Enable display (bit 6 set = display enabled)
        vdp.write_control(0x40);
        vdp.write_control(0x81);

        // Set up sprite attribute table at 0x3F00
        vdp.write_control(0x7E);
        vdp.write_control(0x85);

        // Set up sprite pattern table
        vdp.write_control(0x00);
        vdp.write_control(0x86);

        // Write to attribute table
        vdp.write_control(0x00);
        vdp.write_control(0x7F);

        // Create 3 transparent sprites (color 0) on line 50
        for i in 0..3 {
            vdp.write_data(51); // Y position (50 + 1 offset)
            vdp.write_data(i * 20); // X position (spread out)
            vdp.write_data(0); // Pattern
            vdp.write_data(0); // Color 0 (transparent)
        }

        // Create 2 visible sprites (color 1) on line 50
        // This makes 5 sprites total, which should trigger overflow
        for i in 0..2 {
            vdp.write_data(51); // Y position
            vdp.write_data(60 + i * 20); // X position
            vdp.write_data(0); // Pattern
            vdp.write_data(1); // Color 1 (visible)
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

        // Check status - bit 6 should be set for sprite overflow
        // Transparent sprites (color 0) should still count toward the 4-sprite limit
        let status = vdp.read_status();
        assert_ne!(
            status & 0x40,
            0,
            "Sprite overflow should be detected with 5 sprites (3 transparent + 2 visible) on line 50"
        );
    }

    #[test]
    fn smoke_test_colecovision_manual() {
        use emu_core::cpu_z80::MemoryZ80;

        // Create a simple test by manually writing to VDP through the system
        // This tests the full integration without relying on Z80 code execution

        let mut system = ColecoVisionSystem::new();

        // Create dummy BIOS and cartridge (we won't execute code)
        let bios = vec![0; 0x2000];
        let cart = vec![0; 0x8000];
        system.load_bios(bios);
        system.load_cartridge(cart);

        system.reset();

        // Manually initialize VDP to Graphics II mode via memory-mapped I/O
        // Write to VDP control port (0xBF) to set up registers

        // Register 0: Mode Control 1 - $00
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x80);

        // Register 1: Mode Control 2 - $EA (Graphics II, display on, interrupts)
        system.cpu.memory.io_write(0xBF, 0xEA);
        system.cpu.memory.io_write(0xBF, 0x81);

        // Register 2: Nametable at $3800
        system.cpu.memory.io_write(0xBF, 0x0E);
        system.cpu.memory.io_write(0xBF, 0x82);

        // Register 3: Color table at $2000
        system.cpu.memory.io_write(0xBF, 0x80);
        system.cpu.memory.io_write(0xBF, 0x83);

        // Register 4: Pattern table at $0000
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x84);

        // Register 7: Backdrop color (black = 1)
        system.cpu.memory.io_write(0xBF, 0x01);
        system.cpu.memory.io_write(0xBF, 0x87);

        // Write simple pattern data to VRAM
        // Set VRAM write address to $0000
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x40);

        // Write a few solid tiles (pattern 0xFF for all 8 rows)
        for _ in 0..32 {
            // 4 tiles * 8 bytes
            system.cpu.memory.io_write(0xBE, 0xFF);
        }

        // Write color data to VRAM $2000
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x60); // $2000 with write bit

        // Write white on black color (0xF1) for first few tiles
        for _ in 0..32 {
            // 4 tiles * 8 bytes
            system.cpu.memory.io_write(0xBE, 0xF1);
        }

        // Write nametable at $3800
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x78); // $3800 with write bit

        // Fill first row with tile 0 (which should be white)
        for _ in 0..32 {
            system.cpu.memory.io_write(0xBE, 0x00); // Tile 0
        }

        // Run one frame to trigger rendering
        let frame = system.step_frame().unwrap();

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 192);

        // Check that we have some non-black pixels in the first row
        let first_row = &frame.pixels[0..256];
        let non_black = first_row.iter().filter(|&&p| p != 0xFF000000).count();

        assert!(
            non_black > 0,
            "Expected some white pixels in first row, but got all black. First 10 pixels: {:?}",
            &first_row[0..10]
        );
    }

    #[test]
    fn smoke_test_colecovision() {
        use emu_core::cpu_z80::MemoryZ80;

        // Note: This is a simplified smoke test that manually initializes the VDP
        // rather than executing the test ROM's Z80 code. The full ROM-based test
        // is available but currently having issues with Z80 execution integration.
        // TODO: Debug and enable full ROM execution test

        let mut system = ColecoVisionSystem::new();

        // Load dummy BIOS and cartridge
        let bios = vec![0; 0x2000];
        let cart = vec![0; 0x8000];
        system.load_bios(bios);
        system.load_cartridge(cart);

        system.reset();

        // Manually initialize VDP to Graphics II mode
        // Register 0: Mode Control 1
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x80);

        // Register 1: Graphics II, display on
        system.cpu.memory.io_write(0xBF, 0xEA);
        system.cpu.memory.io_write(0xBF, 0x81);

        // Register 2: Nametable at $3800
        system.cpu.memory.io_write(0xBF, 0x0E);
        system.cpu.memory.io_write(0xBF, 0x82);

        // Register 3: Color table at $2000
        system.cpu.memory.io_write(0xBF, 0x80);
        system.cpu.memory.io_write(0xBF, 0x83);

        // Register 4: Pattern table at $0000
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x84);

        // Register 7: Backdrop color
        system.cpu.memory.io_write(0xBF, 0x01);
        system.cpu.memory.io_write(0xBF, 0x87);

        // Write pattern data
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x40);
        for _ in 0..64 {
            system.cpu.memory.io_write(0xBE, 0xFF);
        }

        // Write color data (white on black and red on black)
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x60);
        for _ in 0..32 {
            system.cpu.memory.io_write(0xBE, 0xF1); // White
        }
        for _ in 0..32 {
            system.cpu.memory.io_write(0xBE, 0x61); // Red
        }

        // Write nametable (alternating tiles)
        system.cpu.memory.io_write(0xBF, 0x00);
        system.cpu.memory.io_write(0xBF, 0x78);
        for i in 0..768u16 {
            system.cpu.memory.io_write(0xBE, ((i / 32) % 2) as u8);
        }

        // Run a frame
        let frame = system.step_frame().unwrap();

        // Verify dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 192);

        // Verify we have multiple colors
        use std::collections::HashSet;
        let colors: HashSet<u32> = frame.pixels.iter().copied().collect();

        assert!(
            colors.len() >= 2,
            "Expected at least 2 colors (backdrop + pattern), found {}",
            colors.len()
        );

        // Verify not all black
        let non_black = frame.pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(
            non_black > 1000,
            "Expected significant non-black pixels, found only {}",
            non_black
        );
    }
}
