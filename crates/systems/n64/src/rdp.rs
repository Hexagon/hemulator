//! RDP (Reality Display Processor) - Graphics chip for Nintendo 64
//!
//! The RDP is part of the RCP (Reality Co-Processor) and handles graphics rendering.
//! It works together with the RSP (Reality Signal Processor) which prepares display lists.
//!
//! # Architecture Overview
//!
//! The N64's graphics pipeline consists of:
//! - **RSP (Reality Signal Processor)**: Geometry processing, vertex transforms, lighting
//! - **RDP (Reality Display Processor)**: Rasterization, texturing, blending
//!
//! ## RDP Features
//!
//! - **Resolution**: Supports various resolutions, commonly 320x240 or 640x480
//! - **Color Depth**: 16-bit (RGBA5551) or 32-bit (RGBA8888) framebuffer
//! - **Texture Mapping**: Perspective-correct texture mapping with filtering
//! - **Z-Buffer**: Depth testing and hidden surface removal
//! - **Anti-aliasing**: Built-in coverage anti-aliasing
//! - **Blending**: Alpha blending and fog effects
//! - **Fill Operations**: Fast rectangle fills for clearing and simple primitives
//!
//! ## Memory Map
//!
//! RDP registers are memory-mapped in the N64 address space:
//! - **0x04100000-0x0410001F**: RDP Command registers
//! - **0x04400000-0x044FFFFF**: RDP Span buffer (internal)
//!
//! ## Display List Commands
//!
//! The RDP executes display list commands prepared by the RSP. Common commands include:
//! - **Fill Rectangle**: Fast solid color rectangle fills
//! - **Texture Rectangle**: Textured rectangle rendering
//! - **Triangle**: Textured or shaded triangle rendering
//! - **Set Combine Mode**: Configure color/alpha blending
//! - **Set Scissor**: Define rendering bounds
//! - **Sync**: Wait for rendering completion
//!
//! # Implementation Details
//!
//! This is a **simplified frame-based implementation** suitable for basic rendering:
//! - Maintains a framebuffer with configurable resolution
//! - Supports basic fill operations and color clearing
//! - Registers for configuration (color, resolution, etc.)
//! - Not cycle-accurate; focuses on correct visual output
//!
//! Full RDP emulation would require:
//! - Complete display list command execution
//! - Texture cache and TMEM (texture memory)
//! - Perspective-correct rasterization
//! - Z-buffer and blending pipeline
//! - Accurate timing and synchronization

use emu_core::types::Frame;

/// RDP register addresses (relative to 0x04100000)
const DPC_START: u32 = 0x00; // DMA start address
const DPC_END: u32 = 0x04; // DMA end address
const DPC_CURRENT: u32 = 0x08; // DMA current address
const DPC_STATUS: u32 = 0x0C; // Status register
const DPC_CLOCK: u32 = 0x10; // Clock counter
const DPC_BUFBUSY: u32 = 0x14; // Buffer busy counter
const DPC_PIPEBUSY: u32 = 0x18; // Pipe busy counter
const DPC_TMEM: u32 = 0x1C; // TMEM load counter

/// RDP status register bits
const DPC_STATUS_XBUS_DMEM_DMA: u32 = 0x001; // Set XBUS DMEM DMA
const DPC_STATUS_FREEZE: u32 = 0x002; // Freeze RDP
const DPC_STATUS_FLUSH: u32 = 0x004; // Flush RDP pipeline
const DPC_STATUS_START_GCLK: u32 = 0x008; // Start GCLK counter
const DPC_STATUS_CBUF_READY: u32 = 0x080; // Command buffer ready
const DPC_STATUS_DMA_BUSY: u32 = 0x100; // DMA in progress
const DPC_STATUS_PIPE_BUSY: u32 = 0x200; // Pipe busy

/// Color format for framebuffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used for future color format support
pub enum ColorFormat {
    /// 16-bit RGBA (5-5-5-1)
    RGBA5551,
    /// 32-bit RGBA (8-8-8-8)
    RGBA8888,
}

