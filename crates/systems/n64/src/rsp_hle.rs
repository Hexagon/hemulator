//! RSP High-Level Emulation (HLE)
//!
//! This module provides high-level emulation of common RSP microcodes.
//! Instead of executing RSP instructions at the microcode level, we detect
//! which microcode is loaded and emulate its behavior at a high level.
//!
//! # Supported Microcodes
//!
//! - **F3DEX/F3DEX2**: Fast3D Extended - most common graphics microcode
//! - **F3DLX/F3DLX2**: Fast3D Line Extended - wireframe rendering
//! - **F3DLP**: Fast3D Line Point - point and line rendering
//!
//! # Architecture
//!
//! When the CPU loads microcode into RSP IMEM, we analyze the code signature
//! to determine which microcode it is. Then when the RSP is triggered to run,
//! we execute the high-level behavior:
//!
//! 1. Parse display list commands from RDRAM
//! 2. Process vertex data, apply transforms
//! 3. Generate RDP display lists for triangle rendering
//! 4. Handle lighting, texture coordinates, etc.
//!
//! # F3DEX Display List Commands
//!
//! Common F3DEX commands (command ID in upper byte):
//! - 0x01: G_VTX - Load vertices into vertex buffer
//! - 0x04: G_TRI1 - Draw single triangle
//! - 0x05: G_TRI2 - Draw two triangles  
//! - 0x06: G_QUAD - Draw quadrilateral (two triangles)
//! - 0xDA: G_MTX - Load transformation matrix
//! - 0xD9: G_GEOMETRYMODE - Set rendering mode flags
//! - 0xDF: G_ENDDL - End of display list
//! - 0xBF: G_RDPHALF_1 - RDP command data (part 1)
//! - 0xE0-0xFF: Various RDP passthrough commands

use super::rdp::Rdp;
use emu_core::logging::{log, LogCategory, LogLevel};

/// RSP microcode types (detected by analyzing IMEM signature)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Not all variants used yet - reserved for future microcode support
pub enum MicrocodeType {
    /// Unknown or unrecognized microcode
    Unknown,
    /// Fast3D Extended (most common graphics microcode)
    F3DEX,
    /// Fast3D Extended 2 (enhanced version)
    F3DEX2,
    /// Audio microcode
    Audio,
}

/// Geometry mode flags
#[allow(dead_code)] // Reserved for future use when geometry modes are fully implemented
const G_ZBUFFER: u32 = 0x00000001; // Enable Z-buffer
#[allow(dead_code)]
const G_TEXTURE_ENABLE: u32 = 0x00000002; // Enable texture mapping (custom flag for demo)
#[allow(dead_code)]
const G_SHADE: u32 = 0x00000004; // Enable shading (Gouraud)
const G_LIGHTING: u32 = 0x00020000; // Enable lighting
const G_CULL_FRONT: u32 = 0x00000200; // Cull front-facing triangles
const G_CULL_BACK: u32 = 0x00000400; // Cull back-facing triangles
#[allow(dead_code)] // Reserved for future texture coordinate generation support
const G_TEXTURE_GEN: u32 = 0x00040000; // Texture coordinate generation

/// Vertex structure for graphics microcode
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    /// Position (x, y, z) in object space
    pub pos: [i16; 3],
    /// Texture coordinates (s, t) in 16.16 fixed point
    pub tex: [i16; 2],
    /// Color (RGBA) 0-255 per channel — when lighting is off, these are literal vertex colors;
    /// when lighting is on, bytes 12-14 hold the normal vector (signed) and byte 15 is alpha.
    pub color: [u8; 4],
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            pos: [0, 0, 0],
            tex: [0, 0],
            color: [255, 255, 255, 255],
        }
    }
}

/// RSP HLE state
pub struct RspHle {
    /// Detected microcode type
    microcode: MicrocodeType,

    /// Vertex buffer (up to 32 vertices cached)
    vertices: [Vertex; 32],

    /// Number of vertices currently loaded
    vertex_count: usize,

    /// Current matrix stack pointer (0-9 for 10 levels)
    matrix_stack_ptr: usize,

    /// Matrix stack for modelview matrices (10 levels of 4x4 matrices)
    /// RSP supports up to 10 levels of matrix nesting for display list calls
    matrix_stack: [[f32; 16]; 10],

    /// Projection matrix (4x4, stored as 16 f32s)
    projection_matrix: [f32; 16],

    /// Modelview matrix (4x4, stored as 16 f32s)
    modelview_matrix: [f32; 16],

    /// Geometry mode flags
    geometry_mode: u32,

    /// Viewport parameters (x, y, width, height, scale_x, scale_y)
    /// Defaults to (0, 0, 320, 240, 160, 120) for 320x240 framebuffer
    viewport: (f32, f32, f32, f32, f32, f32),

    /// Viewport Z scale and translate (from viewport structure)
    viewport_z_scale: f32,
    viewport_z_trans: f32,

    /// Display list call stack for G_DL commands
    /// Stores return addresses when G_DL_PUSH is used
    #[allow(dead_code)] // Reserved for future proper display list call stack implementation
    dl_stack: Vec<u32>,

    /// Temporary storage for G_RDPHALF_1 data
    /// Used for 2-word RDP commands split across display list entries
    #[allow(dead_code)] // Reserved for future use with 2-word RDP commands
    rdp_half: u32,

    /// Light data (up to 8 lights)
    /// Each light has 7 elements: [dx, dy, dz, r, g, b, type]
    /// - dx, dy, dz: direction vector (normalized, -1.0 to 1.0)
    /// - r, g, b: color components (0.0 to 1.0)
    /// - type: 0.0 = directional, 1.0 = point (reserved for future use)
    lights: [[f32; 7]; 8],

    /// Number of active lights
    num_lights: usize,

    /// Ambient light color (RGB as floats 0.0-1.0)
    #[allow(dead_code)] // Reserved for future lighting implementation
    ambient_light: [f32; 3],

    /// Segment base addresses (16 segments, 0x00-0x0F)
    /// Used for segmented addressing in display lists and textures
    segment_bases: [u32; 16],

    /// Other mode high word (pipeline settings)
    /// Controls cycle type, texture filtering, dithering, etc.
    othermode_h: u32,

    /// Other mode low word (rendering settings)
    /// Controls alpha compare, depth source, render mode, etc.
    othermode_l: u32,

    /// Debug: current DL address and depth for diagnostics
    dl_debug_addr: u32,
    dl_debug_depth: u32,
    dl_debug_found_zero: bool,
    /// Count of zero matrices encountered per RSP task
    pub zero_mtx_count: u32,
}

impl RspHle {
    /// Create new RSP HLE state
    pub fn new() -> Self {
        Self {
            microcode: MicrocodeType::Unknown,
            vertices: [Vertex::default(); 32],
            vertex_count: 0,
            matrix_stack_ptr: 0,
            matrix_stack: [Self::identity_matrix(); 10],
            projection_matrix: Self::identity_matrix(),
            modelview_matrix: Self::identity_matrix(),
            geometry_mode: 0,
            // Default viewport for 320x240 framebuffer
            // (x, y, width, height, scale_x, scale_y)
            viewport: (0.0, 0.0, 320.0, 240.0, 160.0, 120.0),
            viewport_z_scale: 511.0,
            viewport_z_trans: 511.0,
            dl_stack: Vec::with_capacity(10),
            rdp_half: 0,
            lights: [[0.0; 7]; 8],
            num_lights: 0,
            ambient_light: [0.3, 0.3, 0.3], // Default ambient light
            segment_bases: [0; 16],         // Initialize all segments to 0
            othermode_h: 0,
            othermode_l: 0,
            dl_debug_addr: 0,
            dl_debug_depth: 0,
            dl_debug_found_zero: false,
            zero_mtx_count: 0,
        }
    }

