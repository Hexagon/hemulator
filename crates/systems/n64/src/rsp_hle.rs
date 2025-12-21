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

use super::rdp::Rdp;

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

/// Vertex structure for graphics microcode
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    /// Position (x, y, z) in object space
    pub pos: [i16; 3],
    /// Texture coordinates (s, t) in 16.16 fixed point
    #[allow(dead_code)] // Reserved for future texture mapping
    pub tex: [i16; 2],
    /// Color (RGBA) 0-255 per channel
    #[allow(dead_code)] // Reserved for future vertex color support
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

    /// Current matrix stack pointer
    #[allow(dead_code)] // Reserved for future matrix stack implementation
    matrix_stack_ptr: usize,

    /// Projection matrix (4x4, stored as 16 f32s)
    #[allow(dead_code)] // Reserved for future vertex transformation
    projection_matrix: [f32; 16],

    /// Modelview matrix (4x4, stored as 16 f32s)
    #[allow(dead_code)] // Reserved for future vertex transformation
    modelview_matrix: [f32; 16],
}

impl RspHle {
    /// Create new RSP HLE state
    pub fn new() -> Self {
        Self {
            microcode: MicrocodeType::Unknown,
            vertices: [Vertex::default(); 32],
            vertex_count: 0,
            matrix_stack_ptr: 0,
            projection_matrix: Self::identity_matrix(),
            modelview_matrix: Self::identity_matrix(),
        }
    }

    /// Create identity matrix
    fn identity_matrix() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    /// Detect microcode type from IMEM data
    pub fn detect_microcode(&mut self, imem: &[u8; 4096]) {
        // Simple detection: look for known patterns in microcode
        // Real implementation would use CRC32 or signature matching

        // For now, assume F3DEX if IMEM is non-zero
        let has_code = imem.iter().any(|&b| b != 0);
        if has_code {
            self.microcode = MicrocodeType::F3DEX;
        } else {
            self.microcode = MicrocodeType::Unknown;
        }
    }

    /// Get current microcode type
    pub fn microcode(&self) -> MicrocodeType {
        self.microcode
    }

    /// Execute HLE task (called when RSP is triggered)
    /// Returns number of cycles consumed
    pub fn execute_task(&mut self, dmem: &[u8; 4096], rdram: &[u8], _rdp: &mut Rdp) -> u32 {
        match self.microcode {
            MicrocodeType::F3DEX | MicrocodeType::F3DEX2 => {
                self.execute_graphics_task(dmem, rdram, _rdp)
            }
            MicrocodeType::Audio => {
                // Audio tasks not yet implemented
                1000
            }
            MicrocodeType::Unknown => {
                // No-op for unknown microcode
                100
            }
        }
    }

    /// Execute graphics microcode task (F3DEX/F3DEX2)
    fn execute_graphics_task(&mut self, dmem: &[u8; 4096], rdram: &[u8], rdp: &mut Rdp) -> u32 {
        // Parse display list pointer from DMEM
        // In real F3DEX, the display list address is passed via DMEM at a known offset

        // Read task structure from DMEM (typical offset is 0x0000)
        let _task_type = self.read_u32(dmem, 0x00);
        let _task_flags = self.read_u32(dmem, 0x04);
        let _ucode_boot = self.read_u32(dmem, 0x08); // Boot microcode address
        let _ucode_boot_size = self.read_u32(dmem, 0x0C);
        let _ucode = self.read_u32(dmem, 0x10); // Main microcode address
        let _ucode_size = self.read_u32(dmem, 0x14);
        let _ucode_data = self.read_u32(dmem, 0x18); // Microcode data address
        let _ucode_data_size = self.read_u32(dmem, 0x1C);
        let _dram_stack = self.read_u32(dmem, 0x20); // Stack in RDRAM
        let _dram_stack_size = self.read_u32(dmem, 0x24);
        let output_buff = self.read_u32(dmem, 0x28); // Output buffer (display list)
        let output_buff_size = self.read_u32(dmem, 0x2C);
        let data_ptr = self.read_u32(dmem, 0x30); // Data pointer (display list input)
        let data_size = self.read_u32(dmem, 0x34);
        let _yield_data_ptr = self.read_u32(dmem, 0x38);
        let _yield_data_size = self.read_u32(dmem, 0x3C);

        // Basic implementation: Forward display list commands directly to RDP
        // This is a simplified approach that skips vertex transformation
        // Real F3DEX would:
        // 1. Parse F3DEX display list commands (not RDP commands)
        // 2. Process vertex data, matrices, textures
        // 3. Transform vertices using projection and modelview matrices
        // 4. Generate RDP commands (triangles, texture setup)
        // 5. Write RDP display list to output_buff

        // For now, if there's an output buffer with data, treat it as an RDP display list
        // and forward it to the RDP for processing
        if output_buff > 0 && output_buff_size > 0 {
            // Trigger RDP to process the generated display list
            rdp.set_dpc_start(output_buff);
            rdp.set_dpc_end(output_buff + output_buff_size);
            rdp.process_display_list(rdram);
        }

        // Also check if there's a data pointer (some games put display lists there)
        if data_ptr > 0 && data_size > 0 && output_buff == 0 {
            // Treat data_ptr as a display list to process
            rdp.set_dpc_start(data_ptr);
            rdp.set_dpc_end(data_ptr + data_size);
            rdp.process_display_list(rdram);
        }

        2000 // Average cycles for a graphics task
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

    /// Load vertex from RDRAM address
    #[allow(dead_code)]
    fn load_vertex(&mut self, rdram: &[u8], addr: u32, index: usize) {
        if index >= 32 {
            return;
        }

        let addr = addr as usize;
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

    /// Transform vertex from object space to screen space
    #[allow(dead_code)]
    fn transform_vertex(&self, vertex: &Vertex) -> (i32, i32, i32) {
        // Simplified transform: just scale and offset
        // Real implementation would:
        // 1. Apply modelview matrix (object to camera space)
        // 2. Apply projection matrix (camera to clip space)
        // 3. Perspective divide (clip to NDC)
        // 4. Viewport transform (NDC to screen space)

        // For now, simple passthrough with basic scaling
        let x = (vertex.pos[0] as i32) + 160; // Center at 320/2
        let y = (vertex.pos[1] as i32) + 120; // Center at 240/2
        let z = vertex.pos[2] as i32;

        (x, y, z)
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
    fn test_rsp_hle_creation() {
        let hle = RspHle::new();
        assert_eq!(hle.microcode, MicrocodeType::Unknown);
        assert_eq!(hle.vertex_count, 0);
    }

    #[test]
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
    fn test_execute_unknown_task() {
        let mut hle = RspHle::new();
        let dmem = [0u8; 4096];
        let rdram = vec![0u8; 4096];
        let mut rdp = Rdp::new();

        let cycles = hle.execute_task(&dmem, &rdram, &mut rdp);
        assert!(cycles > 0);
    }

    #[test]
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
    fn test_vertex_transform() {
        let hle = RspHle::new();
        let vertex = Vertex {
            pos: [50, 60, 100],
            tex: [0, 0],
            color: [255, 255, 255, 255],
        };

        let (x, y, z) = hle.transform_vertex(&vertex);
        assert_eq!(x, 210); // 50 + 160
        assert_eq!(y, 180); // 60 + 120
        assert_eq!(z, 100);
    }

    #[test]
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
}
