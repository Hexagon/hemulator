//! Nintendo Game & Watch emulator system.
//!
//! Emulates the Sharp SM510 4-bit microcontroller used in Game & Watch handhelds.
//! The SM510 runs at 32.768 kHz and drives a segment-based LCD display.
//!
//! # ROM Formats
//!
//! - **`.mgw`** (preferred): Compressed container with CPU ROM, LCD segment artwork,
//!   background image, and keyboard input mapping. From LCD-Game-Shrinker / gw-libretro.
//! - **`.gw`, `.gnw`, `.bin`**: Raw SM510 program ROM binary (1–4 KB).
//!   Without artwork, segments are rendered as a diagnostic grid.
//!
//! # Display
//!
//! With `.mgw` ROMs: 320×240 LCD artwork with composited segment overlays.
//! With raw ROMs: fallback segment grid showing RAM nibble states.

mod debugger;
pub mod mgw;
pub mod sm510;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use mgw::{is_mgw_format, parse_mgw, MgwRom, MGW_SCREEN_HEIGHT, MGW_SCREEN_WIDTH};
use serde_json::Value;
use thiserror::Error;

/// Display width in pixels (matches .mgw native resolution)
const DISPLAY_WIDTH: u32 = MGW_SCREEN_WIDTH;
/// Display height in pixels
const DISPLAY_HEIGHT: u32 = MGW_SCREEN_HEIGHT;

/// SM510 clock frequency (Hz)
const CPU_CLOCK_HZ: u32 = 32_768;
/// Target frame rate
const TARGET_FPS: u32 = 64;
/// CPU cycles per frame
const CYCLES_PER_FRAME: u32 = CPU_CLOCK_HZ / TARGET_FPS; // 512

// LCD color palette (Game Boy-inspired green tones) — used for fallback grid rendering
/// LCD background color (light green)
const LCD_BG: u32 = 0xFF9BBC0F;
/// Active segment color (dark green)
const LCD_ON: u32 = 0xFF0F380F;
/// Inactive segment color (slightly darker than background)
const LCD_OFF: u32 = 0xFF8BAC0F;

/// Segment "on" color for artwork compositing (dark LCD segment)
const SEGMENT_ON_COLOR: u32 = 0xFF1A1A1A;
/// Segment "on" color for inverted LCD (bright segment on dark background)
const SEGMENT_ON_COLOR_INVERTED: u32 = 0xFFE0E0E0;

/// Maximum raw ROM size (no .mgw container)
const MAX_RAW_ROM_SIZE: usize = 4096;

/// Number of display RAM locations (BM 0-3, BL 0-15 = 64 addresses)
const DISPLAY_RAM_SIZE: usize = 64;

#[derive(Error, Debug)]
pub enum GameAndWatchError {
    #[error(
        "ROM too large (max 4 KB for raw ROMs, got {0} bytes). Use .mgw format for larger files."
    )]
    RomTooLarge(usize),

    #[error("Failed to parse .mgw ROM: {0}")]
    MgwParseFailed(String),

    #[error("No ROM loaded")]
    NoRomLoaded,

    #[error("Unknown mount point: {0}")]
    UnknownMountPoint(String),
}

