//! Game Boy and Game Boy Color system implementation
//!
//! This module provides an emulator for the Nintendo Game Boy (DMG) and Game Boy Color (GBC) systems.
//! The implementation includes CPU emulation (Sharp LR35902), PPU (LCD controller), memory bus with
//! joypad support, and basic cartridge loading.
//!
//! # Architecture
//!
//! The Game Boy system consists of several key components:
//!
//! ## CPU: Sharp LR35902
//! - Z80-like CPU with some instructions removed and modified
//! - 8-bit registers: A, F, B, C, D, E, H, L (no shadow registers like Z80)
//! - 16-bit registers: SP (stack pointer), PC (program counter)
//! - Flags: Z (Zero), N (Subtract), H (Half Carry), C (Carry)
//! - Clock speed: 4.194304 MHz (~4.2 MHz)
//! - Implementation in `crates/core/src/cpu_lr35902.rs`
//!
//! ## PPU (Picture Processing Unit)
//! - Resolution: 160x144 pixels
//! - Display modes: DMG (4 shades of gray), CGB (32,768 colors)
//! - **Current implementation**: DMG mode only
//! - Supports:
//!   - Background layer with scrolling (SCX/SCY registers)
//!   - Window layer (overlay window with separate position)
//!   - 40 sprites (8x8 or 8x16 pixels)
//!   - Up to 10 sprites per scanline
//!   - Sprite priority and transparency
//!   - Horizontal/vertical sprite flipping
//! - Tile-based graphics (8x8 pixel tiles, 2 bits per pixel)
//! - Two tile data areas: $8000-$8FFF and $8800-$97FF
//! - Two tile map areas: $9800-$9BFF and $9C00-$9FFF
//!
//! ## Memory Map
//! - `$0000-$3FFF`: ROM Bank 0 (16KB, fixed)
//! - `$4000-$7FFF`: ROM Bank 1-N (16KB, switchable via MBC)
//! - `$8000-$9FFF`: VRAM (8KB, video RAM)
//! - `$A000-$BFFF`: External RAM (8KB, switchable via MBC)
//! - `$C000-$DFFF`: Work RAM (8KB)
//! - `$E000-$FDFF`: Echo RAM (mirror of $C000-$DDFF)
//! - `$FE00-$FE9F`: OAM (Object Attribute Memory - 160 bytes)
//! - `$FF00-$FF7F`: I/O Registers
//! - `$FF80-$FFFE`: High RAM (127 bytes)
//! - `$FFFF`: Interrupt Enable register
//!
//! ## I/O Registers
//! - `$FF00`: Joypad input (P1)
//! - `$FF0F`: Interrupt Flag (IF)
//! - `$FF10-$FF14`: APU Pulse 1 (sweep, duty, envelope, frequency)
//! - `$FF16-$FF19`: APU Pulse 2 (duty, envelope, frequency)
//! - `$FF1A-$FF1E`: APU Wave (DAC, length, volume, frequency)
//! - `$FF20-$FF23`: APU Noise (length, envelope, polynomial, control)
//! - `$FF24-$FF26`: APU Master (volume, panning, power)
//! - `$FF30-$FF3F`: Wave RAM (16 bytes, 32 x 4-bit samples)
//! - `$FF40`: LCD Control (LCDC)
//! - `$FF41`: LCD Status (STAT)
//! - `$FF42-$FF43`: Scroll registers (SCY, SCX)
//! - `$FF44`: LCD Y coordinate (LY)
//! - `$FF45`: LY Compare (LYC)
//! - `$FF47-$FF49`: Palette registers (BGP, OBP0, OBP1)
//! - `$FF4A-$FF4B`: Window position (WY, WX)
//! - `$FF50`: Boot ROM disable
//! - `$FFFF`: Interrupt Enable (IE)
//!
//! ## Joypad Input
//! The joypad register ($FF00) uses a matrix system:
//! - Bit 5: Select button keys (0 = selected)
//! - Bit 4: Select direction keys (0 = selected)
//! - Bits 3-0: Input bits (0 = pressed, 1 = not pressed)
//!   - Button mode: Start, Select, B, A
//!   - Direction mode: Down, Up, Left, Right
//!
//! # Timing
//!
//! - CPU clock: 4.194304 MHz
//! - Frame rate: ~59.73 Hz
//! - Cycles per frame: ~70,224
//! - Scanline cycles: 456 (114 machine cycles)
//! - Scanlines per frame: 154 (144 visible + 10 VBlank)
//!
//! # Current Implementation Status
//!
//! ## Implemented Features
//! - ✅ CPU: Full LR35902 instruction set
//! - ✅ PPU: Background rendering with scrolling
//! - ✅ PPU: Window rendering
//! - ✅ PPU: Sprite rendering (8x8 and 8x16 modes)
//! - ✅ PPU: Sprite priority, flipping, and transparency
//! - ✅ Memory: Full memory map with VRAM/OAM access
//! - ✅ Joypad: Button input via register $FF00
//! - ✅ I/O: Essential PPU and joypad registers
//! - ✅ Save states: Full CPU state preservation
//! - ✅ APU: 4 sound channels (pulse 1/2, wave, noise)
//! - ✅ APU: Frame sequencer and envelope/sweep control
//! - ✅ APU: Audio sample generation at 44.1 kHz
//!
//! ## Not Yet Implemented
//! - ❌ MBC (Memory Bank Controllers): MBC1, MBC3, MBC5
//! - ❌ Game Boy Color: CGB mode, color palettes
//! - ❌ Audio: Frontend integration (APU implemented but not connected)
//! - ❌ Serial: Link cable communication
//! - ❌ Timer: Programmable timer registers
//! - ❌ Interrupts: NMI/IRQ handling
//! - ❌ DMA: OAM DMA transfer
//!
//! # Known Limitations
//!
//! 1. **Timing Model**: Frame-based rendering (not cycle-accurate)
//!    - PPU renders entire frames at once
//!    - Some timing-critical effects may not work
//!    - Trade-off: Better compatibility vs. perfect accuracy
//!
//! 2. **ROM Support**: Only MBC0 (no mapper) cartridges currently work
//!    - Most commercial games require MBC1, MBC3, or MBC5
//!    - Homebrew ROMs under 32KB should work
//!
//! 3. **Game Boy Color**: Not yet supported
//!    - DMG (original Game Boy) mode only
//!    - No color palette support
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use emu_core::System;
//! use emu_gb::GbSystem;
//!
//! // Create a new Game Boy system
//! let mut gb = GbSystem::new();
//!
//! // Load a ROM
//! let rom_data = std::fs::read("game.gb").unwrap();
//! gb.mount("Cartridge", &rom_data).unwrap();
//!
//! // Set controller state (buttons: Right=0, Left=1, Up=2, Down=3, A=4, B=5, Select=6, Start=7)
//! gb.set_controller(0x00); // All buttons released
//! gb.set_controller(0x10); // A button pressed
//!
//! // Run one frame
//! let frame = gb.step_frame().unwrap();
//! assert_eq!(frame.width, 160);
//! assert_eq!(frame.height, 144);
//! ```

