//! Nintendo Game & Watch emulator system.
//!
//! Emulates the Sharp SM510 4-bit microcontroller used in Game & Watch handhelds.
//! The SM510 runs at 32.768 kHz and drives a segment-based LCD display.
//!
//! # ROM Format
//!
//! Accepts raw SM510 program ROM binary files (1–4 KB).
//! Common extensions: `.gw`, `.gnw`
//!
//! # Display
//!
//! Without artwork overlays, LCD segments are rendered as a raw grid showing
//! the state of all 128 RAM nibbles (512 individual segments).

mod debugger;
pub mod sm510;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

/// Display width in pixels
const DISPLAY_WIDTH: u32 = 160;
/// Display height in pixels
const DISPLAY_HEIGHT: u32 = 120;

/// SM510 clock frequency (Hz)
const CPU_CLOCK_HZ: u32 = 32_768;
/// Target frame rate
const TARGET_FPS: u32 = 64;
/// CPU cycles per frame
const CYCLES_PER_FRAME: u32 = CPU_CLOCK_HZ / TARGET_FPS; // 512

// LCD color palette (Game Boy-inspired green tones)
/// LCD background color (light green)
const LCD_BG: u32 = 0xFF9BBC0F;
/// Active segment color (dark green)
const LCD_ON: u32 = 0xFF0F380F;
/// Inactive segment color (slightly darker than background)
const LCD_OFF: u32 = 0xFF8BAC0F;

#[derive(Error, Debug)]
pub enum GameAndWatchError {
    #[error("ROM too large (max 4 KB, got {0} bytes)")]
    RomTooLarge(usize),

    #[error("No ROM loaded")]
    NoRomLoaded,

    #[error("Unknown mount point: {0}")]
    UnknownMountPoint(String),
}

/// Game & Watch emulator system
pub struct GameAndWatchSystem {
    /// SM510 CPU
    pub cpu: sm510::Sm510,
    /// Whether a ROM is loaded
    rom_loaded: bool,
    /// Frame buffer
    frame: Frame,
    /// Audio phase for buzzer tone generation
    audio_phase: f64,
}

impl GameAndWatchSystem {
    /// Create a new Game & Watch system
    pub fn new() -> Self {
        Self {
            cpu: sm510::Sm510::new(),
            rom_loaded: false,
            frame: Frame::new(DISPLAY_WIDTH, DISPLAY_HEIGHT),
            audio_phase: 0.0,
        }
    }

    /// Set controller button state
    ///
    /// Bit mapping:
    /// - 0: Left
    /// - 1: Right
    /// - 2: Up
    /// - 3: Down
    /// - 4: Game A (Z)
    /// - 5: Game B (X)
    /// - 6: Time (T)
    /// - 7: Alarm (Enter)
    /// - 8: ACL (R)
    pub fn set_controller(&mut self, state: u16) {
        self.cpu.controller_state = state;
    }

    /// Render LCD segments from RAM into the frame buffer
    fn render_lcd(&mut self) {
        let pixels = &mut self.frame.pixels;

        // Fill background
        for pixel in pixels.iter_mut() {
            *pixel = LCD_BG;
        }

        // Grid layout: 16 columns (BL) × 8 rows (BM effective)
        // Each cell shows 4 segment bits as vertical bars
        let grid_x_start = 8u32;
        let grid_y_start = 6u32;
        let cell_w = 9u32; // Cell width (4 bars × 2px + 1px gap)
        let cell_h = 13u32; // Cell height
        let bar_w = 2u32;
        let bar_h = 11u32;
        let bar_gap = 0u32;

        for bm in 0u8..8 {
            for bl in 0u8..16 {
                let ram_addr = ((bm as usize) << 4) | (bl as usize);
                let nibble = self.cpu.ram[ram_addr] & 0xF;

                let cx = grid_x_start + bl as u32 * cell_w;
                let cy = grid_y_start + bm as u32 * cell_h;

                // Draw 4 segment bars within the cell
                for bit in 0u8..4 {
                    let is_on = (nibble >> bit) & 1 != 0;
                    let color = if is_on { LCD_ON } else { LCD_OFF };

                    let bx = cx + bit as u32 * (bar_w + bar_gap);
                    let by = cy + 1;

                    for py in by..by + bar_h {
                        for px in bx..bx + bar_w {
                            if px < DISPLAY_WIDTH && py < DISPLAY_HEIGHT {
                                pixels[(py * DISPLAY_WIDTH + px) as usize] = color;
                            }
                        }
                    }
                }
            }
        }

        // Draw a thin border around the grid
        let border_color = 0xFF306230u32;
        let gw = 16 * cell_w + 2;
        let gh = 8 * cell_h + 2;
        let bx0 = grid_x_start.saturating_sub(1);
        let by0 = grid_y_start.saturating_sub(1);

        // Top and bottom lines
        for x in bx0..bx0 + gw {
            if x < DISPLAY_WIDTH {
                if by0 < DISPLAY_HEIGHT {
                    pixels[(by0 * DISPLAY_WIDTH + x) as usize] = border_color;
                }
                let bottom = by0 + gh - 1;
                if bottom < DISPLAY_HEIGHT {
                    pixels[(bottom * DISPLAY_WIDTH + x) as usize] = border_color;
                }
            }
        }
        // Left and right lines
        for y in by0..by0 + gh {
            if y < DISPLAY_HEIGHT {
                if bx0 < DISPLAY_WIDTH {
                    pixels[(y * DISPLAY_WIDTH + bx0) as usize] = border_color;
                }
                let right = bx0 + gw - 1;
                if right < DISPLAY_WIDTH {
                    pixels[(y * DISPLAY_WIDTH + right) as usize] = border_color;
                }
            }
        }
    }