/// RDP state and framebuffer
pub struct Rdp {
    /// Current framebuffer
    framebuffer: Frame,

    /// Framebuffer width
    width: u32,

    /// Framebuffer height
    height: u32,

    /// Color format
    #[allow(dead_code)] // Reserved for future color format support
    color_format: ColorFormat,

    /// Fill color (RGBA8888)
    fill_color: u32,

    /// DPC registers
    dpc_start: u32,
    dpc_end: u32,
    dpc_current: u32,
    dpc_status: u32,
}

impl Rdp {
    /// Create a new RDP with default resolution (320x240)
    pub fn new() -> Self {
        Self::with_resolution(320, 240)
    }

    /// Create a new RDP with specified resolution
    pub fn with_resolution(width: u32, height: u32) -> Self {
        Self {
            framebuffer: Frame::new(width, height),
            width,
            height,
            color_format: ColorFormat::RGBA5551,
            fill_color: 0xFF000000, // Black with full alpha
            dpc_start: 0,
            dpc_end: 0,
            dpc_current: 0,
            dpc_status: DPC_STATUS_CBUF_READY, // Start ready for commands
        }
    }

    /// Reset RDP to initial state
    pub fn reset(&mut self) {
        self.framebuffer = Frame::new(self.width, self.height);
        self.fill_color = 0xFF000000;
        self.dpc_start = 0;
        self.dpc_end = 0;
        self.dpc_current = 0;
        self.dpc_status = DPC_STATUS_CBUF_READY;
    }

    /// Set the fill color for clear operations
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn set_fill_color(&mut self, color: u32) {
        self.fill_color = color;
    }

    /// Clear the framebuffer with the current fill color
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn clear(&mut self) {
        for pixel in &mut self.framebuffer.pixels {
            *pixel = self.fill_color;
        }
    }

    /// Fill a rectangle with the current fill color
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        let x_end = (x + width).min(self.width);
        let y_end = (y + height).min(self.height);

        for py in y..y_end {
            for px in x..x_end {
                let idx = (py * self.width + px) as usize;
                if idx < self.framebuffer.pixels.len() {
                    self.framebuffer.pixels[idx] = self.fill_color;
                }
            }
        }
    }

    /// Set a pixel at the given coordinates
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            if idx < self.framebuffer.pixels.len() {
                self.framebuffer.pixels[idx] = color;
            }
        }
    }

    /// Get the current framebuffer
    pub fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    /// Read from RDP register
    pub fn read_register(&self, offset: u32) -> u32 {
        match offset {
            DPC_START => self.dpc_start,
            DPC_END => self.dpc_end,
            DPC_CURRENT => self.dpc_current,
            DPC_STATUS => self.dpc_status,
            DPC_CLOCK => 0,    // Clock counter not implemented
            DPC_BUFBUSY => 0,  // Buffer busy counter not implemented
            DPC_PIPEBUSY => 0, // Pipe busy counter not implemented
            DPC_TMEM => 0,     // TMEM counter not implemented
            _ => 0,
        }
    }

    /// Write to RDP register
    pub fn write_register(&mut self, offset: u32, value: u32) {
        match offset {
            DPC_START => {
                self.dpc_start = value & 0x00FFFFFF;
            }
            DPC_END => {
                self.dpc_end = value & 0x00FFFFFF;
                // When END is written, trigger DMA processing (simplified)
                self.process_display_list();
            }
            DPC_STATUS => {
                // Status register write (control bits)
                if value & DPC_STATUS_XBUS_DMEM_DMA != 0 {
                    // Toggle XBUS DMEM DMA mode
                }
                if value & DPC_STATUS_FREEZE != 0 {
                    // Freeze/unfreeze RDP
                }
                if value & DPC_STATUS_FLUSH != 0 {
                    // Flush pipeline
                    self.dpc_status &= !DPC_STATUS_PIPE_BUSY;
                }
                if value & DPC_STATUS_START_GCLK != 0 {
                    // Start/stop clock counter
                }
            }
            _ => {}
        }
    }

    /// Process display list commands (simplified stub)
    fn process_display_list(&mut self) {
        // In a full implementation, this would:
        // 1. Read commands from RDRAM between dpc_start and dpc_end
        // 2. Parse and execute each display list command
        // 3. Update status registers appropriately
        //
        // For now, we just mark the buffer as processed
        self.dpc_current = self.dpc_end;
        self.dpc_status |= DPC_STATUS_CBUF_READY;
        self.dpc_status &= !DPC_STATUS_DMA_BUSY;
    }
}

