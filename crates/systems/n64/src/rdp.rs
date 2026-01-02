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
//! - **Triangle**: Textured or shaded triangle rendering (0x08-0x0F)
//! - **Set Combine Mode**: Configure color/alpha blending
//! - **Set Scissor**: Define rendering bounds
//! - **Sync**: Wait for rendering completion
//!
//! # Implementation Details
//!
//! This is a **simplified frame-based implementation** suitable for 3D rendering:
//! - Maintains a framebuffer with configurable resolution
//! - **3D Triangle Rasterization**:
//!   - Flat-shaded triangles (solid color)
//!   - Gouraud-shaded triangles (per-vertex color interpolation)
//!   - Z-buffered rendering (uses modular `ZBuffer` from `emu_core::graphics`)
//!   - Scanline-based edge walking algorithm
//! - **Color Operations**: Uses modular `ColorOps` from `emu_core::graphics`
//! - Supports basic fill operations and color clearing
//! - Registers for configuration (color, resolution, etc.)
//! - Not cycle-accurate; focuses on correct visual output
//!
//! ## Rendering Backends
//!
//! The RDP supports two rendering backends via the `RdpRenderer` trait:
//!
//! ### Software Renderer (Default)
//! - **CPU-based rasterization**: All rendering done on CPU
//! - **Maximum accuracy**: Guaranteed consistent results
//! - **Cross-platform**: Works on all platforms
//! - **Z-buffer**: Full 16-bit depth buffer
//! - **Performance**: Suitable for most N64 games
//! - **Implementation**: `SoftwareRdpRenderer` in `rdp_renderer_software.rs`
//!
//! ### OpenGL Renderer (Optional)
//! - **GPU-accelerated**: Uses OpenGL 3.3 Core Profile
//! - **Hardware depth testing**: Leverages GPU Z-buffer
//! - **Feature parity**: Implements all RdpRenderer methods
//! - **Coordinate conversion**: Handles Y-axis flip (OpenGL bottom-up → framebuffer top-down)
//! - **Color format conversion**: ARGB ↔ RGBA conversion for OpenGL compatibility
//! - **Performance**: Better for complex 3D scenes
//! - **Implementation**: `OpenGLRdpRenderer` in `rdp_renderer_opengl.rs`
//! - **Enabled**: Feature-gated behind `opengl` feature flag
//!
//! Both renderers produce identical output and pass the same test suite.
//!
//! Full RDP emulation would require:
//! - Complete display list command execution
//! - Texture cache and TMEM (texture memory) sampling
//! - Perspective-correct rasterization
//! - Full blending pipeline
//! - Accurate timing and synchronization

use super::rdp_renderer::{RdpRenderer, ScissorBox};
use super::rdp_renderer_software::SoftwareRdpRenderer;
use emu_core::graphics::ColorOps;
use emu_core::logging::{log, LogCategory, LogLevel};
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

/// Texture tile descriptor
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Fields reserved for future texture mapping implementation
struct TileDescriptor {
    format: u32,    // Texture format (RGBA, CI, IA, I)
    size: u32,      // Texel size (4bit, 8bit, 16bit, 32bit)
    line: u32,      // Pitch in 64-bit words
    tmem_addr: u32, // TMEM address (in 64-bit words)
    palette: u32,   // Palette number for CI textures
    s_mask: u32,    // S coordinate mask for wrapping
    t_mask: u32,    // T coordinate mask for wrapping
    s_shift: u32,   // S coordinate shift
    t_shift: u32,   // T coordinate shift
}

/// RDP state and framebuffer
pub struct Rdp {
    /// Rendering backend (software or OpenGL)
    renderer: Box<dyn RdpRenderer>,

    /// Framebuffer width
    width: u32,

    /// Framebuffer height
    height: u32,

    /// Color format
    #[allow(dead_code)] // Reserved for future color format support
    color_format: ColorFormat,

    /// Fill color (RGBA8888)
    fill_color: u32,

    /// Scissor box for clipping
    scissor: ScissorBox,

    /// TMEM (Texture Memory) - 4KB buffer
    tmem: [u8; 4096],

    /// Texture tile descriptors (8 tiles)
    tiles: [TileDescriptor; 8],

    /// Current texture image address in RDRAM
    texture_image_addr: u32,

    /// Color registers for RDP rendering modes
    blend_color: u32, // RGBA8888 - used in blending operations
    prim_color: u32,   // RGBA8888 - primitive color
    env_color: u32,    // RGBA8888 - environment color
    fog_color: u32,    // RGBA8888 - fog color
    combine_mode: u64, // 64-bit combine mode setting

    /// Z-buffer image address in RDRAM
    z_image_addr: u32,

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
        let mut renderer = Box::new(SoftwareRdpRenderer::new(width, height));
        // Initialize framebuffer to visible dark blue (0xFF000040) so display shows something
        // This helps distinguish "no rendering" from "black rendered content"
        renderer.clear(0xFF000040);

