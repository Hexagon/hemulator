//! Sega Master System Video Display Processor (VDP)
//!
//! The VDP is based on the Texas Instruments TMS9918A and handles all video output.
//!
//! # Features
//! - 256×192 pixel resolution
//! - 64 color palette (32 simultaneous)
//! - Tilemap-based background rendering
//! - 64 sprites with 8 per scanline limit
//! - Scrolling support
//! - Line and frame interrupts

use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::renderer::Renderer;
use emu_core::types::Frame;

// Rendering metadata constants
// During rendering, we use the upper 8 bits of the pixel value to store metadata
// that is cleaned up before the frame is presented to the user
const PRIORITY_BIT: u32 = 0x01000000; // Bit 24: Background tile has priority over sprites
const RGB_MASK: u32 = 0x00FFFFFF; // Lower 24 bits: RGB color value (0xRRGGBB)

/// VDP state and rendering
pub struct Vdp {
    // Video RAM (16KB)
    vram: [u8; 0x4000],

    // Color RAM (64 bytes: 32 for SMS, 64 for Game Gear)
    cram: [u8; 0x40],
    /// Number of CRAM bytes used (32 for SMS, 64 for GG)
    cram_size: usize,

    // VDP registers (11 registers)
    registers: [u8; 11],

    // Internal state
    address_register: u16,
    code_register: u8,
    read_buffer: u8,
    write_latch: bool,

    // Game Gear CRAM write latch (GG CRAM is 12-bit, written as two bytes)
    cram_latch: u8,

    // Rendering — always renders to 256×192 internally
    frame: Frame,

    // Game Gear cropped frame (160×144) — updated from `frame` when get_frame() is called
    gg_frame: Frame,

    /// True if this VDP is in Game Gear mode
    is_game_gear: bool,

    // Interrupts
    frame_interrupt_pending: bool,
    line_interrupt_pending: bool,
    line_counter: u8,

    // Sprite flags
    sprite_overflow: bool,
    sprite_collision: bool,

    // Current scanline
    scanline: u16,

    // Timing mode (PAL vs NTSC)
    is_pal: bool,

    // True once set_scanline() has crossed the display-height boundary this frame.
    // Used by the retroactive frame-interrupt logic so that an early R1 write
    // (before any set_scanline call) at the sentinel value 262 does not falsely
    // trigger an interrupt before VBlank has actually been reached.
    in_vblank: bool,

    // H-counter latch value (latched by TH pin transition on controller port 2)
    h_counter_latch: u8,
}

impl Vdp {
    /// Create a new VDP
    pub fn new() -> Self {
        Self {
            vram: [0; 0x4000],
            cram: [0; 0x40],
            cram_size: 0x20, // SMS default
            registers: [0; 11],
            address_register: 0,
            code_register: 0,
            read_buffer: 0,
            write_latch: false,
            cram_latch: 0,
            frame: Frame::new(256, 192),
            gg_frame: Frame::new(160, 144),
            is_game_gear: false,
            frame_interrupt_pending: false,
            line_interrupt_pending: false,
            line_counter: 0,
            sprite_overflow: false,
            sprite_collision: false,
            scanline: 262, // Start at end of frame so first set_scanline(0) wraps around
            is_pal: false,
            in_vblank: false,
            h_counter_latch: 0,
        }
    }

    /// Create a new VDP in Game Gear mode
    pub fn new_game_gear() -> Self {
        Self {
            vram: [0; 0x4000],
            cram: [0; 0x40],
            cram_size: 0x40, // GG uses 64 bytes
            registers: [0; 11],
            address_register: 0,
            code_register: 0,
            read_buffer: 0,
            write_latch: false,
            cram_latch: 0,
            frame: Frame::new(256, 192),    // Internal render buffer
            gg_frame: Frame::new(160, 144), // GG LCD viewport
            is_game_gear: true,
            frame_interrupt_pending: false,
            line_interrupt_pending: false,
            line_counter: 0,
            sprite_overflow: false,
            sprite_collision: false,
            scanline: 262,
            is_pal: false,
            in_vblank: false,
            h_counter_latch: 0,
        }
    }

    /// Set PAL/NTSC timing mode
    pub fn set_pal(&mut self, pal: bool) {
        self.is_pal = pal;
    }

    /// Check if this VDP is in Game Gear mode
    #[allow(dead_code)]
    pub fn is_game_gear(&self) -> bool {
        self.is_game_gear
    }

    /// Get current PAL/NTSC timing mode (used in tests)
    #[cfg(test)]
    pub fn get_pal(&self) -> bool {
        self.is_pal
    }

    /// Get the current video mode
    /// Returns (mode_4_enabled, tms_mode)
    /// TMS modes: 0 = Graphics I, 1 = Text, 2 = Graphics II, 3 = Multicolor
    fn get_video_mode(&self) -> (bool, u8) {
        let m4 = (self.registers[0] & 0x04) != 0; // Register 0, bit 2

        if m4 {
            return (true, 0); // Mode 4 (SMS native mode)
        }

        // TMS9918A mode detection (when M4=0)
        // M1 = Register 1, bit 4
        // M2 = Register 1, bit 3
        // M3 = Register 0, bit 1
        let m1 = (self.registers[1] >> 4) & 1;
        let m2 = (self.registers[1] >> 3) & 1;
        let m3 = (self.registers[0] >> 1) & 1;

        let tms_mode = match (m3, m2, m1) {
            (0, 0, 0) => 0, // Graphics I
            (0, 1, 0) => 1, // Text
            (1, 0, 0) => 2, // Graphics II
            (0, 0, 1) => 3, // Multicolor
            _ => 0,         // Default to Graphics I for invalid combinations
        };

        (false, tms_mode)
    }

    /// Get the active display height based on current mode
    fn get_display_height(&self) -> u32 {
        let m4 = (self.registers[0] & 0x04) != 0; // M4 (Mode 4 enable)

        if m4 {
            // Mode 4 (SMS): Check M2, M1, M3 for resolution
            let m2 = (self.registers[0] & 0x02) != 0; // Register 0, bit 1
            let m1 = (self.registers[1] & 0x10) != 0; // Register 1, bit 4
            let m3 = (self.registers[1] & 0x08) != 0; // Register 1, bit 3

            if m2 {
                // M2=1 allows M1 and M3 to select height
                if m3 && m1 {
                    192 // Both M1 and M3 set: default 192
                } else if m3 {
                    240 // M3 set: 240-line mode
                } else if m1 {
                    224 // M1 set: 224-line mode
                } else {
                    192 // Neither set: 192 lines
                }
            } else {
                192 // M2=0: standard 192 lines
            }
        } else {
            // TMS modes always use 192 lines
            192
        }
    }

    /// Update frame buffer size based on current display mode
    fn update_frame_size(&mut self) {
        let new_height = self.get_display_height();
        if self.frame.height != new_height {
            log(LogCategory::PPU, LogLevel::Info, || {
                format!("SMS VDP: Resizing frame buffer to 256x{}", new_height)
            });
            self.frame = Frame::new(256, new_height);
        }
    }

    /// Write to VDP control port (0xBF)
    pub fn write_control(&mut self, data: u8) {
        if !self.write_latch {
            // First byte - lower 8 bits of address
            self.address_register = (self.address_register & 0x3F00) | data as u16;
            self.write_latch = true;
        } else {
            // Second byte - upper 6 bits of address + code
            self.address_register = (self.address_register & 0x00FF) | ((data as u16 & 0x3F) << 8);
            self.code_register = (data >> 6) & 0x03;
            self.write_latch = false;

            // Handle different code modes
            match self.code_register {
                0x00 => {
                    // VRAM read - perform read-ahead into buffer
                    // This is crucial: the first byte at the address is loaded immediately
                    self.read_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
                    self.address_register = self.address_register.wrapping_add(1);
                }
                0x02 => {
                    // Register write
                    let reg = data & 0x0F;
                    if (reg as usize) < self.registers.len() {
                        let value = (self.address_register & 0xFF) as u8;
                        let old_value = self.registers[reg as usize];
                        self.registers[reg as usize] = value;
                        log(LogCategory::PPU, LogLevel::Info, || {
                            format!("SMS VDP: Register R{} = ${:02X}", reg, value)
                        });

                        // Check if this register write affects display height
                        // Registers 0 and 1 contain mode bits
                        if reg == 0 || reg == 1 {
                            self.update_frame_size();
                        }

                        // If the frame-interrupt-enable bit (R1 bit 5) transitions 0→1
                        // while we are already in VBlank, fire the interrupt retroactively.
                        // Use the in_vblank latch (set by set_scanline) rather than the raw
                        // scanline value, so that the startup sentinel (262) does not cause a
                        // spurious interrupt before any real VBlank has been reached.
                        if reg == 1
                            && (old_value & 0x20) == 0
                            && (value & 0x20) != 0
                            && self.in_vblank
                        {
                            self.frame_interrupt_pending = true;
                        }
                    }
                }
                _ => {
                    // 0x01 = VRAM write mode, 0x03 = CRAM write mode
                    // No immediate action needed, address is set for subsequent data writes
                }
            }
        }
    }