impl Default for Rdp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_creation() {
        let rdp = Rdp::new();
        assert_eq!(rdp.width, 320);
        assert_eq!(rdp.height, 240);
        assert_eq!(rdp.framebuffer.width, 320);
        assert_eq!(rdp.framebuffer.height, 240);
    }

    #[test]
    fn test_rdp_custom_resolution() {
        let rdp = Rdp::with_resolution(640, 480);
        assert_eq!(rdp.width, 640);
        assert_eq!(rdp.height, 480);
    }

    #[test]
    fn test_rdp_clear() {
        let mut rdp = Rdp::new();
        rdp.set_fill_color(0xFFFF0000); // Red
        rdp.clear();

        // Check all pixels are red
        for pixel in &rdp.framebuffer.pixels {
            assert_eq!(*pixel, 0xFFFF0000);
        }
    }

    #[test]
    fn test_rdp_fill_rect() {
        let mut rdp = Rdp::new();
        rdp.set_fill_color(0xFF00FF00); // Green
        rdp.fill_rect(10, 10, 20, 20);

        // Check pixels inside rectangle
        for y in 10..30 {
            for x in 10..30 {
                let idx = (y * 320 + x) as usize;
                assert_eq!(rdp.framebuffer.pixels[idx], 0xFF00FF00);
            }
        }

        // Check pixel outside rectangle is black (default)
        assert_eq!(rdp.framebuffer.pixels[0], 0);
    }

    #[test]
    fn test_rdp_set_pixel() {
        let mut rdp = Rdp::new();
        rdp.set_pixel(100, 100, 0xFFFFFFFF); // White

        let idx = (100 * 320 + 100) as usize;
        assert_eq!(rdp.framebuffer.pixels[idx], 0xFFFFFFFF);
    }

    #[test]
    fn test_rdp_reset() {
        let mut rdp = Rdp::new();
        rdp.set_fill_color(0xFFFF0000);
        rdp.clear();

        rdp.reset();

        // After reset, should be back to black
        assert_eq!(rdp.framebuffer.pixels[0], 0);
        assert_eq!(rdp.fill_color, 0xFF000000);
    }

    #[test]
    fn test_rdp_registers() {
        let mut rdp = Rdp::new();

        // Test DPC_START register
        rdp.write_register(DPC_START, 0x00123456);
        assert_eq!(rdp.read_register(DPC_START), 0x00123456);

        // Test DPC_END register
        rdp.write_register(DPC_END, 0x00789ABC);
        assert_eq!(rdp.read_register(DPC_END), 0x00789ABC);

        // Test DPC_STATUS register
        let status = rdp.read_register(DPC_STATUS);
        assert_ne!(status, 0); // Should have CBUF_READY bit set
    }

    #[test]
    fn test_rdp_bounds_checking() {
        let mut rdp = Rdp::new();

        // Should not panic when drawing outside bounds
        rdp.set_pixel(1000, 1000, 0xFFFFFFFF);
        rdp.fill_rect(300, 220, 100, 100); // Partially outside

        // No assertion - just checking it doesn't panic
    }

    #[test]
    fn test_rdp_get_frame() {
        let mut rdp = Rdp::new();
        rdp.set_fill_color(0xFF0000FF); // Blue
        rdp.clear();

        let frame = rdp.get_frame();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixels[0], 0xFF0000FF);
    }
}