    /// Generate audio samples for this frame
    pub fn generate_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);
        let sample_rate = 44100.0;
        let buzzer_freq = 4000.0; // 4 kHz buzzer tone

        for _ in 0..count {
            let sample = if self.cpu.buzzer_active {
                // Simple square wave
                let val = if self.audio_phase < 0.5 {
                    4000i16
                } else {
                    -4000i16
                };
                self.audio_phase += buzzer_freq / sample_rate;
                if self.audio_phase >= 1.0 {
                    self.audio_phase -= 1.0;
                }
                val
            } else {
                self.audio_phase = 0.0;
                0i16
            };
            samples.push(sample);
        }
        samples
    }

    /// Get debug info as a simple string map
    pub fn debug_info(&self) -> Vec<(String, String)> {
        vec![
            ("ACC".to_string(), format!("${:X}", self.cpu.acc)),
            ("PC".to_string(), format!("${:03X}", self.cpu.pc)),
            ("BL".to_string(), format!("${:X}", self.cpu.bl)),
            ("BM".to_string(), format!("${:X}", self.cpu.bm)),
            (
                "Carry".to_string(),
                format!("{}", if self.cpu.carry { 1 } else { 0 }),
            ),
            ("Stack".to_string(), format!("${:03X}", self.cpu.stack)),
            ("Cycles".to_string(), format!("{}", self.cpu.cycles)),
            (
                "Halted".to_string(),
                (if self.cpu.halted { "Yes" } else { "No" }).to_string(),
            ),
            ("S".to_string(), format!("${:X}", self.cpu.output_s)),
            ("K".to_string(), format!("${:X}", self.cpu.input_k)),
            (
                "Melody".to_string(),
                (if self.cpu.melody_enabled { "On" } else { "Off" }).to_string(),
            ),
        ]
    }
}

impl Default for GameAndWatchSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for GameAndWatchSystem {
    type Error = GameAndWatchError;

    fn reset(&mut self) {
        let had_rom = self.rom_loaded;
        let rom_backup = self.cpu.rom.clone();
        self.cpu.reset();
        if had_rom {
            self.cpu.rom = rom_backup;
            self.rom_loaded = true;
        }
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.rom_loaded {
            // Return empty frame if no ROM
            self.render_lcd();
            return Ok(self.frame.clone());
        }

        // Run CPU for one frame's worth of cycles
        for _ in 0..CYCLES_PER_FRAME {
            self.cpu.step();
        }

        // Render LCD display
        self.render_lcd();

        Ok(self.frame.clone())
    }

