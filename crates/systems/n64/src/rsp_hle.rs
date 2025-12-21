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

    /// Geometry mode flags
    geometry_mode: u32,
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
            geometry_mode: 0,
        }
    }

    /// Create identity matrix
    fn identity_matrix() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    /// Multiply two 4x4 matrices: result = a * b
    /// Matrices are in row-major order
    fn multiply_matrix(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut result = [0.0f32; 16];
        for i in 0..4 {
            for j in 0..4 {
                result[i * 4 + j] = a[i * 4] * b[j]
                    + a[i * 4 + 1] * b[4 + j]
                    + a[i * 4 + 2] * b[8 + j]
                    + a[i * 4 + 3] * b[12 + j];
            }
        }
        result
    }

    /// Load a 4x4 matrix from RDRAM
    /// N64 matrices are stored as 16 signed 16.16 fixed-point values (32 bits each)
    fn load_matrix_from_rdram(&self, rdram: &[u8], addr: u32) -> [f32; 16] {
        let mut matrix = [0.0f32; 16];
        let addr = addr as usize;

        // Safety check
        if addr + 63 >= rdram.len() {
            return Self::identity_matrix();
        }

        // Read 16 32-bit fixed-point values (16.16 format)
        for (i, elem) in matrix.iter_mut().enumerate() {
            let offset = addr + i * 4;
            // Read as signed 32-bit integer
            let fixed_point = i32::from_be_bytes([
                rdram[offset],
                rdram[offset + 1],
                rdram[offset + 2],
                rdram[offset + 3],
            ]);
            // Convert from 16.16 fixed-point to float
            *elem = (fixed_point as f32) / 65536.0;
        }

        matrix
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
            // G_MTX (0xDA) - Load transformation matrix
            0xDA => {
                // word0: cmd_id | param (push/nopush, load/mul, projection/modelview)
                // word1: RDRAM address of matrix (64 bytes, 4x4 matrix of 16.16 fixed point)
                let param = word0 & 0xFF;
                let matrix_addr = word1;

                // Parse matrix parameters
                let push = (param & 0x01) != 0; // G_MTX_PUSH
                let load = (param & 0x02) == 0; // G_MTX_LOAD (vs G_MTX_MUL)
                let projection = (param & 0x04) != 0; // G_MTX_PROJECTION (vs G_MTX_MODELVIEW)

                // Load matrix from RDRAM
                let matrix = self.load_matrix_from_rdram(rdram, matrix_addr);

                // Apply matrix based on type
                if projection {
                    // Projection matrix
                    if load {
                        // Replace projection matrix
                        self.projection_matrix = matrix;
                    } else {
                        // Multiply with existing projection matrix
                        self.projection_matrix =
                            Self::multiply_matrix(&self.projection_matrix, &matrix);
                    }
                } else {
                    // Modelview matrix
                    if push {
                        // In a full implementation, we'd push current matrix to stack
                        // For now, we just use a single matrix slot
                        self.matrix_stack_ptr = (self.matrix_stack_ptr + 1).min(9);
                    }
                    if load {
                        // Replace modelview matrix
                        self.modelview_matrix = matrix;
                    } else {
                        // Multiply with existing modelview matrix
                        self.modelview_matrix =
                            Self::multiply_matrix(&self.modelview_matrix, &matrix);
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
            // G_DL (0xDE) - Display list branch/call
            0xDE => {
                // word0: cmd_id | branch_type (0 = call with return, 1 = branch no return)
                // word1: RDRAM address of display list to execute
                let branch_type = (word0 >> 16) & 0xFF;
                let dl_addr = word1;

                // branch_type: 0 = G_DL_PUSH (call, will return), 1 = G_DL_NOPUSH (branch, no return)
                let is_push = branch_type == 0;

                // For now, we implement a simple non-recursive version
                // A full implementation would use a stack to handle nested display lists
                // To prevent infinite loops, we only support one level of nesting here
                if is_push {
                    // This is a display list call - we should save state and execute the nested DL
                    // For simplicity, we parse it inline without full recursion support
                    // Full implementation would push return address to a stack
                    self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
                } else {
                    // This is a branch - we don't return, but we still parse it inline
                    // In the real implementation, this would update the current DL pointer
                    self.parse_f3dex_display_list(rdram, dl_addr, 10000, rdp);
                }
                true
            }
            // G_ENDDL (0xDF) - End display list
            0xDF => false,
            // RDP passthrough commands (0xE0-0xFF) - forward directly to RDP
            0xE0..=0xFF => {
                // These are RDP commands embedded in F3DEX display list
                // Forward them directly to the RDP for execution
                // Common commands: SET_FILL_COLOR (0xF7/0x37), SET_SCISSOR (0xED/0x2D), etc.

                // The RDP command ID is in the lower 6 bits of the command byte
                let rdp_cmd_id = (word0 >> 24) & 0x3F;

                // Call RDP's execute_command directly with the command data
                // This bypasses the need for RDRAM and properly processes embedded RDP commands
                rdp.execute_rdp_command(rdp_cmd_id, word0, word1);
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
        // Full transformation pipeline:
        // 1. Apply modelview matrix (object to camera space)
        // 2. Apply projection matrix (camera to clip space)
        // 3. Perspective divide (clip to NDC)
        // 4. Viewport transform (NDC to screen space)

        // Convert vertex position to homogeneous coordinates (x, y, z, w=1)
        let v = [
            vertex.pos[0] as f32,
            vertex.pos[1] as f32,
            vertex.pos[2] as f32,
            1.0,
        ];

        // Apply modelview matrix
        let mut mv = [0.0f32; 4];
        for (i, elem) in mv.iter_mut().enumerate() {
            *elem = self.modelview_matrix[i * 4] * v[0]
                + self.modelview_matrix[i * 4 + 1] * v[1]
                + self.modelview_matrix[i * 4 + 2] * v[2]
                + self.modelview_matrix[i * 4 + 3] * v[3];
        }

        // Apply projection matrix
        let mut clip = [0.0f32; 4];
        for (i, elem) in clip.iter_mut().enumerate() {
            *elem = self.projection_matrix[i * 4] * mv[0]
                + self.projection_matrix[i * 4 + 1] * mv[1]
                + self.projection_matrix[i * 4 + 2] * mv[2]
                + self.projection_matrix[i * 4 + 3] * mv[3];
        }

        // Perspective divide (clip space to NDC)
        let w = if clip[3].abs() > 0.0001 { clip[3] } else { 1.0 };
        let ndc_x = clip[0] / w;
        let ndc_y = clip[1] / w;
        let ndc_z = clip[2] / w;

        // Viewport transform (NDC to screen space)
        // NDC range is [-1, 1], screen is [0, width-1] and [0, height-1]
        // Assuming 320x240 resolution
        let screen_x = ((ndc_x + 1.0) * 160.0) as i32; // 320/2 = 160
        let screen_y = ((1.0 - ndc_y) * 120.0) as i32; // 240/2 = 120, inverted Y
        let screen_z = ((ndc_z + 1.0) * 32767.5) as i32; // Map to 0-65535 range

        (screen_x, screen_y, screen_z)
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

        // With identity matrices:
        // Modelview: (50, 60, 100, 1) stays (50, 60, 100, 1)
        // Projection: (50, 60, 100, 1) stays (50, 60, 100, 1)
        // NDC: divide by w=1 gives (50, 60, 100)
        // Screen space:
        //   x = (50 + 1) * 160 = 51 * 160 = 8160
        //   y = (1 - 60) * 120 = -59 * 120 = -7080
        //   z = (100 + 1) * 32767.5 = 3309517.5
        assert_eq!(x, 8160);
        assert_eq!(y, -7080);
        assert!(z > 3000000);
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

    #[test]
    fn test_f3dex_quad_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new();

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
    fn test_f3dex_geometrymode_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new();

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
    fn test_g_mtx_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new();

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
        let mtx_cmd_word0: u32 = (0xDA << 24) | 0x00; // Load modelview
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
    fn test_g_mtx_projection() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 1024];
        let mut rdp = Rdp::new();

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
    fn test_g_dl_command() {
        let mut hle = RspHle::new();
        hle.microcode = MicrocodeType::F3DEX;

        let mut rdram = vec![0u8; 2048];
        let mut rdp = Rdp::new();

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
        let dl_cmd_word0: u32 = (0xDE << 24) | 0x00; // G_DL_PUSH
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
    fn test_vertex_transform_with_matrices() {
        let mut hle = RspHle::new();

        // Set up a simple scaling matrix (scale by 2)
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
        // - Screen: ((20+1)*160, (1-20)*120, (20+1)*32767.5)
        //         = (3360, -2280, 688318.5)
        assert!(x > 1000); // Should be scaled up significantly
        assert!(z > 10); // Z should also be transformed
    }
}