use emu_core::{cpu_lr35902::CpuLr35902, types::Frame, MountPointInfo, System};

mod apu;
mod bus;
pub(crate) mod ppu;

use bus::GbBus;

pub struct GbSystem {
    cpu: CpuLr35902<GbBus>,
    cart_loaded: bool,
}

impl Default for GbSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GbSystem {
    pub fn new() -> Self {
        let bus = GbBus::new();
        let mut cpu = CpuLr35902::new(bus);
        cpu.reset();

        Self {
            cpu,
            cart_loaded: false,
        }
    }

    /// Set controller state (Game Boy buttons)
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_controller(&mut self, state: u8) {
        self.cpu.memory.set_buttons(state);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GbError {
    #[error("No cartridge loaded")]
    NoCartridge,
    #[error("Invalid mount point")]
    InvalidMountPoint,
}

impl System for GbSystem {
    type Error = GbError;

    fn reset(&mut self) {
        self.cpu.reset();
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.cart_loaded {
            return Err(GbError::NoCartridge);
        }

        // Game Boy runs at ~4.194304 MHz
        // Frame rate is ~59.73 Hz
        // Cycles per frame: 4194304 / 59.73 ≈ 70224 cycles
        const CYCLES_PER_FRAME: u32 = 70224;

        let mut cycles = 0;
        while cycles < CYCLES_PER_FRAME {
            let cpu_cycles = self.cpu.step();
            cycles += cpu_cycles;

            // Step PPU
            if self.cpu.memory.ppu.step(cpu_cycles) {
                // V-Blank started - could trigger NMI here
            }
        }

        // Render the frame from PPU
        Ok(self.cpu.memory.ppu.render_frame())
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({
            "system": "gb",
            "version": 1,
            "cpu": {
                "a": self.cpu.a,
                "f": self.cpu.f,
                "b": self.cpu.b,
                "c": self.cpu.c,
                "d": self.cpu.d,
                "e": self.cpu.e,
                "h": self.cpu.h,
                "l": self.cpu.l,
                "sp": self.cpu.sp,
                "pc": self.cpu.pc,
                "ime": self.cpu.ime,
                "halted": self.cpu.halted,
                "stopped": self.cpu.stopped,
            }
        })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        if let Some(cpu_state) = v.get("cpu") {
            load_u8!(cpu_state, "a", self.cpu.a);
            load_u8!(cpu_state, "f", self.cpu.f);
            load_u8!(cpu_state, "b", self.cpu.b);
            load_u8!(cpu_state, "c", self.cpu.c);
            load_u8!(cpu_state, "d", self.cpu.d);
            load_u8!(cpu_state, "e", self.cpu.e);
            load_u8!(cpu_state, "h", self.cpu.h);
            load_u8!(cpu_state, "l", self.cpu.l);
            load_u16!(cpu_state, "sp", self.cpu.sp);
            load_u16!(cpu_state, "pc", self.cpu.pc);
            load_bool!(cpu_state, "ime", self.cpu.ime);
            load_bool!(cpu_state, "halted", self.cpu.halted);
            load_bool!(cpu_state, "stopped", self.cpu.stopped);
        }
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["gb".to_string(), "gbc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cpu.memory.load_cart(data);
        self.cart_loaded = true;
        self.reset();

        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cart_loaded = false;
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Cartridge" && self.cart_loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gb_system_creation() {
        let sys = GbSystem::new();
        assert!(!sys.cart_loaded);
    }

    #[test]
    fn test_gb_mount_points() {
        let sys = GbSystem::new();
        let mount_points = sys.mount_points();
        assert_eq!(mount_points.len(), 1);
        assert_eq!(mount_points[0].id, "Cartridge");
        assert!(mount_points[0].required);
    }

    #[test]
    fn test_gb_mount_unmount() {
        let mut sys = GbSystem::new();
        assert!(!sys.is_mounted("Cartridge"));

        // Mount a minimal ROM
        let rom = vec![0; 0x8000]; // 32KB ROM
        assert!(sys.mount("Cartridge", &rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        assert!(sys.unmount("Cartridge").is_ok());
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_gb_save_load_state() {
        let sys = GbSystem::new();
        let state = sys.save_state();
        assert_eq!(state["system"], "gb");
        assert_eq!(state["version"], 1);

        let mut sys2 = GbSystem::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_gb_supports_save_states() {
        let sys = GbSystem::new();
        assert!(sys.supports_save_states());
    }

    #[test]
    fn test_gb_step_frame_without_cart() {
        let mut sys = GbSystem::new();
        let result = sys.step_frame();
        assert!(result.is_err());
    }

    #[test]
    fn test_gb_step_frame_with_cart() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        let result = sys.step_frame();
        assert!(result.is_ok());
        let frame = result.unwrap();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_gb_controller_input() {
        let mut sys = GbSystem::new();

        // Test setting controller state
        sys.set_controller(0xFF); // All buttons released

        // Test individual buttons
        sys.set_controller(0x01); // Right pressed
        sys.set_controller(0x10); // A pressed
        sys.set_controller(0x80); // Start pressed
    }

    #[test]
    fn test_gb_ppu_registers() {
        let sys = GbSystem::new();

        // Verify initial PPU register values
        assert_eq!(sys.cpu.memory.ppu.lcdc, 0x91);
        assert_eq!(sys.cpu.memory.ppu.bgp, 0xFC);
        assert_eq!(sys.cpu.memory.ppu.ly, 0);
    }

    #[test]
    fn test_gb_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/gb/test.gb");

        let mut sys = GbSystem::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions are correct
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
        assert_eq!(frame.pixels.len(), 160 * 144);

        // The test ROM fills the screen with a checkerboard pattern.
        // Verify that the frame contains non-black pixel data.
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000) // Not black (ARGB format)
            .count();

        // Should have visible pixels from the test pattern (at least 2000 non-black pixels)
        assert!(
            non_black_pixels > 2000,
            "Expected non-black pixels from test ROM, got {} out of {}",
            non_black_pixels,
            160 * 144
        );
    }

    #[test]
    fn test_gbc_smoke_test_rom() {
        // Load the GBC test ROM
        let test_rom = include_bytes!("../../../../test_roms/gbc/test.gbc");

        let mut sys = GbSystem::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        // Note: This ROM has CGB flag set but should work in DMG mode too
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions are correct
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
        assert_eq!(frame.pixels.len(), 160 * 144);

        // The test ROM fills the screen with a checkerboard pattern.
        // Verify that the frame contains non-black pixel data.
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000) // Not black (ARGB format)
            .count();

        // Should have visible pixels from the test pattern (at least 2000 non-black pixels)
        assert!(
            non_black_pixels > 2000,
            "Expected non-black pixels from GBC test ROM in DMG mode, got {} out of {}",
            non_black_pixels,
            160 * 144
        );
    }
}

#[cfg(test)]