    fn save_state(&self) -> Value {
        let ram_b64 = BASE64.encode(self.cpu.ram);
        serde_json::json!({
            "version": 1,
            "system": "gameandwatch",
            "acc": self.cpu.acc,
            "carry": self.cpu.carry,
            "pc": self.cpu.pc,
            "stack": self.cpu.stack,
            "bl": self.cpu.bl,
            "bm": self.cpu.bm,
            "sbm": self.cpu.sbm,
            "skip": self.cpu.skip,
            "ram": ram_b64,
            "output_s": self.cpu.output_s,
            "output_r": self.cpu.output_r,
            "output_l": self.cpu.output_l,
            "output_x": self.cpu.output_x,
            "bp": self.cpu.bp,
            "alpha": self.cpu.alpha,
            "divider": self.cpu.divider,
            "f1_flag": self.cpu.f1_flag,
            "f4_flag": self.cpu.f4_flag,
            "melody_enabled": self.cpu.melody_enabled,
            "melody_step": self.cpu.melody_step,
            "buzzer_active": self.cpu.buzzer_active,
            "halted": self.cpu.halted,
            "cycles": self.cpu.cycles,
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        use serde::de::Error;

        self.cpu.acc = v["acc"].as_u64().unwrap_or(0) as u8;
        self.cpu.carry = v["carry"].as_bool().unwrap_or(false);
        self.cpu.pc = v["pc"].as_u64().unwrap_or(0) as u16;
        self.cpu.stack = v["stack"].as_u64().unwrap_or(0) as u16;
        self.cpu.bl = v["bl"].as_u64().unwrap_or(0) as u8;
        self.cpu.bm = v["bm"].as_u64().unwrap_or(0) as u8;
        self.cpu.sbm = v["sbm"].as_bool().unwrap_or(false);
        self.cpu.skip = v["skip"].as_bool().unwrap_or(false);

        if let Some(ram_str) = v["ram"].as_str() {
            let ram_data = BASE64
                .decode(ram_str)
                .map_err(|e| serde_json::Error::custom(format!("RAM decode error: {}", e)))?;
            let len = ram_data.len().min(128);
            self.cpu.ram[..len].copy_from_slice(&ram_data[..len]);
        }

        self.cpu.output_s = v["output_s"].as_u64().unwrap_or(0) as u8;
        self.cpu.output_r = v["output_r"].as_u64().unwrap_or(0) as u8;
        self.cpu.output_l = v["output_l"].as_u64().unwrap_or(0) as u8;
        self.cpu.output_x = v["output_x"].as_u64().unwrap_or(0) as u8;
        self.cpu.bp = v["bp"].as_u64().unwrap_or(0) as u8;
        self.cpu.alpha = v["alpha"].as_bool().unwrap_or(false);
        self.cpu.divider = v["divider"].as_u64().unwrap_or(0) as u16;
        self.cpu.f1_flag = v["f1_flag"].as_bool().unwrap_or(false);
        self.cpu.f4_flag = v["f4_flag"].as_bool().unwrap_or(false);
        self.cpu.melody_enabled = v["melody_enabled"].as_bool().unwrap_or(false);
        self.cpu.melody_step = v["melody_step"].as_u64().unwrap_or(0) as u8;
        self.cpu.buzzer_active = v["buzzer_active"].as_bool().unwrap_or(false);
        self.cpu.halted = v["halted"].as_bool().unwrap_or(false);
        self.cpu.cycles = v["cycles"].as_u64().unwrap_or(0);

        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Program".to_string(),
            name: "Program ROM".to_string(),
            extensions: vec!["gw".to_string(), "gnw".to_string(), "bin".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "Program" => {
                if data.len() > 4096 {
                    return Err(GameAndWatchError::RomTooLarge(data.len()));
                }
                self.cpu.load_rom(data);
                self.cpu.reset();
                self.rom_loaded = true;
                Ok(())
            }
            _ => Err(GameAndWatchError::UnknownMountPoint(
                mount_point_id.to_string(),
            )),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "Program" => {
                self.cpu.rom = vec![0; 4096];
                self.rom_loaded = false;
                Ok(())
            }
            _ => Err(GameAndWatchError::UnknownMountPoint(
                mount_point_id.to_string(),
            )),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "Program" => self.rom_loaded,
            _ => false,
        }
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }

    fn get_total_cycles(&self) -> u64 {
        self.cpu.cycles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_system() {
        let sys = GameAndWatchSystem::new();
        assert!(!sys.rom_loaded);
        assert_eq!(sys.frame.width, DISPLAY_WIDTH);
        assert_eq!(sys.frame.height, DISPLAY_HEIGHT);
    }

    #[test]
    fn test_mount_rom() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0x25, 0x80]; // LAX 5, T $00 (loop)
        assert!(sys.mount("Program", &rom).is_ok());
        assert!(sys.rom_loaded);
        assert!(sys.is_mounted("Program"));
    }

    #[test]
    fn test_mount_oversized_rom() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0u8; 5000];
        assert!(sys.mount("Program", &rom).is_err());
    }

    #[test]
    fn test_unmount() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0x25, 0x80];
        sys.mount("Program", &rom).unwrap();
        assert!(sys.is_mounted("Program"));
        sys.unmount("Program").unwrap();
        assert!(!sys.is_mounted("Program"));
    }

    #[test]
    fn test_step_frame_no_rom() {
        let mut sys = GameAndWatchSystem::new();
        let frame = sys.step_frame().unwrap();
        assert_eq!(frame.width, DISPLAY_WIDTH);
        assert_eq!(frame.height, DISPLAY_HEIGHT);
    }

    #[test]
    fn test_step_frame_with_rom() {
        let mut sys = GameAndWatchSystem::new();
        // Simple ROM: LAX 5, T $00 (loop forever with ACC=5)
        let rom = vec![0x25, 0x80];
        sys.mount("Program", &rom).unwrap();
        let frame = sys.step_frame().unwrap();
        assert_eq!(frame.width, DISPLAY_WIDTH);
        assert_eq!(
            frame.pixels.len(),
            (DISPLAY_WIDTH * DISPLAY_HEIGHT) as usize
        );
    }

    #[test]
    fn test_save_load_state() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0x25, 0x80];
        sys.mount("Program", &rom).unwrap();
        sys.step_frame().unwrap();

        let state = sys.save_state();
        let state_str = serde_json::to_string(&state).unwrap();
        let state_val: Value = serde_json::from_str(&state_str).unwrap();

        let mut sys2 = GameAndWatchSystem::new();
        sys2.mount("Program", &rom).unwrap();
        assert!(sys2.load_state(&state_val).is_ok());
        assert_eq!(sys2.cpu.cycles, sys.cpu.cycles);
    }

    #[test]
    fn test_controller_input() {
        let mut sys = GameAndWatchSystem::new();
        sys.set_controller(0x0005); // Left + Up
        assert_eq!(sys.cpu.controller_state, 0x0005);
    }

    #[test]
    fn test_mount_points() {
        let sys = GameAndWatchSystem::new();
        let mps = sys.mount_points();
        assert_eq!(mps.len(), 1);
        assert_eq!(mps[0].id, "Program");
        assert!(mps[0].required);
    }

    #[test]
    fn test_supports_save_states() {
        let sys = GameAndWatchSystem::new();
        assert!(sys.supports_save_states());
    }

    #[test]
    fn test_reset_preserves_rom() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0x25, 0x33, 0x80];
        sys.mount("Program", &rom).unwrap();
        sys.step_frame().unwrap();

        sys.reset();
        assert!(sys.rom_loaded);
        assert_eq!(sys.cpu.rom[0], 0x25);
        assert_eq!(sys.cpu.pc, 0);
        assert_eq!(sys.cpu.cycles, 0);
    }

    #[test]
    fn test_debugger_available() {
        let sys = GameAndWatchSystem::new();
        assert!(sys.debugger().is_some());
    }

    #[test]
    fn test_audio_silence() {
        let mut sys = GameAndWatchSystem::new();
        let samples = sys.generate_audio_samples(100);
        assert_eq!(samples.len(), 100);
        assert!(samples.iter().all(|&s| s == 0)); // Silent when buzzer off
    }

    #[test]
    fn test_audio_buzzer() {
        let mut sys = GameAndWatchSystem::new();
        sys.cpu.buzzer_active = true;
        let samples = sys.generate_audio_samples(100);
        assert_eq!(samples.len(), 100);
        assert!(samples.iter().any(|&s| s != 0)); // Not silent when buzzer active
    }

    #[test]
    fn test_lcd_rendering() {
        let mut sys = GameAndWatchSystem::new();
        // Set some RAM bits to verify rendering doesn't panic
        sys.cpu.ram[0] = 0xF;
        sys.cpu.ram[16] = 0xA;
        sys.cpu.ram[127] = 0x5;
        sys.render_lcd();

        // Verify frame has non-uniform data (segments rendered)
        let unique: std::collections::HashSet<u32> = sys.frame.pixels.iter().copied().collect();
        assert!(unique.len() > 1); // At least background + segments
    }
}
