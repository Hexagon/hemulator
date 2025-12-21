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

    /// Get current vertex count in vertex buffer
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
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
        // Parse task structure from DMEM
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
        let output_buff = self.read_u32(dmem, 0x28); // Output buffer (RDP display list)
        let output_buff_size = self.read_u32(dmem, 0x2C);
        let data_ptr = self.read_u32(dmem, 0x30); // Data pointer (F3DEX display list input)
        let data_size = self.read_u32(dmem, 0x34);
        let _yield_data_ptr = self.read_u32(dmem, 0x38);
        let _yield_data_size = self.read_u32(dmem, 0x3C);

        // Parse F3DEX display list if data_ptr is provided
        if data_ptr > 0 && data_size > 0 {
            self.parse_f3dex_display_list(rdram, data_ptr, data_size, rdp);
        }

        // If there's an output buffer with data (pre-generated RDP commands),
        // forward it directly to the RDP for processing
        if output_buff > 0 && output_buff_size > 0 {
            rdp.set_dpc_start(output_buff);
            rdp.set_dpc_end(output_buff + output_buff_size);
            rdp.process_display_list(rdram);
        }

        2000 // Average cycles for a graphics task
    }

    /// Parse F3DEX display list and generate RDP commands
    fn parse_f3dex_display_list(
        &mut self,
        rdram: &[u8],
        start_addr: u32,
        _size: u32,
        rdp: &mut Rdp,
    ) {
        let mut addr = start_addr as usize;
        let max_commands = 1000; // Safety limit to prevent infinite loops
        let mut commands_processed = 0;

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

            // Process F3DEX command
            let should_continue = self.execute_f3dex_command(cmd_id, word0, word1, rdram, rdp);

            if !should_continue {
                break; // G_ENDDL or branch command
            }

            addr += 8;
            commands_processed += 1;
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
        match cmd_id {
            // G_VTX (0x01) - Load vertices
            0x01 => {
                // word0: cmd_id | vn (vertex count, bits 20-11) | v0 (buffer index, bits 16-1)
                // word1: vertex data address in RDRAM
                let vertex_count = ((word0 >> 12) & 0xFF) as usize;
                let buffer_index = ((word0 >> 1) & 0x7F) as usize;
                let vertex_addr = word1;

                // Load vertices from RDRAM into vertex buffer
                for i in 0..vertex_count.min(32 - buffer_index) {
                    let vaddr = vertex_addr + (i as u32 * 16);
                    self.load_vertex(rdram, vaddr, buffer_index + i);
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
            // G_ENDDL (0xDF) - End display list
            0xDF => false,
            // RDP passthrough commands (0xE0-0xFF) - forward directly to RDP
            0xE0..=0xFF => {
                // These are RDP commands embedded in F3DEX display list
                // Process them directly (SET_COLOR, SET_SCISSOR, etc.)
                let _rdp_cmd_id = cmd_id & 0x3F;
                // Create a small display list with just this command
                let rdp_dl = [0u8; 8];
                // Note: We would copy word0/word1 here, but for now this is just a placeholder
                // In a real implementation, we'd execute the RDP command directly
                rdp.set_dpc_start(0);
                rdp.set_dpc_end(8);
                let _ = rdp_dl; // Suppress unused variable warning
                true
            }
            // Unknown/unsupported command - skip it
            _ => true,
        }
    }

    /// Transform vertices and draw triangle via RDP
    fn draw_transformed_triangle(&self, v0: usize, v1: usize, v2: usize, rdp: &mut Rdp) {
        // Get vertices from buffer
        let vert0 = &self.vertices[v0];
        let vert1 = &self.vertices[v1];
        let vert2 = &self.vertices[v2];

        // Transform vertices to screen space
        let (x0, y0, z0) = self.transform_vertex(vert0);
        let (x1, y1, z1) = self.transform_vertex(vert1);
        let (x2, y2, z2) = self.transform_vertex(vert2);

        // Convert vertex colors to ARGB format
        let c0 = u32::from_be_bytes([0xFF, vert0.color[0], vert0.color[1], vert0.color[2]]);
        let c1 = u32::from_be_bytes([0xFF, vert1.color[0], vert1.color[1], vert1.color[2]]);
        let c2 = u32::from_be_bytes([0xFF, vert2.color[0], vert2.color[1], vert2.color[2]]);

        // Draw shaded triangle with Z-buffer (assuming depth values fit in u16)
        let z0_u16 = z0.clamp(0, 0xFFFF) as u16;
        let z1_u16 = z1.clamp(0, 0xFFFF) as u16;
        let z2_u16 = z2.clamp(0, 0xFFFF) as u16;

        rdp.draw_triangle_shaded_zbuffer(
            x0, y0, z0_u16, c0, x1, y1, z1_u16, c1, x2, y2, z2_u16, c2,
        );
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

    #[test]
    fn test_f3dex_display_list_parsing() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new();

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
}