    /// Create identity matrix
    fn identity_matrix() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    /// Multiply two 4x4 matrices: result = a * b
    /// Matrices are in column-major order (N64 format)
    fn multiply_matrix(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut result = [0.0f32; 16];
        // Column-major multiplication: C[col][row] = sum(A[k][row] * B[col][k])
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    // A[k][row] is at index row + k*4 (column-major)
                    // B[col][k] is at index k + col*4 (column-major)
                    sum += a[row + k * 4] * b[k + col * 4];
                }
                // C[col][row] is at index row + col*4 (column-major)
                result[row + col * 4] = sum;
            }
        }
        result
    }

    /// Convert N64 virtual address to physical RDRAM address
    /// KSEG0 (0x80000000-0x9FFFFFFF) and KSEG1 (0xA0000000-0xBFFFFFFF) map to physical 0x00000000-0x1FFFFFFF
    /// Also handles already-physical addresses (0x00000000-0x1FFFFFFF) by passing them through
    fn virt_to_phys(addr: u32) -> usize {
        // Check if address is in KSEG0 or KSEG1 (virtual address ranges)
        // KSEG0: 0x80000000-0x9FFFFFFF (cached)
        // KSEG1: 0xA0000000-0xBFFFFFFF (uncached)
        // KSEG2: 0xC0000000-0xDFFFFFFF (kernel)
        // KSEG3: 0xE0000000-0xFFFFFFFF (kernel)
        let is_virtual = addr >= 0x80000000;

        if is_virtual {
            // Virtual address - mask to get physical address
            (addr & 0x1FFFFFFF) as usize
        } else {
            // Already a physical address - pass through
            // This handles cases where ROM data uses physical addresses directly
            addr as usize
        }
    }

    /// Resolve segmented address using segment base table
    /// N64 display list commands use segmented addressing where the top byte
    /// is a segment index (0x00-0x0F) and the remaining 24 bits are an offset.
    /// The resolved address = segment_bases[seg] + offset.
    /// Addresses in KSEG0/KSEG1 (>=0x80000000) are passed through directly
    /// since they are already absolute virtual addresses.
    fn resolve_segment_addr(&self, addr: u32) -> u32 {
        // If address is already a kernel virtual address, pass it through
        if addr >= 0x80000000 {
            return addr;
        }
        let segment = ((addr >> 24) & 0x0F) as usize;
        let offset = addr & 0x00FFFFFF;
        self.segment_bases[segment].wrapping_add(offset)
    }

    /// Load a 4x4 matrix from RDRAM
    /// N64 matrices are stored as two 32-byte halves (64 bytes total):
    ///   Bytes 0-31:  16× 16-bit signed integer parts (big-endian)
    ///   Bytes 32-63: 16× 16-bit unsigned fractional parts (big-endian)
    /// Element[i] = (int_part[i] << 16) | frac_part[i], as signed 16.16 fixed-point
    fn load_matrix_from_rdram(&self, rdram: &[u8], addr: u32) -> [f32; 16] {
        let mut matrix = [0.0f32; 16];
        let addr = Self::virt_to_phys(addr);

        // Safety check: need 64 bytes
        if addr + 63 >= rdram.len() {
            return Self::identity_matrix();
        }

        for (i, element) in matrix.iter_mut().enumerate() {
            let int_offset = addr + i * 2;
            let frac_offset = addr + 32 + i * 2;

            let int_part = i16::from_be_bytes([rdram[int_offset], rdram[int_offset + 1]]);
            let frac_part = u16::from_be_bytes([rdram[frac_offset], rdram[frac_offset + 1]]);

            // Combine: (int_part << 16) | frac_part as signed 16.16
            let fixed = ((int_part as i32) << 16) | (frac_part as i32);
            *element = (fixed as f32) / 65536.0;
        }

        matrix
    }

    /// Detect microcode type from IMEM data using CRC32 signatures
    pub fn detect_microcode(&mut self, imem: &[u8; 4096]) {
        // Check if IMEM has any code
        let has_code = imem.iter().any(|&b| b != 0);
        if !has_code {
            self.microcode = MicrocodeType::Unknown;
            return;
        }

        // Calculate CRC32 of the first 4KB of IMEM
        let crc = crc32fast::hash(imem);

        // Known microcode CRC32 signatures
        // These are common F3DEX/F3DEX2 variants from various games
        self.microcode = match crc {
            // F3DEX2 variants (most common in later N64 games)
            0xB545B679 | 0x9F0B2B0E | 0x3A1C2B34 | 0x4AED6B3A => MicrocodeType::F3DEX2,

            // F3DEX variants (common in earlier N64 games)
            0xBF0DA4E5 | 0xE9C86D0F | 0xD7C3B8B5 | 0x5EC6E85F => MicrocodeType::F3DEX,

            // Audio microcodes
            0x1A7DDD1E | 0x3E3E0CA2 => MicrocodeType::Audio,

            // If CRC doesn't match known signatures, try pattern matching
            _ => {
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "RSP: Unknown microcode CRC 0x{:08X}, using heuristic detection",
                        crc
                    )
                });

                // Heuristic: Look for common instruction patterns
                // F3DEX/F3DEX2 microcodes have distinctive patterns in their code

                // Check for F3DEX2 patterns (more optimized)
                // F3DEX2 typically has more vector operations (LQV/SQV) early in code
                let has_f3dex2_pattern = (0..256).step_by(4).any(|i| {
                    let word = u32::from_be_bytes([imem[i], imem[i + 1], imem[i + 2], imem[i + 3]]);
                    // LQV instruction opcode pattern
                    (word & 0xFC000000) == 0xC8000000
                });

                // Check for F3DEX patterns (older, more general)
                // F3DEX typically has more branching and scalar operations
                let has_f3dex_pattern = (0..256).step_by(4).any(|i| {
                    let word = u32::from_be_bytes([imem[i], imem[i + 1], imem[i + 2], imem[i + 3]]);
                    // BGEZ/BLTZ instruction opcodes (common in F3DEX control flow)
                    (word & 0xFC1F0000) == 0x04010000 || (word & 0xFC1F0000) == 0x04000000
                });

                if has_f3dex2_pattern {
                    MicrocodeType::F3DEX2
                } else if has_f3dex_pattern {
                    MicrocodeType::F3DEX
                } else {
                    // Default to F3DEX for unknown graphics microcodes
                    log(LogCategory::PPU, LogLevel::Warn, || {
                        "RSP: Microcode detection failed, defaulting to F3DEX".to_string()
                    });
                    MicrocodeType::F3DEX
                }
            }
        };

        log(LogCategory::PPU, LogLevel::Info, || {
            format!(
                "RSP: Detected microcode: {:?} (CRC: 0x{:08X})",
                self.microcode, crc
            )
        });
    }

    /// Get current microcode type
    pub fn microcode(&self) -> MicrocodeType {
        self.microcode
    }

    /// Get current vertex count in vertex buffer
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Execute HLE task (called when RSP is triggered)
    /// Returns number of cycles consumed
    pub fn execute_task(&mut self, dmem: &[u8; 4096], rdram: &mut [u8], _rdp: &mut Rdp) -> u32 {
        // Check the ACTUAL task type from the OSTask structure in DMEM, not just
        // the detected microcode. Games like SM64 submit both graphics (type=1)
        // and audio (type=2) tasks using the same RSP, and the microcode type
        // detected from IMEM reflects graphics code even for audio tasks.
        const TASK_BASE: usize = 0xFC0;
        let task_type = self.read_u32(dmem, TASK_BASE);
        let _data_ptr = self.read_u32(dmem, TASK_BASE + 0x30);

        if task_type == 2 {
            // Audio task (M_AUDTASK) - always use audio handler
            let mut dmem_scratch = *dmem;
            return self.execute_audio_task(&mut dmem_scratch, rdram);
        }

        match self.microcode {
            MicrocodeType::F3DEX | MicrocodeType::F3DEX2 => {
                self.execute_graphics_task(dmem, rdram, _rdp)
            }
            MicrocodeType::Audio => {
                let mut dmem_scratch = *dmem;
                self.execute_audio_task(&mut dmem_scratch, rdram)
            }
            MicrocodeType::Unknown => {
                // No-op for unknown microcode
                100
            }
        }
    }

    /// Execute graphics microcode task (F3DEX/F3DEX2)
    fn execute_graphics_task(&mut self, dmem: &[u8; 4096], rdram: &mut [u8], rdp: &mut Rdp) -> u32 {
        // Clear the OpenGL depth buffer at the start of each graphics task.
        // The N64 game clears its Z-buffer via FILL_RECTANGLE to RDRAM, which
        // does NOT affect the GL depth buffer. Without this clear, triangle
        // depth values from previous frames persist and cause new triangles
        // to fail the GL_LESS depth test.
        rdp.clear_zbuffer();

        const TASK_BASE: usize = 0xFC0;
        let _task_type = self.read_u32(dmem, TASK_BASE);
        let mut data_ptr = self.read_u32(dmem, TASK_BASE + 0x30);
        let mut data_size = self.read_u32(dmem, TASK_BASE + 0x34);
        let phys_data_ptr = Self::virt_to_phys(data_ptr) as u32;

        if phys_data_ptr == 0 || phys_data_ptr as usize >= rdram.len() {
            const TASK_STRUCT_ADDR: usize = 0x00200000;
            if TASK_STRUCT_ADDR + 0x40 <= rdram.len() {
                data_ptr = self.read_u32_rdram(rdram, TASK_STRUCT_ADDR + 0x30);
                data_size = self.read_u32_rdram(rdram, TASK_STRUCT_ADDR + 0x34);

                if data_ptr > 0 && self.microcode == MicrocodeType::Unknown {
                    self.microcode = MicrocodeType::F3DEX;
                }
            }
        }

        if data_ptr > 0 && data_size > 0 {
            self.parse_f3dex_display_list(rdram, data_ptr, data_size, rdp);
        }

        // In HLE mode, the RSP does not write to the output buffer - we handle
        // display list commands directly via draw_triangle/fill_rect calls.
        // Processing the output buffer would just feed garbage data to the RDP.

        2000
    }

    /// Execute audio microcode task (ABI1/ABI2 high-level emulation).
    ///
    /// The N64 audio pipeline:
    ///   1. The game fills an OS_TASK structure (in RDRAM) describing the audio work.
    ///   2. The OS DMA's the task header to DMEM offset 0x00 before starting the RSP.
    ///   3. The audio microcode reads a command list from RDRAM, executes each
    ///      command using DMEM as scratch space, and writes the final interleaved
    ///      16-bit stereo PCM samples to the RDRAM output buffer.
    ///   4. The AI DMA then copies those samples to the audio DAC.
    ///
    /// OS_TASK layout in DMEM (32-bit fields, big-endian):
    ///   0x00 type, 0x02 flags,
    ///   0x04 ucode_boot,  0x08 ucode_boot_size,
    ///   0x0C ucode,       0x10 ucode_size,
    ///   0x18 ucode_data (= ABI1 command list RDRAM ptr),
    ///   0x1C ucode_data_size,
    ///   0x20 dram_stack,  0x24 dram_stack_size,
    ///   0x28 output_buff (= PCM output RDRAM ptr),
    ///   0x2C output_buff_size,
    ///   …
    ///
    /// This implements the most common ABI1 commands:
    ///   0x00 SPNOOP, 0x01 ADPCM, 0x02 CLEARBUFF, 0x05 DMEMMOVE,
    ///   0x07 MIXER,  0x08 INTERLEAVE, 0x14 LOADBUFF, 0x15 SAVEBUFF.
    fn execute_audio_task(&mut self, dmem: &mut [u8; 4096], rdram: &mut [u8]) -> u32 {
        // OSTask structure is at DMEM offset 0xFC0
        // OSTask fields: 0x18=ucode_data, 0x1C=ucode_data_size, 0x28=output_buff, 0x2C=output_buff_size
        const TASK_BASE: usize = 0xFC0;
        let ucode_data_ptr = self.read_u32(dmem, TASK_BASE + 0x18); // ABI1 command list in RDRAM
        let ucode_data_size = self.read_u32(dmem, TASK_BASE + 0x1C);
        let output_buff = self.read_u32(dmem, TASK_BASE + 0x28); // PCM output buffer in RDRAM
        let output_buff_size = self.read_u32(dmem, TASK_BASE + 0x2C);

        log(LogCategory::APU, LogLevel::Debug, || {
            format!(
                "RSP HLE ABI1: cmd_list=0x{:08X}[0x{:X}] output=0x{:08X}[0x{:X}]",
                ucode_data_ptr, ucode_data_size, output_buff, output_buff_size
            )
        });

        let rdram_len = rdram.len();

        // Validate command list pointer
        let cmd_phys = Self::virt_to_phys(ucode_data_ptr);
        if cmd_phys >= rdram_len || ucode_data_size == 0 {
            log(LogCategory::APU, LogLevel::Warn, || {
                format!(
                    "RSP HLE ABI1: invalid command list ptr 0x{:08X}",
                    ucode_data_ptr
                )
            });
            return 1500;
        }

        // Each ABI1 command is 8 bytes (2 × 32-bit words)
        let num_cmds = (ucode_data_size as usize) / 8;
        let mut cycles: u32 = 0;

        for i in 0..num_cmds {
            let base = cmd_phys + i * 8;
            if base + 8 > rdram_len {
                break;
            }
            let word0 = u32::from_be_bytes([
                rdram[base],
                rdram[base + 1],
                rdram[base + 2],
                rdram[base + 3],
            ]);
            let word1 = u32::from_be_bytes([
                rdram[base + 4],
                rdram[base + 5],
                rdram[base + 6],
                rdram[base + 7],
            ]);
            let cmd = (word0 >> 24) as u8;

            match cmd {
                // SPNOOP (0x00): no-op
                0x00 => {}

                // ADPCM (0x01): VADPCM decode
                // N64 uses a variant of ADPCM with vector codebook.
                // word0: cmd(8) | flags(8) | count(16) — count = bytes of *output* PCM
                // word1: in_addr(16, DMEM) | out_addr(16, DMEM)
                //
                // The codebook is loaded into DMEM by the game before calling ADPCM.
                // Each ADPCM frame is 9 bytes → 16 PCM samples (4-bit per sample).
                // Byte 0 of each frame: scale_shift(4 upper) | predictor_index(4 lower)
                // Bytes 1-8: 16 packed 4-bit signed residuals.
                0x01 => {
                    let in_addr = ((word1 >> 16) & 0x0FFF) as usize;
                    let out_addr = (word1 & 0x0FFF) as usize;
                    let count = (word0 & 0xFFFF) as usize; // bytes of PCM output
                    let num_samples = count / 2; // 16-bit samples

                    // Decode ADPCM frames
                    let mut src = in_addr;
                    let mut dst = out_addr;
                    let mut prev1: i32 = 0; // previous sample state
                    let mut prev2: i32 = 0; // second previous sample state
                    let mut samples_written = 0usize;

                    while samples_written < num_samples && src < 4096 && dst + 1 < 4096 {
                        if src + 9 > 4096 {
                            break;
                        }
                        let header = dmem[src];
                        let scale_shift = (header >> 4) & 0x0F;
                        let scale = 1i32 << scale_shift;
                        src += 1;

                        // Decode 16 samples from 8 bytes (two 4-bit nibbles each)
                        for _byte_idx in 0..8 {
                            if samples_written >= num_samples || src >= 4096 {
                                break;
                            }
                            let packed = dmem[src];
                            src += 1;

                            // High nibble first, then low nibble
                            for nibble_sel in [4i32, 0] {
                                if samples_written >= num_samples || dst + 1 >= 4096 {
                                    break;
                                }
                                // Sign-extend 4-bit nibble
                                let nibble = ((packed as i32 >> nibble_sel) & 0x0F) as i8;
                                let nibble = if nibble >= 8 {
                                    nibble as i32 - 16
                                } else {
                                    nibble as i32
                                };

                                // Simple 2nd-order IIR prediction (simplified codebook)
                                let predicted = prev1 + (prev1 - prev2);
                                let sample = (predicted + nibble * scale).clamp(-32768, 32767);

                                // Write 16-bit big-endian sample
                                let s16 = sample as i16;
                                let bytes = s16.to_be_bytes();
                                dmem[dst] = bytes[0];
                                dmem[dst + 1] = bytes[1];
                                dst += 2;

                                prev2 = prev1;
                                prev1 = sample;
                                samples_written += 1;
                            }
                        }
                    }

                    // Zero-fill remaining output if we ran out of input
                    let remaining = num_samples.saturating_sub(samples_written) * 2;
                    if remaining > 0 && dst + remaining <= 4096 {
                        dmem[dst..dst + remaining].fill(0);
                    }
                    cycles += num_samples as u32;
                }

                // CLEARBUFF (0x02): zero a DMEM range
                // word1[31:16] = dmem_addr, word1[15:0] = count
                0x02 => {
                    let dmem_addr = ((word1 >> 16) & 0x0FFF) as usize;
                    let count = (word1 & 0xFFFF) as usize;
                    if dmem_addr + count <= 4096 {
                        dmem[dmem_addr..dmem_addr + count].fill(0);
                    }
                    cycles += 10 + (count as u32 / 16);
                }

                // RESAMPLE (0x03): linear interpolation resampling
                // word0: cmd(8) | flags(8) | count(16) — count = output sample *pairs* (bytes/2)
                // word1: pitch(16) | in_addr(16, DMEM)
                // Output goes to a second DMEM region (usually in_addr + count).
                // Pitch is a 16-bit unsigned fixed-point value where 0x8000 = 1.0 (no change).
                0x03 => {
                    let count = (word0 & 0xFFFF) as usize; // bytes of output
                    let pitch = (word1 >> 16) & 0xFFFF;
                    let in_addr = (word1 & 0x0FFF) as usize;
                    let out_addr = in_addr; // resample in place for simplified HLE

                    let num_out_samples = count / 2; // 16-bit samples
                    if pitch > 0 && in_addr + count <= 4096 {
                        // Read source samples into a temporary buffer
                        let max_src = (count / 2).min(2048);
                        let mut src_buf = vec![0i16; max_src];
                        for (i, sample) in src_buf.iter_mut().enumerate() {
                            let off = in_addr + i * 2;
                            if off + 1 < 4096 {
                                *sample = i16::from_be_bytes([dmem[off], dmem[off + 1]]);
                            }
                        }

                        // Resample with linear interpolation
                        // accumulator in 16.16 fixed point (pitch 0x8000 = 1.0)
                        let mut accum: u32 = 0;
                        let mut dst = out_addr;
                        for _ in 0..num_out_samples {
                            let idx = (accum >> 15) as usize; // integer part
                            let frac = ((accum & 0x7FFF) as i32) << 1; // 0..65534
                            let s0 = if idx < src_buf.len() {
                                src_buf[idx] as i32
                            } else {
                                0
                            };
                            let s1 = if idx + 1 < src_buf.len() {
                                src_buf[idx + 1] as i32
                            } else {
                                s0
                            };
                            let interp = (s0 + (((s1 - s0) * frac) >> 16)).clamp(-32768, 32767);
                            if dst + 1 < 4096 {
                                let bytes = (interp as i16).to_be_bytes();
                                dmem[dst] = bytes[0];
                                dmem[dst + 1] = bytes[1];
                            }
                            dst += 2;
                            accum += pitch;
                        }
                    }
                    cycles += 20 + num_out_samples as u32;
                }

                // DMEMMOVE (0x05): memcpy within DMEM
                // word1[31:16] = src_dmem, word1[15:0] = dst_dmem,  word0[15:0] = count
                0x05 => {
                    let src = ((word1 >> 16) & 0x0FFF) as usize;
                    let dst = (word1 & 0x0FFF) as usize;
                    let count = (word0 & 0xFFFF) as usize;
                    if src + count <= 4096 && dst + count <= 4096 && src != dst {
                        dmem.copy_within(src..src + count, dst);
                    }
                    cycles += 10 + (count as u32 / 16);
                }

                // MIXER (0x07): mix (add) two DMEM buffers, with scaling
                // word0[23:16] = flags, word0[15:0] = count
                // word1[31:16] = src, word1[15:0] = dst (in DMEM)
                0x07 => {
                    let count = (word0 & 0xFFFF) as usize;
                    let src = ((word1 >> 16) & 0x0FFF) as usize;
                    let dst = (word1 & 0x0FFF) as usize;
                    // Mix by saturating addition: dst[i] = clamp(dst[i] + src[i], -32768, 32767)
                    if src + count <= 4096 && dst + count <= 4096 && count.is_multiple_of(2) {
                        for j in (0..count).step_by(2) {
                            let a = i16::from_be_bytes([dmem[dst + j], dmem[dst + j + 1]]) as i32;
                            let b = i16::from_be_bytes([dmem[src + j], dmem[src + j + 1]]) as i32;
                            let mixed = (a + b).clamp(-32768, 32767) as i16;
                            let bytes = mixed.to_be_bytes();
                            dmem[dst + j] = bytes[0];
                            dmem[dst + j + 1] = bytes[1];
                        }
                    }
                    cycles += 20 + (count as u32 / 4);
                }

                // INTERLEAVE (0x08): interleave L and R DMEM channels into output buffer.
                // word1[31:16] = left_dmem_addr, word1[15:0] = right_dmem_addr
                // word0[15:0]  = count (bytes in each channel; output is 2× this size)
                // The interleaved output goes to the RDRAM output_buff set in the OS_TASK.
                0x08 => {
                    let count = (word0 & 0xFFFF) as usize; // bytes per channel
                    let left = ((word1 >> 16) & 0x0FFF) as usize;
                    let right = (word1 & 0x0FFF) as usize;

                    let out_phys = Self::virt_to_phys(output_buff);
                    let out_size = output_buff_size as usize;

                    // Interleave left/right 16-bit samples into RDRAM output buffer.
                    // Limit pairs to what fits within the declared output buffer size so we
                    // never write beyond the task's output region.
                    if count > 0 && out_phys + out_size <= rdram_len {
                        let pairs = (count / 2).min(out_size / 4);
                        for j in 0..pairs {
                            let li = left + j * 2;
                            let ri = right + j * 2;
                            let oi = out_phys + j * 4;
                            if li + 1 < 4096 && ri + 1 < 4096 && oi + 3 < rdram_len {
                                rdram[oi] = dmem[li];
                                rdram[oi + 1] = dmem[li + 1];
                                rdram[oi + 2] = dmem[ri];
                                rdram[oi + 3] = dmem[ri + 1];
                            }
                        }
                        log(LogCategory::APU, LogLevel::Debug, || {
                            format!(
                                "RSP HLE ABI1: INTERLEAVE wrote {} stereo frames to RDRAM 0x{:08X}",
                                pairs, output_buff
                            )
                        });
                    }
                    cycles += 20 + (count as u32 / 4);
                }

                // LOADBUFF (0x14): DMA from RDRAM to DMEM
                // word1 = RDRAM source ptr, word0[23:12] = DMEM dest, word0[11:0] = count-1
                0x14 => {
                    let src_rdram = Self::virt_to_phys(word1);
                    let dmem_dst = ((word0 >> 12) & 0x0FFF) as usize;
                    let count = ((word0 & 0x0FFF) + 1) as usize;
                    if src_rdram + count <= rdram_len && dmem_dst + count <= 4096 {
                        dmem[dmem_dst..dmem_dst + count]
                            .copy_from_slice(&rdram[src_rdram..src_rdram + count]);
                    }
                    cycles += 10 + (count as u32 / 16);
                }

                // SAVEBUFF (0x15): DMA from DMEM to RDRAM
                // word1 = RDRAM destination ptr, word0[23:12] = DMEM src, word0[11:0] = count-1
                0x15 => {
                    let dst_rdram = Self::virt_to_phys(word1);
                    let dmem_src = ((word0 >> 12) & 0x0FFF) as usize;
                    let count = ((word0 & 0x0FFF) + 1) as usize;
                    if dmem_src + count <= 4096 && dst_rdram + count <= rdram_len {
                        rdram[dst_rdram..dst_rdram + count]
                            .copy_from_slice(&dmem[dmem_src..dmem_src + count]);
                    }
                    cycles += 10 + (count as u32 / 16);
                }

                // SETBUFF (0x0F): set source/destination buffer pointers in DMEM
                // (These update internal state; for HLE we use OS_TASK fields instead)
                0x0F => {}

                // ENVMIXER (0x0D): envelope-controlled mixing
                // Mixes a mono source into separate left/right DMEM buffers with
                // per-channel volume (envelope).
                // word0: cmd(8) | flags(8) | count(16) — count in bytes (of output per channel)
                // word1: src_addr(16, DMEM) | dst_left(16, DMEM)  (dst_right = dst_left + count)
                0x0D => {
                    let count = (word0 & 0xFFFF) as usize;
                    let src = ((word1 >> 16) & 0x0FFF) as usize;
                    let dst_left = (word1 & 0x0FFF) as usize;
                    let dst_right = dst_left + count;
                    let num_samples = count / 2;

                    if src + count <= 4096
                        && dst_left + count <= 4096
                        && dst_right + count <= 4096
                        && count.is_multiple_of(2)
                    {
                        for j in 0..num_samples {
                            let off = j * 2;
                            let s =
                                i16::from_be_bytes([dmem[src + off], dmem[src + off + 1]]) as i32;
                            // Add source sample to both left and right with saturation
                            let left_val = i16::from_be_bytes([
                                dmem[dst_left + off],
                                dmem[dst_left + off + 1],
                            ]) as i32;
                            let right_val = i16::from_be_bytes([
                                dmem[dst_right + off],
                                dmem[dst_right + off + 1],
                            ]) as i32;
                            let l = (left_val + s).clamp(-32768, 32767) as i16;
                            let r_out = (right_val + s).clamp(-32768, 32767) as i16;
                            let lb = l.to_be_bytes();
                            let rb = r_out.to_be_bytes();
                            dmem[dst_left + off] = lb[0];
                            dmem[dst_left + off + 1] = lb[1];
                            dmem[dst_right + off] = rb[0];
                            dmem[dst_right + off + 1] = rb[1];
                        }
                    }
                    cycles += 20 + (num_samples as u32);
                }

                // POLEF (0x17) / INTERL (0x09) / ADDMIXER (0x0A): filters/misc
                0x09 | 0x0A | 0x17 => {
                    cycles += 30;
                }

                // Unknown/unimplemented
                _ => {
                    log(LogCategory::Stubs, LogLevel::Debug, || {
                        format!(
                            "RSP HLE ABI1: Unknown command 0x{:02X} (word0=0x{:08X} word1=0x{:08X})",
                            cmd, word0, word1
                        )
                    });
                }
            }
        }

        1500 + cycles
    }

    /// Parse F3DEX display list and generate RDP commands
    fn parse_f3dex_display_list(
        &mut self,
        rdram: &[u8],
        start_addr: u32,
        _size: u32,
        rdp: &mut Rdp,
    ) {
        // Convert virtual address to physical address
        let mut addr = Self::virt_to_phys(start_addr);
        let max_commands = 100_000; // Safety limit to prevent infinite loops
        let mut commands_processed = 0;
        self.dl_debug_addr = start_addr;
        self.dl_debug_depth += 1;
        
        // Debug: we'll track commands and dump if we see a zero matrix
        static DUMP_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let should_track = self.dl_debug_depth == 1 && !DUMP_DONE.load(std::sync::atomic::Ordering::Relaxed);
        let mut cmd_log: Vec<String> = Vec::new();
        let mut found_zero_mtx = false;

        log(LogCategory::PPU, LogLevel::Info, || {
            format!(
                "RSP HLE: Parsing F3DEX display list at virt:0x{:08X} phys:0x{:08X}",
                start_addr, addr
            )
        });

        while addr + 7 < rdram.len() && commands_processed < max_commands {
            // Read 64-bit F3DEX command
            let word0 = u32::from_be_bytes([
                rdram[addr],
                rdram[addr + 1],
                rdram[addr + 2],
                rdram[addr + 3],
            ]);
            let word1 = u32::from_be_bytes([
                rdram[addr + 4],
                rdram[addr + 5],
                rdram[addr + 6],
                rdram[addr + 7],
            ]);

            let cmd_id = (word0 >> 24) & 0xFF;
            
            if should_track {
                cmd_log.push(format!("  cmd[{}] @ 0x{:06X}: id=0x{:02X} w0=0x{:08X} w1=0x{:08X}", commands_processed, addr, cmd_id, word0, word1));
            }


            // TEXTURE_RECTANGLE (0xE4/0xE5) is a 3-entry compound command in F3DEX2:
            // Entry 1 (0xE4): rect coords + tile
            // Entry 2: 0x00000000 | S.10.5 | T.10.5
            // Entry 3: 0x00000000 | DSDX.10.5 | DTDY.10.5
            let extra_stride = if (cmd_id == 0xE4 || cmd_id == 0xE5) && addr + 23 < rdram.len() {
                let w2 = u32::from_be_bytes([
                    rdram[addr + 12],
                    rdram[addr + 13],
                    rdram[addr + 14],
                    rdram[addr + 15],
                ]);
                let w3 = u32::from_be_bytes([
                    rdram[addr + 20],
                    rdram[addr + 21],
                    rdram[addr + 22],
                    rdram[addr + 23],
                ]);
                // Forward to RDP with texture coordinate data
                let rdp_cmd_id = cmd_id & 0x3F;
                rdp.execute_rdp_command(rdp_cmd_id, word0, word1, w2, w3, rdram);
                16 // skip the 2 extra 8-byte entries
            } else {
                let should_continue =
                    self.execute_display_list_command(cmd_id, word0, word1, rdram, rdp);

                if !should_continue {
                    break; // G_ENDDL or branch command
                }
                0
            };

            addr += 8 + extra_stride;
            commands_processed += 1;
            if should_track && self.dl_debug_found_zero {
                found_zero_mtx = true;
            }
        }
        self.dl_debug_depth -= 1;
        if should_track && found_zero_mtx {
            DUMP_DONE.store(true, std::sync::atomic::Ordering::Relaxed);
            eprintln!("ZERO_MTX_DL at 0x{:08X} ({} cmds):", start_addr, commands_processed);
            for line in &cmd_log {
                eprintln!("{}", line);
            }
        }
    }

    /// Dispatch a display list command, translating F3DEX IDs to F3DEX2 equivalents.
    ///
    /// F3DEX (original Fast3D, used by SM64/MK64) and F3DEX2 use different command IDs:
    ///   | Command    | F3DEX | F3DEX2 |
    ///   |------------|-------|--------|
    ///   | G_VTX      | 0x04  | 0x01   |
    ///   | G_MTX      | 0x01  | 0xDA   |
    ///   | G_DL       | 0x06  | 0xDE   |
    ///   | G_ENDDL    | 0xB8  | 0xDF   |
    ///   | G_TRI1     | 0xBF  | 0x05   |
    ///   | G_TRI2     | 0xB1  | 0x06   |
    ///   | G_TEXTURE  | 0xBB  | 0xD7   |
    ///   | G_POPMTX   | 0xBD  | 0xD8   |
    ///   | G_MOVEWORD | 0xBC  | 0xDB   |
    ///   | G_MOVEMEM  | 0x03  | 0xDC   |
    fn execute_display_list_command(
        &mut self,
        cmd_id: u32,
        word0: u32,
        word1: u32,
        rdram: &[u8],
        rdp: &mut Rdp,
    ) -> bool {
        if self.microcode == MicrocodeType::F3DEX {
            // F3DEX (original Fast3D) command dispatch
            match cmd_id {
                // --- F3DEX-specific IDs that conflict with F3DEX2 ---
                // 0x01 = G_MTX in F3DEX (vs G_VTX in F3DEX2)
                0x01 => self.handle_g_mtx(word0, word1, rdram, rdp),
                // 0x03 = G_MOVEMEM in F3DEX (vs unused in F3DEX2)
                0x03 => self.handle_g_movemem(word0, word1, rdram),
                // 0x04 = G_VTX in F3DEX (vs G_BRANCH_Z in F3DEX2)
                0x04 => self.handle_g_vtx_f3dex(word0, word1, rdram),
                // 0x06 = G_DL in F3DEX (vs G_TRI2 in F3DEX2!)
                0x06 => self.handle_g_dl(word0, word1, rdram, rdp),
                // 0xB1 = G_TRI2 in F3DEX
                0xB1 => self.handle_g_tri2(word0, word1, rdp),
                // 0xB8 = G_ENDDL in F3DEX
                0xB8 => false,
                // 0xB9 = G_SETOTHERMODE_L in F3DEX
                0xB9 => {
                    self.handle_g_setothermode_l(word0, word1, rdp);
                    true
                }
                // 0xBA = G_SETOTHERMODE_H in F3DEX
                0xBA => {
                    self.handle_g_setothermode_h(word0, word1, rdp);
                    true
                }
                // 0xBB = G_TEXTURE in F3DEX
                0xBB => {
                    self.handle_g_texture(word0, word1);
                    true
                }
                // 0xBC = G_MOVEWORD in F3DEX
                0xBC => {
                    self.handle_g_moveword(word0, word1, rdram);
                    true
                }
                // 0xB6 = G_CLEARGEOMETRYMODE in F3D/F3DEX
                // F3D format: bits in word0 lower 24 bits, word1 = 0
                0xB6 => {
                    let clear_bits = word0 & 0x00FFFFFF;
                    self.geometry_mode &= !clear_bits;
                    log(LogCategory::PPU, LogLevel::Debug, || {
                        format!(
                            "RSP HLE: F3D G_CLEARGEOMETRYMODE - cleared 0x{:08X}, mode now 0x{:08X}",
                            clear_bits, self.geometry_mode
                        )
                    });
                    true
                }
                // 0xB7 = G_SETGEOMETRYMODE in F3D/F3DEX
                // F3D format: bits in word0 lower 24 bits, word1 = 0
                0xB7 => {
                    let set_bits = word0 & 0x00FFFFFF;
                    self.geometry_mode |= set_bits;
                    log(LogCategory::PPU, LogLevel::Debug, || {
                        format!(
                            "RSP HLE: F3D G_SETGEOMETRYMODE - set 0x{:08X}, mode now 0x{:08X}",
                            set_bits, self.geometry_mode
                        )
                    });
                    true
                }
                // 0xBD = G_POPMTX in F3DEX
                0xBD => {
                    self.handle_g_popmtx();
                    true
                }
                // 0xBF = G_TRI1 in F3DEX (!!!)
                0xBF => self.handle_g_tri1_f3dex(word0, word1, rdp),
                // Common commands shared between F3DEX and F3DEX2
                _ => self.execute_f3dex_command(cmd_id, word0, word1, rdram, rdp),
            }
        } else {
            // F3DEX2 command dispatch (existing behavior)
            self.execute_f3dex_command(cmd_id, word0, word1, rdram, rdp)
        }
    }

    /// Handle G_TRI1 in F3D/F3DEX format (0xBF)
    /// F3D format:   word1 = flag(8) | v0*10(8) | v1*10(8) | v2*10(8)
    /// F3DEX format: word1 = flag(8) | v0*2(8) | v1*2(8) | v2*2(8)
    fn handle_g_tri1_f3dex(&mut self, _word0: u32, word1: u32, rdp: &mut Rdp) -> bool {
        // F3D uses index*10 (DMEM vertex size), F3DEX uses index*2
        let divisor = if self.microcode == MicrocodeType::F3DEX2 { 2 } else { 10 };
        let v0 = ((word1 >> 16) & 0xFF) as usize / divisor;
        let v1 = ((word1 >> 8) & 0xFF) as usize / divisor;
        let v2 = (word1 & 0xFF) as usize / divisor;

        if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
            self.draw_transformed_triangle(v0, v1, v2, rdp);
        }
        true
    }

    /// Handle G_VTX in F3DEX format (0x04)
    /// Format differs from F3DEX2 G_VTX (0x01)
    fn handle_g_vtx_f3dex(&mut self, word0: u32, word1: u32, rdram: &[u8]) -> bool {
        // F3D G_VTX (0x04) format:
        //   word0: cmd(8) | ((n-1)<<4)|v0 (8, [23:16]) | byte_count(16, [15:0])
        //   word1: RDRAM address of vertex data
        // The byte at [23:16] encodes n-1 in upper nibble and v0 in lower nibble.
        let n = (((word0 >> 20) & 0xF) + 1) as usize;
        let v0_index = ((word0 >> 16) & 0xF) as usize;
        let vertex_addr = self.resolve_segment_addr(word1);

        for i in 0..n.min(32 - v0_index) {
            let vaddr = vertex_addr + (i as u32 * 16);
            self.load_vertex(rdram, vaddr, v0_index + i);
        }
        true
    }

    /// Handle G_TRI2 (draw two triangles)
    fn handle_g_tri2(&mut self, word0: u32, word1: u32, rdp: &mut Rdp) -> bool {
        // First triangle from word0
        let v0 = ((word0 >> 16) & 0xFF) as usize / 2;
        let v1 = ((word0 >> 8) & 0xFF) as usize / 2;
        let v2 = (word0 & 0xFF) as usize / 2;
        // Second triangle from word1
        let v3 = ((word1 >> 16) & 0xFF) as usize / 2;
        let v4 = ((word1 >> 8) & 0xFF) as usize / 2;
        let v5 = (word1 & 0xFF) as usize / 2;

        if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
            self.draw_transformed_triangle(v0, v1, v2, rdp);
        }
        if v3 < self.vertex_count && v4 < self.vertex_count && v5 < self.vertex_count {
            self.draw_transformed_triangle(v3, v4, v5, rdp);
        }
        true
    }

    /// Handle G_MTX command (load matrix)
    fn handle_g_mtx(&mut self, word0: u32, word1: u32, rdram: &[u8], _rdp: &mut Rdp) -> bool {
        // F3DEX G_MTX (0x01) format:
        //   word0: cmd(8) | params(8) | index(8) | length(8)
        //   word1: RDRAM address of 4x4 matrix (64 bytes, N64 fixed-point format)
        // The params byte is at bits [23:16], NOT the low bits.
        // F3D params (from gbi.h):
        //   bit0 = G_MTX_PROJECTION (0x01) — projection(1) or modelview(0)
        //   bit1 = G_MTX_LOAD (0x02)       — load(1) or multiply(0)
        //   bit2 = G_MTX_PUSH (0x04)       — push(1) or nopush(0)
        // NOTE: F3DEX2 swaps bit0 and bit2!
        let params = (word0 >> 16) & 0xFF;
        let is_projection = (params & 0x01) != 0;
        let is_load = (params & 0x02) != 0;
        let is_push = (params & 0x04) != 0;

        let raw_addr = self.resolve_segment_addr(word1);
        let matrix = self.load_matrix_from_rdram(rdram, raw_addr);

        // Only log matrices that are mostly zero (likely the broken one)
        let nonzero_count = matrix.iter().filter(|&&v| v.abs() > 0.001).count();
        if nonzero_count <= 1 {
            self.dl_debug_found_zero = true;
            self.zero_mtx_count += 1;
            let phys = Self::virt_to_phys(raw_addr);
            eprintln!("ZERO_MTX: params=0x{:02X} proj={} load={} push={} addr=0x{:08X} word1=0x{:08X} dl_depth={} dl_addr=0x{:08X}",
                params, is_projection, is_load, is_push, raw_addr, word1, self.dl_debug_depth, self.dl_debug_addr);
            // Also dump segment table
            let segs: Vec<String> = self.segment_bases.iter().enumerate()
                .filter(|(_, &v)| v != 0)
                .map(|(i, &v)| format!("{}=0x{:06X}", i, v))
                .collect();
            eprintln!("  segments: [{}]", segs.join(", "));
            if phys + 63 < rdram.len() {
                // Dump all 16 u32 words (64 bytes) for full picture
                for row in 0..4 {
                    let o = phys + row * 8;
                    let w0 = u32::from_be_bytes([rdram[o], rdram[o+1], rdram[o+2], rdram[o+3]]);
                    let w1 = u32::from_be_bytes([rdram[o+4], rdram[o+5], rdram[o+6], rdram[o+7]]);
                    let fo = phys + 32 + row * 8;
                    let f0 = u32::from_be_bytes([rdram[fo], rdram[fo+1], rdram[fo+2], rdram[fo+3]]);
                    let f1 = u32::from_be_bytes([rdram[fo+4], rdram[fo+5], rdram[fo+6], rdram[fo+7]]);
                    eprintln!("  row{}: int={:08X} {:08X}  frac={:08X} {:08X}", row, w0, w1, f0, f1);
                }
            }
        }

        if is_projection {
            if is_load {
                self.projection_matrix = matrix;
            } else {
                self.projection_matrix = Self::multiply_matrix(&matrix, &self.projection_matrix);
            }
        } else {
            if is_push && self.matrix_stack_ptr < 9 {
                self.matrix_stack[self.matrix_stack_ptr] = self.modelview_matrix;
                self.matrix_stack_ptr += 1;
            }
            if is_load {
                self.modelview_matrix = matrix;
            } else {
                self.modelview_matrix = Self::multiply_matrix(&matrix, &self.modelview_matrix);
            }
        }

        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "RSP HLE: G_MTX - proj={}, load={}, push={}, addr=0x{:08X}",
                is_projection, is_load, is_push, raw_addr
            )
        });

        true
    }

    /// Handle G_DL command (display list call/branch)
    fn handle_g_dl(&mut self, word0: u32, word1: u32, rdram: &[u8], rdp: &mut Rdp) -> bool {
        let branch_type = (word0 >> 16) & 0xFF;
        let dl_addr = self.resolve_segment_addr(word1);
        let is_push = branch_type == 0;

        if is_push {
            self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
            true
        } else {
            self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
            false
        }
    }

    /// Handle G_MOVEMEM command
    fn handle_g_movemem(&mut self, word0: u32, word1: u32, rdram: &[u8]) -> bool {
        // F3D  (0x03): gDma1p layout: cmd(8) | index(8) | unused(8) | length(8)
        //   index at bits[23:16] (e.g. 0x80 = viewport, 0x86+ = lights)
        //   length at bits[7:0]
        // F3DEX2 (0xDC): word0 = cmd(8) | (size/8-1 << 3)(5+3) | offset/8(8) | index(8)
        //   index at bits[7:0]
        let index = (word0 >> 16) & 0xFF;
        let offset = (word0 >> 8) & 0xFF;
        let addr = self.resolve_segment_addr(word1);
        let phys_addr = Self::virt_to_phys(addr);

        // Handle specific movemem types
        match index {
            // Viewport
            0x80 | 0x08 => {
                if phys_addr + 15 < rdram.len() {
                    let scale_x =
                        i16::from_be_bytes([rdram[phys_addr], rdram[phys_addr + 1]]) as f32 / 4.0;
                    let scale_y =
                        i16::from_be_bytes([rdram[phys_addr + 2], rdram[phys_addr + 3]]) as f32
                            / 4.0;
                    let scale_z =
                        i16::from_be_bytes([rdram[phys_addr + 4], rdram[phys_addr + 5]]) as f32;
                    let trans_x =
                        i16::from_be_bytes([rdram[phys_addr + 8], rdram[phys_addr + 9]]) as f32
                            / 4.0;
                    let trans_y =
                        i16::from_be_bytes([rdram[phys_addr + 10], rdram[phys_addr + 11]]) as f32
                            / 4.0;
                    let trans_z =
                        i16::from_be_bytes([rdram[phys_addr + 12], rdram[phys_addr + 13]]) as f32;
                    self.viewport = (
                        trans_x - scale_x,
                        trans_y - scale_y,
                        scale_x * 2.0,
                        scale_y * 2.0,
                        scale_x,
                        scale_y,
                    );
                    self.viewport_z_scale = scale_z;
                    self.viewport_z_trans = trans_z;
                }
            }
            // Light data
            0x86..=0x9E | 0x0A..=0x0E => {
                let light_idx = if index >= 0x86 {
                    ((index - 0x86) / 0x18) as usize
                } else {
                    ((index - 0x0A) / 2) as usize
                };
                if light_idx < 8 && phys_addr + 15 < rdram.len() {
                    let r = rdram[phys_addr] as f32 / 255.0;
                    let g = rdram[phys_addr + 1] as f32 / 255.0;
                    let b = rdram[phys_addr + 2] as f32 / 255.0;
                    let dx = rdram[phys_addr + 8] as i8 as f32 / 127.0;
                    let dy = rdram[phys_addr + 9] as i8 as f32 / 127.0;
                    let dz = rdram[phys_addr + 10] as i8 as f32 / 127.0;
                    self.lights[light_idx] = [dx, dy, dz, r, g, b, 0.0];
                }
            }
            _ => {}
        }

        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "RSP HLE: G_MOVEMEM - index=0x{:02X}, offset=0x{:02X}, addr=0x{:08X}",
                index, offset, addr
            )
        });
        true
    }

    /// Handle G_SETOTHERMODE_L command
    fn handle_g_setothermode_l(&mut self, word0: u32, word1: u32, rdp: &mut Rdp) {
        // F3D/F3DEX format: word0 = cmd(8) | 0(8) | shift(8) | length(8)
        //   gsSPSetOtherMode: _SHIFTL(cmd,24,8) | _SHIFTL(sft,8,8) | _SHIFTL(len,0,8)
        let (shift, length) = if self.microcode == MicrocodeType::F3DEX {
            ((word0 >> 8) & 0xFF, word0 & 0xFF)
        } else {
            // F3DEX2 format: word0 = cmd(8) | (32-sft-len)(8) | (len-1)(8)
            let n = (word0 >> 8) & 0xFF;
            let len_m1 = word0 & 0xFF;
            let length = len_m1 + 1;
            let shift = 32u32.wrapping_sub(n).wrapping_sub(length);
            (shift, length)
        };
        let length = length.min(32);
        let mask = if length >= 32 { !0u32 } else { ((1u32 << length) - 1) << shift };
        self.othermode_l = (self.othermode_l & !mask) | (word1 & mask);

        // Forward combined othermode to RDP as SET_OTHER_MODES
        let combined = ((self.othermode_h as u64) << 32) | (self.othermode_l as u64);
        rdp.set_othermode(combined);
    }

    /// Handle G_SETOTHERMODE_H command
    fn handle_g_setothermode_h(&mut self, word0: u32, word1: u32, rdp: &mut Rdp) {
        let (shift, length) = if self.microcode == MicrocodeType::F3DEX {
            // F3D/F3DEX: shift at bits 15:8, length at bits 7:0
            ((word0 >> 8) & 0xFF, word0 & 0xFF)
        } else {
            // F3DEX2 format
            let n = (word0 >> 8) & 0xFF;
            let len_m1 = word0 & 0xFF;
            let length = len_m1 + 1;
            let shift = 32u32.wrapping_sub(n).wrapping_sub(length);
            (shift, length)
        };
        let length = length.min(32);
        let mask = if length >= 32 { !0u32 } else { ((1u32 << length) - 1) << shift };
        self.othermode_h = (self.othermode_h & !mask) | (word1 & mask);

        // Forward combined othermode to RDP
        let combined = ((self.othermode_h as u64) << 32) | (self.othermode_l as u64);
        rdp.set_othermode(combined);
    }

    /// Handle G_TEXTURE command (F3D/F3DEX: 0xBB)
    fn handle_g_texture(&mut self, word0: u32, word1: u32) {
        let _tile = (word0 >> 8) & 0x07;
        let _level = (word0 >> 11) & 0x07;
        // F3D format: on is at bit 1, 7 bits wide (_SHIFTL(on, 1, 7))
        let on = (word0 >> 1) & 0x7F;
        let _scale_s = (word1 >> 16) & 0xFFFF;
        let _scale_t = word1 & 0xFFFF;

        // Update geometry mode texture enable flag
        if on != 0 {
            self.geometry_mode |= G_TEXTURE_ENABLE;
        } else {
            self.geometry_mode &= !G_TEXTURE_ENABLE;
        }

        log(LogCategory::PPU, LogLevel::Debug, || {
            format!("RSP HLE: G_TEXTURE - tile={}, on={}", _tile, on)
        });
    }

    /// Handle G_MOVEWORD command
    fn handle_g_moveword(&mut self, word0: u32, word1: u32, _rdram: &[u8]) {
        // F3DEX and F3DEX2 have different field layouts for G_MOVEWORD:
        //   F3DEX  (0xBC): word0 = cmd(8) | offset(16) | index(8)
        //   F3DEX2 (0xDB): word0 = cmd(8) | index(8) | offset(16)
        let (index, offset) = if self.microcode == MicrocodeType::F3DEX {
            (word0 & 0xFF, (word0 >> 8) & 0xFFFF)
        } else {
            ((word0 >> 16) & 0xFF, word0 & 0xFFFF)
        };
        match index {
            0x06 => {
                // G_MW_SEGMENT - set segment base address
                let segment = (offset >> 2) & 0x0F;
                self.segment_bases[segment as usize] = word1 & 0x00FFFFFF;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "RSP HLE: G_MOVEWORD SEGMENT[{}] = 0x{:08X}",
                        segment, word1
                    )
                });
            }
            0x0E => {
                // G_MW_NUMLIGHT - set number of lights
                self.num_lights = (word1 / 24) as usize;
            }
            _ => {
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "RSP HLE: G_MOVEWORD index=0x{:02X} offset=0x{:04X} data=0x{:08X}",
                        index, offset, word1
                    )
                });
            }
        }
    }

    /// Handle G_POPMTX command
    fn handle_g_popmtx(&mut self) {
        if self.matrix_stack_ptr > 0 {
            self.matrix_stack_ptr -= 1;
            self.modelview_matrix = self.matrix_stack[self.matrix_stack_ptr];
        }
    }

    /// Execute a single F3DEX display list command
    /// Returns false if display list should terminate (G_ENDDL)
    fn execute_f3dex_command(
        &mut self,
        cmd_id: u32,
        word0: u32,
        word1: u32,
        rdram: &[u8],
        rdp: &mut Rdp,
    ) -> bool {
        // Log F3DEX commands for debugging
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "F3DEX cmd: 0x{:02X} w0:0x{:08X} w1:0x{:08X}",
                cmd_id, word0, word1
            )
        });

        match cmd_id {
            // G_BRANCH_Z (0xB0) - Conditional branch based on Z-buffer
            0xB0 => {
                // word0: cmd_id | vtx (vertex index, bits 11-1) | zval (Z value for comparison)
                // word1: RDRAM address to branch to if condition is met
                let vertex_index = ((word0 >> 1) & 0x7FF) as usize / 2;
                let branch_addr = self.resolve_segment_addr(word1);

                // For now, implement simplified version that always branches
                // Full implementation would:
                // 1. Get Z value from specified vertex
                // 2. Compare with RDP Z-buffer at vertex's screen position
                // 3. Branch only if Z test passes (vertex is visible)
                // Since we don't have per-vertex Z-buffer access yet, we conditionally branch
                // based on whether the vertex is in bounds (simplified heuristic)

                if vertex_index < self.vertex_count {
                    // Vertex exists, parse the branch target display list
                    // In a more complete implementation, we'd check Z-buffer value
                    self.parse_f3dex_display_list(rdram, branch_addr, 10000, rdp);
                }
                true
            }
            // G_CLEARGEOMETRYMODE (0xB6) - Clear geometry mode bits
            0xB6 => {
                // word1: bits to clear
                let clear_bits = word1;
                self.geometry_mode &= !clear_bits;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "RSP HLE: G_CLEARGEOMETRYMODE - cleared 0x{:08X}, mode now 0x{:08X}",
                        clear_bits, self.geometry_mode
                    )
                });
                true
            }
            // G_SETGEOMETRYMODE (0xB7) - Set geometry mode bits
            0xB7 => {
                // word1: bits to set
                let set_bits = word1;
                self.geometry_mode |= set_bits;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "RSP HLE: G_SETGEOMETRYMODE - set 0x{:08X}, mode now 0x{:08X}",
                        set_bits, self.geometry_mode
                    )
                });
                true
            }
            // G_VTX (0x01) - Load vertices
            0x01 => {
                // F3DEX2 format:
                // word0: cmd_id(8) | numv(8, bits 19:12) | (vbidx + numv)(7, bits 7:1)
                // word1: vertex data address in RDRAM
                let vertex_count = ((word0 >> 12) & 0xFF) as usize;
                let vbidx_plus_n = ((word0 >> 1) & 0x7F) as usize;
                let buffer_index = vbidx_plus_n.saturating_sub(vertex_count);
                let vertex_addr = self.resolve_segment_addr(word1);

                // Load vertices from RDRAM into vertex buffer
                for i in 0..vertex_count.min(32 - buffer_index) {
                    let vaddr = vertex_addr + (i as u32 * 16);
                    self.load_vertex(rdram, vaddr, buffer_index + i);
                }
                true
            }
            // G_TRI1 (0x04) - Draw single triangle (alternate encoding)
            0x04 => {
                // Alternative encoding used by some games
                // word0: cmd_id | v0_index | v1_index | v2_index
                let v0 = ((word0 >> 16) & 0xFF) as usize / 2;
                let v1 = ((word0 >> 8) & 0xFF) as usize / 2;
                let v2 = (word0 & 0xFF) as usize / 2;

                if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
                    self.draw_transformed_triangle(v0, v1, v2, rdp);
                }
                true
            }
            // G_TRI1 (0x05) - Draw single triangle
            0x05 => {
                // word0: cmd_id | v0_index (bits 16-23) | v1_index (bits 8-15) | v2_index (bits 0-7)
                let v0 = ((word0 >> 16) & 0xFF) as usize / 2;
                let v1 = ((word0 >> 8) & 0xFF) as usize / 2;
                let v2 = (word0 & 0xFF) as usize / 2;

                if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
                    self.draw_transformed_triangle(v0, v1, v2, rdp);
                }
                true
            }
            // G_TRI2 (0x06) - Draw two triangles
            0x06 => {
                // First triangle
                let v0 = ((word0 >> 16) & 0xFF) as usize / 2;
                let v1 = ((word0 >> 8) & 0xFF) as usize / 2;
                let v2 = (word0 & 0xFF) as usize / 2;

                if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
                    self.draw_transformed_triangle(v0, v1, v2, rdp);
                }

                // Second triangle
                let v3 = ((word1 >> 16) & 0xFF) as usize / 2;
                let v4 = ((word1 >> 8) & 0xFF) as usize / 2;
                let v5 = (word1 & 0xFF) as usize / 2;

                if v3 < self.vertex_count && v4 < self.vertex_count && v5 < self.vertex_count {
                    self.draw_transformed_triangle(v3, v4, v5, rdp);
                }
                true
            }
            // G_QUAD (0x07) - Draw quadrilateral (two triangles)
            0x07 => {
                // Quad is drawn as two triangles sharing an edge
                // word0: cmd_id | v0_index | v1_index | v2_index
                // word1: v0_index | v2_index | v3_index (second triangle)
                let v0 = ((word0 >> 16) & 0xFF) as usize / 2;
                let v1 = ((word0 >> 8) & 0xFF) as usize / 2;
                let v2 = (word0 & 0xFF) as usize / 2;

                // First triangle: v0, v1, v2
                if v0 < self.vertex_count && v1 < self.vertex_count && v2 < self.vertex_count {
                    self.draw_transformed_triangle(v0, v1, v2, rdp);
                }

                // Second triangle uses vertices from word1
                let v0_2 = ((word1 >> 16) & 0xFF) as usize / 2;
                let v2_2 = ((word1 >> 8) & 0xFF) as usize / 2;
                let v3 = (word1 & 0xFF) as usize / 2;

                if v0_2 < self.vertex_count && v2_2 < self.vertex_count && v3 < self.vertex_count {
                    self.draw_transformed_triangle(v0_2, v2_2, v3, rdp);
                }
                true
            }
            // G_POPMTX (0xD8) - Pop matrix from stack
            0xD8 => {
                // word0: cmd_id | padding
                // word1: number of matrices to pop (in units of 64 bytes each)
                let num_matrices = ((word1 >> 6) & 0xFF) as usize; // Divide by 64 to get count

                // Pop the specified number of matrices from the modelview stack
                for _ in 0..num_matrices.min(self.matrix_stack_ptr) {
                    if self.matrix_stack_ptr > 0 {
                        self.matrix_stack_ptr -= 1;
                        self.modelview_matrix = self.matrix_stack[self.matrix_stack_ptr];
                    }
                }
                true
            }
            // G_MTX (0xDA) - Load transformation matrix
            0xDA => {
                // word0: cmd_id | param (push/nopush, load/mul, projection/modelview)
                // word1: RDRAM address of matrix (64 bytes, 4x4 matrix of 16.16 fixed point)
                let param = word0 & 0xFF;
                let matrix_addr = self.resolve_segment_addr(word1);

                // Parse matrix parameters
                // F3DEX2 inverts the push bit (XOR with G_MTX_PUSH in gsSPMatrix macro)
                let push = (param & 0x01) == 0; // Inverted: 0=push, 1=nopush
                let load = (param & 0x02) != 0; // G_MTX_LOAD (bit set = load)
                let projection = (param & 0x04) != 0; // G_MTX_PROJECTION (vs G_MTX_MODELVIEW)

                // Load matrix from RDRAM
                let matrix = self.load_matrix_from_rdram(rdram, matrix_addr);

                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "RSP HLE: G_MTX addr=0x{:08X} type={} mode={} push={}",
                        matrix_addr,
                        if projection { "PROJ" } else { "MV" },
                        if load { "LOAD" } else { "MUL" },
                        push
                    )
                });

                // Apply matrix based on type
                if projection {
                    // Projection matrix
                    if load {
                        // Replace projection matrix
                        self.projection_matrix = matrix;
                    } else {
                        // Multiply with existing projection matrix: stack = stack * new
                        self.projection_matrix =
                            Self::multiply_matrix(&matrix, &self.projection_matrix);
                    }
                } else {
                    // Modelview matrix
                    if push {
                        // Push current modelview matrix to stack
                        if self.matrix_stack_ptr < 10 {
                            self.matrix_stack[self.matrix_stack_ptr] = self.modelview_matrix;
                            self.matrix_stack_ptr += 1;
                        }
                    }
                    if load {
                        // Replace modelview matrix
                        self.modelview_matrix = matrix;
                    } else {
                        // Multiply with existing modelview matrix: stack = stack * new
                        self.modelview_matrix =
                            Self::multiply_matrix(&matrix, &self.modelview_matrix);
                    }
                }
                true
            }
            // G_GEOMETRYMODE (0xD9) - Set rendering mode flags
            0xD9 => {
                // word0: bits to clear (inverted mask)
                // word1: bits to set
                let clear_bits = word0 & 0x00FFFFFF;
                let set_bits = word1;

                // Clear specified bits then set new bits
                self.geometry_mode = (self.geometry_mode & !clear_bits) | set_bits;
                true
            }
            // G_MOVEWORD (0xDB) - Modify internal state word
            0xDB => {
                // word0: cmd_id | index (which word to modify) | offset (within that word)
                // word1: value to write
                let index = (word0 >> 16) & 0xFF;
                let offset = word0 & 0xFFFF;
                let value = word1;

                // Common indices:
                // 0x00: G_MW_MATRIX - Modify current matrix
                // 0x02: G_MW_NUMLIGHT - Set number of lights
                // 0x04: G_MW_CLIP - Modify clipping planes
                // 0x06: G_MW_SEGMENT - Set segment address
                // 0x08: G_MW_FOG - Fog parameters
                // 0x0A: G_MW_LIGHTCOL - Light color
                // 0x0C: G_MW_POINTS - Point rendering params
                // 0x0E: G_MW_PERSPNORM - Perspective normalization

                match index {
                    0x02 => {
                        // G_MW_NUMLIGHT - Set number of lights
                        // value is the number of lights to use (usually multiplied by 24 in the macro)
                        // The actual number of lights is value / 24
                        let num_lights = (value / 24).min(8) as usize;
                        self.num_lights = num_lights;
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            format!("RSP HLE: G_MOVEWORD numlight - set to {}", num_lights)
                        });
                    }
                    0x06 => {
                        // G_MW_SEGMENT - Set segment base address
                        // offset contains the segment number (0-15)
                        // value is the base address for that segment
                        let segment = (offset / 4) as usize;
                        if segment < 16 {
                            self.segment_bases[segment] = value;
                            log(LogCategory::PPU, LogLevel::Debug, || {
                                format!(
                                    "RSP HLE: G_MOVEWORD segment - segment 0x{:X} = 0x{:08X}",
                                    segment, value
                                )
                            });
                        }
                    }
                    _ => {
                        // Other indices not yet implemented
                        // Log at debug level since these may not be critical for basic rendering
                        log(LogCategory::Stubs, LogLevel::Debug, || {
                            format!(
                                "N64 RSP HLE: G_MOVEWORD - index=0x{:02X}, offset=0x{:04X}, value=0x{:08X}",
                                index, offset, value
                            )
                        });
                    }
                }
                true
            }
            // G_MOVEMEM (0xDC) - Load memory segment
            0xDC => {
                // F3DEX2 G_MOVEMEM: word0 = cmd(8) | ((size-1)/8 << 3)(5) | ... | (offset/8)(8) | index(8)
                // word1: RDRAM address to load from
                let index = (word0 & 0xFF) as usize;
                let offset = ((word0 >> 8) & 0xFF) as usize * 8; // stored as offset/8
                let size = ((((word0 >> 19) & 0x1F) + 1) * 8) as usize;
                let rdram_addr = self.resolve_segment_addr(word1);

                // F3DEX2 G_MV indices:
                // 0x08 = G_MV_VIEWPORT
                // 0x0A = G_MV_LIGHT (with offset determining which light)
                const G_MV_VIEWPORT: usize = 0x08;
                const G_MV_LIGHT: usize = 0x0A;

                if index == G_MV_VIEWPORT {
                    // Load viewport data from RDRAM
                    // Viewport format: Vp_t structure with i16 values in s13.2 format
                    // vscale[4]: i16 (divide by 4 for pixel values)
                    // vtrans[4]: i16 (divide by 4 for pixel values)
                    let addr = Self::virt_to_phys(rdram_addr);
                    if addr + 15 < rdram.len() {
                        // Read scale values (i16, s13.2 fixed point)
                        let vscale_x =
                            i16::from_be_bytes([rdram[addr], rdram[addr + 1]]) as f32 / 4.0;
                        let vscale_y =
                            i16::from_be_bytes([rdram[addr + 2], rdram[addr + 3]]) as f32 / 4.0;
                        let vscale_z =
                            i16::from_be_bytes([rdram[addr + 4], rdram[addr + 5]]) as f32;

                        // Read translation values (i16, s13.2, at offset +8)
                        let vtrans_x =
                            i16::from_be_bytes([rdram[addr + 8], rdram[addr + 9]]) as f32 / 4.0;
                        let vtrans_y =
                            i16::from_be_bytes([rdram[addr + 10], rdram[addr + 11]]) as f32 / 4.0;
                        let vtrans_z =
                            i16::from_be_bytes([rdram[addr + 12], rdram[addr + 13]]) as f32;

                        // Calculate viewport bounds
                        // x = vtrans_x - vscale_x, y = vtrans_y - vscale_y
                        // width = vscale_x * 2, height = vscale_y * 2
                        let vp_x = vtrans_x - vscale_x;
                        let vp_y = vtrans_y - vscale_y;
                        let vp_width = vscale_x * 2.0;
                        let vp_height = vscale_y * 2.0;

                        self.viewport = (vp_x, vp_y, vp_width, vp_height, vscale_x, vscale_y);
                        self.viewport_z_scale = vscale_z;
                        self.viewport_z_trans = vtrans_z;

                        log(LogCategory::PPU, LogLevel::Debug, || {
                            format!(
                                "RSP HLE: G_MOVEMEM viewport - x={:.1}, y={:.1}, w={:.1}, h={:.1}",
                                vp_x, vp_y, vp_width, vp_height
                            )
                        });
                    }
                } else if index == G_MV_LIGHT {
                    // Load light data from RDRAM
                    // F3DEX2: offset determines which light (offset/0x18 = light index)
                    let light_index = offset / 0x18;

                    if light_index < 8 {
                        let addr = Self::virt_to_phys(rdram_addr);
                        if addr + 15 < rdram.len() {
                            // Read light color (RGB, 0-255)
                            let r = rdram[addr] as f32 / 255.0;
                            let g = rdram[addr + 1] as f32 / 255.0;
                            let b = rdram[addr + 2] as f32 / 255.0;

                            // Read light direction (signed bytes at offset 8-10 in Light_t)
                            // Light_t: col[3](0-2), pad(3), colc[3](4-6), pad(7), dir[3](8-10), pad(11)
                            let dx = (rdram[addr + 8] as i8) as f32 / 127.0;
                            let dy = (rdram[addr + 9] as i8) as f32 / 127.0;
                            let dz = (rdram[addr + 10] as i8) as f32 / 127.0;

                            // Store light data: [dx, dy, dz, r, g, b, type]
                            // type: 0.0 = directional, 1.0 = point (for future use)
                            self.lights[light_index] = [dx, dy, dz, r, g, b, 0.0];

                            if light_index >= self.num_lights {
                                self.num_lights = light_index + 1;
                            }

                            log(LogCategory::PPU, LogLevel::Debug, || {
                                format!(
                                    "RSP HLE: G_MOVEMEM light {} - color=({:.2},{:.2},{:.2}), dir=({:.2},{:.2},{:.2})",
                                    light_index, r, g, b, dx, dy, dz
                                )
                            });
                        }
                    }
                } else {
                    // Other MOVEMEM types - log but don't implement
                    log(LogCategory::Stubs, LogLevel::Debug, || {
                        format!(
                            "N64 RSP HLE: G_MOVEMEM stub - size={}, offset=0x{:04X}, addr=0x{:08X}",
                            size, offset, rdram_addr
                        )
                    });
                }
                true
            }
            // G_TEXTURE (0xD7) - Configure texture settings
            0xD7 => {
                // word0: cmd_id | level (mipmap level) | tile | on (enable/disable)
                // word1: scaleS(16) | scaleT(16) - texture coordinate scaling
                let _level = (word0 >> 11) & 0x07;
                let _tile = (word0 >> 8) & 0x07;
                let on = (word0 >> 1) & 0x7F; // Non-zero = texture on
                let _scale_s = (word1 >> 16) & 0xFFFF;
                let _scale_t = word1 & 0xFFFF;

                // Update geometry mode texture enable flag
                if on != 0 {
                    self.geometry_mode |= G_TEXTURE_ENABLE;
                } else {
                    self.geometry_mode &= !G_TEXTURE_ENABLE;
                }

                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!(
                        "N64 RSP HLE: G_TEXTURE - tile={}, on={}, scaleS=0x{:04X}, scaleT=0x{:04X}",
                        _tile, on, _scale_s, _scale_t
                    )
                });
                true
            }
            // G_SETOTHERMODE_L (0xE2 in F3DEX2) - Set lower other modes  
            0xE2 => {
                self.handle_g_setothermode_l(word0, word1, rdp);
                true
            }
            // G_SETOTHERMODE_H (0xE3 in F3DEX2) - Set upper other modes
            0xE3 => {
                self.handle_g_setothermode_h(word0, word1, rdp);
                true
            }
            // G_LOAD_UCODE (0xAF) - Load new microcode
            0xAF => {
                // word0: cmd_id | size
                // word1: RDRAM address of microcode
                let _size = (word0 & 0xFFFF) as usize;
                let _ucode_addr = word1;

                // For HLE, we don't actually load and execute microcode
                // Instead, we detect the microcode type by signature
                // A full LLE implementation would copy from RDRAM to IMEM
                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!(
                        "N64 RSP HLE: G_LOAD_UCODE - size=0x{:04X}, addr=0x{:08X}",
                        _size, _ucode_addr
                    )
                });
                true
            }
            // G_DL (0xDE) - Display list branch/call
            0xDE => {
                // word0: cmd_id | branch_type (0 = call with return, 1 = branch no return)
                // word1: RDRAM address of display list to execute
                let branch_type = (word0 >> 16) & 0xFF;
                let dl_addr = self.resolve_segment_addr(word1);

                // branch_type: 0 = G_DL_PUSH (call, will return), 1 = G_DL_NOPUSH (branch, no return)
                let is_push = branch_type == 0;

                if is_push {
                    // Call: execute nested DL, then continue current DL
                    self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
                    true
                } else {
                    // Branch: execute new DL, terminate current DL (no return)
                    self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
                    false
                }
            }
            // G_ENDDL (0xDF) - End display list
            0xDF => false,
            // G_RDPHALF_2 (0xB4 in F3DEX, 0xF1 in F3DEX2) - Second half of 2-word RDP command
            0xB4 | 0xF1 => {
                // word1: data (second word for RDP command)
                // This completes a 2-word RDP command using the stored rdp_half value
                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!("N64 RSP HLE: G_RDPHALF_2 - data=0x{:08X}, combining with rdp_half=0x{:08X}", word1, self.rdp_half)
                });
                true
            }
            // G_RDPHALF_1 (0xE1 in F3DEX2) - First half of 2-word RDP command
            0xE1 => {
                self.rdp_half = word1;
                true
            }
            // G_SETPRIMDEPTH (0xEE) - Set primitive depth
            0xEE => {
                // word0: cmd_id | padding
                // word1: z (16-bit) | dz (16-bit) - depth value and delta
                let _z = (word1 >> 16) & 0xFFFF;
                let _dz = word1 & 0xFFFF;

                // For HLE, we log but don't implement primitive depth override
                // Full implementation would set a base depth for subsequent primitives
                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!(
                        "N64 RSP HLE: G_SETPRIMDEPTH - z=0x{:04X}, dz=0x{:04X}",
                        _z, _dz
                    )
                });
                true
            }
            // RDP passthrough commands (0xE0-0xFF) - forward directly to RDP
            0xE0..=0xFF => {
                // These are RDP commands embedded in F3DEX display list
                // Forward them directly to the RDP for execution
                // Common commands: SET_FILL_COLOR (0xF7/0x37), SET_SCISSOR (0xED/0x2D), etc.

                // The RDP command ID is in the lower 6 bits of the command byte
                let rdp_cmd_id = (word0 >> 24) & 0x3F;

                // Call RDP's execute_command directly with the command data
                // Pass rdram for texture loading and other DRAM-dependent commands
                rdp.execute_rdp_command(rdp_cmd_id, word0, word1, 0, 0, rdram);
                true
            }
            // Unknown/unsupported command - skip it
            _ => true,
        }
    }

    /// Compute lit color for a vertex by evaluating all active directional lights.
    /// When lighting is enabled (G_LIGHTING set in geometry_mode), bytes 12-14 of
    /// the vertex data contain the object-space normal (as signed bytes) instead of
    /// RGB colour.  We transform that normal by the upper-left 3×3 of the modelview
    /// matrix, then accumulate light contributions:
    ///   color = ambient + Σ clamp(dot(N, L_i), 0, 1) * light_color_i
    /// The result is returned as [R, G, B] each in 0..=255.
    fn compute_lit_color(&self, vert: &Vertex) -> [u8; 3] {
        // Extract object-space normal from vertex color bytes (reinterpreted as signed)
        let nx = (vert.color[0] as i8) as f32 / 127.0;
        let ny = (vert.color[1] as i8) as f32 / 127.0;
        let nz = (vert.color[2] as i8) as f32 / 127.0;

        // Transform normal by upper-left 3×3 of modelview (column-major layout)
        let tnx = self.modelview_matrix[0] * nx
            + self.modelview_matrix[4] * ny
            + self.modelview_matrix[8] * nz;
        let tny = self.modelview_matrix[1] * nx
            + self.modelview_matrix[5] * ny
            + self.modelview_matrix[9] * nz;
        let tnz = self.modelview_matrix[2] * nx
            + self.modelview_matrix[6] * ny
            + self.modelview_matrix[10] * nz;

        // Normalize
        let len = (tnx * tnx + tny * tny + tnz * tnz).sqrt();
        let (tnx, tny, tnz) = if len > 0.0001 {
            (tnx / len, tny / len, tnz / len)
        } else {
            (0.0, 0.0, 1.0) // fallback upward
        };

        // Start with ambient (last light entry is ambient when num_lights > 0)
        let ambient_idx = self.num_lights; // ambient is one past the last directional
        let (mut r, mut g, mut b) = if ambient_idx < 8 {
            (
                self.lights[ambient_idx][3],
                self.lights[ambient_idx][4],
                self.lights[ambient_idx][5],
            )
        } else {
            (
                self.ambient_light[0],
                self.ambient_light[1],
                self.ambient_light[2],
            )
        };

        // Accumulate directional light contributions
        for i in 0..self.num_lights.min(8) {
            let lx = self.lights[i][0];
            let ly = self.lights[i][1];
            let lz = self.lights[i][2];
            let dot = (tnx * lx + tny * ly + tnz * lz).max(0.0);
            r += dot * self.lights[i][3];
            g += dot * self.lights[i][4];
            b += dot * self.lights[i][5];
        }

        [
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }

    /// Transform vertices and draw triangle via RDP
    fn draw_transformed_triangle(&self, v0: usize, v1: usize, v2: usize, rdp: &mut Rdp) {
        // Get vertices from buffer
        let vert0 = &self.vertices[v0];
        let vert1 = &self.vertices[v1];
        let vert2 = &self.vertices[v2];

        // Transform vertices to clip space
        let clip0 = self.transform_vertex_to_clip(vert0);
        let clip1 = self.transform_vertex_to_clip(vert1);
        let clip2 = self.transform_vertex_to_clip(vert2);

        eprintln!("TRI: v0_pos=[{},{},{}] v1_pos=[{},{},{}] v2_pos=[{},{},{}]",
            vert0.pos[0], vert0.pos[1], vert0.pos[2],
            vert1.pos[0], vert1.pos[1], vert1.pos[2],
            vert2.pos[0], vert2.pos[1], vert2.pos[2]);
        eprintln!("TRI: MV diag=[{:.4},{:.4},{:.4},{:.4}] PJ diag=[{:.4},{:.4},{:.4},{:.4}]",
            self.modelview_matrix[0], self.modelview_matrix[5], self.modelview_matrix[10], self.modelview_matrix[15],
            self.projection_matrix[0], self.projection_matrix[5], self.projection_matrix[10], self.projection_matrix[15]);
        eprintln!("TRI: clip0=[{:.1},{:.1},{:.1},{:.1}] clip1=[{:.1},{:.1},{:.1},{:.1}] clip2=[{:.1},{:.1},{:.1},{:.1}]",
            clip0[0], clip0[1], clip0[2], clip0[3],
            clip1[0], clip1[1], clip1[2], clip1[3],
            clip2[0], clip2[1], clip2[2], clip2[3]);

        // Near-plane clipping: clip against W = NEAR_W plane
        // Vertices with W <= NEAR_W are behind or at the camera
        const NEAR_W: f32 = 0.001;
        let behind0 = clip0[3] <= NEAR_W;
        let behind1 = clip1[3] <= NEAR_W;
        let behind2 = clip2[3] <= NEAR_W;
        let num_behind = behind0 as u8 + behind1 as u8 + behind2 as u8;

        match num_behind {
            3 => (), // All behind camera - reject
            0 => {
                // All in front - check frustum for trivial reject
                // Only reject if ALL 3 vertices are outside the SAME plane
                let all_left = clip0[0] < -clip0[3] && clip1[0] < -clip1[3] && clip2[0] < -clip2[3];
                let all_right = clip0[0] > clip0[3] && clip1[0] > clip1[3] && clip2[0] > clip2[3];
                let all_below = clip0[1] < -clip0[3] && clip1[1] < -clip1[3] && clip2[1] < -clip2[3];
                let all_above = clip0[1] > clip0[3] && clip1[1] > clip1[3] && clip2[1] > clip2[3];
                let all_near = clip0[2] < -clip0[3] && clip1[2] < -clip1[3] && clip2[2] < -clip2[3];
                let all_far = clip0[2] > clip0[3] && clip1[2] > clip1[3] && clip2[2] > clip2[3];
                if all_left || all_right || all_below || all_above || all_near || all_far {
                    return;
                }
                // Draw the triangle directly
                self.draw_clipped_triangle(vert0, &clip0, vert1, &clip1, vert2, &clip2, rdp);
            }
            1 => {
                // One vertex behind camera - clip to produce 2 triangles
                // Reorder so the behind vertex is v_a
                let (va, ca, vb, cb, vc, cc) = if behind0 {
                    (vert0, clip0, vert1, clip1, vert2, clip2)
                } else if behind1 {
                    (vert1, clip1, vert2, clip2, vert0, clip0)
                } else {
                    (vert2, clip2, vert0, clip0, vert1, clip1)
                };
                // Clip edge a->b at W=NEAR_W
                let t_ab = (NEAR_W - ca[3]) / (cb[3] - ca[3]);
                let (v_ab, c_ab) = Self::interpolate_vertex(va, &ca, vb, &cb, t_ab.clamp(0.0, 1.0));
                // Clip edge a->c at W=NEAR_W
                let t_ac = (NEAR_W - ca[3]) / (cc[3] - ca[3]);
                let (v_ac, c_ac) = Self::interpolate_vertex(va, &ca, vc, &cc, t_ac.clamp(0.0, 1.0));
                // Two triangles: (v_ab, vb, vc) and (v_ab, vc, v_ac)
                self.draw_clipped_triangle(&v_ab, &c_ab, vb, &cb, vc, &cc, rdp);
                self.draw_clipped_triangle(&v_ab, &c_ab, vc, &cc, &v_ac, &c_ac, rdp);
            }
            2 => {
                // Two vertices behind camera - clip to produce 1 triangle
                // Reorder so the in-front vertex is v_a
                let (va, ca, vb, cb, vc, cc) = if !behind0 {
                    (vert0, clip0, vert1, clip1, vert2, clip2)
                } else if !behind1 {
                    (vert1, clip1, vert2, clip2, vert0, clip0)
                } else {
                    (vert2, clip2, vert0, clip0, vert1, clip1)
                };
                // Clip edge a->b at W=NEAR_W
                let t_ab = (NEAR_W - ca[3]) / (cb[3] - ca[3]);
                let (v_ab, c_ab) = Self::interpolate_vertex(va, &ca, vb, &cb, t_ab.clamp(0.0, 1.0));
                // Clip edge a->c at W=NEAR_W
                let t_ac = (NEAR_W - ca[3]) / (cc[3] - ca[3]);
                let (v_ac, c_ac) = Self::interpolate_vertex(va, &ca, vc, &cc, t_ac.clamp(0.0, 1.0));
                // One triangle: (va, v_ab, v_ac)
                self.draw_clipped_triangle(va, &ca, &v_ab, &c_ab, &v_ac, &c_ac, rdp);
            }
            _ => unreachable!(),
        }
    }

    /// Draw a triangle that has already been clipped against the near plane.
    /// Performs back-face culling and submits to RDP.
    #[allow(clippy::too_many_arguments)]
    fn draw_clipped_triangle(
        &self,
        vert0: &Vertex,
        clip0: &[f32; 4],
        vert1: &Vertex,
        clip1: &[f32; 4],
        vert2: &Vertex,
        clip2: &[f32; 4],
        rdp: &mut Rdp,
    ) {
        let (sx0, sy0, sz0) = self.clip_to_screen(clip0);
        let (sx1, sy1, sz1) = self.clip_to_screen(clip1);
        let (sx2, sy2, sz2) = self.clip_to_screen(clip2);

        // Back-face culling (in screen space)
        if self.geometry_mode & (G_CULL_FRONT | G_CULL_BACK) != 0 {
            let cross =
                (sx1 - sx0) as i64 * (sy2 - sy0) as i64 - (sx2 - sx0) as i64 * (sy1 - sy0) as i64;
            if (self.geometry_mode & G_CULL_BACK) != 0 && cross >= 0 {
                return;
            }
            if (self.geometry_mode & G_CULL_FRONT) != 0 && cross <= 0 {
                return;
            }
        }

        // Compute vertex colors: if lighting is enabled, evaluate lights;
        // otherwise use the raw vertex colors.
        let (c0, c1, c2) = if self.geometry_mode & G_LIGHTING != 0 && self.num_lights > 0 {
            let lit0 = self.compute_lit_color(vert0);
            let lit1 = self.compute_lit_color(vert1);
            let lit2 = self.compute_lit_color(vert2);
            (
                u32::from_be_bytes([vert0.color[3], lit0[0], lit0[1], lit0[2]]),
                u32::from_be_bytes([vert1.color[3], lit1[0], lit1[1], lit1[2]]),
                u32::from_be_bytes([vert2.color[3], lit2[0], lit2[1], lit2[2]]),
            )
        } else {
            (
                u32::from_be_bytes([
                    vert0.color[3],
                    vert0.color[0],
                    vert0.color[1],
                    vert0.color[2],
                ]),
                u32::from_be_bytes([
                    vert1.color[3],
                    vert1.color[0],
                    vert1.color[1],
                    vert1.color[2],
                ]),
                u32::from_be_bytes([
                    vert2.color[3],
                    vert2.color[0],
                    vert2.color[1],
                    vert2.color[2],
                ]),
            )
        };

        // Draw shaded triangle with Z-buffer (assuming depth values fit in u16)
        let z0_u16 = sz0.clamp(0, 0xFFFF) as u16;
        let z1_u16 = sz1.clamp(0, 0xFFFF) as u16;
        let z2_u16 = sz2.clamp(0, 0xFFFF) as u16;

        eprintln!("DRAW_TRI: screen=({},{})({},{})({},{}) z=({},{},{}) tex={} geom=0x{:08X}",
            sx0, sy0, sx1, sy1, sx2, sy2, z0_u16, z1_u16, z2_u16,
            self.geometry_mode & G_TEXTURE_ENABLE != 0,
            self.geometry_mode);

        // Check if texturing is enabled — use textured draw path when active
        let textures_enabled = self.geometry_mode & G_TEXTURE_ENABLE != 0;
        if textures_enabled {
            // Convert vertex texture coordinates from S.10.5 fixed-point to float texel coords
            let s0_f = vert0.tex[0] as f32 / 32.0;
            let t0_f = vert0.tex[1] as f32 / 32.0;
            let s1_f = vert1.tex[0] as f32 / 32.0;
            let t1_f = vert1.tex[1] as f32 / 32.0;
            let s2_f = vert2.tex[0] as f32 / 32.0;
            let t2_f = vert2.tex[1] as f32 / 32.0;

            rdp.draw_triangle_textured_zbuf(
                sx0, sy0, z0_u16, s0_f, t0_f, sx1, sy1, z1_u16, s1_f, t1_f, sx2, sy2, z2_u16, s2_f, t2_f,
                0, // tile 0
            );
        } else {
            rdp.draw_triangle_shaded_zbuffer(
                sx0, sy0, z0_u16, c0, sx1, sy1, z1_u16, c1, sx2, sy2, z2_u16, c2,
            );
        }
    }

    /// Read 32-bit big-endian value from buffer
    fn read_u32(&self, buffer: &[u8], offset: usize) -> u32 {
        if offset + 3 < buffer.len() {
            u32::from_be_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ])
        } else {
            0
        }
    }

    /// Read 32-bit big-endian value from RDRAM with physical address masking
    fn read_u32_rdram(&self, rdram: &[u8], addr: usize) -> u32 {
        // Mask to physical RDRAM range (4MB = 0x400000)
        let phys_addr = addr & 0x003FFFFF;
        if phys_addr + 3 < rdram.len() {
            u32::from_be_bytes([
                rdram[phys_addr],
                rdram[phys_addr + 1],
                rdram[phys_addr + 2],
                rdram[phys_addr + 3],
            ])
        } else {
            0
        }
    }

    /// Load vertex from RDRAM address
    #[allow(dead_code)]
    fn load_vertex(&mut self, rdram: &[u8], addr: u32, index: usize) {
        if index >= 32 {
            return;
        }

        // Convert virtual address to physical address
        let addr = Self::virt_to_phys(addr);
        if addr + 15 >= rdram.len() {
            return;
        }

        // Vertex format (16 bytes):
        // 0-1: X position (signed 16-bit)
        // 2-3: Y position (signed 16-bit)
        // 4-5: Z position (signed 16-bit)
        // 6-7: Reserved/flags
        // 8-9: S texture coordinate (signed 16-bit)
        // 10-11: T texture coordinate (signed 16-bit)
        // 12: R color (unsigned 8-bit)
        // 13: G color (unsigned 8-bit)
        // 14: B color (unsigned 8-bit)
        // 15: A alpha (unsigned 8-bit)

        let x = i16::from_be_bytes([rdram[addr], rdram[addr + 1]]);
        let y = i16::from_be_bytes([rdram[addr + 2], rdram[addr + 3]]);
        let z = i16::from_be_bytes([rdram[addr + 4], rdram[addr + 5]]);

        let s = i16::from_be_bytes([rdram[addr + 8], rdram[addr + 9]]);
        let t = i16::from_be_bytes([rdram[addr + 10], rdram[addr + 11]]);

        let r = rdram[addr + 12];
        let g = rdram[addr + 13];
        let b = rdram[addr + 14];
        let a = rdram[addr + 15];

        self.vertices[index] = Vertex {
            pos: [x, y, z],
            tex: [s, t],
            color: [r, g, b, a],
        };

        if index >= self.vertex_count {
            self.vertex_count = index + 1;
        }
    }

    /// Transform vertex from object space to clip space (before perspective divide)
    /// Returns clip space coordinates (x, y, z, w) for clipping
    fn transform_vertex_to_clip(&self, vertex: &Vertex) -> [f32; 4] {
        // Convert vertex position to homogeneous coordinates (x, y, z, w=1)
        let v = [
            vertex.pos[0] as f32,
            vertex.pos[1] as f32,
            vertex.pos[2] as f32,
            1.0,
        ];

        // Apply modelview matrix (column-major layout)
        let mut mv = [0.0f32; 4];
        for (i, elem) in mv.iter_mut().enumerate() {
            *elem = self.modelview_matrix[i] * v[0]
                + self.modelview_matrix[i + 4] * v[1]
                + self.modelview_matrix[i + 8] * v[2]
                + self.modelview_matrix[i + 12] * v[3];
        }

        // Apply projection matrix (column-major layout)
        let mut clip = [0.0f32; 4];
        for (i, elem) in clip.iter_mut().enumerate() {
            *elem = self.projection_matrix[i] * mv[0]
                + self.projection_matrix[i + 4] * mv[1]
                + self.projection_matrix[i + 8] * mv[2]
                + self.projection_matrix[i + 12] * mv[3];
        }

        clip
    }

    /// Convert clip space coordinates to screen space
    fn clip_to_screen(&self, clip: &[f32; 4]) -> (i32, i32, i32) {
        // Perspective divide (clip space to NDC)
        // Only divide by positive W; vertices with W <= 0 are behind camera
        let w = if clip[3] > 0.0001 { clip[3] } else { 0.0001 };
        let ndc_x = clip[0] / w;
        let ndc_y = clip[1] / w;
        let ndc_z = clip[2] / w;

        // Clamp NDC coordinates to prevent extreme values from near-plane vertices
        let ndc_x = ndc_x.clamp(-2.0, 2.0);
        let ndc_y = ndc_y.clamp(-2.0, 2.0);
        let ndc_z = ndc_z.clamp(-1.0, 1.0);

        // Viewport transform (NDC to screen space)
        // N64 viewport format:
        //   vscale[0] = width/2, vscale[1] = height/2
        //   vtrans[0] = x + width/2, vtrans[1] = y + height/2
        // Standard transform: screen = vtrans + ndc * vscale
        // With vp_x = vtrans_x - vscale_x and scale_x = vscale_x:
        //   screen_x = (vtrans_x - vscale_x) + (ndc_x + 1.0) * vscale_x
        //            = vtrans_x + ndc_x * vscale_x  ✓
        //   screen_y = (vtrans_y - vscale_y) + (ndc_y + 1.0) * vscale_y
        //            = vtrans_y + ndc_y * vscale_y  ✓
        let (vp_x, vp_y, _vp_width, _vp_height, scale_x, scale_y) = self.viewport;
        let screen_x = (vp_x + (ndc_x + 1.0) * scale_x) as i32;
        let screen_y = (vp_y + (ndc_y + 1.0) * scale_y) as i32;
        let screen_z = (self.viewport_z_trans + ndc_z * self.viewport_z_scale) as i32;

        (screen_x, screen_y, screen_z)
    }

    /// Transform vertex from object space to screen space
    #[allow(dead_code)]
    fn transform_vertex(&self, vertex: &Vertex) -> (i32, i32, i32) {
        let clip = self.transform_vertex_to_clip(vertex);
        self.clip_to_screen(&clip)
    }

    /// Interpolate between two vertices in clip space
    /// t = 0.0 returns v0, t = 1.0 returns v1
    fn interpolate_vertex(
        v0: &Vertex,
        clip0: &[f32; 4],
        v1: &Vertex,
        clip1: &[f32; 4],
        t: f32,
    ) -> (Vertex, [f32; 4]) {
        // Interpolate position in clip space (before perspective divide)
        let clip = [
            clip0[0] + t * (clip1[0] - clip0[0]),
            clip0[1] + t * (clip1[1] - clip0[1]),
            clip0[2] + t * (clip1[2] - clip0[2]),
            clip0[3] + t * (clip1[3] - clip0[3]),
        ];

        // Interpolate vertex attributes
        let pos = [
            (v0.pos[0] as f32 + t * (v1.pos[0] as f32 - v0.pos[0] as f32)) as i16,
            (v0.pos[1] as f32 + t * (v1.pos[1] as f32 - v0.pos[1] as f32)) as i16,
            (v0.pos[2] as f32 + t * (v1.pos[2] as f32 - v0.pos[2] as f32)) as i16,
        ];

        let tex = [
            (v0.tex[0] as f32 + t * (v1.tex[0] as f32 - v0.tex[0] as f32)) as i16,
            (v0.tex[1] as f32 + t * (v1.tex[1] as f32 - v0.tex[1] as f32)) as i16,
        ];

        let color = [
            (v0.color[0] as f32 + t * (v1.color[0] as f32 - v0.color[0] as f32)) as u8,
            (v0.color[1] as f32 + t * (v1.color[1] as f32 - v0.color[1] as f32)) as u8,
            (v0.color[2] as f32 + t * (v1.color[2] as f32 - v0.color[2] as f32)) as u8,
            (v0.color[3] as f32 + t * (v1.color[3] as f32 - v0.color[3] as f32)) as u8,
        ];

        (Vertex { pos, tex, color }, clip)
    }

    /// Clip a line segment against a frustum plane
    /// Returns Some(t) where the intersection occurs, or None if no intersection
    #[allow(dead_code)] // Reserved for future proper triangle clipping
    fn clip_line_to_plane(
        clip0: &[f32; 4],
        clip1: &[f32; 4],
        plane: usize,
        positive: bool,
    ) -> Option<f32> {
        // plane: 0=x, 1=y, 2=z, 3=w
        // positive: true for +plane (right/top/far), false for -plane (left/bottom/near)

        let d0 = if positive {
            clip0[plane] - clip0[3]
        } else {
            -clip0[plane] - clip0[3]
        };
        let d1 = if positive {
            clip1[plane] - clip1[3]
        } else {
            -clip1[plane] - clip1[3]
        };

        // Both inside or both outside - no intersection with this plane
        if (d0 >= 0.0) == (d1 >= 0.0) {
            return None;
        }

        // Calculate intersection parameter t
        // d0 + t * (d1 - d0) = 0
        // t = -d0 / (d1 - d0)
        let denom = d1 - d0;
        if denom.abs() < 0.0001 {
            return None;
        }

        let t = -d0 / denom;
        if (0.0..=1.0).contains(&t) {
            Some(t)
        } else {
            None
        }
    }
}