        Self {
            renderer,
            width,
            height,
            color_format: ColorFormat::RGBA5551,
            fill_color: 0xFF000000, // Black with full alpha
            scissor: ScissorBox {
                x_min: 0,
                y_min: 0,
                x_max: width,
                y_max: height,
            },
            tmem: [0; 4096],
            tiles: [TileDescriptor {
                format: 0,
                size: 0,
                line: 0,
                tmem_addr: 0,
                palette: 0,
                s_mask: 0,
                t_mask: 0,
                s_shift: 0,
                t_shift: 0,
            }; 8],
            texture_image_addr: 0,
            blend_color: 0xFF000000, // Black
            prim_color: 0xFFFFFFFF,  // White
            env_color: 0xFF000000,   // Black
            fog_color: 0xFF000000,   // Black
            combine_mode: 0,         // No combine mode
            z_image_addr: 0,
            dpc_start: 0,
            dpc_end: 0,
            dpc_current: 0,
            dpc_status: DPC_STATUS_CBUF_READY, // Start ready for commands
        }
    }

    /// Reset RDP to initial state
    pub fn reset(&mut self) {
        self.renderer.reset();
        // Clear to visible dark blue so we can see the display is active
        self.renderer.clear(0xFF000040);
        self.fill_color = 0xFF000000;
        self.scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: self.width,
            y_max: self.height,
        };
        self.tmem.fill(0);
        self.tiles = [TileDescriptor {
            format: 0,
            size: 0,
            line: 0,
            tmem_addr: 0,
            palette: 0,
            s_mask: 0,
            t_mask: 0,
            s_shift: 0,
            t_shift: 0,
        }; 8];
        self.texture_image_addr = 0;
        self.blend_color = 0xFF000000;
        self.prim_color = 0xFFFFFFFF;
        self.env_color = 0xFF000000;
        self.fog_color = 0xFF000000;
        self.combine_mode = 0;
        self.z_image_addr = 0;
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
        self.renderer.clear(self.fill_color);
    }

    /// Fill a rectangle with the current fill color
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.renderer
            .fill_rect(x, y, width, height, self.fill_color, &self.scissor);
    }

    /// Set a pixel at the given coordinates
    #[allow(dead_code)] // Used in tests and reserved for future display list commands
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        self.renderer.set_pixel(x, y, color);
    }

    /// Clear the Z-buffer to maximum depth (far plane)
    #[allow(dead_code)] // Public API for future use
    pub fn clear_zbuffer(&mut self) {
        self.renderer.clear_zbuffer();
    }

    /// Enable or disable Z-buffer testing
    #[allow(dead_code)] // Public API for future use
    pub fn set_zbuffer_enabled(&mut self, enabled: bool) {
        self.renderer.set_zbuffer_enabled(enabled);
    }

    /// Draw a flat-shaded triangle (basic rasterization)
    /// This is a simplified implementation for basic 3D rendering
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)] // Triangle vertices need 6 coordinates + color
    fn draw_triangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        self.renderer
            .draw_triangle(x0, y0, x1, y1, x2, y2, color, &self.scissor);
    }

    /// Draw a flat-shaded triangle with Z-buffer support
    /// depth values are u16 (0 = near, 0xFFFF = far)
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        x1: i32,
        y1: i32,
        z1: u16,
        x2: i32,
        y2: i32,
        z2: u16,
        color: u32,
    ) {
        self.renderer.draw_triangle_zbuffer(
            x0,
            y0,
            z0,
            x1,
            y1,
            z1,
            x2,
            y2,
            z2,
            color,
            &self.scissor,
        );
    }

    /// Draw a Gouraud-shaded triangle (per-vertex color interpolation)
    /// Colors are RGBA8888 format
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle_shaded(
        &mut self,
        x0: i32,
        y0: i32,
        c0: u32,
        x1: i32,
        y1: i32,
        c1: u32,
        x2: i32,
        y2: i32,
        c2: u32,
    ) {
        self.renderer
            .draw_triangle_shaded(x0, y0, c0, x1, y1, c1, x2, y2, c2, &self.scissor);
    }

    /// Draw a Gouraud-shaded triangle with Z-buffer support
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle_shaded_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        c0: u32,
        x1: i32,
        y1: i32,
        z1: u16,
        c1: u32,
        x2: i32,
        y2: i32,
        z2: u16,
        c2: u32,
    ) {
        self.renderer.draw_triangle_shaded_zbuffer(
            x0,
            y0,
            z0,
            c0,
            x1,
            y1,
            z1,
            c1,
            x2,
            y2,
            z2,
            c2,
            &self.scissor,
        );
    }

    /// Get the current framebuffer
    pub fn get_frame(&self) -> &Frame {
        self.renderer.get_frame()
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
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "N64 RDP: Write register offset=0x{:02X} value=0x{:08X}",
                offset, value
            )
        });

        match offset {
            DPC_START => {
                self.dpc_start = value & 0x00FFFFFF;
            }
            DPC_END => {
                self.dpc_end = value & 0x00FFFFFF;
                // Set flag to indicate display list needs processing (only if there's work to do)
                // The bus will call process_display_list after this write
                if self.dpc_start != self.dpc_end {
                    self.dpc_status &= !DPC_STATUS_CBUF_READY;
                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "N64 RDP: Display list queued, start=0x{:08X} end=0x{:08X}",
                            self.dpc_start, self.dpc_end
                        )
                    });
                }
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

    /// Check if display list processing is needed
    pub fn needs_processing(&self) -> bool {
        (self.dpc_status & DPC_STATUS_CBUF_READY) == 0 && self.dpc_start != self.dpc_end
    }

    /// Set DPC_START register (for RSP to trigger display list processing)
    pub fn set_dpc_start(&mut self, addr: u32) {
        self.dpc_start = addr & 0x00FFFFFF;
    }

    /// Set DPC_END register (for RSP to trigger display list processing)
    pub fn set_dpc_end(&mut self, addr: u32) {
        self.dpc_end = addr & 0x00FFFFFF;
        if self.dpc_start != self.dpc_end {
            self.dpc_status &= !DPC_STATUS_CBUF_READY;
        }
    }

    /// Process display list commands from RDRAM
    pub fn process_display_list(&mut self, rdram: &[u8]) {
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "N64 RDP: Processing display list from 0x{:08X} to 0x{:08X}",
                self.dpc_start, self.dpc_end
            )
        });

        // Set DMA busy flag
        self.dpc_status |= DPC_STATUS_DMA_BUSY;
        self.dpc_status &= !DPC_STATUS_CBUF_READY;

        // Process commands from dpc_start to dpc_end
        let mut addr = self.dpc_start as usize;
        let end = self.dpc_end as usize;

        while addr < end && addr + 7 < rdram.len() {
            // Read 64-bit command (8 bytes)
            let cmd_word0 = u32::from_be_bytes([
                rdram[addr],
                rdram[addr + 1],
                rdram[addr + 2],
                rdram[addr + 3],
            ]);
            let cmd_word1 = u32::from_be_bytes([
                rdram[addr + 4],
                rdram[addr + 5],
                rdram[addr + 6],
                rdram[addr + 7],
            ]);

            // Extract command ID from top 6 bits of first word
            let cmd_id = (cmd_word0 >> 24) & 0x3F;

            log(LogCategory::PPU, LogLevel::Trace, || {
                format!(
                    "N64 RDP: Command 0x{:02X} at 0x{:08X}: word0=0x{:08X} word1=0x{:08X}",
                    cmd_id, addr, cmd_word0, cmd_word1
                )
            });

            // Execute command
            self.execute_rdp_command(cmd_id, cmd_word0, cmd_word1, rdram);

            // Move to next command (all RDP commands are 8 bytes)
            addr += 8;
        }

        // Update current pointer and clear busy flags
        self.dpc_current = self.dpc_end;
        self.dpc_status |= DPC_STATUS_CBUF_READY;
        self.dpc_status &= !DPC_STATUS_DMA_BUSY;

        log(LogCategory::PPU, LogLevel::Debug, || {
            "N64 RDP: Display list processing completed".to_string()
        });
    }

    /// Load texture data from RDRAM to TMEM (used by LOAD_BLOCK and LOAD_TILE)
    fn load_texture_to_tmem(
        &mut self,
        rdram: &[u8],
        tile: usize,
        s_start: u32,
        t_start: u32,
        s_end: u32,
        t_end: u32,
    ) {
        if tile >= 8 {
            return;
        }

        let tile_desc = &self.tiles[tile];
        let size = tile_desc.size;
        let tmem_addr = (tile_desc.tmem_addr * 8) as usize; // Convert from 64-bit words to bytes

        // Calculate bytes per texel based on size
        let bytes_per_texel = match size {
            0 => 1, // 4-bit (actually 0.5, but we'll handle specially)
            1 => 1, // 8-bit
            2 => 2, // 16-bit
            3 => 4, // 32-bit
            _ => 2,
        };

        // Width and height in texels
        let width = (s_end - s_start + 1) as usize;
        let height = (t_end - t_start + 1) as usize;

        // Source address in RDRAM
        let src_addr = self.texture_image_addr as usize;

        // For LOAD_BLOCK and LOAD_TILE, we copy data linearly
        // Calculate total bytes to copy
        let total_texels = width * height;
        let total_bytes = if size == 0 {
            total_texels.div_ceil(2) // 4-bit: two texels per byte
        } else {
            total_texels * bytes_per_texel
        };

        // Bounds check and copy
        let bytes_to_copy = total_bytes.min(self.tmem.len() - tmem_addr);
        let src_end = (src_addr + bytes_to_copy).min(rdram.len());

        if src_addr < rdram.len() && tmem_addr < self.tmem.len() {
            let actual_bytes = (src_end - src_addr).min(bytes_to_copy);
            self.tmem[tmem_addr..tmem_addr + actual_bytes]
                .copy_from_slice(&rdram[src_addr..src_addr + actual_bytes]);
        }
    }

    /// Sample a texel from TMEM at the given tile and texture coordinates
    /// Returns RGBA8888 color
    #[allow(dead_code)] // Will be used for textured triangle rendering
    fn sample_texture(&self, tile: usize, s: u32, t: u32) -> u32 {
        if tile >= 8 {
            return 0xFFFF00FF; // Return magenta for invalid tile (debugging)
        }

        let tile_desc = &self.tiles[tile];
        let format = tile_desc.format;
        let size = tile_desc.size;
        let tmem_addr = (tile_desc.tmem_addr * 8) as usize;

        // Apply wrapping/clamping based on mask
        let s_masked = if tile_desc.s_mask > 0 {
            s & ((1 << tile_desc.s_mask) - 1)
        } else {
            s
        };
        let t_masked = if tile_desc.t_mask > 0 {
            t & ((1 << tile_desc.t_mask) - 1)
        } else {
            t
        };

        // Calculate bytes per texel
        let bytes_per_texel = match size {
            0 => 0.5, // 4-bit (two texels per byte)
            1 => 1.0, // 8-bit
            2 => 2.0, // 16-bit
            3 => 4.0, // 32-bit
            _ => 2.0,
        };

        // Calculate texel offset
        let line_width = tile_desc.line * 8; // Convert from 64-bit words to bytes
        let texel_offset = if size == 0 {
            // 4-bit texels: special handling
            (t_masked * line_width + s_masked / 2) as usize
        } else {
            (t_masked * line_width + s_masked * bytes_per_texel as u32) as usize
        };
        let addr = tmem_addr + texel_offset;

        // Sample based on format and size
        if addr >= self.tmem.len() {
            return 0xFFFF00FF; // Magenta for out of bounds
        }

        match (format, size) {
            // RGBA 16-bit (5-5-5-1)
            (0, 2) => {
                if addr + 1 < self.tmem.len() {
                    let texel = u16::from_be_bytes([self.tmem[addr], self.tmem[addr + 1]]);
                    let r = ((texel >> 11) & 0x1F) as u32;
                    let g = ((texel >> 6) & 0x1F) as u32;
                    let b = ((texel >> 1) & 0x1F) as u32;
                    let a = (texel & 0x01) as u32;
                    // Convert 5-bit to 8-bit
                    let r8 = (r * 255 / 31) & 0xFF;
                    let g8 = (g * 255 / 31) & 0xFF;
                    let b8 = (b * 255 / 31) & 0xFF;
                    let a8 = if a != 0 { 0xFF } else { 0x00 };
                    (a8 << 24) | (r8 << 16) | (g8 << 8) | b8
                } else {
                    0xFFFF00FF
                }
            }
            // RGBA 32-bit (8-8-8-8)
            (0, 3) => {
                if addr + 3 < self.tmem.len() {
                    u32::from_be_bytes([
                        self.tmem[addr],
                        self.tmem[addr + 1],
                        self.tmem[addr + 2],
                        self.tmem[addr + 3],
                    ])
                } else {
                    0xFFFF00FF
                }
            }
            // CI (Color Index) 8-bit (format=2, size=1)
            (2, 1) => {
                // Color index format - lookup in palette
                let index = self.tmem[addr] as usize;
                // Palette is stored in upper half of TMEM (0x800-0xFFF)
                let palette_offset = 0x800 + (tile_desc.palette as usize * 16 * 2);
                let color_addr = palette_offset + (index * 2);

                if color_addr + 1 < self.tmem.len() {
                    let texel =
                        u16::from_be_bytes([self.tmem[color_addr], self.tmem[color_addr + 1]]);
                    let r = ((texel >> 11) & 0x1F) as u32;
                    let g = ((texel >> 6) & 0x1F) as u32;
                    let b = ((texel >> 1) & 0x1F) as u32;
                    let a = (texel & 0x01) as u32;
                    let r8 = (r * 255 / 31) & 0xFF;
                    let g8 = (g * 255 / 31) & 0xFF;
                    let b8 = (b * 255 / 31) & 0xFF;
                    let a8 = if a != 0 { 0xFF } else { 0x00 };
                    (a8 << 24) | (r8 << 16) | (g8 << 8) | b8
                } else {
                    0xFFFF00FF
                }
            }
            // CI (Color Index) 4-bit (format=2, size=0)
            (2, 0) => {
                // 4-bit color index - two texels per byte
                let byte = self.tmem[addr];
                let index = if s & 1 == 0 {
                    (byte >> 4) & 0x0F // High nibble
                } else {
                    byte & 0x0F // Low nibble
                } as usize;

                let palette_offset = 0x800 + (tile_desc.palette as usize * 16 * 2);
                let color_addr = palette_offset + (index * 2);

                if color_addr + 1 < self.tmem.len() {
                    let texel =
                        u16::from_be_bytes([self.tmem[color_addr], self.tmem[color_addr + 1]]);
                    let r = ((texel >> 11) & 0x1F) as u32;
                    let g = ((texel >> 6) & 0x1F) as u32;
                    let b = ((texel >> 1) & 0x1F) as u32;
                    let a = (texel & 0x01) as u32;
                    let r8 = (r * 255 / 31) & 0xFF;
                    let g8 = (g * 255 / 31) & 0xFF;
                    let b8 = (b * 255 / 31) & 0xFF;
                    let a8 = if a != 0 { 0xFF } else { 0x00 };
                    (a8 << 24) | (r8 << 16) | (g8 << 8) | b8
                } else {
                    0xFFFF00FF
                }
            }
            // IA (Intensity + Alpha) 16-bit (format=3, size=2)
            (3, 2) => {
                if addr + 1 < self.tmem.len() {
                    let texel = u16::from_be_bytes([self.tmem[addr], self.tmem[addr + 1]]);
                    let intensity = ((texel >> 8) & 0xFF) as u32;
                    let alpha = (texel & 0xFF) as u32;
                    (alpha << 24) | (intensity << 16) | (intensity << 8) | intensity
                } else {
                    0xFFFF00FF
                }
            }
            // IA (Intensity + Alpha) 8-bit (format=3, size=1)
            (3, 1) => {
                let texel = self.tmem[addr];
                let intensity = ((texel >> 4) & 0x0F) as u32;
                let alpha = (texel & 0x0F) as u32;
                let i8 = (intensity * 255 / 15) & 0xFF;
                let a8 = (alpha * 255 / 15) & 0xFF;
                (a8 << 24) | (i8 << 16) | (i8 << 8) | i8
            }
            // IA (Intensity + Alpha) 4-bit (format=3, size=0)
            (3, 0) => {
                let byte = self.tmem[addr];
                let texel = if s & 1 == 0 {
                    (byte >> 4) & 0x0F
                } else {
                    byte & 0x0F
                };
                let intensity = ((texel >> 1) & 0x07) as u32;
                let alpha = (texel & 0x01) as u32;
                let i8 = (intensity * 255 / 7) & 0xFF;
                let a8 = if alpha != 0 { 0xFF } else { 0x00 };
                (a8 << 24) | (i8 << 16) | (i8 << 8) | i8
            }
            // I (Intensity) 8-bit (format=4, size=1)
            (4, 1) => {
                let intensity = self.tmem[addr] as u32;
                0xFF000000 | (intensity << 16) | (intensity << 8) | intensity
            }
            // I (Intensity) 4-bit (format=4, size=0)
            (4, 0) => {
                let byte = self.tmem[addr];
                let nibble = if s & 1 == 0 {
                    (byte >> 4) & 0x0F
                } else {
                    byte & 0x0F
                };
                let intensity = (nibble * 255 / 15) as u32;
                0xFF000000 | (intensity << 16) | (intensity << 8) | intensity
            }
            // Other formats not yet implemented - return white
            _ => 0xFFFFFFFF,
        }
    }

    /// Execute a single RDP command
    pub fn execute_rdp_command(&mut self, cmd_id: u32, word0: u32, word1: u32, rdram: &[u8]) {
        match cmd_id {
            // Triangle commands (0x08-0x0F)
            // Note: Real N64 triangle commands have complex formats with edge coefficients
            // This is a simplified implementation using a custom packed format for basic 3D rendering

            // Non-shaded triangle (0x08)
            0x08 => {
                // Simplified custom format for testing (not real RDP format):
                // word0: bits 23-12 = x0, bits 11-0 = y0
                // word1: bits 31-24 = x1, bits 23-16 = y1, bits 15-8 = x2, bits 7-0 = y2
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                self.draw_triangle(x0, y0, x1, y1, x2, y2, self.fill_color);
            }
            // Non-shaded triangle with Z-buffer (0x09)
            0x09 => {
                // Simplified custom format (not real RDP format):
                // Similar to 0x08 but assumes mid-range depth for all vertices
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                // Use mid-range depth (0x8000) for all vertices in simplified format
                self.draw_triangle_zbuffer(
                    x0,
                    y0,
                    0x8000,
                    x1,
                    y1,
                    0x8000,
                    x2,
                    y2,
                    0x8000,
                    self.fill_color,
                );
            }
            // Shaded triangle (0x0C)
            0x0C => {
                // Simplified custom format (not real RDP format):
                // For testing, extract coordinates from word0/word1
                // and use fill_color with slight variations for each vertex
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                // Create color variations for Gouraud shading demonstration
                let c0 = self.fill_color;
                let c1 = ColorOps::adjust_brightness(self.fill_color, 0.8);
                let c2 = ColorOps::adjust_brightness(self.fill_color, 0.6);

                self.draw_triangle_shaded(x0, y0, c0, x1, y1, c1, x2, y2, c2);
            }
            // Shaded triangle with Z-buffer (0x0D)
            0x0D => {
                // Simplified custom format (not real RDP format):
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                // Create color variations
                let c0 = self.fill_color;
                let c1 = ColorOps::adjust_brightness(self.fill_color, 0.8);
                let c2 = ColorOps::adjust_brightness(self.fill_color, 0.6);

                self.draw_triangle_shaded_zbuffer(
                    x0, y0, 0x8000, c0, x1, y1, 0x8000, c1, x2, y2, 0x8000, c2,
                );
            }
            // Textured triangle (0x0A) - custom format for texture mapping demo
            0x0A => {
                // Simplified custom format:
                // word0: bits 23-12 = x0, bits 11-0 = y0
                // word1: bits 31-24 = x1, bits 23-16 = y1, bits 15-8 = x2, bits 7-0 = y2
                // Texture coordinates: simple mapping based on vertex position
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                // Generate simple texture coordinates based on vertex positions
                // Use normalized coordinates [0, 1] based on screen position
                let s0 = (x0 as f32 / self.width as f32) * 64.0; // Scale to texture size
                let t0 = (y0 as f32 / self.height as f32) * 64.0;
                let s1 = (x1 as f32 / self.width as f32) * 64.0;
                let t1 = (y1 as f32 / self.height as f32) * 64.0;
                let s2 = (x2 as f32 / self.width as f32) * 64.0;
                let t2 = (y2 as f32 / self.height as f32) * 64.0;

                // Use tile 0 for texture sampling
                let tile = 0;
                let rdp_ref = self as *const Self;
                let texture_sampler = move |s: f32, t: f32| -> u32 {
                    unsafe { (*rdp_ref).sample_texture(tile, s as u32, t as u32) }
                };

                self.renderer.draw_triangle_textured(
                    x0,
                    y0,
                    s0,
                    t0,
                    x1,
                    y1,
                    s1,
                    t1,
                    x2,
                    y2,
                    s2,
                    t2,
                    &texture_sampler,
                    &self.scissor,
                );
            }
            // Textured triangle with Z-buffer (0x0B)
            0x0B => {
                // Similar to 0x0A but with Z-buffer testing
                let x0 = ((word0 >> 12) & 0xFFF) as i32;
                let y0 = (word0 & 0xFFF) as i32;
                let x1 = ((word1 >> 24) & 0xFF) as i32;
                let y1 = ((word1 >> 16) & 0xFF) as i32;
                let x2 = ((word1 >> 8) & 0xFF) as i32;
                let y2 = (word1 & 0xFF) as i32;

                // Generate texture coordinates
                let s0 = (x0 as f32 / self.width as f32) * 64.0;
                let t0 = (y0 as f32 / self.height as f32) * 64.0;
                let s1 = (x1 as f32 / self.width as f32) * 64.0;
                let t1 = (y1 as f32 / self.height as f32) * 64.0;
                let s2 = (x2 as f32 / self.width as f32) * 64.0;
                let t2 = (y2 as f32 / self.height as f32) * 64.0;

                // Mid-range depth for all vertices
                let z0 = 0x8000u16;
                let z1 = 0x8000u16;
                let z2 = 0x8000u16;

                // Use tile 0 for texture sampling
                let tile = 0;
                let rdp_ref = self as *const Self;
                let texture_sampler = move |s: f32, t: f32| -> u32 {
                    unsafe { (*rdp_ref).sample_texture(tile, s as u32, t as u32) }
                };

                self.renderer.draw_triangle_textured_zbuffer(
                    x0,
                    y0,
                    z0,
                    s0,
                    t0,
                    x1,
                    y1,
                    z1,
                    s1,
                    t1,
                    x2,
                    y2,
                    z2,
                    s2,
                    t2,
                    &texture_sampler,
                    &self.scissor,
                );
            }
            // FILL_RECTANGLE (0x36)
            0x36 => {
                // RDP FILL_RECTANGLE format (verified from test ROM):
                // word0: cmd_id(bits 31-24) | XH(bits 23-12) | YH(bits 11-0) - END coordinates
                // word1: XL(bits 23-12) | YL(bits 11-0) - START coordinates
                // Coordinates are in 10.2 fixed point format (divide by 4 to get pixels)

                let xh = ((word0 >> 12) & 0xFFF).div_ceil(4); // Right/end X, round up
                let yh = (word0 & 0xFFF).div_ceil(4); // Bottom/end Y, round up
                let xl = ((word1 >> 12) & 0xFFF) / 4; // Left/start X
                let yl = (word1 & 0xFFF) / 4; // Top/start Y

                // Calculate width and height
                let width = xh.saturating_sub(xl);
                let height = yh.saturating_sub(yl);

                self.fill_rect(xl, yl, width, height);
            }
            // SET_FILL_COLOR (0x37)
            0x37 => {
                // word1 contains the fill color (RGBA)
                self.fill_color = word1;
            }
            // SET_SCISSOR (0x2D)
            0x2D => {
                // word0: bits 23-12 = XH (right), bits 11-0 = YH (bottom) in 10.2 fixed point
                // word1: bits 23-12 = XL (left), bits 11-0 = YL (top) in 10.2 fixed point
                let x_max = ((word0 >> 12) & 0xFFF) / 4;
                let y_max = (word0 & 0xFFF) / 4;
                let x_min = ((word1 >> 12) & 0xFFF) / 4;
                let y_min = (word1 & 0xFFF) / 4;

                self.scissor = ScissorBox {
                    x_min,
                    y_min,
                    x_max,
                    y_max,
                };
            }
            // TEXTURE_RECTANGLE (0x24)
            0x24 => {
                // Texture rectangle command - for now, just fill with fill color
                // Real implementation would load and sample texture from TMEM
                // word0: cmd | XH(12) | YH(12)
                // word1: tile(3) | XL(12) | YL(12)
                // This is a basic stub implementation
                let xh = ((word0 >> 12) & 0xFFF) / 4;
                let yh = (word0 & 0xFFF) / 4;
                let xl = ((word1 >> 12) & 0xFFF) / 4;
                let yl = (word1 & 0xFFF) / 4;
                let tile = (word1 >> 24) & 0x07;

                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!(
                        "N64 RDP: TEXTURE_RECTANGLE stub - rendering as solid fill (xl={}, yl={}, xh={}, yh={}, tile={})",
                        xl, yl, xh, yh, tile
                    )
                });

                let width = xh.saturating_sub(xl);
                let height = yh.saturating_sub(yl);

                // Stub: render as solid rectangle with current fill color
                self.fill_rect(xl, yl, width, height);
            }
            // SET_OTHER_MODES (0x2F - full 64-bit command)
            0x2F => {
                // Configure rendering modes (cycle type, alpha blend, Z-buffer, etc.)
                // For now, we just accept and ignore these settings
                // Full implementation would configure the rendering pipeline
                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!(
                        "N64 RDP: SET_OTHER_MODES stub - ignoring rendering mode configuration (word0=0x{:08X}, word1=0x{:08X})",
                        word0, word1
                    )
                });
            }
            // SET_TILE (0x35)
            0x35 => {
                // Configure tile descriptor (texture format, size, palette)
                // word0: cmd | format(3) | size(2) | line(9) | tmem_addr(9)
                // word1: tile(3) | palette(4) | ct(1) | mt(1) | mask_t(4) | shift_t(4) | cs(1) | ms(1) | mask_s(4) | shift_s(4)
                let format = (word0 >> 21) & 0x07;
                let size = (word0 >> 19) & 0x03;
                let line = (word0 >> 9) & 0x1FF;
                let tmem_addr = word0 & 0x1FF;

                let tile_num = ((word1 >> 24) & 0x07) as usize;
                let palette = (word1 >> 20) & 0x0F;
                let mask_t = (word1 >> 14) & 0x0F;
                let shift_t = (word1 >> 10) & 0x0F;
                let mask_s = (word1 >> 4) & 0x0F;
                let shift_s = word1 & 0x0F;

                if tile_num < 8 {
                    self.tiles[tile_num] = TileDescriptor {
                        format,
                        size,
                        line,
                        tmem_addr,
                        palette,
                        s_mask: mask_s,
                        t_mask: mask_t,
                        s_shift: shift_s,
                        t_shift: shift_t,
                    };
                }
            }
            // SET_TEXTURE_IMAGE (0x3D)
            0x3D => {
                // Set source address for texture loading
                // word0: cmd | format(3) | size(2) | width(10)
                // word1: DRAM address
                self.texture_image_addr = word1 & 0x00FFFFFF;
            }
            // LOAD_BLOCK (0x33)
            0x33 => {
                // Load texture block from DRAM to TMEM
                // word0: cmd | uls(12) | ult(12)
                // word1: tile(3) | texels(12) | dxt(12)
                let uls = (word0 >> 12) & 0xFFF; // S coordinate (start)
                let ult = word0 & 0xFFF; // T coordinate (start)
                let tile = ((word1 >> 24) & 0x07) as usize;
                let texels = (word1 >> 12) & 0xFFF; // Number of texels to load

                // Calculate end coordinates based on texels count
                // LOAD_BLOCK loads a linear block of texels
                let lrs = uls + texels;
                let lrt = ult;

                // Load texture data from RDRAM to TMEM
                self.load_texture_to_tmem(rdram, tile, uls, ult, lrs, lrt);
            }
            // LOAD_TILE (0x34)
            0x34 => {
                // Load texture tile from DRAM to TMEM
                // word0: cmd | uls(12) | ult(12)
                // word1: tile(3) | lrs(12) | lrt(12)
                let uls = (word0 >> 12) & 0xFFF; // S coordinate (start)
                let ult = word0 & 0xFFF; // T coordinate (start)
                let tile = ((word1 >> 24) & 0x07) as usize;
                let lrs = (word1 >> 12) & 0xFFF; // S coordinate (end)
                let lrt = word1 & 0xFFF; // T coordinate (end)

                // Load texture data from RDRAM to TMEM
                self.load_texture_to_tmem(rdram, tile, uls, ult, lrs, lrt);
            }
            // SYNC_FULL (0x29)
            0x29 => {
                // Full synchronization - wait for all rendering to complete
                // In frame-based implementation, this is a no-op
            }
            // SET_FOG_COLOR (0x38)
            0x38 => {
                // word1 contains the fog color (RGBA8888)
                self.fog_color = word1;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_FOG_COLOR = 0x{:08X}", word1)
                });
            }
            // SET_BLEND_COLOR (0x39)
            0x39 => {
                // word1 contains the blend color (RGBA8888)
                self.blend_color = word1;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_BLEND_COLOR = 0x{:08X}", word1)
                });
            }
            // SET_PRIM_COLOR (0x3A)
            0x3A => {
                // word0: cmd | min_level(8) | prim_level(8)
                // word1: primitive color (RGBA8888)
                self.prim_color = word1;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_PRIM_COLOR = 0x{:08X}", word1)
                });
            }
            // SET_ENV_COLOR (0x3B)
            0x3B => {
                // word1 contains the environment color (RGBA8888)
                self.env_color = word1;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_ENV_COLOR = 0x{:08X}", word1)
                });
            }
            // SET_COMBINE_MODE (0x3C)
            0x3C => {
                // 64-bit combine mode command
                // word0 and word1 together form the combine mode settings
                self.combine_mode = ((word0 as u64) << 32) | (word1 as u64);
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_COMBINE_MODE = 0x{:08X}{:08X}", word0, word1)
                });
            }
            // SET_Z_IMAGE (0x3E)
            0x3E => {
                // word1: DRAM address of Z-buffer
                self.z_image_addr = word1 & 0x00FFFFFF;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 RDP: SET_Z_IMAGE = 0x{:08X}", self.z_image_addr)
                });
            }
            // SET_COLOR_IMAGE (0x3F)
            0x3F => {
                // word0: bits 21-19 = format, bits 18-17 = size, bits 11-0 = width-1
                // word1: DRAM address of color buffer
                // For now, we ignore this and use our internal framebuffer
            }
            // SYNC_PIPE (0x27), SYNC_TILE (0x28), SYNC_LOAD (0x26)
            0x26..=0x28 => {
                // Synchronization commands - no-op in frame-based implementation
            }
            _ => {
                // Unknown or unimplemented command - log warning
                log(LogCategory::Stubs, LogLevel::Warn, || {
                    format!(
                        "N64 RDP: Unknown RDP command: 0x{:02X} (word0=0x{:08X}, word1=0x{:08X})",
                        cmd_id, word0, word1
                    )
                });
            }
        }
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
        assert_eq!(rdp.get_frame().width, 320);
        assert_eq!(rdp.get_frame().height, 240);
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
        for pixel in &rdp.get_frame().pixels {
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
                assert_eq!(rdp.get_frame().pixels[idx], 0xFF00FF00);
            }
        }

        // Check pixel outside rectangle is black (default)
        assert_eq!(rdp.get_frame().pixels[0], 0);
    }

    #[test]
    fn test_rdp_set_pixel() {
        let mut rdp = Rdp::new();
        rdp.set_pixel(100, 100, 0xFFFFFFFF); // White

        let idx = (100 * 320 + 100) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFFFFFFFF);
    }

    #[test]
    fn test_rdp_reset() {
        let mut rdp = Rdp::new();
        rdp.set_fill_color(0xFFFF0000);
        rdp.clear();

        rdp.reset();

        // After reset, should be back to black
        assert_eq!(rdp.get_frame().pixels[0], 0);
        assert_eq!(rdp.fill_color, 0xFF000000);
    }

    #[test]
    fn test_rdp_registers() {
        let mut rdp = Rdp::new();

        // Test DPC_START register
        rdp.write_register(DPC_START, 0x00100000);
        assert_eq!(rdp.read_register(DPC_START), 0x00100000);

        // Test DPC_END register (same as start, so no display list processing)
        rdp.write_register(DPC_END, 0x00100000);
        assert_eq!(rdp.read_register(DPC_END), 0x00100000);

        // Test DPC_STATUS register - should still have CBUF_READY since start == end
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

    #[test]
    fn test_rdp_display_list_fill_rect() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 1024];

        // Create a display list with FILL_RECTANGLE command
        // Fill a 100x100 rectangle at (50, 50)

        // SET_FILL_COLOR (0x37) - Red color
        let set_color_cmd = 0x37000000u32; // Command ID in top bits
        let color = 0xFFFF0000u32; // RGBA red
        rdram[0..4].copy_from_slice(&set_color_cmd.to_be_bytes());
        rdram[4..8].copy_from_slice(&color.to_be_bytes());

        // FILL_RECTANGLE (0x36)
        // Format: word0: cmd(8) | XH(12) | YH(12), word1: XL(12) | YL(12)
        // Coordinates in 10.2 fixed point: 50*4=0xC8, 150*4=0x258
        let rect_cmd_word0: u32 = (0x36 << 24) | (0x258 << 12) | 0x258; // XH=150, YH=150
        let rect_cmd_word1: u32 = (0xC8 << 12) | 0xC8; // XL=50, YL=50
        rdram[8..12].copy_from_slice(&rect_cmd_word0.to_be_bytes());
        rdram[12..16].copy_from_slice(&rect_cmd_word1.to_be_bytes());

        // Set up RDP registers to point to display list
        rdp.write_register(0x00, 0); // DPC_START = 0
        rdp.write_register(0x04, 16); // DPC_END = 16 (2 commands * 8 bytes)

        // Process the display list
        rdp.process_display_list(&rdram);

        // Verify the rectangle was filled
        // Check a pixel inside the rectangle (75, 75)
        let idx = (75 * 320 + 75) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFFFF0000);

        // Check a pixel outside the rectangle
        assert_eq!(rdp.get_frame().pixels[0], 0);
    }

    #[test]
    fn test_rdp_display_list_sync_commands() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 64];

        // Create a display list with sync commands
        // SYNC_FULL (0x29)
        let sync_full = 0x29000000u32;
        rdram[0..4].copy_from_slice(&sync_full.to_be_bytes());
        rdram[4..8].copy_from_slice(&[0, 0, 0, 0]);

        // SYNC_PIPE (0x27)
        let sync_pipe = 0x27000000u32;
        rdram[8..12].copy_from_slice(&sync_pipe.to_be_bytes());
        rdram[12..16].copy_from_slice(&[0, 0, 0, 0]);

        rdp.write_register(0x00, 0); // DPC_START
        rdp.write_register(0x04, 16); // DPC_END

        // Should not panic
        rdp.process_display_list(&rdram);

        // Verify status updated correctly
        assert_eq!(rdp.dpc_current, 16);
        assert!(rdp.read_register(0x0C) & DPC_STATUS_CBUF_READY != 0);
    }

    #[test]
    fn test_rdp_needs_processing() {
        let mut rdp = Rdp::new();

        // Initially should not need processing
        assert!(!rdp.needs_processing());

        // Set start and end
        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);

        // Now should need processing (CBUF_READY cleared when END written)
        assert!(rdp.needs_processing());

        // After processing, should not need it anymore
        let rdram = vec![0u8; 64];
        rdp.process_display_list(&rdram);
        assert!(!rdp.needs_processing());
    }

    #[test]
    fn test_rdp_scissor_command() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 64];

        // Create a display list with SET_SCISSOR command
        // SET_SCISSOR (0x2D) - set to (10,10) to (100,100)
        // Format: word0: cmd(8) | XH(12 bits at bit 12) | YH(12 bits)
        //         word1: XL(12 bits at bit 12) | YL(12 bits)
        // Coordinates in 10.2 fixed point: multiply by 4
        let set_scissor_cmd: u32 = (0x2D << 24) | ((100 * 4) << 12) | (100 * 4);
        let set_scissor_data: u32 = ((10 * 4) << 12) | (10 * 4);
        rdram[0..4].copy_from_slice(&set_scissor_cmd.to_be_bytes());
        rdram[4..8].copy_from_slice(&set_scissor_data.to_be_bytes());

        // SET_FILL_COLOR - Red
        rdram[8..12].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[12..16].copy_from_slice(&0xFFFF0000u32.to_be_bytes());

        // FILL_RECTANGLE covering (5,5) to (150,150)
        // Should be clipped to scissor bounds (10,10) to (100,100)
        let rect_cmd: u32 = (0x36 << 24) | ((150 * 4) << 12) | (150 * 4);
        let rect_data: u32 = ((5 * 4) << 12) | (5 * 4);
        rdram[16..20].copy_from_slice(&rect_cmd.to_be_bytes());
        rdram[20..24].copy_from_slice(&rect_data.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 24);
        rdp.process_display_list(&rdram);

        // Check that pixels inside scissor bounds are red
        let idx_inside = (50 * 320 + 50) as usize;
        assert_eq!(rdp.get_frame().pixels[idx_inside], 0xFFFF0000);

        // Check that pixels outside scissor bounds (but inside rectangle) are still black
        let idx_outside = (5 * 320 + 5) as usize;
        assert_eq!(rdp.get_frame().pixels[idx_outside], 0);

        // Check another pixel outside scissor (105, 105) - outside max bounds
        let idx_outside2 = (105 * 320 + 105) as usize;
        assert_eq!(rdp.get_frame().pixels[idx_outside2], 0);
    }

    #[test]
    fn test_rdp_triangle_rendering() {
        let mut rdp = Rdp::new();

        // Draw a simple triangle
        rdp.set_fill_color(0xFF00FF00); // Green
        rdp.draw_triangle(100, 50, 150, 150, 50, 150, 0xFF00FF00);

        // Check that some pixels in the triangle are green
        // Center of triangle should be around (100, 116)
        let idx = (116 * 320 + 100) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFF00FF00);
    }

    #[test]
    fn test_rdp_texture_rectangle_stub() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 64];

        // SET_FILL_COLOR - Blue (for stub texture rect)
        rdram[0..4].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[4..8].copy_from_slice(&0xFF0000FFu32.to_be_bytes());

        // TEXTURE_RECTANGLE (0x24) - stub implementation fills with solid color
        // Coordinates: (50,50) to (100,100)
        let tex_rect_cmd: u32 = (0x24 << 24) | ((100 * 4) << 12) | (100 * 4);
        let tex_rect_data: u32 = ((50 * 4) << 12) | (50 * 4); // tile=0, coords
        rdram[8..12].copy_from_slice(&tex_rect_cmd.to_be_bytes());
        rdram[12..16].copy_from_slice(&tex_rect_data.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);
        rdp.process_display_list(&rdram);

        // Verify the rectangle was filled (stub implementation)
        let idx = (75 * 320 + 75) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFF0000FF);
    }

    #[test]
    fn test_rdp_set_tile_command() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 64];

        // SET_TILE (0x35) - configure tile 0
        // format=RGBA(0), size=16bit(2), line=32, tmem_addr=0
        let format = 0u32;
        let size = 2u32;
        let line = 32u32;
        let tmem_addr = 0u32;
        let set_tile_cmd: u32 =
            (0x35 << 24) | (format << 21) | (size << 19) | (line << 9) | tmem_addr;
        // tile=0, palette=0, mask_t=5, shift_t=0, mask_s=5, shift_s=0
        let tile = 0u32;
        let palette = 0u32;
        let mask_t = 5u32;
        let shift_t = 0u32;
        let mask_s = 5u32;
        let shift_s = 0u32;
        let set_tile_data: u32 = (tile << 24)
            | (palette << 20)
            | (mask_t << 14)
            | (shift_t << 10)
            | (mask_s << 4)
            | shift_s;
        rdram[0..4].copy_from_slice(&set_tile_cmd.to_be_bytes());
        rdram[4..8].copy_from_slice(&set_tile_data.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 8);
        rdp.process_display_list(&rdram);

        // Verify tile descriptor was set
        assert_eq!(rdp.tiles[0].format, 0);
        assert_eq!(rdp.tiles[0].size, 2);
        assert_eq!(rdp.tiles[0].line, 32);
        assert_eq!(rdp.tiles[0].tmem_addr, 0);
        assert_eq!(rdp.tiles[0].s_mask, 5);
        assert_eq!(rdp.tiles[0].t_mask, 5);
    }

    #[test]
    fn test_rdp_set_texture_image() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 64];

        // SET_TEXTURE_IMAGE (0x3D)
        // format=RGBA, size=16bit, width=31
        let format = 0u32;
        let size = 2u32;
        let width = 31u32;
        let set_tex_img_cmd: u32 = (0x3D << 24) | (format << 21) | (size << 19) | width;
        let tex_addr: u32 = 0x00200000; // Texture address in RDRAM
        rdram[0..4].copy_from_slice(&set_tex_img_cmd.to_be_bytes());
        rdram[4..8].copy_from_slice(&tex_addr.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 8);
        rdp.process_display_list(&rdram);

        // Verify texture image address was set
        assert_eq!(rdp.texture_image_addr, 0x00200000);
    }

    #[test]
    fn test_rdp_tmem_initialized() {
        let rdp = Rdp::new();

        // Verify TMEM is zero-initialized
        assert_eq!(rdp.tmem.len(), 4096);
        assert!(rdp.tmem.iter().all(|&b| b == 0));

        // Verify tiles are initialized
        for tile in &rdp.tiles {
            assert_eq!(tile.format, 0);
            assert_eq!(tile.size, 0);
        }
    }

    // Note: Z-buffer implementation details are now tested in rdp_renderer_software tests
    // These high-level tests verify the Z-buffer still works correctly via the RDP API

    #[test]
    fn test_rdp_zbuffer_enable_disable() {
        let mut rdp = Rdp::new();
        // Z-buffer should start disabled
        rdp.set_zbuffer_enabled(false);

        // Should be able to enable it
        rdp.set_zbuffer_enabled(true);

        // And disable it again
        rdp.set_zbuffer_enabled(false);
        // Test passes if no panics occur
    }

    #[test]
    fn test_rdp_zbuffer_clear() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);

        // Should be able to clear the Z-buffer
        rdp.clear_zbuffer();
        // Test passes if no panics occur
    }

    #[test]
    fn test_rdp_triangle_zbuffer() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);

        // Draw a triangle with Z-buffer
        rdp.draw_triangle_zbuffer(
            100, 50, 0x8000, // Top vertex
            150, 150, 0x8000, // Bottom-right vertex
            50, 150, 0x8000,     // Bottom-left vertex
            0xFF00FF00, // Green color
        );

        // Check that pixels in the triangle are green
        let idx = (116 * 320 + 100) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFF00FF00);

        // Z-buffer functionality is tested in rdp_renderer_software tests
    }

    #[test]
    fn test_rdp_triangle_zbuffer_occlusion() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);

        // Draw a near triangle (small Z value = close to camera)
        rdp.draw_triangle_zbuffer(
            100, 50, 0x4000, 150, 150, 0x4000, 50, 150, 0x4000, 0xFF00FF00, // Green
        );

        // Draw a far triangle (large Z value = far from camera) at same location
        // This should be occluded by the first triangle
        rdp.draw_triangle_zbuffer(
            100, 50, 0xC000, 150, 150, 0xC000, 50, 150, 0xC000, 0xFFFF0000, // Red
        );

        // Pixel should remain green (near triangle visible)
        let idx = (116 * 320 + 100) as usize;
        assert_eq!(rdp.get_frame().pixels[idx], 0xFF00FF00);
    }

    #[test]
    fn test_rdp_triangle_shaded() {
        let mut rdp = Rdp::new();

        // Draw a triangle with Gouraud shading
        // Top vertex: Red (0xFFFF0000), Bottom vertices: Blue (0xFF0000FF)
        rdp.draw_triangle_shaded(
            100, 50, 0xFFFF0000, // Top: Red
            150, 150, 0xFF0000FF, // Bottom-right: Blue
            50, 150, 0xFF0000FF, // Bottom-left: Blue
        );

        // Check that center pixel has interpolated color (between red and blue = purple)
        let idx = (100 * 320 + 100) as usize;
        let color = rdp.get_frame().pixels[idx];

        // Should have both red and blue components (ARGB format)
        let r = (color >> 16) & 0xFF;
        let b = color & 0xFF;
        assert!(r > 0x00, "Should have red component");
        assert!(b > 0x00, "Should have blue component");
    }

    #[test]
    fn test_rdp_triangle_shaded_zbuffer() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);

        // Draw a shaded triangle with Z-buffer
        rdp.draw_triangle_shaded_zbuffer(
            100, 50, 0x8000, 0xFFFF0000, // Top: Red
            150, 150, 0x8000, 0xFF00FF00, // Bottom-right: Green
            50, 150, 0x8000, 0xFF0000FF, // Bottom-left: Blue
        );

        // Check that triangle was drawn
        let idx = (100 * 320 + 100) as usize;
        let color = rdp.get_frame().pixels[idx];

        // Should have interpolated color components
        assert_ne!(color, 0, "Pixel should be colored");

        // Z-buffer functionality is tested in rdp_renderer_software tests
    }

    #[test]
    fn test_rdp_color_interpolation() {
        // Test linear color interpolation
        // ARGB format: 0xAARRGGBB
        let c0 = 0xFFFF0000; // Red with full alpha
        let c1 = 0xFF0000FF; // Blue with full alpha

        // 50% interpolation should give purple
        let c_mid = ColorOps::lerp(c0, c1, 0.5);
        let a = (c_mid >> 24) & 0xFF;
        let r = (c_mid >> 16) & 0xFF;
        let g = (c_mid >> 8) & 0xFF;
        let b = c_mid & 0xFF;

        assert_eq!(a, 255, "Alpha should be full");
        // Allow for rounding: 127 or 128 are both valid for 50%
        assert!((127..=128).contains(&r), "Red component should be ~50%");
        assert_eq!(g, 0, "Green component should be 0");
        assert!((127..=128).contains(&b), "Blue component should be ~50%");

        // 0% should give c0
        let c_start = ColorOps::lerp(c0, c1, 0.0);
        assert_eq!(c_start, c0);

        // 100% should give c1
        let c_end = ColorOps::lerp(c0, c1, 1.0);
        assert_eq!(c_end, c1);
    }

    #[test]
    fn test_rdp_adjust_brightness() {
        // Test brightness adjustment
        let color = 0xFFFF8040; // ARGB: Full alpha, R=255, G=128, B=64

        // Factor 1.0 should return original color
        let same = ColorOps::adjust_brightness(color, 1.0);
        assert_eq!(same, color);

        // Factor 0.5 should halve RGB values
        let darker = ColorOps::adjust_brightness(color, 0.5);
        let a = (darker >> 24) & 0xFF;
        let r = (darker >> 16) & 0xFF;
        let g = (darker >> 8) & 0xFF;
        let b = darker & 0xFF;
        assert_eq!(a, 255, "Alpha should remain unchanged");
        assert!((127..=128).contains(&r), "Red should be halved (~128)");
        assert_eq!(g, 64, "Green should be halved (64)");
        assert_eq!(b, 32, "Blue should be halved (32)");

        // Factor 2.0 should double but cap at 255
        let brighter = ColorOps::adjust_brightness(0xFF804020, 2.0);
        let r2 = (brighter >> 16) & 0xFF;
        let g2 = (brighter >> 8) & 0xFF;
        let b2 = brighter & 0xFF;
        assert_eq!(r2, 255, "Red should cap at 255");
        assert_eq!(g2, 128, "Green should double (128)");
        assert_eq!(b2, 64, "Blue should double (64)");
    }

    #[test]
    fn test_rdp_triangle_command_0x08() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 1024];

        // SET_FILL_COLOR - Blue
        rdram[0..4].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[4..8].copy_from_slice(&0xFF0000FFu32.to_be_bytes());

        // Triangle command 0x08 (non-shaded triangle)
        // Custom format: word0: bits 23-12 = x0, bits 11-0 = y0
        //                word1: bits 31-24 = x1, bits 23-16 = y1, bits 15-8 = x2, bits 7-0 = y2
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x08 << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[8..12].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[12..16].copy_from_slice(&cmd_word1.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);
        rdp.process_display_list(&rdram);

        // Verify triangle was drawn
        let idx = (100 * 320 + 100) as usize;
        assert_eq!(
            rdp.get_frame().pixels[idx],
            0xFF0000FF,
            "Triangle should be blue"
        );
    }

    #[test]
    fn test_rdp_triangle_command_0x09() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);
        let mut rdram = vec![0u8; 1024];

        // SET_FILL_COLOR - Green
        rdram[0..4].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[4..8].copy_from_slice(&0xFF00FF00u32.to_be_bytes());

        // Triangle command 0x09 (non-shaded triangle with Z-buffer)
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x09 << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[8..12].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[12..16].copy_from_slice(&cmd_word1.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);
        rdp.process_display_list(&rdram);

        // Verify triangle was drawn
        let idx = (100 * 320 + 100) as usize;
        assert_eq!(
            rdp.get_frame().pixels[idx],
            0xFF00FF00,
            "Triangle should be green"
        );

        // Z-buffer functionality is tested in rdp_renderer_software tests
    }

    #[test]
    fn test_rdp_triangle_command_0x0c() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 1024];

        // SET_FILL_COLOR - Red (will be used with brightness variations)
        rdram[0..4].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[4..8].copy_from_slice(&0xFFFF0000u32.to_be_bytes());

        // Triangle command 0x0C (shaded triangle)
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x0C << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[8..12].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[12..16].copy_from_slice(&cmd_word1.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);
        rdp.process_display_list(&rdram);

        // Verify triangle was drawn with shading (should have some red component)
        let idx = (100 * 320 + 100) as usize;
        let pixel = rdp.get_frame().pixels[idx];
        let red = (pixel >> 16) & 0xFF;
        assert!(red > 0, "Triangle should have red component from shading");
    }

    #[test]
    fn test_rdp_triangle_command_0x0d() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);
        let mut rdram = vec![0u8; 1024];

        // SET_FILL_COLOR - Magenta
        rdram[0..4].copy_from_slice(&0x37000000u32.to_be_bytes());
        rdram[4..8].copy_from_slice(&0xFFFF00FFu32.to_be_bytes());

        // Triangle command 0x0D (shaded triangle with Z-buffer)
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x0D << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[8..12].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[12..16].copy_from_slice(&cmd_word1.to_be_bytes());

        rdp.write_register(0x00, 0);
        rdp.write_register(0x04, 16);
        rdp.process_display_list(&rdram);

        // Verify triangle was drawn
        let idx = (100 * 320 + 100) as usize;
        let pixel = rdp.get_frame().pixels[idx];
        assert_ne!(pixel, 0, "Triangle should be colored");

        // Z-buffer functionality is tested in rdp_renderer_software tests
    }

    #[test]
    fn test_rdp_triangle_scissor_clipping() {
        let mut rdp = Rdp::new();

        // Set scissor to small region (50,50) to (150,150)
        rdp.scissor = ScissorBox {
            x_min: 50,
            y_min: 50,
            x_max: 150,
            y_max: 150,
        };

        // Draw triangle that extends beyond scissor region
        rdp.draw_triangle_shaded(
            100, 20, 0xFFFF0000, // Top (outside scissor)
            200, 200, 0xFF00FF00, // Bottom-right (outside scissor)
            0, 200, 0xFF0000FF, // Bottom-left (outside scissor)
        );

        // Pixels outside scissor should be black
        assert_eq!(rdp.get_frame().pixels[(20 * 320 + 100) as usize], 0);
        assert_eq!(rdp.get_frame().pixels[(200 * 320 + 200) as usize], 0);

        // Pixel inside scissor should be colored
        let idx = (100 * 320 + 100) as usize;
        assert_ne!(rdp.get_frame().pixels[idx], 0);
    }

    #[test]
    fn test_rdp_texture_loading_load_tile() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 8192]; // Larger buffer for texture data

        // Set up a texture image in RDRAM (8x8 RGBA16 texture)
        let texture_addr = 0x1000;
        for i in 0..64 {
            // Create a simple checkerboard pattern
            let color = if (i / 8 + i % 8) % 2 == 0 {
                0xF801u16 // Red (RGBA5551: 11111 00000 00000 1)
            } else {
                0x07E1u16 // Green (RGBA5551: 00000 11111 00000 1)
            };
            let offset = texture_addr + i * 2;
            rdram[offset..offset + 2].copy_from_slice(&color.to_be_bytes());
        }

        // Set up tile descriptor for tile 0
        rdp.tiles[0] = TileDescriptor {
            format: 0,    // RGBA
            size: 2,      // 16-bit
            line: 8,      // 8 texels per line = 1 64-bit word (8 texels * 2 bytes / 8)
            tmem_addr: 0, // Start at TMEM address 0
            palette: 0,
            s_mask: 3, // 2^3 = 8 texels wide
            t_mask: 3, // 2^3 = 8 texels tall
            s_shift: 0,
            t_shift: 0,
        };

        // Set texture image address
        rdp.texture_image_addr = texture_addr as u32;

        let dl_addr = 0x100;

        // SET_TEXTURE_IMAGE (0x3D) - set source texture
        let set_tex_img: u32 = (0x3D << 24) | (2 << 19) | 8; // format=RGBA(default 0), size=16bit, width=8
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&set_tex_img.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&(texture_addr as u32).to_be_bytes());

        // SET_TILE (0x35) - configure tile 0
        let set_tile: u32 = (0x35 << 24) | (2 << 19) | (8 << 9); // format=RGBA(default 0), size=16bit, line=8, tmem=0
        let set_tile_data: u32 = (3 << 4) | 3; // tile=0, mask_s=3, mask_t=3
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&set_tile.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&set_tile_data.to_be_bytes());

        // LOAD_TILE (0x34) - load 8x8 texture
        let load_tile: u32 = 0x34 << 24; // uls=0, ult=0
        let load_tile_data: u32 = (7 << 12) | 7; // tile=0, lrs=7, lrt=7
        rdram[dl_addr + 16..dl_addr + 20].copy_from_slice(&load_tile.to_be_bytes());
        rdram[dl_addr + 20..dl_addr + 24].copy_from_slice(&load_tile_data.to_be_bytes());

        rdp.write_register(0x00, dl_addr as u32);
        rdp.write_register(0x04, (dl_addr + 24) as u32);
        rdp.process_display_list(&rdram);

        // Verify texture was loaded to TMEM (check first few texels)
        // First texel should be red (0xF801 in RGBA5551)
        assert_eq!(rdp.tmem[0], 0xF8);
        assert_eq!(rdp.tmem[1], 0x01);

        // Second texel should be green (0x07E1)
        assert_eq!(rdp.tmem[2], 0x07);
        assert_eq!(rdp.tmem[3], 0xE1);
    }

    #[test]
    fn test_rdp_texture_loading_load_block() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 16384]; // Larger buffer

        // Set up a simple texture (16 texels of RGBA16)
        let texture_addr = 0x2000;
        for i in 0..16 {
            let color = 0xFC00u16 + i as u16; // Varying red shades
            let offset = texture_addr + i * 2;
            rdram[offset..offset + 2].copy_from_slice(&color.to_be_bytes());
        }

        rdp.tiles[0] = TileDescriptor {
            format: 0,
            size: 2, // 16-bit
            line: 2, // 2 64-bit words per line (16 bytes = 8 texels)
            tmem_addr: 0,
            palette: 0,
            s_mask: 0,
            t_mask: 0,
            s_shift: 0,
            t_shift: 0,
        };

        rdp.texture_image_addr = texture_addr as u32;

        let dl_addr = 0x100;

        // SET_TEXTURE_IMAGE
        let set_tex_img: u32 = 0x3D << 24;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&set_tex_img.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&(texture_addr as u32).to_be_bytes());

        // LOAD_BLOCK (0x33) - load 16 texels
        let load_block: u32 = 0x33 << 24;
        let load_block_data: u32 = 15 << 12; // tile=0, texels=15 (load 16 texels)
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&load_block.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&load_block_data.to_be_bytes());

        rdp.write_register(0x00, dl_addr as u32);
        rdp.write_register(0x04, (dl_addr + 16) as u32);
        rdp.process_display_list(&rdram);

        // Verify first texel was loaded
        assert_eq!(rdp.tmem[0], 0xFC);
        assert_eq!(rdp.tmem[1], 0x00);

        // Verify last texel (texel 15)
        assert_eq!(rdp.tmem[30], 0xFC);
        assert_eq!(rdp.tmem[31], 0x0F);
    }

    #[test]
    fn test_rdp_texture_sampling() {
        let mut rdp = Rdp::new();

        // Set up a simple 2x2 texture in TMEM manually
        rdp.tiles[0] = TileDescriptor {
            format: 0, // RGBA
            size: 2,   // 16-bit
            line: 1,   // 1 64-bit word = 4 texels (2x2)
            tmem_addr: 0,
            palette: 0,
            s_mask: 1, // 2^1 = 2 texels wide
            t_mask: 1, // 2^1 = 2 texels tall
            s_shift: 0,
            t_shift: 0,
        };

        // Fill TMEM with test pattern
        // (0,0) = Red (0xF801)
        rdp.tmem[0] = 0xF8;
        rdp.tmem[1] = 0x01;
        // (1,0) = Green (0x07E1)
        rdp.tmem[2] = 0x07;
        rdp.tmem[3] = 0xE1;
        // (0,1) = Blue (0x003F)
        rdp.tmem[8] = 0x00;
        rdp.tmem[9] = 0x3F;
        // (1,1) = White (0xFFFF)
        rdp.tmem[10] = 0xFF;
        rdp.tmem[11] = 0xFF;

        // Sample texels
        let color_00 = rdp.sample_texture(0, 0, 0);
        let color_10 = rdp.sample_texture(0, 1, 0);
        let color_01 = rdp.sample_texture(0, 0, 1);
        let color_11 = rdp.sample_texture(0, 1, 1);

        // Verify colors (approximately - 5-bit to 8-bit conversion may vary)
        // Red should have high R, low G/B
        assert!((color_00 >> 16) & 0xFF > 200); // Red channel
        assert!((color_00 >> 8) & 0xFF < 50); // Green channel

        // Green should have high G, low R/B
        assert!((color_10 >> 8) & 0xFF > 200); // Green channel
        assert!((color_10 >> 16) & 0xFF < 50); // Red channel

        // Blue should have high B, low R/G
        assert!(color_01 & 0xFF > 200); // Blue channel
        assert!((color_01 >> 16) & 0xFF < 50); // Red channel

        // White should have all channels high
        assert!((color_11 >> 16) & 0xFF > 200); // Red
        assert!((color_11 >> 8) & 0xFF > 200); // Green
        assert!(color_11 & 0xFF > 200); // Blue
    }

    #[test]
    fn test_rdp_textured_triangle_command_0x0a() {
        let mut rdp = Rdp::new();
        let mut rdram = vec![0u8; 0x400000];

        // Set up a simple texture in TMEM
        rdp.tiles[0] = TileDescriptor {
            format: 0, // RGBA
            size: 2,   // 16-bit
            line: 8,   // 8 64-bit words per line (64 bytes = 32 texels)
            tmem_addr: 0,
            palette: 0,
            s_mask: 6, // 64 texels width (2^6)
            t_mask: 6, // 64 texels height (2^6)
            s_shift: 0,
            t_shift: 0,
        };

        // Fill TMEM with a simple pattern (checkerboard)
        for i in 0..2048 {
            // 4KB / 2 bytes per texel
            let color: u16 = if (i / 32) % 2 == (i % 32) % 2 {
                0xFFFF // White
            } else {
                0x0001 // Black with alpha
            };
            rdp.tmem[i * 2] = (color >> 8) as u8;
            rdp.tmem[i * 2 + 1] = (color & 0xFF) as u8;
        }

        // Create display list with textured triangle command
        let dl_addr = 0x100;

        // Textured triangle command 0x0A
        // word0: bits 23-12 = x0, bits 11-0 = y0
        // word1: bits 31-24 = x1, bits 23-16 = y1, bits 15-8 = x2, bits 7-0 = y2
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x0A << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[dl_addr..dl_addr + 4].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&cmd_word1.to_be_bytes());

        // Process the display list
        rdp.write_register(0x00, dl_addr as u32);
        rdp.write_register(0x04, (dl_addr + 8) as u32);
        rdp.process_display_list(&rdram);

        // Verify triangle was rendered with texture
        let frame = rdp.get_frame();
        // Check center of triangle - should have texture pattern (either white or black)
        let idx = (100 * 320 + 100) as usize;
        let pixel = frame.pixels[idx];
        // Should be from texture pattern - either white or black
        assert!(pixel != 0); // Should have been rendered
    }

    #[test]
    fn test_rdp_textured_triangle_zbuffer_command_0x0b() {
        let mut rdp = Rdp::new();
        rdp.set_zbuffer_enabled(true);
        let mut rdram = vec![0u8; 0x400000];

        // Set up texture in TMEM
        rdp.tiles[0] = TileDescriptor {
            format: 0,
            size: 2,
            line: 8,
            tmem_addr: 0,
            palette: 0,
            s_mask: 6,
            t_mask: 6,
            s_shift: 0,
            t_shift: 0,
        };

        // Fill with solid red texture
        for i in 0..2048 {
            let color: u16 = 0xF801; // Red in RGB5551
            rdp.tmem[i * 2] = (color >> 8) as u8;
            rdp.tmem[i * 2 + 1] = (color & 0xFF) as u8;
        }

        let dl_addr = 0x100;

        // Textured triangle with Z-buffer command 0x0B
        let x0 = 100u32;
        let y0 = 50u32;
        let x1 = 150u32;
        let y1 = 150u32;
        let x2 = 50u32;
        let y2 = 150u32;

        let cmd_word0 = (0x0B << 24) | ((x0 & 0xFFF) << 12) | (y0 & 0xFFF);
        let cmd_word1 =
            ((x1 & 0xFF) << 24) | ((y1 & 0xFF) << 16) | ((x2 & 0xFF) << 8) | (y2 & 0xFF);

        rdram[dl_addr..dl_addr + 4].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&cmd_word1.to_be_bytes());

        rdp.write_register(0x00, dl_addr as u32);
        rdp.write_register(0x04, (dl_addr + 8) as u32);
        rdp.process_display_list(&rdram);

        // Verify triangle was rendered
        let frame = rdp.get_frame();
        let idx = (100 * 320 + 100) as usize;
        let pixel = frame.pixels[idx];
        // Should be red from texture
        assert!(pixel != 0);
        // Check that red channel is high
        assert!((pixel >> 16) & 0xFF > 200);
    }
}