/// Composite a single LCD segment's artwork onto a framebuffer.
///
/// Uses the segment's grayscale pixel data as an alpha mask to blend
/// `seg_color` over the existing background pixels.
fn composite_segment(
    pixels: &mut [u32],
    segment: &mgw::MgwSegment,
    seg_color: u32,
    fb_width: usize,
    fb_height: usize,
) {
    let sx = segment.x as usize;
    let sy = segment.y as usize;
    let sw = segment.width as usize;
    let sh = segment.height as usize;

    let seg_r = (seg_color >> 16) & 0xFF;
    let seg_g = (seg_color >> 8) & 0xFF;
    let seg_b = seg_color & 0xFF;

    for py in 0..sh {
        let screen_y = sy + py;
        if screen_y >= fb_height {
            break;
        }
        for px in 0..sw {
            let screen_x = sx + px;
            if screen_x >= fb_width {
                break;
            }

            let pixel_idx = py * sw + px;
            if pixel_idx >= segment.pixels.len() {
                continue;
            }

            let alpha = segment.pixels[pixel_idx] as u32;
            if alpha == 0 {
                continue; // Fully transparent
            }

            let fb_idx = screen_y * fb_width + screen_x;
            let bg = pixels[fb_idx];
            let bg_r = (bg >> 16) & 0xFF;
            let bg_g = (bg >> 8) & 0xFF;
            let bg_b = bg & 0xFF;

            // Alpha blend: out = seg * alpha + bg * (255 - alpha)
            let inv_alpha = 255 - alpha;
            let r = (seg_r * alpha + bg_r * inv_alpha) / 255;
            let g = (seg_g * alpha + bg_g * inv_alpha) / 255;
            let b = (seg_b * alpha + bg_b * inv_alpha) / 255;

            pixels[fb_idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }
    }
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
    /// Parsed .mgw ROM data (artwork, keyboard mapping, etc.)
    mgw_data: Option<MgwRom>,
}

impl GameAndWatchSystem {
    /// Create a new Game & Watch system
    pub fn new() -> Self {
        Self {
            cpu: sm510::Sm510::new(),
            rom_loaded: false,
            frame: Frame::new(DISPLAY_WIDTH, DISPLAY_HEIGHT),
            audio_phase: 0.0,
            mgw_data: None,
        }
    }

    /// Set controller button state.
    ///
    /// When an .mgw ROM is loaded, uses the .mgw button encoding:
    /// - Bit 0: Left
    /// - Bit 1: Up
    /// - Bit 2: Right
    /// - Bit 3: Down
    /// - Bit 4: A (Game A)
    /// - Bit 5: B (Game B)
    /// - Bit 6: Time
    /// - Bit 7: Game (Alarm/ACL)
    ///
    /// When a raw ROM is loaded, uses the legacy encoding:
    /// - Bit 0: Left
    /// - Bit 1: Right
    /// - Bit 2: Up
    /// - Bit 3: Down
    /// - Bit 4: Game A (Z)
    /// - Bit 5: Game B (X)
    /// - Bit 6: Time (T)
    /// - Bit 7: Alarm (Enter)
    /// - Bit 8: ACL (R)
    pub fn set_controller(&mut self, state: u16) {
        if self.mgw_data.is_some() {
            // .mgw mode: store button state for keyboard-mapped input
            self.cpu.pressed_buttons = (state & 0xFF) as u8;
        } else {
            // Raw ROM mode: use legacy controller state
            self.cpu.controller_state = state;
        }
    }

    /// Render LCD segments from RAM into the frame buffer.
    ///
    /// If .mgw artwork is available, composites active segments over the background.
    /// Otherwise renders a diagnostic grid showing raw RAM nibble states.
    fn render_lcd(&mut self) {
        if self.mgw_data.is_some() {
            self.render_lcd_artwork();
        } else {
            self.render_lcd_grid();
        }
    }

    /// Render LCD using .mgw artwork overlays.
    fn render_lcd_artwork(&mut self) {
        let mgw = match &self.mgw_data {
            Some(d) => d,
            None => return,
        };

        let pixels = &mut self.frame.pixels;
        let w = DISPLAY_WIDTH as usize;
        let h = DISPLAY_HEIGHT as usize;

        // Step 1: Draw background
        if mgw.background.len() == w * h {
            pixels[..w * h].copy_from_slice(&mgw.background[..w * h]);
        } else {
            // No background image — fill with neutral color
            let bg_color = if mgw.lcd_inverted {
                0xFF1A1A1A // Dark background for inverted LCD
            } else {
                0xFFD4C8A0 // Warm beige (approximate G&W faceplate color)
            };
            for pixel in pixels.iter_mut() {
                *pixel = bg_color;
            }
        }

        // Step 2: Composite active segments
        let seg_color = if mgw.lcd_inverted {
            SEGMENT_ON_COLOR_INVERTED
        } else {
            SEGMENT_ON_COLOR
        };

        // Check each display RAM address (BM 0-3, BL 0-15 = 64 locations)
        for addr in 0..DISPLAY_RAM_SIZE {
            let nibble = self.cpu.ram[addr] & 0xF;
            if nibble == 0 {
                continue; // No segments active at this address
            }

            for bit in 0..4u8 {
                if nibble & (1 << bit) == 0 {
                    continue; // This segment is off
                }

                let seg_idx = addr * 4 + bit as usize;
                if seg_idx >= mgw.segments.len() {
                    continue;
                }

                if let Some(ref segment) = mgw.segments[seg_idx] {
                    // Composite segment artwork onto the framebuffer
                    composite_segment(pixels, segment, seg_color, w, h);
                }
            }
        }
    }

    /// Render LCD as a diagnostic grid (fallback for raw ROMs without artwork).
    fn render_lcd_grid(&mut self) {
        let pixels = &mut self.frame.pixels;
        let w = DISPLAY_WIDTH;
        let h = DISPLAY_HEIGHT;

        // Fill background
        for pixel in pixels.iter_mut() {
            *pixel = LCD_BG;
        }

        // Grid layout scaled for 320×240: 16 columns × 8 rows
        let grid_x_start = 16u32;
        let grid_y_start = 12u32;
        let cell_w = 18u32; // Cell width (4 bars × 4px + 2px gaps)
        let cell_h = 26u32; // Cell height
        let bar_w = 4u32;
        let bar_h = 22u32;
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
                    let by = cy + 2;

                    for py in by..by + bar_h {
                        for px in bx..bx + bar_w {
                            if px < w && py < h {
                                pixels[(py * w + px) as usize] = color;
                            }
                        }
                    }
                }
            }
        }

        // Draw border
        let border_color = 0xFF306230u32;
        let gw = 16 * cell_w + 4;
        let gh = 8 * cell_h + 4;
        let bx0 = grid_x_start.saturating_sub(2);
        let by0 = grid_y_start.saturating_sub(2);

        for x in bx0..bx0 + gw {
            if x < w {
                if by0 < h {
                    pixels[(by0 * w + x) as usize] = border_color;
                }
                let bottom = by0 + gh - 1;
                if bottom < h {
                    pixels[(bottom * w + x) as usize] = border_color;
                }
            }
        }
        for y in by0..by0 + gh {
            if y < h {
                if bx0 < w {
                    pixels[(y * w + bx0) as usize] = border_color;
                }
                let right = bx0 + gw - 1;
                if right < w {
                    pixels[(y * w + right) as usize] = border_color;
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
        let mut info = vec![
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
        ];

        // Show .mgw info if loaded
        if let Some(ref mgw) = self.mgw_data {
            info.push(("ROM".to_string(), "MGW".to_string()));
            info.push(("CPU".to_string(), mgw.cpu_type.clone()));
            info.push((
                "Segments".to_string(),
                format!("{}", mgw.segments.iter().filter(|s| s.is_some()).count()),
            ));
        }

        info
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
        let keyboard_backup = self.cpu.keyboard_mapping;
        self.cpu.reset();
        if had_rom {
            self.cpu.rom = rom_backup;
            self.cpu.keyboard_mapping = keyboard_backup;
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
            extensions: vec![
                "mgw".to_string(),
                "gw".to_string(),
                "gnw".to_string(),
                "bin".to_string(),
            ],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "Program" => {
                if is_mgw_format(data) {
                    // Parse .mgw container
                    let mgw_rom = parse_mgw(data)
                        .map_err(|e| GameAndWatchError::MgwParseFailed(e.to_string()))?;

                    // Load extracted CPU program ROM
                    self.cpu.load_rom(&mgw_rom.program);

                    // Install keyboard mapping
                    self.cpu.keyboard_mapping = Some(mgw_rom.keyboard);

                    // Store artwork data for rendering
                    self.mgw_data = Some(mgw_rom);

                    self.cpu.reset();
                    self.rom_loaded = true;
                } else {
                    // Raw ROM mode
                    if data.len() > MAX_RAW_ROM_SIZE {
                        return Err(GameAndWatchError::RomTooLarge(data.len()));
                    }
                    self.cpu.load_rom(data);
                    self.cpu.keyboard_mapping = None;
                    self.mgw_data = None;
                    self.cpu.reset();
                    self.rom_loaded = true;
                }
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
                self.cpu.keyboard_mapping = None;
                self.mgw_data = None;
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
    fn test_mount_raw_rom() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0x25, 0x80]; // LAX 5, T $00 (loop)
        assert!(sys.mount("Program", &rom).is_ok());
        assert!(sys.rom_loaded);
        assert!(sys.is_mounted("Program"));
        assert!(sys.mgw_data.is_none()); // No artwork
    }

    #[test]
    fn test_mount_oversized_raw_rom() {
        let mut sys = GameAndWatchSystem::new();
        let rom = vec![0u8; 5000]; // Too big for raw, not .mgw format
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
        assert!(sys.mgw_data.is_none());
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
        sys.set_controller(0x0005); // Left + Up (in raw mode)
        assert_eq!(sys.cpu.controller_state, 0x0005);
    }

    #[test]
    fn test_mount_points() {
        let sys = GameAndWatchSystem::new();
        let mps = sys.mount_points();
        assert_eq!(mps.len(), 1);
        assert_eq!(mps[0].id, "Program");
        assert!(mps[0].required);
        assert!(mps[0].extensions.contains(&"mgw".to_string()));
        assert!(mps[0].extensions.contains(&"gw".to_string()));
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
    fn test_lcd_grid_rendering() {
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

    #[test]
    fn test_mgw_format_detection() {
        // Test that .mgw-sized data without magic is treated as raw
        let small_raw = vec![0u8; 100];
        assert!(!is_mgw_format(&small_raw));

        // LZ4 magic
        assert!(is_mgw_format(&[0x04, 0x22, 0x4D, 0x18, 0x00]));

        // SM5 prefix
        assert!(is_mgw_format(b"SM510\0\0\0more"));
    }

    #[test]
    fn test_mgw_mount_sets_keyboard() {
        // Build a minimal valid uncompressed .mgw in memory
        let mut data = vec![0u8; 512];
        data[0..5].copy_from_slice(b"SM510");
        data[8..16].copy_from_slice(b"test_rom");

        // Program section at offset 108
        let sec_base = 0x1C;
        let prg_start: u32 = 108;
        let prg_size: u32 = 2;
        data[sec_base + 64..sec_base + 68].copy_from_slice(&prg_start.to_le_bytes());
        data[sec_base + 68..sec_base + 72].copy_from_slice(&prg_size.to_le_bytes());

        // Keyboard at offset 110
        let kbd_start = prg_start + prg_size;
        let kbd_size: u32 = 40;
        data[sec_base + 72..sec_base + 76].copy_from_slice(&kbd_start.to_le_bytes());
        data[sec_base + 76..sec_base + 80].copy_from_slice(&kbd_size.to_le_bytes());

        // Write keyboard[0] = 0x10 (A button → S1/K1)
        let kbd_off = kbd_start as usize;
        data[kbd_off..kbd_off + 4].copy_from_slice(&0x10u32.to_le_bytes());

        data[108] = 0x00;
        data[109] = 0x00;

        let mut sys = GameAndWatchSystem::new();
        assert!(sys.mount("Program", &data).is_ok());
        assert!(sys.rom_loaded);
        assert!(sys.mgw_data.is_some());
        assert!(sys.cpu.keyboard_mapping.is_some());
        assert_eq!(sys.cpu.keyboard_mapping.unwrap()[0], 0x10);
    }
}