impl Default for RspHle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_rsp_hle_creation() {
        let hle = RspHle::new();
        assert_eq!(hle.microcode, MicrocodeType::Unknown);
        assert_eq!(hle.vertex_count, 0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_microcode_detection() {
        let mut hle = RspHle::new();
        let mut imem = [0u8; 4096];

        // Empty IMEM should be Unknown
        hle.detect_microcode(&imem);
        assert_eq!(hle.microcode, MicrocodeType::Unknown);

        // Non-empty IMEM should be detected as F3DEX (simplified)
        imem[0] = 0x12;
        imem[1] = 0x34;
        hle.detect_microcode(&imem);
        assert_eq!(hle.microcode, MicrocodeType::F3DEX);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_execute_unknown_task() {
        let mut hle = RspHle::new();
        let dmem = [0u8; 4096];
        let mut rdram = vec![0u8; 4096];
        let mut rdp = Rdp::new_for_test();

        let cycles = hle.execute_task(&dmem, &mut rdram[..], &mut rdp);
        assert!(cycles > 0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_vertex_loading() {
        let mut hle = RspHle::new();
        let mut rdram = vec![0u8; 4096];

        // Create vertex data at address 0x100
        let addr = 0x100;
        // X = 100 (0x0064)
        rdram[addr] = 0x00;
        rdram[addr + 1] = 0x64;
        // Y = 200 (0x00C8)
        rdram[addr + 2] = 0x00;
        rdram[addr + 3] = 0xC8;
        // Z = 300 (0x012C)
        rdram[addr + 4] = 0x01;
        rdram[addr + 5] = 0x2C;
        // Texture S = 10 (0x000A)
        rdram[addr + 8] = 0x00;
        rdram[addr + 9] = 0x0A;
        // Texture T = 20 (0x0014)
        rdram[addr + 10] = 0x00;
        rdram[addr + 11] = 0x14;
        // Color RGBA = (255, 128, 64, 255)
        rdram[addr + 12] = 255;
        rdram[addr + 13] = 128;
        rdram[addr + 14] = 64;
        rdram[addr + 15] = 255;

        hle.load_vertex(&rdram, addr as u32, 0);

        assert_eq!(hle.vertices[0].pos, [100, 200, 300]);
        assert_eq!(hle.vertices[0].tex, [10, 20]);
        assert_eq!(hle.vertices[0].color, [255, 128, 64, 255]);
        assert_eq!(hle.vertex_count, 1);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_vertex_transform() {
        let hle = RspHle::new();
        let vertex = Vertex {
            pos: [50, 60, 100],
            tex: [0, 0],
            color: [255, 255, 255, 255],
        };

        let (x, y, z) = hle.transform_vertex(&vertex);

        // With identity matrices:
        // Modelview: (50, 60, 100, 1) stays (50, 60, 100, 1)
        // Projection: (50, 60, 100, 1) stays (50, 60, 100, 1)
        // NDC: divide by w=1 gives (50, 60, 100)
        // Clamping: NDC is clamped to (-10, 10) for x/y and (-1, 1) for z
        //   ndc_x = 10.0 (clamped from 50.0)
        //   ndc_y = 10.0 (clamped from 60.0)
        //   ndc_z = 1.0 (clamped from 100.0)
        // Screen space with corrected viewport transform:
        //   x = 0 + (10.0 + 1.0) * 160 = 1760
        //   y = 0 + (10.0 + 1.0) * 120 = 1320
        //   z = (1.0 + 1.0) * 32767.5 = 65535
        assert_eq!(x, 1760);
        assert_eq!(y, 1320);
        assert_eq!(z, 65535);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_identity_matrix() {
        let matrix = RspHle::identity_matrix();

        // Check diagonal elements are 1.0
        assert_eq!(matrix[0], 1.0);
        assert_eq!(matrix[5], 1.0);
        assert_eq!(matrix[10], 1.0);
        assert_eq!(matrix[15], 1.0);

        // Check off-diagonal elements are 0.0
        assert_eq!(matrix[1], 0.0);
        assert_eq!(matrix[4], 0.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_viewport_transformation() {
        // Test viewport transformation correctness
        let hle = RspHle::new();

        // Default viewport: (0, 0, 320, 240, 160, 120)
        // This means: vp_x=0, vp_y=0, scale_x=160, scale_y=120
        // Standard N64 viewport transform:
        //   screen_x = vp_x + (ndc_x + 1.0) * scale_x
        //   screen_y = vp_y + (ndc_y + 1.0) * scale_y

        // Test with NDC coordinates at origin (0, 0, 0, 1)
        let clip = [0.0, 0.0, 0.0, 1.0];
        let (x, y, z) = hle.clip_to_screen(&clip);

        // NDC (0, 0) should map to screen center
        // screen_x = 0 + (0 + 1.0) * 160 = 160
        // screen_y = 0 + (0 + 1.0) * 120 = 120
        // screen_z = (0 + 1.0) * 32767.5 = 32767.5
        assert_eq!(x, 160, "X coordinate at NDC origin should be screen center");
        assert_eq!(y, 120, "Y coordinate at NDC origin should be screen center");
        assert_eq!(z, 32767, "Z coordinate at NDC origin should be mid-depth");

        // Test with NDC at top-left corner (-1, -1)
        let clip = [-1.0, -1.0, -1.0, 1.0];
        let (x, y, z) = hle.clip_to_screen(&clip);

        // screen_x = 0 + (-1 + 1.0) * 160 = 0
        // screen_y = 0 + (-1 + 1.0) * 120 = 0
        // screen_z = (-1 + 1.0) * 32767.5 = 0
        assert_eq!(x, 0, "X coordinate at NDC (-1,-1) should be left edge");
        assert_eq!(y, 0, "Y coordinate at NDC (-1,-1) should be top edge");
        assert_eq!(z, 0, "Z coordinate at NDC -1 should be near plane");

        // Test with NDC at bottom-right corner (1, 1)
        let clip = [1.0, 1.0, 1.0, 1.0];
        let (x, y, z) = hle.clip_to_screen(&clip);

        // screen_x = 0 + (1 + 1.0) * 160 = 320
        // screen_y = 0 + (1 + 1.0) * 120 = 240
        // screen_z = (1 + 1.0) * 32767.5 = 65535
        assert_eq!(x, 320, "X coordinate at NDC (1,1) should be right edge");
        assert_eq!(y, 240, "Y coordinate at NDC (1,1) should be bottom edge");
        assert_eq!(z, 65535, "Z coordinate at NDC 1 should be far plane");
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_f3dex_display_list_parsing() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new_for_test();

        // Create a simple F3DEX display list in RDRAM at address 0x100
        let dl_addr = 0x100;

        // G_VTX command - Load 3 vertices at address 0x200
        // word0: cmd(0x01) | count(3 << 12) | index(0)
        let vtx_cmd_word0: u32 = (0x01 << 24) | (3 << 12);
        let vtx_cmd_word1: u32 = 0x200; // Vertex data address
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&vtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&vtx_cmd_word1.to_be_bytes());

        // Create vertex data at 0x200 (3 vertices * 16 bytes each)
        // Vertex 0: pos(10,10,0), tex(0,0), color(255,0,0,255) - red
        let v0_data: [u8; 16] = [
            0, 10, 0, 10, 0, 0, 0, 0, // x=10, y=10, z=0, flags=0
            0, 0, 0, 0, // s=0, t=0
            255, 0, 0, 255, // r=255, g=0, b=0, a=255
        ];
        rdram[0x200..0x210].copy_from_slice(&v0_data);

        // Vertex 1: pos(100,10,0), tex(0,0), color(0,255,0,255) - green
        let v1_data: [u8; 16] = [0, 100, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 255];
        rdram[0x210..0x220].copy_from_slice(&v1_data);

        // Vertex 2: pos(55,100,0), tex(0,0), color(0,0,255,255) - blue
        let v2_data: [u8; 16] = [0, 55, 0, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255];
        rdram[0x220..0x230].copy_from_slice(&v2_data);

        // G_TRI1 command - Draw triangle using vertices 0, 1, 2
        // word0: cmd(0x05) | v0(0 << 16) | v1(2 << 8) | v2(4)
        let tri_cmd_word0: u32 = (0x05 << 24) | (2 << 8) | 4;
        let tri_cmd_word1: u32 = 0;
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&tri_cmd_word0.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&tri_cmd_word1.to_be_bytes());

        // G_ENDDL command - End display list
        let end_cmd_word0: u32 = 0xDF000000;
        let end_cmd_word1: u32 = 0;
        rdram[dl_addr + 16..dl_addr + 20].copy_from_slice(&end_cmd_word0.to_be_bytes());
        rdram[dl_addr + 20..dl_addr + 24].copy_from_slice(&end_cmd_word1.to_be_bytes());

        // Parse the display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 24, &mut rdp);

        // Verify vertices were loaded
        assert_eq!(hle.vertex_count, 3);
        assert_eq!(hle.vertices[0].pos[0], 10);
        assert_eq!(hle.vertices[0].pos[1], 10);
        assert_eq!(hle.vertices[0].color[0], 255); // Red

        // Note: We can't easily verify the triangle was drawn without checking the framebuffer
        // but the test ensures the parsing doesn't crash
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_f3dex_quad_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new_for_test();

        // Load 4 vertices for a quad
        let dl_addr = 0x100;

        // G_VTX command - Load 4 vertices
        let vtx_cmd_word0: u32 = (0x01 << 24) | (4 << 12);
        let vtx_cmd_word1: u32 = 0x200;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&vtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&vtx_cmd_word1.to_be_bytes());

        // Create 4 vertices for a quad
        for i in 0..4 {
            let vdata: [u8; 16] = [
                0,
                (10 + i * 20) as u8,
                0,
                (10 + i * 20) as u8,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                255,
                255,
                255,
                255,
            ];
            rdram[0x200 + i * 16..0x210 + i * 16].copy_from_slice(&vdata);
        }

        // G_QUAD command - Draw quad using vertices 0,1,2,3
        // word0: cmd(0x07) | v0(0) | v1(2) | v2(4)
        // word1: v0(0) | v2(4) | v3(6)
        let quad_cmd_word0: u32 = (0x07 << 24) | (2 << 8) | 4;
        let quad_cmd_word1: u32 = (4 << 8) | 6;
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&quad_cmd_word0.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&quad_cmd_word1.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 16..dl_addr + 20].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 20..dl_addr + 24].copy_from_slice(&0u32.to_be_bytes());

        // Parse the display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 24, &mut rdp);

        // Verify vertices were loaded
        assert_eq!(hle.vertex_count, 4);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_f3dex_geometrymode_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new_for_test();

        let dl_addr = 0x100;

        // Initial geometry mode should be 0
        assert_eq!(hle.geometry_mode, 0);

        // G_GEOMETRYMODE command - Set some flags
        // word0: cmd(0xD9) | clear_bits (bits to clear)
        // word1: set_bits (bits to set)
        let geom_cmd_word0: u32 = 0xD9 << 24; // Don't clear any bits
        let geom_cmd_word1: u32 = 0x00000123; // Set some test flags
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&geom_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&geom_cmd_word1.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        // Parse the display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 16, &mut rdp);

        // Verify geometry mode was set
        assert_eq!(hle.geometry_mode, 0x00000123);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_matrix_multiplication() {
        // Test identity matrix multiplication
        let identity = RspHle::identity_matrix();
        let test_matrix = [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        let result = RspHle::multiply_matrix(&identity, &test_matrix);
        for i in 0..16 {
            assert_eq!(result[i], test_matrix[i]);
        }
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_matrix_multiplication_scaling() {
        // Test scaling matrix multiplication
        let scale2 = [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let scale3 = [
            3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        let result = RspHle::multiply_matrix(&scale2, &scale3);
        // Result should be scale by 6 (2 * 3)
        assert_eq!(result[0], 6.0);
        assert_eq!(result[5], 6.0);
        assert_eq!(result[10], 6.0);
        assert_eq!(result[15], 1.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_load_matrix_from_rdram() {
        let hle = RspHle::new();
        let mut rdram = vec![0u8; 1024];

        // Create an identity matrix in RDRAM (16.16 fixed point format)
        // Identity: diagonal = 1.0 = 0x00010000 in 16.16 fixed point
        let identity_fixed: i32 = 0x00010000; // 1.0 in 16.16 fixed point
        let zero_fixed: i32 = 0x00000000; // 0.0 in 16.16 fixed point

        let addr = 0x100;
        for i in 0..16 {
            let value = if i == 0 || i == 5 || i == 10 || i == 15 {
                identity_fixed
            } else {
                zero_fixed
            };
            let offset = addr + i * 4;
            rdram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        let matrix = hle.load_matrix_from_rdram(&rdram, addr as u32);

        // Verify identity matrix was loaded
        assert_eq!(matrix[0], 1.0);
        assert_eq!(matrix[5], 1.0);
        assert_eq!(matrix[10], 1.0);
        assert_eq!(matrix[15], 1.0);
        assert_eq!(matrix[1], 0.0);
        assert_eq!(matrix[2], 0.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_g_mtx_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new_for_test();

        // Create a scaling matrix in RDRAM (scale by 2.0)
        let addr = 0x200;
        let scale2_fixed: i32 = 0x00020000; // 2.0 in 16.16 fixed point
        let zero_fixed: i32 = 0x00000000;
        let one_fixed: i32 = 0x00010000;

        for i in 0..16 {
            let value = if i == 0 || i == 5 || i == 10 {
                scale2_fixed
            } else if i == 15 {
                one_fixed
            } else {
                zero_fixed
            };
            let offset = addr + i * 4;
            rdram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        // Create display list with G_MTX command
        let dl_addr = 0x100;

        // G_MTX command (0xDA) - load modelview matrix
        // param: G_MTX_MODELVIEW | G_MTX_LOAD (0x00)
        let mtx_cmd_word0: u32 = 0xDA << 24; // Load modelview (param = 0x00)
        let mtx_cmd_word1: u32 = addr as u32; // Matrix address
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&mtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&mtx_cmd_word1.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        // Parse the display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 16, &mut rdp);

        // Verify modelview matrix was loaded (scale by 2)
        assert_eq!(hle.modelview_matrix[0], 2.0);
        assert_eq!(hle.modelview_matrix[5], 2.0);
        assert_eq!(hle.modelview_matrix[10], 2.0);
        assert_eq!(hle.modelview_matrix[15], 1.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_g_mtx_projection() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new_for_test();

        // Create a projection matrix in RDRAM
        let addr = 0x200;
        let one_fixed: i32 = 0x00010000;
        let zero_fixed: i32 = 0x00000000;

        for i in 0..16 {
            let value = if i == 0 || i == 5 || i == 10 || i == 15 {
                one_fixed
            } else {
                zero_fixed
            };
            let offset = addr + i * 4;
            rdram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        let dl_addr = 0x100;

        // G_MTX command (0xDA) - load projection matrix
        // param: G_MTX_PROJECTION | G_MTX_LOAD (0x04)
        let mtx_cmd_word0: u32 = (0xDA << 24) | 0x04; // Load projection
        let mtx_cmd_word1: u32 = addr as u32;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&mtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&mtx_cmd_word1.to_be_bytes());

        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 16, &mut rdp);

        // Verify projection matrix was loaded
        assert_eq!(hle.projection_matrix[0], 1.0);
        assert_eq!(hle.projection_matrix[15], 1.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_g_dl_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 2048];
        let mut rdp = Rdp::new_for_test();

        // Create a nested display list at 0x200 that loads vertices
        let nested_dl_addr = 0x200;

        // G_VTX in nested DL
        let vtx_cmd_word0: u32 = (0x01 << 24) | (2 << 12);
        let vtx_cmd_word1: u32 = 0x300;
        rdram[nested_dl_addr..nested_dl_addr + 4].copy_from_slice(&vtx_cmd_word0.to_be_bytes());
        rdram[nested_dl_addr + 4..nested_dl_addr + 8].copy_from_slice(&vtx_cmd_word1.to_be_bytes());

        // G_ENDDL in nested DL
        rdram[nested_dl_addr + 8..nested_dl_addr + 12]
            .copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[nested_dl_addr + 12..nested_dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        // Create vertex data
        for i in 0..2 {
            let vdata: [u8; 16] = [0, 10, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255];
            rdram[0x300 + i * 16..0x310 + i * 16].copy_from_slice(&vdata);
        }

        // Create main display list at 0x100 with G_DL command
        let dl_addr = 0x100;

        // G_DL command (0xDE) - call nested display list
        let dl_cmd_word0: u32 = 0xDE << 24; // G_DL_PUSH (param = 0x00)
        let dl_cmd_word1: u32 = nested_dl_addr as u32;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&dl_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&dl_cmd_word1.to_be_bytes());

        // G_ENDDL in main DL
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        // Parse main display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 16, &mut rdp);

        // Verify vertices were loaded from nested display list
        assert_eq!(hle.vertex_count, 2);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_vertex_transform_with_matrices() {
        let mut hle = RspHle::new();

        // Set up a simple scaling matrix (scale by 2) in column-major format
        // Diagonal elements [0, 5, 10, 15] contain the scale factors
        hle.modelview_matrix = [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        let vertex = Vertex {
            pos: [10, 10, 10],
            tex: [0, 0],
            color: [255, 255, 255, 255],
        };

        let (x, _y, z) = hle.transform_vertex(&vertex);

        // With identity projection and scale-by-2 modelview:
        // - Modelview transforms (10,10,10) to (20,20,20)
        // - Projection (identity) keeps it (20,20,20)
        // - NDC: divide by w=1 gives (20,20,20)
        // - Clamping: NDC is clamped to (-10, 10) for x/y and (-1, 1) for z
        //   ndc_x = 10.0 (clamped from 20.0)
        //   ndc_y = 10.0 (clamped from 20.0)
        //   ndc_z = 1.0 (clamped from 20.0)
        // - Screen with corrected viewport: ((10.0+1)*160, (10.0+1)*120, (1.0+1)*32767.5)
        //         = (1760, 1320, 65535)
        assert_eq!(x, 1760);
        assert!(z > 10); // Z should be transformed (will be clamped to max)
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_matrix_stack_push_pop() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 2048];
        let mut rdp = Rdp::new_for_test();

        // Create a scaling matrix (scale by 2)
        let addr1 = 0x200;
        let scale2_fixed: i32 = 0x00020000; // 2.0 in 16.16 fixed point
        let one_fixed: i32 = 0x00010000;
        let zero_fixed: i32 = 0x00000000;

        for i in 0..16 {
            let value = if i == 0 || i == 5 || i == 10 {
                scale2_fixed
            } else if i == 15 {
                one_fixed
            } else {
                zero_fixed
            };
            let offset = addr1 + i * 4;
            rdram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        // Create another matrix (scale by 3)
        let addr2 = 0x300;
        let scale3_fixed: i32 = 0x00030000; // 3.0 in 16.16 fixed point

        for i in 0..16 {
            let value = if i == 0 || i == 5 || i == 10 {
                scale3_fixed
            } else if i == 15 {
                one_fixed
            } else {
                zero_fixed
            };
            let offset = addr2 + i * 4;
            rdram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }

        let dl_addr = 0x100;

        // G_MTX with PUSH - load scale-by-2 matrix
        let mtx_cmd_word0: u32 = (0xDA << 24) | 0x01; // PUSH flag set
        let mtx_cmd_word1: u32 = addr1 as u32;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&mtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&mtx_cmd_word1.to_be_bytes());

        // G_MTX with PUSH - load scale-by-3 matrix
        let mtx_cmd_word0_2: u32 = (0xDA << 24) | 0x01; // PUSH flag set
        let mtx_cmd_word1_2: u32 = addr2 as u32;
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&mtx_cmd_word0_2.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&mtx_cmd_word1_2.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 16..dl_addr + 20].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 20..dl_addr + 24].copy_from_slice(&0u32.to_be_bytes());

        // Parse display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 24, &mut rdp);

        // After two pushes, stack pointer should be 2
        assert_eq!(hle.matrix_stack_ptr, 2);

        // Current modelview matrix should be scale-by-3
        assert_eq!(hle.modelview_matrix[0], 3.0);
        assert_eq!(hle.modelview_matrix[5], 3.0);

        // Stack should contain identity at [0] and scale-by-2 at [1]
        assert_eq!(hle.matrix_stack[0][0], 1.0); // Identity
        assert_eq!(hle.matrix_stack[1][0], 2.0); // Scale-by-2
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_g_popmtx_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 2048];
        let mut rdp = Rdp::new_for_test();

        // Manually set up a matrix stack state
        hle.matrix_stack_ptr = 2;
        hle.matrix_stack[0] = RspHle::identity_matrix();
        hle.matrix_stack[1] = [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]; // Scale by 2
        hle.modelview_matrix = [
            3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]; // Scale by 3

        let dl_addr = 0x100;

        // G_POPMTX command (0xD8) - pop 1 matrix (64 bytes)
        let pop_cmd_word0: u32 = 0xD8 << 24;
        let pop_cmd_word1: u32 = 64; // Pop 1 matrix (64 bytes / 64 = 1)
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&pop_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&pop_cmd_word1.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&0u32.to_be_bytes());

        // Parse display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 16, &mut rdp);

        // Stack pointer should be decremented
        assert_eq!(hle.matrix_stack_ptr, 1);

        // Current modelview matrix should now be scale-by-2 (popped from stack)
        assert_eq!(hle.modelview_matrix[0], 2.0);
        assert_eq!(hle.modelview_matrix[5], 2.0);
    }

    #[test]
    #[ignore] // Requires OpenGL context
    fn test_g_branch_z_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 2048];
        let mut rdp = Rdp::new_for_test();

        // Load a vertex first
        let vtx_data_addr = 0x300;
        let vdata: [u8; 16] = [0, 10, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255];
        rdram[vtx_data_addr..vtx_data_addr + 16].copy_from_slice(&vdata);

        let dl_addr = 0x100;

        // G_VTX - load 1 vertex
        let vtx_cmd_word0: u32 = (0x01 << 24) | (1 << 12);
        let vtx_cmd_word1: u32 = vtx_data_addr as u32;
        rdram[dl_addr..dl_addr + 4].copy_from_slice(&vtx_cmd_word0.to_be_bytes());
        rdram[dl_addr + 4..dl_addr + 8].copy_from_slice(&vtx_cmd_word1.to_be_bytes());

        // Create a nested display list that will be branched to
        let branch_target = 0x400;
        // G_VTX - load another vertex in the branch target at index 1
        let vtx2_data_addr = 0x500;
        let vdata2: [u8; 16] = [0, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 128, 128, 128, 255];
        rdram[vtx2_data_addr..vtx2_data_addr + 16].copy_from_slice(&vdata2);

        // Load 1 vertex at buffer index 1: (1 << 12) for count, (1 << 1) for buffer_index
        let vtx2_cmd_word0: u32 = (0x01 << 24) | (1 << 12) | (1 << 1);
        let vtx2_cmd_word1: u32 = vtx2_data_addr as u32;
        rdram[branch_target..branch_target + 4].copy_from_slice(&vtx2_cmd_word0.to_be_bytes());
        rdram[branch_target + 4..branch_target + 8].copy_from_slice(&vtx2_cmd_word1.to_be_bytes());
        rdram[branch_target + 8..branch_target + 12].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[branch_target + 12..branch_target + 16].copy_from_slice(&0u32.to_be_bytes());

        // G_BRANCH_Z - conditional branch to nested DL
        let branch_cmd_word0: u32 = 0xB0 << 24; // Vertex index 0
        let branch_cmd_word1: u32 = branch_target as u32;
        rdram[dl_addr + 8..dl_addr + 12].copy_from_slice(&branch_cmd_word0.to_be_bytes());
        rdram[dl_addr + 12..dl_addr + 16].copy_from_slice(&branch_cmd_word1.to_be_bytes());

        // G_ENDDL
        rdram[dl_addr + 16..dl_addr + 20].copy_from_slice(&0xDF000000u32.to_be_bytes());
        rdram[dl_addr + 20..dl_addr + 24].copy_from_slice(&0u32.to_be_bytes());

        // Parse display list
        hle.parse_f3dex_display_list(&rdram, dl_addr as u32, 24, &mut rdp);

        // Verify that the branch was taken and vertex was loaded
        assert_eq!(hle.vertex_count, 2);
    }

    #[test]
    fn test_compute_lit_color_basic() {
        // Test lighting calculation without OpenGL context
        let mut hle = RspHle::new();

        // Set up a single directional light pointing in +Z direction
        // Light color: white (1.0, 1.0, 1.0)
        hle.lights[0] = [0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0]; // dir=(0,0,1), color=white
                                                             // Ambient light (stored at index num_lights)
        hle.lights[1] = [0.0, 0.0, 0.0, 0.2, 0.2, 0.2, 0.0]; // ambient = (0.2, 0.2, 0.2)
        hle.num_lights = 1;

        // Vertex with normal pointing in +Z direction (should be fully lit)
        let vert = Vertex {
            pos: [0, 0, 0],
            tex: [0, 0],
            color: [0, 0, 127, 255], // normal = (0, 0, 1.0) as signed bytes
        };

        let lit = hle.compute_lit_color(&vert);

        // With identity modelview, normal points at (0,0,1), dot with light (0,0,1) = 1.0
        // Result: ambient(0.2) + 1.0 * white(1.0) = 1.2 -> clamped to 1.0 -> 255
        assert_eq!(lit[0], 255); // R
        assert_eq!(lit[1], 255); // G
        assert_eq!(lit[2], 255); // B
    }

    #[test]
    fn test_compute_lit_color_perpendicular() {
        // Normal perpendicular to light should produce only ambient
        let mut hle = RspHle::new();

        // Light pointing in +Z
        hle.lights[0] = [0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0];
        // Ambient
        hle.lights[1] = [0.0, 0.0, 0.0, 0.2, 0.2, 0.2, 0.0];
        hle.num_lights = 1;

        // Normal pointing in +X (perpendicular to light)
        let vert = Vertex {
            pos: [0, 0, 0],
            tex: [0, 0],
            color: [127, 0, 0, 255], // normal = (1.0, 0, 0)
        };

        let lit = hle.compute_lit_color(&vert);

        // dot(N=(1,0,0), L=(0,0,1)) = 0 -> only ambient (0.2*255 = 51)
        assert_eq!(lit[0], 51);
        assert_eq!(lit[1], 51);
        assert_eq!(lit[2], 51);
    }

    #[test]
    fn test_compute_lit_color_opposing() {
        // Normal pointing away from light -> dot is negative, clamp to 0 -> only ambient
        let mut hle = RspHle::new();

        hle.lights[0] = [0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0];
        hle.lights[1] = [0.0, 0.0, 0.0, 0.1, 0.1, 0.1, 0.0];
        hle.num_lights = 1;

        // Normal pointing in -Z (away from light)
        let vert = Vertex {
            pos: [0, 0, 0],
            tex: [0, 0],
            color: [0, 0, 129, 255], // normal = (0, 0, -1.0) as i8 (129 as u8 = -127 as i8)
        };

        let lit = hle.compute_lit_color(&vert);

        // dot(N=(0,0,-1), L=(0,0,1)) = -1.0 -> clamp to 0 -> only ambient (0.1 * 255 ≈ 25)
        assert_eq!(lit[0], 25);
    }

    #[test]
    fn test_backface_culling_flags() {
        // Test that geometry mode flags are stored correctly
        let mut hle = RspHle::new();
        assert_eq!(hle.geometry_mode & G_CULL_BACK, 0);
        assert_eq!(hle.geometry_mode & G_CULL_FRONT, 0);

        hle.geometry_mode |= G_CULL_BACK;
        assert_ne!(hle.geometry_mode & G_CULL_BACK, 0);
        assert_eq!(hle.geometry_mode & G_CULL_FRONT, 0);

        hle.geometry_mode |= G_CULL_FRONT;
        assert_ne!(hle.geometry_mode & G_CULL_FRONT, 0);

        hle.geometry_mode &= !G_CULL_BACK;
        assert_eq!(hle.geometry_mode & G_CULL_BACK, 0);
        assert_ne!(hle.geometry_mode & G_CULL_FRONT, 0);
    }

    #[test]
    fn test_lighting_mode_flag() {
        let mut hle = RspHle::new();
        assert_eq!(hle.geometry_mode & G_LIGHTING, 0);

        hle.geometry_mode |= G_LIGHTING;
        assert_ne!(hle.geometry_mode & G_LIGHTING, 0);
    }

    #[test]
    fn test_adpcm_decode_basic() {
        // Test that ADPCM decode produces non-silence output
        let mut hle = RspHle::new();
        let mut dmem = [0u8; 4096];
        let mut rdram = vec![0u8; 4 * 1024 * 1024];

        // Set up a minimal ADPCM frame at DMEM address 0x100
        let in_addr = 0x100usize;
        let out_addr = 0x200usize;
        let num_output_bytes = 32usize; // 16 samples × 2 bytes each

        // ADPCM frame header: scale_shift=2, predictor_index=0
        dmem[in_addr] = 0x20;
        // Residuals: each byte has two 4-bit nibbles
        // Use non-zero nibbles to generate non-silence
        for i in 0..8 {
            dmem[in_addr + 1 + i] = 0x37; // nibbles: 3, 7 (7 sign-extends to -1)
        }

        // Set up OSTask at DMEM 0xFC0
        let task_base = 0xFC0usize;
        // Build a single ADPCM command
        let cmd_addr = 0x1000usize; // command list in RDRAM
        let cmd_word0: u32 = (0x01u32 << 24) | (num_output_bytes as u32);
        let cmd_word1: u32 = ((in_addr as u32) << 16) | (out_addr as u32);

        // Write command to RDRAM
        rdram[cmd_addr..cmd_addr + 4].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[cmd_addr + 4..cmd_addr + 8].copy_from_slice(&cmd_word1.to_be_bytes());

        // Set up OSTask fields
        let ucode_data_ptr: u32 = 0x80000000 + cmd_addr as u32; // KSEG0 address
        let ucode_data_size: u32 = 8; // one command
        let output_ptr: u32 = 0x80002000;
        let output_size: u32 = 4096;

        dmem[task_base + 0x18..task_base + 0x1C].copy_from_slice(&ucode_data_ptr.to_be_bytes());
        dmem[task_base + 0x1C..task_base + 0x20].copy_from_slice(&ucode_data_size.to_be_bytes());
        dmem[task_base + 0x28..task_base + 0x2C].copy_from_slice(&output_ptr.to_be_bytes());
        dmem[task_base + 0x2C..task_base + 0x30].copy_from_slice(&output_size.to_be_bytes());

        hle.microcode = MicrocodeType::Audio;
        let mut dmem_copy: [u8; 4096] = dmem;
        let cycles = hle.execute_audio_task(&mut dmem_copy, &mut rdram);
        assert!(cycles > 0);

        // Check that output area is not all zeros (ADPCM should produce something)
        let mut any_nonzero = false;
        for i in (out_addr..out_addr + num_output_bytes).step_by(2) {
            let sample = i16::from_be_bytes([dmem_copy[i], dmem_copy[i + 1]]);
            if sample != 0 {
                any_nonzero = true;
                break;
            }
        }
        assert!(
            any_nonzero,
            "ADPCM decode should produce non-silence output"
        );
    }

    #[test]
    fn test_resample_passthrough() {
        // Test resampling with pitch = 0x8000 (1.0x) should produce similar output
        let mut hle = RspHle::new();
        let mut dmem = [0u8; 4096];
        let mut rdram = vec![0u8; 4 * 1024 * 1024];

        // Fill input buffer with a known pattern at DMEM 0x100
        let in_addr = 0x100usize;
        let count = 64usize; // 32 samples × 2 bytes
        for i in 0..32 {
            let sample = ((i as i16) * 1000).to_be_bytes();
            dmem[in_addr + i * 2] = sample[0];
            dmem[in_addr + i * 2 + 1] = sample[1];
        }

        // Set up a RESAMPLE command
        let cmd_addr = 0x1000usize;
        let cmd_word0: u32 = (0x03u32 << 24) | (count as u32);
        let cmd_word1: u32 = (0x8000u32 << 16) | (in_addr as u32); // pitch=1.0, in_addr
        rdram[cmd_addr..cmd_addr + 4].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[cmd_addr + 4..cmd_addr + 8].copy_from_slice(&cmd_word1.to_be_bytes());

        // OSTask setup
        let task_base = 0xFC0usize;
        let ucode_data_ptr: u32 = 0x80000000 + cmd_addr as u32;
        let ucode_data_size: u32 = 8;
        dmem[task_base + 0x18..task_base + 0x1C].copy_from_slice(&ucode_data_ptr.to_be_bytes());
        dmem[task_base + 0x1C..task_base + 0x20].copy_from_slice(&ucode_data_size.to_be_bytes());
        dmem[task_base + 0x28..task_base + 0x2C].copy_from_slice(&0x80002000u32.to_be_bytes());
        dmem[task_base + 0x2C..task_base + 0x30].copy_from_slice(&4096u32.to_be_bytes());

        hle.microcode = MicrocodeType::Audio;
        let mut dmem_copy: [u8; 4096] = dmem;
        let cycles = hle.execute_audio_task(&mut dmem_copy, &mut rdram);
        assert!(cycles > 0);

        // At pitch 1.0, first sample should be very close to the original
        let first_out = i16::from_be_bytes([dmem_copy[in_addr], dmem_copy[in_addr + 1]]);
        assert_eq!(
            first_out, 0,
            "First sample at index 0 should be 0 (0 * 1000)"
        );
    }

    #[test]
    fn test_envmixer_basic() {
        // Test that ENVMIXER adds source to L/R channels
        let mut hle = RspHle::new();
        let mut dmem = [0u8; 4096];
        let mut rdram = vec![0u8; 4 * 1024 * 1024];

        let src = 0x100usize;
        let dst_left = 0x200usize;
        let count = 8usize; // 4 samples × 2 bytes

        // Source: 4 samples of value 1000
        for i in 0..4 {
            let s = 1000i16.to_be_bytes();
            dmem[src + i * 2] = s[0];
            dmem[src + i * 2 + 1] = s[1];
        }
        // Left channel already has value 500
        for i in 0..4 {
            let s = 500i16.to_be_bytes();
            dmem[dst_left + i * 2] = s[0];
            dmem[dst_left + i * 2 + 1] = s[1];
        }

        // ENVMIXER command
        let cmd_addr = 0x1000usize;
        let cmd_word0: u32 = (0x0Du32 << 24) | (count as u32);
        let cmd_word1: u32 = ((src as u32) << 16) | (dst_left as u32);
        rdram[cmd_addr..cmd_addr + 4].copy_from_slice(&cmd_word0.to_be_bytes());
        rdram[cmd_addr + 4..cmd_addr + 8].copy_from_slice(&cmd_word1.to_be_bytes());

        // OSTask setup
        let task_base = 0xFC0usize;
        dmem[task_base + 0x18..task_base + 0x1C]
            .copy_from_slice(&(0x80000000u32 + cmd_addr as u32).to_be_bytes());
        dmem[task_base + 0x1C..task_base + 0x20].copy_from_slice(&8u32.to_be_bytes());
        dmem[task_base + 0x28..task_base + 0x2C].copy_from_slice(&0x80002000u32.to_be_bytes());
        dmem[task_base + 0x2C..task_base + 0x30].copy_from_slice(&4096u32.to_be_bytes());

        hle.microcode = MicrocodeType::Audio;
        let mut dmem_copy: [u8; 4096] = dmem;
        let cycles = hle.execute_audio_task(&mut dmem_copy, &mut rdram);
        assert!(cycles > 0);

        // Left channel should now have 500 + 1000 = 1500
        let left_sample = i16::from_be_bytes([dmem_copy[dst_left], dmem_copy[dst_left + 1]]);
        assert_eq!(left_sample, 1500);
    }
}