    /// Write to VDP data port (0xBE)
    pub fn write_data(&mut self, data: u8) {
        self.write_latch = false;
        self.read_buffer = data;

        match self.code_register {
            0x03 => {
                // CRAM write
                if self.is_game_gear {
                    // Game Gear: 12-bit colors, 2 bytes per entry
                    // Even address: latch the low byte
                    // Odd address: write both bytes
                    let addr = self.address_register & 0x3F;
                    if (addr & 1) == 0 {
                        // Even: latch low byte
                        self.cram_latch = data;
                    } else {
                        // Odd: write latched low byte + this high byte
                        let base = (addr & 0x3E) as usize;
                        self.cram[base] = self.cram_latch;
                        self.cram[base + 1] = data;
                    }
                } else {
                    // SMS: 6-bit colors, 1 byte per entry
                    let addr = (self.address_register & 0x1F) as usize;
                    self.cram[addr] = data;
                    if addr == 16 {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            format!("SMS VDP: Backdrop color = ${:02X}", data)
                        });
                    }
                }
            }
            _ => {
                // VRAM write
                self.vram[(self.address_register & 0x3FFF) as usize] = data;
            }
        }

        self.address_register = self.address_register.wrapping_add(1);
    }

    /// Read from VDP data port (0xBE)
    pub fn read_data(&mut self) -> u8 {
        self.write_latch = false;
        let value = self.read_buffer;
        self.read_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
        self.address_register = self.address_register.wrapping_add(1);
        value
    }

    /// Read from VDP status port (0xBF)
    pub fn read_status(&mut self) -> u8 {
        self.write_latch = false;
        let mut status = 0;

        // Bit 7: Frame interrupt pending
        if self.frame_interrupt_pending {
            status |= 0x80;
        }

        // Bit 6: Sprite overflow
        if self.sprite_overflow {
            status |= 0x40;
        }

        // Bit 5: Sprite collision
        if self.sprite_collision {
            status |= 0x20;
        }

        // Clear frame interrupt flag on read
        self.frame_interrupt_pending = false;

        // Clear line interrupt flag on read — on real hardware, reading the
        // status register de-asserts /INT completely, clearing both frame and
        // line pending latches.  Without this, a stale line_interrupt_pending
        // causes an interrupt storm (Sonic 2 black screen).
        self.line_interrupt_pending = false;

        // Clear sprite flags on read
        self.sprite_overflow = false;
        self.sprite_collision = false;

        status
    }

    /// Read vertical counter
    pub fn read_vcounter(&self) -> u8 {
        // The SMS V-counter mapping depends on both region and display height.
        // The V-counter counts up linearly through the active display and into
        // the blanking area, then "jumps" to skip some values:
        //
        // NTSC 192-line: 0x00..=0xDA, then 0xD5..=0xFF (jump at 219)
        // NTSC 224-line: 0x00..=0xEA, then 0xE5..=0xFF (jump at 235)
        // NTSC 240-line: 0x00..=0xFF, then 0x00..=0x05 (no jump, wraps)
        // PAL  192-line: 0x00..=0xF2, then 0xBA..=0xFF (jump at 243)
        // PAL  224-line: 0x00..=0xFF, then 0x00..=0x02, then 0xCA..=0xFF (jump at ~259)
        // PAL  240-line: 0x00..=0xFF, then 0x00..=0x0A, then 0xD2..=0xFF (jump at ~267)
        let display_height = self.get_display_height();
        let vcounter = if self.is_pal {
            match display_height {
                224 => {
                    if self.scanline <= 0xFF {
                        self.scanline as u8
                    } else if self.scanline <= 0x102 {
                        (self.scanline - 0x100) as u8
                    } else {
                        (self.scanline - 0x39) as u8 // 0xCA..0xFF
                    }
                }
                240 => {
                    if self.scanline <= 0xFF {
                        self.scanline as u8
                    } else if self.scanline <= 0x10A {
                        (self.scanline - 0x100) as u8
                    } else {
                        (self.scanline - 0x39) as u8 // 0xD2..0xFF
                    }
                }
                _ => {
                    // 192-line PAL
                    if self.scanline <= 0xF2 {
                        self.scanline as u8
                    } else {
                        self.scanline.wrapping_sub(57) as u8
                    }
                }
            }
        } else {
            // NTSC
            match display_height {
                224 => {
                    if self.scanline <= 0xEA {
                        self.scanline as u8
                    } else {
                        self.scanline.wrapping_sub(6) as u8 // 0xE5..0xFF
                    }
                }
                240 => {
                    // 240-line NTSC: no jump, just wraps at 262
                    (self.scanline % 256) as u8
                }
                _ => {
                    // 192-line NTSC
                    if self.scanline <= 0xDA {
                        self.scanline as u8
                    } else {
                        self.scanline.wrapping_sub(6) as u8
                    }
                }
            }
        };
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "SMS VDP: V-counter read = ${:02X} (scanline={}, height={})",
                vcounter, self.scanline, display_height
            )
        });
        vcounter
    }

    /// Read horizontal counter.
    ///
    /// The SMS H-counter is latched by a TH-pin transition on controller
    /// port 2. Reads return the most recently latched value. The counter
    /// represents the horizontal pixel position divided by 2, ranging from
    /// 0x00 to 0x93 during active display and continuing through HBlank.
    pub fn read_hcounter(&self) -> u8 {
        self.h_counter_latch
    }

    /// Latch the current H-counter value.
    ///
    /// Called when the TH pin on controller port 2 transitions high-to-low,
    /// or can be called with the current cycle position within the scanline
    /// to provide a reasonable H-counter value.
    pub fn latch_h_counter(&mut self, cycle_in_scanline: u32) {
        // The SMS H-counter counts from 0x00 to 0x93 during the visible portion
        // of the scanline (256 pixels / 2 = 128 values = 0x00-0x7F) and continues
        // through HBlank (0x80-0x93, then 0xE9-0xFF wrapping around).
        // Total dots per line: 342 NTSC, mapped to ~228 cycles.
        // We approximate: visible area is ~171 dots (0-170), blanking after.
        let cycles_per_line = 228u32; // NTSC: ~228 Z80 cycles per scanline
        let clamped = cycle_in_scanline.min(cycles_per_line - 1);
        // Map cycle position to H-counter value (0x00-0x93 visible, 0xE9-0xFF blanking)
        let dot = (clamped * 342) / cycles_per_line;
        self.h_counter_latch = if dot < 256 {
            (dot / 2) as u8 // Active display: 0x00-0x7F
        } else if dot < 296 {
            (0x80 + (dot - 256) / 2) as u8 // Right border + HSync: 0x80-0x93
        } else {
            (0xE9 + (dot - 296) / 2) as u8 // Left border: 0xE9-0xFF
        };
    }

    /// Step VDP by one scanline
    #[allow(dead_code)]
    pub fn step_scanline(&mut self) {
        let display_height = self.get_display_height() as u16;

        if self.scanline < display_height {
            // Render visible scanline
            self.render_scanline(self.scanline as u8);
        } else if self.scanline == display_height {
            // Frame interrupt occurs at start of VBlank
            if (self.registers[1] & 0x20) != 0 {
                // Frame interrupt enable
                self.frame_interrupt_pending = true;
            }
        }

        self.scanline += 1;
        if self.scanline >= 262 {
            // NTSC: 262 scanlines per frame
            self.scanline = 0;
        }
    }

    /// Set current scanline (for cycle-accurate timing)
    pub fn set_scanline(&mut self, scanline: u16) {
        let old_scanline = self.scanline;

        // Only update if scanline has actually changed
        if scanline == old_scanline {
            return;
        }

        self.scanline = scanline;

        // Get the active display height
        let display_height = self.get_display_height() as u16;

        // Render any scanlines that were crossed
        if scanline < old_scanline {
            // Wrapped around to new frame
            // Render remaining scanlines from old_scanline to end of visible area
            for line in old_scanline..display_height {
                self.render_scanline(line as u8);
            }

            // The VBlank period reloads the line counter from R10 on every
            // scanline.  When we skip directly from end-of-frame to early
            // active display (the common case), we must ensure the counter
            // is reloaded as it would have been during VBlank.
            self.line_counter = self.registers[10];

            // Render scanlines from start of new frame up to and including current scanline
            for line in 0..=scanline.min(display_height - 1) {
                self.render_scanline(line as u8);
            }
            // Only trigger VBlank if we haven't already entered VBlank during
            // normal forward progression.  If in_vblank is true, VBlank was
            // already triggered at scanline 192 (and the ISR likely cleared the
            // flag already).  Re-setting frame_interrupt_pending here would cause
            // a double VBlank per frame — leading to flickering and game desync
            // (Sonic 2 random restarts).
            //
            // If in_vblank is false (first frame from sentinel scanline, or a
            // mid-frame wrap), we DO need to trigger VBlank since it was missed.
            if !self.in_vblank {
                self.frame_interrupt_pending = true;
            }
            // Crop internal buffer to GG viewport on frame boundary
            if self.is_game_gear {
                self.crop_gg_viewport();
            }
            // New frame: reset the VBlank latch, then set it if we're already past the
            // active display area (e.g. set_scanline jumped straight to scanline 192+).
            self.in_vblank = scanline >= display_height;
            // Reload line counter if in VBlank area
            if scanline >= display_height {
                self.line_counter = self.registers[10];
            }
        } else {
            // Normal forward progress within same frame
            // Render all scanlines from old_scanline+1 up to and including scanline
            for line in (old_scanline + 1)..=scanline.min(display_height - 1) {
                self.render_scanline(line as u8);
            }
            // Check for frame interrupt when crossing into VBlank.
            // Status register bit 7 is ALWAYS set on VBlank entry,
            // regardless of IE0 (R1 bit 5).  IE0 only gates /INT.
            if old_scanline < display_height && scanline >= display_height {
                self.frame_interrupt_pending = true;
                self.in_vblank = true;
                // Crop internal buffer to GG viewport on VBlank entry
                if self.is_game_gear {
                    self.crop_gg_viewport();
                }
                // Reload line counter at start of VBlank (every VBlank scanline)
                self.line_counter = self.registers[10];
            }
            // Continue reloading line counter on every VBlank scanline
            if scanline >= display_height {
                self.line_counter = self.registers[10];
            }
        }
    }

    /// Check if frame interrupt is pending
    #[allow(dead_code)]
    pub fn frame_interrupt_pending(&self) -> bool {
        self.frame_interrupt_pending
    }

    /// Check if line interrupt is pending
    #[allow(dead_code)]
    pub fn line_interrupt_pending(&self) -> bool {
        self.line_interrupt_pending
    }

    /// Clear line interrupt
    #[allow(dead_code)]
    pub fn clear_line_interrupt(&mut self) {
        self.line_interrupt_pending = false;
    }

    /// Check if the VDP /INT line is active (any enabled interrupt pending).
    /// On real SMS hardware, /INT = (frame_pending AND IE0) OR (line_pending AND IE1).
    pub fn irq_line_active(&self) -> bool {
        let frame_irq = self.frame_interrupt_pending && (self.registers[1] & 0x20) != 0;
        let line_irq = self.line_interrupt_pending && (self.registers[0] & 0x10) != 0;
        frame_irq || line_irq
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> crate::system::TileViewerData {
        // Convert CRAM colors to RGB
        let num_colors = 32;
        let mut palette = Vec::new();
        for i in 0..num_colors {
            palette.push(self.cram_color(i));
        }

        crate::system::TileViewerData {
            vram: self.vram.to_vec(),
            cram: self.cram[..self.cram_size].to_vec(),
            palette,
            registers: self.registers.to_vec(),
        }
    }

    /// Get VDP state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "vram": self.vram.to_vec(),
            "cram": self.cram[..self.cram_size].to_vec(),
            "cram_size": self.cram_size,
            "registers": self.registers.to_vec(),
            "address_register": self.address_register,
            "code_register": self.code_register,
            "read_buffer": self.read_buffer,
            "write_latch": self.write_latch,
            "cram_latch": self.cram_latch,
            "is_game_gear": self.is_game_gear,
            "frame_interrupt_pending": self.frame_interrupt_pending,
            "line_interrupt_pending": self.line_interrupt_pending,
            "line_counter": self.line_counter,
            "sprite_overflow": self.sprite_overflow,
            "sprite_collision": self.sprite_collision,
            "scanline": self.scanline,
            "is_pal": self.is_pal,
            "in_vblank": self.in_vblank,
        })
    }

    /// Set VDP state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
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

        // Load VRAM
        if let Some(vram) = state.get("vram").and_then(|v| v.as_array()) {
            for (i, val) in vram.iter().enumerate() {
                if i >= self.vram.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.vram[i] = byte as u8;
                }
            }
        }

        // Load CRAM
        if let Some(cram) = state.get("cram").and_then(|v| v.as_array()) {
            for (i, val) in cram.iter().enumerate() {
                if i >= self.cram.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.cram[i] = byte as u8;
                }
            }
        }

        // Load registers
        if let Some(registers) = state.get("registers").and_then(|v| v.as_array()) {
            for (i, val) in registers.iter().enumerate() {
                if i >= self.registers.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.registers[i] = byte as u8;
                }
            }
        }

        load_u16!(state, "address_register", self.address_register);
        load_u8!(state, "code_register", self.code_register);
        load_u8!(state, "read_buffer", self.read_buffer);
        load_bool!(state, "write_latch", self.write_latch);
        load_u8!(state, "cram_latch", self.cram_latch);
        load_bool!(state, "is_game_gear", self.is_game_gear);
        if let Some(cs) = state.get("cram_size").and_then(|v| v.as_u64()) {
            self.cram_size = cs as usize;
        }
        load_bool!(
            state,
            "frame_interrupt_pending",
            self.frame_interrupt_pending
        );
        load_bool!(state, "line_interrupt_pending", self.line_interrupt_pending);
        load_u8!(state, "line_counter", self.line_counter);
        load_bool!(state, "sprite_overflow", self.sprite_overflow);
        load_bool!(state, "sprite_collision", self.sprite_collision);
        load_u16!(state, "scanline", self.scanline);
        load_bool!(state, "is_pal", self.is_pal);
        load_bool!(state, "in_vblank", self.in_vblank);

        Ok(())
    }

    /// Render a single scanline
    fn render_scanline(&mut self, line: u8) {
        let display_height = self.get_display_height() as u8;

        // Handle line counter and line interrupts.
        // Per SMS Power VDP documentation: the counter is loaded from R10
        // during VBlank.  On each active-display scanline it is decremented.
        // When it reaches -1 (wraps to 0xFF), a line interrupt is asserted
        // and the counter is reloaded from R10.
        // The pending flag is ALWAYS set on counter underflow, regardless of
        // whether IE1 (register 0 bit 4) is set.  The enable bit only gates
        // whether /INT is actually asserted (checked in irq_line_active()).
        if line < display_height {
            self.line_counter = self.line_counter.wrapping_sub(1);
            if self.line_counter == 0xFF {
                // Counter underflowed → reload from R10 and latch line interrupt
                self.line_counter = self.registers[10];
                self.line_interrupt_pending = true;
            }
        }

        // Clear scanline to backdrop color (RGB only; alpha restored at end of scanline)
        let backdrop_color = self.backdrop_color() & RGB_MASK;
        let line_offset = (line as usize) * 256;
        for x in 0..256 {
            self.frame.pixels[line_offset + x] = backdrop_color;
        }

        // Register 1 bit 6 (0x40) is the screen enable bit for both Mode 4 and TMS modes:
        // - 1 = display active, 0 = display blanked
        // This matches SMSlib VDPFEATURE_SHOWDISPLAY (0x0140) which ORs bit 6 to enable display.
        let display_enabled = (self.registers[1] & 0x40) != 0;

        // Render background and sprites if display is enabled
        if display_enabled {
            let (mode_4, tms_mode) = self.get_video_mode();
            self.render_background(line, line_offset);
            if mode_4 {
                self.render_sprites(line, line_offset);
            } else if tms_mode != 1 {
                // TMS Text Mode (mode 1) does not support sprites
                self.render_tms_sprites(line, line_offset);
            }
        }

        // Register 0 bit 5: Left column blank - mask leftmost 8 pixels with backdrop color
        if (self.registers[0] & 0x20) != 0 {
            for x in 0..8 {
                self.frame.pixels[line_offset + x] = backdrop_color;
            }
        }

        // Restore alpha for all pixels in this scanline (priority bit and intermediate state cleared)
        for x in 0..256 {
            self.frame.pixels[line_offset + x] =
                (self.frame.pixels[line_offset + x] & RGB_MASK) | 0xFF00_0000;
        }
    }

    /// Render background layer for a scanline
    fn render_background(&mut self, line: u8, line_offset: usize) {
        // Check which mode we're in
        let (mode_4, tms_mode) = self.get_video_mode();

        if mode_4 {
            // SMS Mode 4 rendering (current implementation)
            self.render_mode4_background(line, line_offset);
        } else {
            // TMS9918A mode rendering
            match tms_mode {
                0 => self.render_tms_graphics1(line, line_offset),
                1 => self.render_tms_text(line, line_offset),
                2 => self.render_tms_graphics2(line, line_offset),
                3 => self.render_tms_multicolor(line, line_offset),
                _ => {} // Invalid mode, do nothing
            }
        }
    }

    /// Render Mode 4 (SMS native) background for a scanline
    fn render_mode4_background(&mut self, line: u8, line_offset: usize) {
        let display_height = self.get_display_height();

        // Name table address depends on display height mode.
        // 192-line: Register 2 bits 3-1 → base = (R2 & 0x0E) << 10
        // 224/240-line: Only bit 2 of Register 2 is used → base = (R2 & 0x0C) << 10
        //   (bit 1 is forced to 1 on SMS2, and the name table needs 2KB for 32 rows)
        let name_table_addr = if display_height > 192 {
            ((self.registers[2] as u16) & 0x0C) << 10
        } else {
            ((self.registers[2] as u16) & 0x0E) << 10
        };

        // Vertical scroll wrap height:
        // 192-line mode: 28 rows × 8 pixels = 224
        // 224/240-line mode: 32 rows × 8 pixels = 256
        let v_scroll_wrap: u16 = if display_height > 192 { 256 } else { 224 };

        // Get scroll values
        // Register 0 bit 6: Horizontal scroll inhibit for top 2 tile rows (lines 0-15)
        let scroll_x = if line < 16 && (self.registers[0] & 0x40) != 0 {
            0 // H-scroll lock for top 2 tile rows
        } else {
            self.registers[8]
        };
        let scroll_y = self.registers[9];

        // Register 0 bit 7: Vertical scroll inhibit for rightmost 8 columns (24-31)
        let vscroll_inhibit = (self.registers[0] & 0x80) != 0;

        for x in 0..256u16 {
            let adj_x = (x as u8).wrapping_sub(scroll_x);
            let tile_col = (adj_x >> 3) as u16;
            let pixel_x = (adj_x & 7) as u16;

            // Apply vertical scroll (inhibited for rightmost 8 screen columns if reg0 bit 7 set)
            let (tile_row, pixel_y) = if vscroll_inhibit && x >= 192 {
                // No vertical scroll for columns 24-31
                let y = line as u16;
                (y >> 3, y & 7)
            } else {
                let y = ((line as u16) + (scroll_y as u16)) % v_scroll_wrap;
                (y >> 3, y & 7)
            };

            // Read name table entry (2 bytes per tile)
            let name_addr = name_table_addr + (tile_row * 32 + tile_col) * 2;
            if name_addr >= 0x3FFE {
                continue;
            }

            let tile_data_low = self.vram[name_addr as usize];
            let tile_data_high = self.vram[(name_addr + 1) as usize];
            let tile_data = tile_data_low as u16 | ((tile_data_high as u16) << 8);

            // SMS Mode 4 name table format (16-bit):
            // Bits 0-8: Tile index (9 bits, 512 tiles max)
            // Bit 9: Horizontal flip
            // Bit 10: Vertical flip
            // Bit 11: Palette select (0 or 1)
            // Bit 12: Priority (1 = sprite behind bg, 0 = sprite in front)
            let tile_index = tile_data & 0x1FF;
            let h_flip = (tile_data >> 9) & 1;
            let v_flip = (tile_data >> 10) & 1;
            let palette = ((tile_data >> 11) & 1) as usize;
            let priority = (tile_data >> 12) & 1;

            // Calculate pixel position within tile
            let px = if h_flip != 0 { 7 - pixel_x } else { pixel_x };
            let py = if v_flip != 0 { 7 - pixel_y } else { pixel_y };

            // Read tile pattern (32 bytes per 8x8 tile, 4 bits per pixel)
            let tile_addr = tile_index * 32 + py * 4;
            if tile_addr >= 0x3FFC {
                continue;
            }

            let byte0 = self.vram[tile_addr as usize];
            let byte1 = self.vram[(tile_addr + 1) as usize];
            let byte2 = self.vram[(tile_addr + 2) as usize];
            let byte3 = self.vram[(tile_addr + 3) as usize];

            // Extract 4-bit pixel value
            let shift = 7 - px;
            let pixel = ((byte0 >> shift) & 1)
                | (((byte1 >> shift) & 1) << 1)
                | (((byte2 >> shift) & 1) << 2)
                | (((byte3 >> shift) & 1) << 3);

            // Pixel 0 is transparent
            if pixel != 0 {
                let color_index = palette * 16 + pixel as usize;
                // Strip alpha during rendering so PRIORITY_BIT (bit 24) is unambiguous
                let color = self.cram_color(color_index) & RGB_MASK;
                // Store the color and priority bit; alpha is restored at end of render_scanline
                let pixel_data = if priority != 0 {
                    color | PRIORITY_BIT // Bit 24 set = sprite renders behind this tile
                } else {
                    color
                };
                self.frame.pixels[line_offset + x as usize] = pixel_data;
            }
        }
    }

    /// Render sprites for a scanline
    fn render_sprites(&mut self, line: u8, line_offset: usize) {
        let sprite_attr_table = ((self.registers[5] as u16) & 0x7E) << 7;
        // Register 6 bit 2 selects sprite pattern generator base: 0=$0000, 1=$2000
        let sprite_pattern_base = ((self.registers[6] as u16) & 0x04) << 11;
        let tall_sprites = (self.registers[1] & 0x02) != 0;
        let zoomed = (self.registers[1] & 0x01) != 0;
        let base_height: u8 = if tall_sprites { 16 } else { 8 };
        // Zoom doubles the effective pixel size of sprites
        let effective_height: u8 = if zoomed {
            base_height.saturating_mul(2)
        } else {
            base_height
        };
        let display_height = self.get_display_height();

        let mut sprites_on_line = 0;

        // Track which pixels have sprites for collision detection
        let mut sprite_pixels = [false; 256];

        // First pass: scan forward to find visible sprites on this line (respects $D0 terminator)
        let mut visible_sprites: Vec<u16> = Vec::new();
        for i in 0..64u16 {
            let y = self.vram[(sprite_attr_table + i) as usize];

            // Check for end-of-sprite-list marker.
            // In 192-line mode, Y=$D0 (208) terminates the list because it is
            // beyond the visible area.  In 224/240-line modes those Y values are
            // valid sprite positions, so the terminator does not apply.
            if display_height == 192 && y == 0xD0 {
                break;
            }

            // Y position is offset by 1
            let y_pos = y.wrapping_add(1);
            let diff = line.wrapping_sub(y_pos);
            if diff >= effective_height {
                continue;
            }

            sprites_on_line += 1;
            if sprites_on_line > 8 {
                self.sprite_overflow = true;
                break;
            }

            visible_sprites.push(i);
        }

        // Second pass: render in reverse order so lower-numbered sprites have priority
        for &i in visible_sprites.iter().rev() {
            let y = self.vram[(sprite_attr_table + i) as usize];
            let y_pos = y.wrapping_add(1);

            // Get sprite X position and tile number
            let mut x_pos = self.vram[(sprite_attr_table + 128 + i * 2) as usize];

            // Early Clock (Register 0, bit 3): shift all sprites left 8 pixels.
            // Used by Sonic 1 for smooth left-edge scrolling.
            if (self.registers[0] & 0x08) != 0 {
                x_pos = x_pos.wrapping_sub(8);
            }
            let mut tile_num = self.vram[(sprite_attr_table + 128 + i * 2 + 1) as usize];

            // In 8x16 (tall) mode, bit 0 of tile number is forced to 0
            if tall_sprites {
                tile_num &= 0xFE;
            }

            // Calculate sprite row within the sprite.
            // When zoomed, each source pixel is doubled, so divide screen offset by 2
            // to get the actual tile-data row.
            let raw_y = line.wrapping_sub(y_pos);
            let sprite_y = if zoomed { raw_y / 2 } else { raw_y };

            // For 8x16 (tall) sprites in Mode 4:
            // - Uses 2 tiles vertically (tile N and tile N+1)
            // - Each tile is still 8 pixels wide
            let (actual_tile, actual_y) = if tall_sprites {
                if sprite_y < 8 {
                    (tile_num, sprite_y)
                } else {
                    (tile_num.wrapping_add(1), sprite_y - 8)
                }
            } else {
                (tile_num, sprite_y)
            };

            // Read tile pattern for this row (using sprite pattern generator base from Register 6)
            let tile_addr = sprite_pattern_base + (actual_tile as u16) * 32 + (actual_y as u16) * 4;
            if tile_addr >= 0x3FFC {
                continue;
            }

            let byte0 = self.vram[tile_addr as usize];
            let byte1 = self.vram[(tile_addr + 1) as usize];
            let byte2 = self.vram[(tile_addr + 2) as usize];
            let byte3 = self.vram[(tile_addr + 3) as usize];

            // Render sprite pixels.
            // When zoomed, each source pixel is doubled horizontally (16 screen pixels).
            let pixel_width: u8 = if zoomed { 16 } else { 8 };
            for px in 0..pixel_width {
                let x = x_pos.wrapping_add(px);
                if x as u16 >= 256 {
                    continue;
                }

                // Map screen pixel to source tile bit; zoom doubles each column.
                let src_px = if zoomed { px / 2 } else { px };
                let shift = 7 - src_px;
                let pixel = ((byte0 >> shift) & 1)
                    | (((byte1 >> shift) & 1) << 1)
                    | (((byte2 >> shift) & 1) << 2)
                    | (((byte3 >> shift) & 1) << 3);

                // Sprite pixel 0 is transparent
                if pixel != 0 {
                    let x_index = x as usize;

                    // Check for sprite collision (two non-transparent sprite pixels overlap)
                    if sprite_pixels[x_index] {
                        self.sprite_collision = true;
                    }

                    // Mark this pixel as having a sprite
                    sprite_pixels[x_index] = true;

                    // Check if background pixel has priority bit set
                    let bg_pixel = self.frame.pixels[line_offset + x_index];
                    let bg_has_priority = (bg_pixel & PRIORITY_BIT) != 0;

                    // Only render sprite if:
                    // 1. Background pixel is transparent (backdrop color), OR
                    // 2. Background pixel doesn't have priority bit set
                    let backdrop_color = self.backdrop_color();
                    let bg_is_backdrop = (bg_pixel & RGB_MASK) == (backdrop_color & RGB_MASK);

                    if bg_is_backdrop || !bg_has_priority {
                        // Sprites always use palette 1 (colors 16-31 in CRAM)
                        let color_index = 16 + pixel as usize;
                        // Strip alpha; render_scanline restores it after all rendering
                        let color = self.cram_color(color_index) & RGB_MASK;
                        self.frame.pixels[line_offset + x_index] = color;
                    }
                }
            }
        }
    }

    /// TMS9918A sprite rendering for modes 0-3
    ///
    /// TMS9918A sprites:
    /// - 32 sprites max (vs 64 in Mode 4)
    /// - Sprite Attribute Table: 4 bytes per sprite (Y, X, pattern, color/flags)
    /// - 4 sprites per scanline limit (5th sprite sets overflow flag)
    /// - Per-sprite color from TMS fixed palette
    /// - Sprite sizes: 8×8 or 16×16 (Register 1 bit 1)
    /// - Magnification: 2× when Register 1 bit 0 is set
    /// - Y=$D0 terminates sprite list
    /// - Byte 3 bit 7: Early Clock (shift left 32 pixels)
    fn render_tms_sprites(&mut self, line: u8, line_offset: usize) {
        // Sprite Attribute Table: (Register 5 & 0x7F) << 7
        let sat_base = ((self.registers[5] as u16) & 0x7F) << 7;
        // Sprite Pattern Generator: (Register 6 & 0x07) << 11
        let spg_base = ((self.registers[6] as u16) & 0x07) << 11;

        let large_sprites = (self.registers[1] & 0x02) != 0; // 16×16 sprites
        let magnified = (self.registers[1] & 0x01) != 0; // 2× magnification

        let base_size: u8 = if large_sprites { 16 } else { 8 };
        let effective_size: u8 = if magnified {
            base_size.saturating_mul(2)
        } else {
            base_size
        };

        let mut sprites_on_line = 0;
        let mut sprite_pixels = [false; 256];

        // Collect visible sprites on this line (forward scan, respects $D0 terminator)
        let mut visible: Vec<usize> = Vec::new();
        for i in 0..32 {
            let y = self.vram[(sat_base + i * 4) as usize];

            // $D0 terminates sprite list in 192-line mode
            if y == 0xD0 {
                break;
            }

            // Y is the actual display line minus 1 (top of sprite)
            let y_pos = y.wrapping_add(1);
            let diff = line.wrapping_sub(y_pos);
            if diff >= effective_size {
                continue;
            }

            sprites_on_line += 1;
            if sprites_on_line > 4 {
                self.sprite_overflow = true;
                break;
            }

            visible.push(i as usize);
        }

        // Render in reverse order so lower-numbered sprites have priority
        for &i in visible.iter().rev() {
            let sat_addr = sat_base + (i as u16) * 4;
            let y = self.vram[sat_addr as usize];
            let y_pos = y.wrapping_add(1);
            let mut x_pos = self.vram[(sat_addr + 1) as usize] as i16;
            let mut pattern = self.vram[(sat_addr + 2) as usize];
            let attr = self.vram[(sat_addr + 3) as usize];

            // Bit 7 of attribute: Early Clock — shift sprite left 32 pixels
            if (attr & 0x80) != 0 {
                x_pos -= 32;
            }

            // Lower 4 bits of attribute: sprite color (TMS palette)
            let color_index = attr & 0x0F;
            // Color 0 means transparent sprite
            if color_index == 0 {
                continue;
            }
            let color = self.decode_tms_color(color_index) & RGB_MASK;

            // For 16×16 sprites, bit 0 and bit 1 of pattern are forced to 0
            // (selects a group of 4 patterns: N, N+1, N+2, N+3 in 2×2 layout)
            if large_sprites {
                pattern &= 0xFC;
            }

            // Calculate sprite row
            let raw_y = line.wrapping_sub(y_pos);
            let sprite_y = if magnified { raw_y / 2 } else { raw_y };

            // Determine which sub-pattern to use for 16×16 sprites
            // Layout: top-left=N, bottom-left=N+1, top-right=N+2, bottom-right=N+3
            let pixel_width: u8 = if magnified {
                base_size.saturating_mul(2)
            } else {
                base_size
            };

            for px in 0..pixel_width {
                let screen_x = x_pos + px as i16;
                if !(0..256).contains(&screen_x) {
                    continue;
                }
                let sx = screen_x as usize;

                // Map to source pixel within sprite
                let src_px = if magnified { px / 2 } else { px };

                // For 16×16 sprites, determine which 8×8 quadrant
                let (quad_pattern, qx, qy) = if large_sprites {
                    let col = src_px / 8; // 0=left, 1=right
                    let row = sprite_y / 8; // 0=top, 1=bottom
                                            // TMS layout: N=top-left, N+1=bottom-left, N+2=top-right, N+3=bottom-right
                    let quad = row + col * 2;
                    (pattern + quad, src_px % 8, sprite_y % 8)
                } else {
                    (pattern, src_px, sprite_y)
                };

                // Read pattern data (8 bytes per 8×8 pattern, 1 bit per pixel)
                let pattern_addr = spg_base + (quad_pattern as u16) * 8 + qy as u16;
                if pattern_addr as usize >= self.vram.len() {
                    continue;
                }
                let pattern_byte = self.vram[pattern_addr as usize];

                // Extract pixel bit (MSB first)
                let bit = (pattern_byte >> (7 - qx)) & 1;
                if bit == 0 {
                    continue; // Transparent pixel
                }

                // Collision detection
                if sprite_pixels[sx] {
                    self.sprite_collision = true;
                }
                sprite_pixels[sx] = true;

                // TMS sprites always render on top of background
                self.frame.pixels[line_offset + sx] = color;
            }
        }
    }

    /// TMS9918A Graphics I Mode (Mode 0) rendering
    /// 256x192 resolution, 32x24 tiles, 2 colors per 8 tiles
    fn render_tms_graphics1(&mut self, line: u8, line_offset: usize) {
        // Pattern name table at $0800 + (Register 2 & 0x0F) * 0x400
        let name_table_base = ((self.registers[2] as u16) & 0x0F) << 10;

        // Pattern generator table at (Register 4 & 0x07) * 0x800
        let pattern_gen_base = ((self.registers[4] as u16) & 0x07) << 11;

        // Color table at (Register 3 & 0xFF) * 0x40
        let color_table_base = (self.registers[3] as u16) << 6;

        let y = line as u16;
        let tile_row = y / 8;
        let pixel_y = y % 8;

        for x in 0..256u16 {
            let tile_col = x / 8;

            // Get pattern name (which tile to use)
            let name_addr = name_table_base + (tile_row * 32 + tile_col);
            let pattern_name = self.vram[name_addr as usize] as u16;

            // Get pattern data (8 bytes per pattern)
            let pattern_addr = pattern_gen_base + pattern_name * 8 + pixel_y;
            let pattern_byte = self.vram[pattern_addr as usize];

            // Get color (1 byte per 8 patterns)
            let color_addr = color_table_base + (pattern_name / 8);
            let color_byte = self.vram[color_addr as usize];

            // Extract pixel bit (MSB first)
            let pixel_x = x % 8;
            let pixel_bit = (pattern_byte >> (7 - pixel_x)) & 1;

            // Select foreground or background color
            let color_index = if pixel_bit != 0 {
                (color_byte >> 4) & 0x0F // Foreground color (upper 4 bits)
            } else {
                color_byte & 0x0F // Background color (lower 4 bits)
            };

            // TMS9918A fixed palette (16 colors)
            let color = self.decode_tms_color(color_index);
            self.frame.pixels[line_offset + x as usize] = color;
        }
    }

    /// TMS9918A Text Mode (Mode 1) rendering
    /// 40x24 characters, 6x8 pixels per character, monochrome
    fn render_tms_text(&mut self, line: u8, line_offset: usize) {
        // Pattern name table at (Register 2 & 0x0F) * 0x400
        let name_table_base = ((self.registers[2] as u16) & 0x0F) << 10;

        // Pattern generator table at (Register 4 & 0x07) * 0x800
        let pattern_gen_base = ((self.registers[4] as u16) & 0x07) << 11;

        // Text mode uses register 7 for foreground/background colors
        let fg_color = self.decode_tms_color((self.registers[7] >> 4) & 0x0F);
        let bg_color = self.decode_tms_color(self.registers[7] & 0x0F);

        let y = line as u16;
        let char_row = y / 8;
        let pixel_y = y % 8;

        // Text mode: 40 characters wide, each 6 pixels, left-aligned
        for char_col in 0..40 {
            // Get character pattern
            let name_addr = name_table_base + (char_row * 40 + char_col);
            let pattern_name = self.vram[name_addr as usize] as u16;

            // Get pattern data
            let pattern_addr = pattern_gen_base + pattern_name * 8 + pixel_y;
            let pattern_byte = self.vram[pattern_addr as usize];

            // Draw 6 pixels (text mode uses only 6 bits, MSB first)
            for pixel_x in 0..6 {
                let pixel_bit = (pattern_byte >> (7 - pixel_x)) & 1;
                let color = if pixel_bit != 0 { fg_color } else { bg_color };
                let screen_x = char_col * 6 + pixel_x;
                if (screen_x as usize) < 256 {
                    self.frame.pixels[line_offset + screen_x as usize] = color;
                }
            }
        }

        // Fill remaining pixels with background color (240-256)
        for x in 240..256 {
            self.frame.pixels[line_offset + x] = bg_color;
        }
    }

    /// TMS9918A Graphics II Mode (Mode 2) rendering
    /// 256x192 resolution, enhanced color flexibility (2 colors per 8x1 row)
    fn render_tms_graphics2(&mut self, line: u8, line_offset: usize) {
        // Pattern name table at (Register 2 & 0x0F) * 0x400
        let name_table_base = ((self.registers[2] as u16) & 0x0F) << 10;

        // Pattern generator table - can address up to 3 sections
        // Register 4: bits 2-0 select the pattern base, bit 2 is AND mask for pattern addressing
        let pattern_gen_base = ((self.registers[4] as u16) & 0x04) << 11;
        let pattern_gen_mask = if (self.registers[4] & 0x03) == 0x03 {
            0x1FFF // All three sections
        } else {
            0x07FF // Single section
        };

        // Color table - similar masking to pattern table
        let color_table_base = (self.registers[3] as u16) << 6;
        let color_table_mask = if (self.registers[3] & 0x7F) == 0x7F {
            0x1FFF
        } else {
            0x07FF
        };

        let y = line as u16;
        let tile_row = y / 8;
        let pixel_y = y % 8;

        // Graphics II divides screen into thirds vertically for addressing
        let third = (y / 64) * 0x0800;

        for x in 0..256u16 {
            let tile_col = x / 8;

            // Get pattern name
            let name_addr = name_table_base + (tile_row * 32 + tile_col);
            let pattern_name = self.vram[name_addr as usize] as u16;

            // Calculate pattern address with third offset
            let pattern_offset = (pattern_name * 8 + pixel_y + third) & pattern_gen_mask;
            let pattern_addr = pattern_gen_base + pattern_offset;
            let pattern_byte = self.vram[pattern_addr as usize];

            // Calculate color address with third offset
            let color_offset = (pattern_name * 8 + pixel_y + third) & color_table_mask;
            let color_addr = color_table_base + color_offset;
            let color_byte = self.vram[color_addr as usize];

            // Extract pixel bit
            let pixel_x = x % 8;
            let pixel_bit = (pattern_byte >> (7 - pixel_x)) & 1;

            // Select color
            let color_index = if pixel_bit != 0 {
                (color_byte >> 4) & 0x0F
            } else {
                color_byte & 0x0F
            };

            let color = self.decode_tms_color(color_index);
            self.frame.pixels[line_offset + x as usize] = color;
        }
    }

    /// TMS9918A Multicolor Mode (Mode 3) rendering
    /// 64x48 blocks, each block is 4x4 pixels of a single color
    fn render_tms_multicolor(&mut self, line: u8, line_offset: usize) {
        // Pattern name table at (Register 2 & 0x0F) * 0x400
        let name_table_base = ((self.registers[2] as u16) & 0x0F) << 10;

        // Pattern generator table at (Register 4 & 0x07) * 0x800
        let pattern_gen_base = ((self.registers[4] as u16) & 0x07) << 11;

        let y = line as u16;
        let block_row = y / 4; // 4 pixel rows per block
        let block_y = (y % 4) / 2; // 2 pixel rows share same pattern byte

        for x in 0..256u16 {
            let block_col = x / 4; // 4 pixel columns per block
            let block_x = (x % 4) / 2; // 2 pixel columns share same nibble

            // Each "tile" in name table represents 4 blocks vertically
            let tile_row = block_row / 4;
            let tile_col = block_col / 4;

            // Get pattern name
            let name_addr = name_table_base + (tile_row * 8 + tile_col);
            let pattern_name = self.vram[name_addr as usize] as u16;

            // Pattern data: each pattern is 8 bytes, but only 2 are used in multicolor
            // Each byte defines colors for a 2x4 pixel area
            let pattern_addr = pattern_gen_base + pattern_name * 8 + (block_row % 4) * 2 + block_y;
            let pattern_byte = self.vram[pattern_addr as usize];

            // Get color from nibble (upper or lower 4 bits)
            let color_index = if block_x == 0 {
                (pattern_byte >> 4) & 0x0F
            } else {
                pattern_byte & 0x0F
            };

            let color = self.decode_tms_color(color_index);
            self.frame.pixels[line_offset + x as usize] = color;
        }
    }

    /// Decode TMS9918A fixed color palette
    fn decode_tms_color(&self, color_index: u8) -> u32 {
        // TMS9918A fixed 16-color palette
        match color_index & 0x0F {
            0 => 0xFF000000,  // Transparent (black)
            1 => 0xFF000000,  // Black
            2 => 0xFF21C842,  // Medium Green
            3 => 0xFF5EDC78,  // Light Green
            4 => 0xFF5455ED,  // Dark Blue
            5 => 0xFF7D76FC,  // Light Blue
            6 => 0xFFD4524D,  // Dark Red
            7 => 0xFF42EBF5,  // Cyan
            8 => 0xFFFC5554,  // Medium Red
            9 => 0xFFFF7978,  // Light Red
            10 => 0xFFD4C154, // Dark Yellow
            11 => 0xFFE6CE80, // Light Yellow
            12 => 0xFF21B03B, // Dark Green
            13 => 0xFFC95BBA, // Magenta
            14 => 0xFFCCCCCC, // Gray
            15 => 0xFFFFFFFF, // White
            _ => 0xFF000000,
        }
    }

    /// Get the current backdrop/overscan color (ARGB).
    /// In Mode 4: Register 7 bits 3-0 select a colour from the sprite palette (CRAM 16-31).
    /// In TMS modes: Register 7 bits 3-0 select a colour from the fixed TMS palette.
    fn backdrop_color(&self) -> u32 {
        let (mode_4, _) = self.get_video_mode();
        if mode_4 {
            let index = 16 + (self.registers[7] & 0x0F) as usize;
            self.cram_color(index)
        } else {
            self.decode_tms_color(self.registers[7] & 0x0F)
        }
    }

    /// Look up a CRAM colour entry and return ARGB8888.
    /// Handles both SMS (6-bit, 1 byte/entry) and GG (12-bit, 2 bytes/entry).
    fn cram_color(&self, index: usize) -> u32 {
        if self.is_game_gear {
            // GG: 2 bytes per entry, format: ----BBBBGGGGRRRR
            let base = (index * 2) % self.cram_size;
            let lo = self.cram[base] as u32;
            let hi = self.cram[base + 1] as u32;
            let word = lo | (hi << 8);
            let r = word & 0x0F;
            let g = (word >> 4) & 0x0F;
            let b = (word >> 8) & 0x0F;
            // Scale 4-bit to 8-bit (0-15 -> 0-255)
            let r8 = (r << 4) | r;
            let g8 = (g << 4) | g;
            let b8 = (b << 4) | b;
            0xFF000000 | (r8 << 16) | (g8 << 8) | b8
        } else {
            let cram_val = self.cram[index % self.cram_size];
            self.decode_color(cram_val & 0x3F)
        }
    }

    /// Decode 6-bit SMS color to 32-bit ARGB
    fn decode_color(&self, color: u8) -> u32 {
        // SMS uses 6-bit color: --BBGGRR
        let r = (color & 0x03) as u32;
        let g = ((color >> 2) & 0x03) as u32;
        let b = ((color >> 4) & 0x03) as u32;

        // Scale 2-bit to 8-bit (0-3 -> 0-255)
        let r8 = (r * 85) & 0xFF;
        let g8 = (g * 85) & 0xFF;
        let b8 = (b * 85) & 0xFF;

        // Return ARGB8888
        0xFF000000 | (r8 << 16) | (g8 << 8) | b8
    }

    /// Crop the internal 256×192 frame to the GG LCD viewport (160×144).
    /// The viewport is centered: X offset = 48, Y offset = 24.
    fn crop_gg_viewport(&mut self) {
        const GG_X_OFFSET: usize = 48;
        const GG_Y_OFFSET: usize = 24;
        const GG_WIDTH: usize = 160;
        const GG_HEIGHT: usize = 144;

        for y in 0..GG_HEIGHT {
            let src_y = y + GG_Y_OFFSET;
            let src_offset = src_y * 256 + GG_X_OFFSET;
            let dst_offset = y * GG_WIDTH;
            self.gg_frame.pixels[dst_offset..dst_offset + GG_WIDTH]
                .copy_from_slice(&self.frame.pixels[src_offset..src_offset + GG_WIDTH]);
        }
    }
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for Vdp {
    fn get_frame(&self) -> &Frame {
        // Log every time frame is retrieved
        let backdrop = self.backdrop_color();
        let display_enabled = (self.registers[1] & 0x40) != 0;
        let sprite_enabled = display_enabled;
        let target_frame = if self.is_game_gear {
            &self.gg_frame
        } else {
            &self.frame
        };
        let mut non_backdrop = 0;
        for &pixel in &target_frame.pixels {
            if pixel != backdrop {
                non_backdrop += 1;
            }
        }
        log(LogCategory::PPU, LogLevel::Info, || {
            format!(
                "SMS VDP: get_frame() - Display={} SPR={} R1=${:02X} backdrop=${:08X} non-backdrop={}",
                display_enabled, sprite_enabled, self.registers[1], backdrop, non_backdrop
            )
        });
        target_frame
    }

    fn clear(&mut self, color: u32) {
        self.frame.pixels.fill(color);
    }

    fn reset(&mut self) {
        self.vram.fill(0);
        self.cram.fill(0);
        self.registers.fill(0);
        // Set Register 0 to 0x04 to enable Mode 4 by default
        // Real SMS hardware defaults to Mode 4 enabled (M4 bit set)
        // This ensures real SMS ROMs work without explicit VDP initialization
        self.registers[0] = 0x04;
        self.address_register = 0;
        self.code_register = 0;
        self.read_buffer = 0;
        self.write_latch = false;
        self.cram_latch = 0;
        self.frame_interrupt_pending = false;
        self.line_interrupt_pending = false;
        self.line_counter = 0;
        self.sprite_overflow = false;
        self.sprite_collision = false;
        // Set to end of frame so first set_scanline(0) will properly render frame
        self.scanline = 262;
        // in_vblank starts false: the startup sentinel (262) must not be treated as VBlank
        // so that early R1 writes don't fire a spurious retroactive interrupt.
        self.in_vblank = false;
        self.h_counter_latch = 0;
        self.clear(0xFF000000);
    }

    fn resize(&mut self, width: u32, height: u32) {
        // SMS has fixed 256×192 resolution, but allow resizing frame buffer
        self.frame = Frame::new(width, height);
    }

    fn name(&self) -> &str {
        "SMS VDP"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdp_creation() {
        let vdp = Vdp::new();
        assert_eq!(vdp.frame.pixels.len(), 256 * 192);
    }

    #[test]
    fn test_vdp_register_write() {
        let mut vdp = Vdp::new();

        // Write to register 0
        vdp.write_control(0x00); // Low byte
        vdp.write_control(0x80); // High byte (register write, reg 0)

        assert_eq!(vdp.registers[0], 0x00);

        // Write to register 1 with value 0xA0
        vdp.write_control(0xA0); // Low byte (value)
        vdp.write_control(0x81); // High byte (register write, reg 1)

        assert_eq!(vdp.registers[1], 0xA0);
    }

    #[test]
    fn test_vdp_vram_write() {
        let mut vdp = Vdp::new();

        // Set VRAM address to 0x1234
        vdp.write_control(0x34); // Low byte
        vdp.write_control(0x52); // High byte (VRAM write, 0x12)

        // Write data
        vdp.write_data(0x42);

        assert_eq!(vdp.vram[0x1234], 0x42);
    }

    #[test]
    fn test_vdp_color_decode() {
        let vdp = Vdp::new();

        // Test black (all zeros)
        assert_eq!(vdp.decode_color(0x00), 0xFF000000);

        // Test white (all ones in 6 bits)
        assert_eq!(vdp.decode_color(0x3F), 0xFFFFFFFF);

        // Test red (0x03)
        assert_eq!(vdp.decode_color(0x03), 0xFFFF0000);
    }

    #[test]
    fn test_backdrop_uses_register7() {
        let mut vdp = Vdp::new();

        // Enable Mode 4 so backdrop uses CRAM
        vdp.registers[0] = 0x04;

        // Set CRAM[16] = sky blue, CRAM[17] = black
        vdp.cram[16] = 0x30; // Blue (--11_00_00)
        vdp.cram[17] = 0x00; // Black

        // With R7 = 0 (default), backdrop should come from CRAM[16]
        assert_eq!(vdp.backdrop_color(), vdp.decode_color(0x30));

        // Set R7 = 1 → backdrop selects CRAM[17] (black)
        vdp.registers[7] = 0x01;
        assert_eq!(vdp.backdrop_color(), vdp.decode_color(0x00));

        // Set R7 = 0x0F → backdrop selects CRAM[31]
        vdp.cram[31] = 0x03; // Red
        vdp.registers[7] = 0x0F;
        assert_eq!(vdp.backdrop_color(), vdp.decode_color(0x03));

        // Upper bits of R7 are ignored
        vdp.registers[7] = 0xF1;
        assert_eq!(vdp.backdrop_color(), vdp.decode_color(0x00)); // CRAM[17]
    }

    #[test]
    fn test_sprite_overflow_detection() {
        let mut vdp = Vdp::new();

        // Enable Mode 4 and display
        vdp.registers[0] = 0x04;
        vdp.registers[1] = 0x40;

        // Set sprite attribute table at 0x3F00 (register 5)
        vdp.registers[5] = 0x7E; // (0x7E << 7) = 0x3F00

        // Create 9 sprites on the same scanline (line 100)
        let sprite_attr_table = 0x3F00;
        for i in 0..9 {
            // Y position - 1 (since Y is offset by 1)
            vdp.vram[sprite_attr_table + i] = 99;
        }

        // Render the scanline
        vdp.render_scanline(100);

        // Check that sprite overflow flag is set
        assert!(vdp.sprite_overflow);

        // Read status should return bit 6 set
        let status = vdp.read_status();
        assert_eq!(status & 0x40, 0x40);

        // After reading status, flag should be cleared
        assert!(!vdp.sprite_overflow);
    }

    #[test]
    fn test_sprite_collision_detection() {
        let mut vdp = Vdp::new();

        // Enable Mode 4 and display
        vdp.registers[0] = 0x04;
        vdp.registers[1] = 0x40;

        // Set sprite attribute table
        vdp.registers[5] = 0x7E; // 0x3F00

        let sprite_attr_table = 0x3F00;

        // Create two sprites that overlap on line 100 at X position 50
        // Sprite 0
        vdp.vram[sprite_attr_table] = 99; // Y position - 1
        vdp.vram[sprite_attr_table + 128] = 50; // X position
        vdp.vram[sprite_attr_table + 128 + 1] = 0; // Tile 0

        // Sprite 1 (overlapping)
        vdp.vram[sprite_attr_table + 1] = 99; // Y position - 1
        vdp.vram[sprite_attr_table + 128 + 2] = 50; // X position (same)
        vdp.vram[sprite_attr_table + 128 + 3] = 1; // Tile 1

        // Set up tile patterns with non-transparent pixels
        // Tile 0 - all white pixels
        for i in 0..32 {
            vdp.vram[i] = 0xFF;
        }
        // Tile 1 - all white pixels
        for i in 32..64 {
            vdp.vram[i] = 0xFF;
        }

        // Set up palette
        vdp.cram[16] = 0x3F; // Sprite palette entry 0 = white

        // Render the scanline
        vdp.render_scanline(100);

        // Check that sprite collision flag is set
        assert!(vdp.sprite_collision);

        // Read status should return bit 5 set
        let status = vdp.read_status();
        assert_eq!(status & 0x20, 0x20);

        // After reading status, flag should be cleared
        assert!(!vdp.sprite_collision);
    }

    #[test]
    fn test_line_interrupt_triggering() {
        let mut vdp = Vdp::new();

        // Enable line interrupts (bit 4 of register 0)
        vdp.registers[0] = 0x10;

        // Set line counter reload value (register 10)
        vdp.registers[10] = 5;

        // Initialize line counter to register 10 value (simulating VBlank reload)
        vdp.line_counter = vdp.registers[10];

        // With R10=5, the counter decrements each scanline and underflows
        // after R10+1 = 6 scanlines:
        //   scanline 0: 5 → 4
        //   scanline 1: 4 → 3
        //   scanline 2: 3 → 2
        //   scanline 3: 2 → 1
        //   scanline 4: 1 → 0
        //   scanline 5: 0 → 0xFF (underflow!) → reload to 5 + fire interrupt

        // First scanline should decrement counter from 5 to 4
        vdp.render_scanline(0);
        assert_eq!(vdp.line_counter, 4);
        assert!(!vdp.line_interrupt_pending);

        // After 4 more scanlines, counter reaches 0
        for line in 1..=4 {
            vdp.render_scanline(line);
        }
        assert_eq!(vdp.line_counter, 0);
        assert!(!vdp.line_interrupt_pending);

        // Next scanline: counter underflows (0 → 0xFF), triggers interrupt and reloads
        vdp.render_scanline(5);
        assert!(vdp.line_interrupt_pending);
        assert_eq!(vdp.line_counter, 5);

        // Clear interrupt
        vdp.clear_line_interrupt();
        assert!(!vdp.line_interrupt_pending);
    }

    #[test]
    fn test_status_flags_cleared_on_read() {
        let mut vdp = Vdp::new();

        // Set all flags
        vdp.frame_interrupt_pending = true;
        vdp.line_interrupt_pending = true;
        vdp.sprite_overflow = true;
        vdp.sprite_collision = true;

        // Read status
        let status = vdp.read_status();

        // All flags should be set
        assert_eq!(status & 0x80, 0x80); // Frame interrupt
        assert_eq!(status & 0x40, 0x40); // Sprite overflow
        assert_eq!(status & 0x20, 0x20); // Sprite collision

        // After read, ALL flags should be cleared (including line_interrupt_pending)
        assert!(!vdp.frame_interrupt_pending);
        assert!(!vdp.line_interrupt_pending);
        assert!(!vdp.sprite_overflow);
        assert!(!vdp.sprite_collision);

        // Second read should return 0
        let status2 = vdp.read_status();
        assert_eq!(status2 & 0xE0, 0);
    }

    /// Enabling the frame-interrupt bit (R1 bit 5) while the scanline is already inside
    /// VBlank must immediately set frame_interrupt_pending (retroactive fire).
    /// This mirrors what SMS_init does: it writes R1=0x20 after the VBlank boundary has
    /// been crossed, so without the retroactive check the first interrupt would never fire.
    #[test]
    fn test_frame_interrupt_retroactive_when_already_in_vblank() {
        let mut vdp = Vdp::new();

        // Advance into VBlank via set_scanline so the in_vblank latch is set.
        // Vdp::new() starts at scanline 262; set_scanline(192) wraps (new frame) and
        // ends at scanline 192 >= display_height(192), so in_vblank becomes true.
        vdp.set_scanline(192);
        assert!(
            vdp.in_vblank,
            "in_vblank should be true after crossing display_height"
        );

        // Status bit 7 is always set on VBlank entry, regardless of IE0
        assert!(vdp.frame_interrupt_pending);
        // Clear it by reading status, simulating the game reading status before
        // enabling interrupts
        vdp.read_status();
        assert!(!vdp.frame_interrupt_pending);

        // Write R1 = 0x20 (frame interrupt enable) via the control port
        vdp.write_control(0x20); // First byte: value
        vdp.write_control(0x81); // Second byte: register 1 write

        // The interrupt must have fired retroactively
        assert!(
            vdp.frame_interrupt_pending,
            "frame_interrupt_pending should be set retroactively when R1 bit 5 is enabled during VBlank"
        );
    }

    /// Setting frame-interrupt enable while still in the active display must NOT
    /// fire the interrupt — it should only fire when VBlank is entered.
    #[test]
    fn test_frame_interrupt_not_retroactive_outside_vblank() {
        let mut vdp = Vdp::new();

        // Advance into active display via set_scanline.
        // From initial scanline 262, set_scanline(100) wraps to a new frame and
        // ends at scanline 100 < display_height(192), so in_vblank becomes false.
        // The wrap crosses VBlank, so frame_interrupt_pending is set.
        vdp.set_scanline(100);
        assert!(
            !vdp.in_vblank,
            "in_vblank should be false inside active display"
        );

        // The VBlank crossing during the wrap unconditionally set the flag.
        // Clear it to simulate the game having read status already.
        vdp.read_status();
        assert!(!vdp.frame_interrupt_pending);

        // Write R1 = 0x20 (frame interrupt enable)
        vdp.write_control(0x20);
        vdp.write_control(0x81);

        // Must NOT fire — we are not in VBlank, and the flag was already consumed
        assert!(
            !vdp.frame_interrupt_pending,
            "frame_interrupt_pending must not be set when scanline is inside active display"
        );
    }

    /// Verifies that the startup sentinel value (scanline = 262) does NOT cause a
    /// spurious retroactive interrupt before any real VBlank has been entered.
    #[test]
    fn test_frame_interrupt_no_spurious_at_startup() {
        let mut vdp = Vdp::new();

        // VDP just created: scanline = 262 (sentinel), in_vblank = false
        assert!(!vdp.in_vblank, "in_vblank must be false at startup");

        // Write R1 = 0x20 without calling set_scanline first
        vdp.write_control(0x20);
        vdp.write_control(0x81);

        // No interrupt should fire — we haven't actually entered VBlank yet
        assert!(
            !vdp.frame_interrupt_pending,
            "startup sentinel scanline must not trigger a spurious retroactive interrupt"
        );
    }

    #[test]
    fn test_vcounter_ntsc_mapping() {
        let mut vdp = Vdp::new();
        vdp.is_pal = false;

        // Active display: direct mapping
        vdp.scanline = 0;
        assert_eq!(vdp.read_vcounter(), 0x00);
        vdp.scanline = 0xDA; // last direct-mapped scanline
        assert_eq!(vdp.read_vcounter(), 0xDA);

        // After the jump: scanline 219 (0xDB) → 0xD5
        vdp.scanline = 219;
        assert_eq!(vdp.read_vcounter(), 0xD5);

        // Last scanline before wrap: 261 → 0xFF
        vdp.scanline = 261;
        assert_eq!(vdp.read_vcounter(), 0xFF);
    }

    #[test]
    fn test_vcounter_pal_mapping() {
        let mut vdp = Vdp::new();
        vdp.is_pal = true;

        // Active display: direct mapping
        vdp.scanline = 0;
        assert_eq!(vdp.read_vcounter(), 0x00);
        vdp.scanline = 0xF2; // last direct-mapped PAL scanline
        assert_eq!(vdp.read_vcounter(), 0xF2);

        // After the jump: scanline 243 → 0xBA
        vdp.scanline = 243;
        assert_eq!(vdp.read_vcounter(), 0xBA);

        // Last PAL scanline before wrap: 312 → 0xFF
        vdp.scanline = 312;
        assert_eq!(vdp.read_vcounter(), 0xFF);
    }

    #[test]
    fn test_set_pal_switches_vcounter_mapping() {
        let mut vdp = Vdp::new();
        // Place scanline in the diverging range (scanline 250 is past both thresholds)
        vdp.scanline = 250;

        vdp.set_pal(false); // NTSC: 250 - 6 = 244 = 0xF4
        assert_eq!(vdp.read_vcounter(), 0xF4);

        vdp.set_pal(true); // PAL: 250 - 57 = 193 = 0xC1
        assert_eq!(vdp.read_vcounter(), 0xC1);
    }

    #[test]
    fn test_line_counter_mid_frame_r10_change() {
        let mut vdp = Vdp::new();

        // Enable line interrupts
        vdp.registers[0] = 0x10;
        vdp.registers[10] = 3;
        vdp.line_counter = 3;

        // Scanlines 0-2: counter 3→2→1→0 (3 decrements)
        for line in 0..3 {
            vdp.render_scanline(line);
        }
        assert_eq!(vdp.line_counter, 0);
        assert!(!vdp.line_interrupt_pending);

        // Change R10 mid-frame BEFORE the underflow scanline
        vdp.registers[10] = 10;

        // Scanline 3: counter underflows (0→0xFF), should reload from NEW R10 value (10)
        vdp.render_scanline(3);
        assert!(vdp.line_interrupt_pending);
        assert_eq!(vdp.line_counter, 10);
    }

    #[test]
    fn test_tms_sprite_rendering() {
        let mut vdp = Vdp::new();

        // Set to TMS Graphics I mode (M4=0, M1=M2=M3=0)
        vdp.registers[0] = 0x00; // M4=0, M3=0
        vdp.registers[1] = 0x40; // Display enabled, M1=0, M2=0

        // Set SAT at 0x3F00: (R5 & 0x7F) << 7 = 0x7E << 7 = 0x3F00
        vdp.registers[5] = 0x7E;
        // Set SPG at 0x0000: (R6 & 0x07) << 11 = 0
        vdp.registers[6] = 0x00;

        let sat_base: usize = 0x3F00;

        // Create one sprite at line 100, X=50, pattern 0, color=8 (medium red)
        vdp.vram[sat_base] = 99; // Y = 99 (sprite at line 100 since Y+1)
        vdp.vram[sat_base + 1] = 50; // X
        vdp.vram[sat_base + 2] = 0; // Pattern 0
        vdp.vram[sat_base + 3] = 8; // Color index 8 (medium red), no early clock

        // Terminate sprite list
        vdp.vram[sat_base + 4] = 0xD0;

        // Set pattern 0 with a solid row at row 0 (all bits set)
        vdp.vram[0] = 0xFF; // Row 0: all 8 pixels set

        // Render scanline 100
        vdp.render_scanline(100);

        // Check that pixels 50-57 have the TMS medium red color
        let expected_color = 0xFFFC5554; // Medium red from TMS palette
        let line_offset = 100 * 256;
        for x in 50..58 {
            assert_eq!(
                vdp.frame.pixels[line_offset + x],
                expected_color,
                "Sprite pixel at x={} should be TMS medium red",
                x
            );
        }
    }

    #[test]
    fn test_tms_sprite_overflow_4_limit() {
        let mut vdp = Vdp::new();

        // TMS Graphics I mode
        vdp.registers[0] = 0x00;
        vdp.registers[1] = 0x40; // Display enabled
        vdp.registers[5] = 0x7E; // SAT at 0x3F00
        vdp.registers[6] = 0x00;

        let sat_base: usize = 0x3F00;

        // Create 5 sprites on same line (TMS has 4 per scanline limit)
        for i in 0..5u8 {
            vdp.vram[sat_base + i as usize * 4] = 99; // Y
            vdp.vram[sat_base + i as usize * 4 + 1] = 10 + i * 10; // X
            vdp.vram[sat_base + i as usize * 4 + 2] = 0; // Pattern
            vdp.vram[sat_base + i as usize * 4 + 3] = 2; // Color (green)
        }
        // Terminate
        vdp.vram[sat_base + 5 * 4] = 0xD0;

        // Set pattern 0 with solid pixels
        vdp.vram[0] = 0xFF;

        vdp.render_scanline(100);

        // 5th sprite should trigger overflow
        assert!(vdp.sprite_overflow);
    }
}
