//! OpenGL RDP Renderer - GPU-accelerated rasterization
//!
//! This module implements the `RdpRenderer` trait using OpenGL for hardware-accelerated
//! rendering. This provides better performance than software rasterization for complex 3D scenes.
//!
//! **Architecture**:
//! - Uses OpenGL 3.3 Core Profile
//! - Renders to FBO (Framebuffer Object) for offscreen rendering
//! - Supports flat and Gouraud shading via separate shader programs
//! - Hardware depth testing for Z-buffer
//! - Pixels read back to Frame for compatibility with current API
//!
//! **Integration**:
//! - Requires OpenGL context from frontend (SDL2)
//! - Feature-gated behind `opengl` feature flag
//! - Falls back to software renderer if GL context unavailable

#[cfg(feature = "opengl")]
use super::rdp_renderer::{RdpRenderer, ScissorBox};
#[cfg(feature = "opengl")]
use emu_core::types::Frame;
#[cfg(feature = "opengl")]
use glow::HasContext;

/// Wrapper for glow::Context that implements Send
/// Safety: OpenGL contexts are generally safe to send between threads as long as
/// they're not actively being used on multiple threads simultaneously. The RDP
/// renderer is only used from the emulation thread, so this is safe.
#[cfg(feature = "opengl")]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
struct SendContext(glow::Context);

#[cfg(feature = "opengl")]
unsafe impl Send for SendContext {}

#[cfg(feature = "opengl")]
impl std::ops::Deref for SendContext {
    type Target = glow::Context;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Shader program type for different rendering modes
#[cfg(feature = "opengl")]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
enum ShaderProgram {
    Flat,    // Solid color triangles
    Gouraud, // Per-vertex color interpolation
}

/// OpenGL-based RDP renderer
#[cfg(feature = "opengl")]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
pub struct OpenGLRdpRenderer {
    gl: SendContext,
    width: u32,
    height: u32,
    framebuffer: Frame,

    // OpenGL resources
    fbo: glow::Framebuffer,
    color_texture: glow::Texture,
    depth_renderbuffer: glow::Renderbuffer,

    // Shader programs
    flat_program: glow::Program,
    gouraud_program: glow::Program,
    current_program: ShaderProgram,

    // Vertex data
    vao: glow::VertexArray,
    vbo: glow::Buffer,

    // Z-buffer state
    zbuffer_enabled: bool,
}

#[cfg(feature = "opengl")]
impl OpenGLRdpRenderer {
    /// Create a new OpenGL renderer with the given GL context
    #[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
    pub fn new(gl: glow::Context, width: u32, height: u32) -> Result<Self, String> {
        let gl = SendContext(gl);
        unsafe {
            // Create framebuffer for offscreen rendering
            let fbo = gl
                .create_framebuffer()
                .map_err(|e| format!("Failed to create framebuffer: {}", e))?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));

            // Create color texture
            let color_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(color_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(color_texture),
                0,
            );

            // Create depth renderbuffer
            let depth_renderbuffer = gl
                .create_renderbuffer()
                .map_err(|e| format!("Failed to create renderbuffer: {}", e))?;
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth_renderbuffer));
            gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT16,
                width as i32,
                height as i32,
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(depth_renderbuffer),
            );

            // Check framebuffer status
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!("Framebuffer incomplete: status = 0x{:X}", status));
            }

            // Create shader programs
            let flat_program = create_flat_program(&gl)?;
            let gouraud_program = create_gouraud_program(&gl)?;

            // Create VAO and VBO
            let vao = gl
                .create_vertex_array()
                .map_err(|e| format!("Failed to create VAO: {}", e))?;
            let vbo = gl
                .create_buffer()
                .map_err(|e| format!("Failed to create VBO: {}", e))?;

            // Set viewport
            gl.viewport(0, 0, width as i32, height as i32);

            // Unbind framebuffer
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            Ok(Self {
                gl,
                width,
                height,
                framebuffer: Frame::new(width, height),
                fbo,
                color_texture,
                depth_renderbuffer,
                flat_program,
                gouraud_program,
                current_program: ShaderProgram::Flat,
                vao,
                vbo,
                zbuffer_enabled: false,
            })
        }
    }

    /// Read pixels from framebuffer to CPU memory
    #[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
    unsafe fn read_pixels(&mut self) {
        self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

        // Read pixels in RGBA format
        let mut pixels = vec![0u8; (self.width * self.height * 4) as usize];
        self.gl.read_pixels(
            0,
            0,
            self.width as i32,
            self.height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut pixels),
        );

        // Convert RGBA to ARGB and flip vertically (OpenGL Y is bottom-up)
        for y in 0..self.height {
            for x in 0..self.width {
                let src_idx = ((self.height - 1 - y) * self.width + x) * 4;
                let dst_idx = (y * self.width + x) as usize;

                let r = pixels[src_idx as usize];
                let g = pixels[src_idx as usize + 1];
                let b = pixels[src_idx as usize + 2];
                let a = pixels[src_idx as usize + 3];

                self.framebuffer.pixels[dst_idx] =
                    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            }
        }

        self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    }

    /// Convert screen coordinates to normalized device coordinates
    #[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
    fn screen_to_ndc(&self, x: i32, y: i32) -> (f32, f32) {
        let nx = (x as f32 / self.width as f32) * 2.0 - 1.0;
        let ny = 1.0 - (y as f32 / self.height as f32) * 2.0;
        (nx, ny)
    }

    /// Convert ARGB color to RGBA vec4
    #[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
    fn argb_to_rgba(color: u32) -> [f32; 4] {
        let a = ((color >> 24) & 0xFF) as f32 / 255.0;
        let r = ((color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((color >> 8) & 0xFF) as f32 / 255.0;
        let b = (color & 0xFF) as f32 / 255.0;
        [r, g, b, a]
    }

    /// Convert Z-buffer depth (0-65535) to OpenGL depth (0.0-1.0)
    #[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
    fn zbuffer_to_depth(z: u16) -> f32 {
        z as f32 / 65535.0
    }
}

#[cfg(feature = "opengl")]
impl RdpRenderer for OpenGLRdpRenderer {
    fn init(&mut self, width: u32, height: u32) {
        unsafe {
            self.width = width;
            self.height = height;
            self.framebuffer = Frame::new(width, height);

            // Resize textures
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.color_texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );

            self.gl
                .bind_renderbuffer(glow::RENDERBUFFER, Some(self.depth_renderbuffer));
            self.gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT16,
                width as i32,
                height as i32,
            );

            self.gl.viewport(0, 0, width as i32, height as i32);
        }
    }

    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn clear(&mut self, color: u32) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            let rgba = Self::argb_to_rgba(color);
            self.gl.clear_color(rgba[0], rgba[1], rgba[2], rgba[3]);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            self.read_pixels();
        }
    }

    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: u32,
        scissor: &ScissorBox,
    ) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            // Enable scissor test
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                scissor.x_min as i32,
                (self.height - scissor.y_max) as i32,
                (scissor.x_max - scissor.x_min) as i32,
                (scissor.y_max - scissor.y_min) as i32,
            );

            // Use flat shader
            self.gl.use_program(Some(self.flat_program));
            self.current_program = ShaderProgram::Flat;

            // Set color uniform
            let rgba = Self::argb_to_rgba(color);
            let u_color = self.gl.get_uniform_location(self.flat_program, "uColor");
            if let Some(loc) = u_color {
                self.gl
                    .uniform_4_f32(Some(&loc), rgba[0], rgba[1], rgba[2], rgba[3]);
            }

            // Create rectangle as two triangles
            let (x0, y0) = self.screen_to_ndc(x as i32, y as i32);
            let (x1, y1) = self.screen_to_ndc((x + width) as i32, (y + height) as i32);

            #[rustfmt::skip]
            let vertices: [f32; 12] = [
                x0, y0,  // Triangle 1, Vertex 1
                x1, y0,  // Triangle 1, Vertex 2
                x0, y1,  // Triangle 1, Vertex 3
                x0, y1,  // Triangle 2, Vertex 1
                x1, y0,  // Triangle 2, Vertex 2
                x1, y1,  // Triangle 2, Vertex 3
            ];

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STREAM_DRAW,
            );

            // Position attribute
            self.gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                2 * std::mem::size_of::<f32>() as i32,
                0,
            );
            self.gl.enable_vertex_attrib_array(0);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);

            self.gl.disable(glow::SCISSOR_TEST);
            self.read_pixels();
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        // For individual pixels, just update the frame directly (software fallback)
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            if idx < self.framebuffer.pixels.len() {
                self.framebuffer.pixels[idx] = color;
            }
        }
    }

    fn draw_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u32,
        scissor: &ScissorBox,
    ) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            // Enable scissor test
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                scissor.x_min as i32,
                (self.height - scissor.y_max) as i32,
                (scissor.x_max - scissor.x_min) as i32,
                (scissor.y_max - scissor.y_min) as i32,
            );

            // Use flat shader
            self.gl.use_program(Some(self.flat_program));
            self.current_program = ShaderProgram::Flat;

            // Set color uniform
            let rgba = Self::argb_to_rgba(color);
            let u_color = self.gl.get_uniform_location(self.flat_program, "uColor");
            if let Some(loc) = u_color {
                self.gl
                    .uniform_4_f32(Some(&loc), rgba[0], rgba[1], rgba[2], rgba[3]);
            }

            // Convert to NDC
            let (nx0, ny0) = self.screen_to_ndc(x0, y0);
            let (nx1, ny1) = self.screen_to_ndc(x1, y1);
            let (nx2, ny2) = self.screen_to_ndc(x2, y2);

            #[rustfmt::skip]
            let vertices: [f32; 6] = [
                nx0, ny0,
                nx1, ny1,
                nx2, ny2,
            ];

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STREAM_DRAW,
            );

            // Position attribute
            self.gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                2 * std::mem::size_of::<f32>() as i32,
                0,
            );
            self.gl.enable_vertex_attrib_array(0);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);

            self.gl.disable(glow::SCISSOR_TEST);
            self.read_pixels();
        }
    }

    fn draw_triangle_zbuffer(
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
        scissor: &ScissorBox,
    ) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            // Enable depth testing
            if self.zbuffer_enabled {
                self.gl.enable(glow::DEPTH_TEST);
                self.gl.depth_func(glow::LESS);
            }

            // Enable scissor test
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                scissor.x_min as i32,
                (self.height - scissor.y_max) as i32,
                (scissor.x_max - scissor.x_min) as i32,
                (scissor.y_max - scissor.y_min) as i32,
            );

            // Use flat shader
            self.gl.use_program(Some(self.flat_program));
            self.current_program = ShaderProgram::Flat;

            // Set color uniform
            let rgba = Self::argb_to_rgba(color);
            let u_color = self.gl.get_uniform_location(self.flat_program, "uColor");
            if let Some(loc) = u_color {
                self.gl
                    .uniform_4_f32(Some(&loc), rgba[0], rgba[1], rgba[2], rgba[3]);
            }

            // Convert to NDC and depth
            let (nx0, ny0) = self.screen_to_ndc(x0, y0);
            let (nx1, ny1) = self.screen_to_ndc(x1, y1);
            let (nx2, ny2) = self.screen_to_ndc(x2, y2);
            let d0 = Self::zbuffer_to_depth(z0);
            let d1 = Self::zbuffer_to_depth(z1);
            let d2 = Self::zbuffer_to_depth(z2);

            #[rustfmt::skip]
            let vertices: [f32; 9] = [
                nx0, ny0, d0,
                nx1, ny1, d1,
                nx2, ny2, d2,
            ];

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STREAM_DRAW,
            );

            // Position + depth attribute
            self.gl.vertex_attrib_pointer_f32(
                0,
                3,
                glow::FLOAT,
                false,
                3 * std::mem::size_of::<f32>() as i32,
                0,
            );
            self.gl.enable_vertex_attrib_array(0);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);

            self.gl.disable(glow::DEPTH_TEST);
            self.gl.disable(glow::SCISSOR_TEST);
            self.read_pixels();
        }
    }

    fn draw_triangle_shaded(
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
        scissor: &ScissorBox,
    ) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            // Enable scissor test
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                scissor.x_min as i32,
                (self.height - scissor.y_max) as i32,
                (scissor.x_max - scissor.x_min) as i32,
                (scissor.y_max - scissor.y_min) as i32,
            );

            // Use Gouraud shader
            self.gl.use_program(Some(self.gouraud_program));
            self.current_program = ShaderProgram::Gouraud;

            // Convert to NDC and RGBA
            let (nx0, ny0) = self.screen_to_ndc(x0, y0);
            let (nx1, ny1) = self.screen_to_ndc(x1, y1);
            let (nx2, ny2) = self.screen_to_ndc(x2, y2);
            let rgba0 = Self::argb_to_rgba(c0);
            let rgba1 = Self::argb_to_rgba(c1);
            let rgba2 = Self::argb_to_rgba(c2);

            #[rustfmt::skip]
            let vertices: [f32; 18] = [
                nx0, ny0, rgba0[0], rgba0[1], rgba0[2], rgba0[3],
                nx1, ny1, rgba1[0], rgba1[1], rgba1[2], rgba1[3],
                nx2, ny2, rgba2[0], rgba2[1], rgba2[2], rgba2[3],
            ];

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STREAM_DRAW,
            );

            let stride = 6 * std::mem::size_of::<f32>() as i32;

            // Position attribute
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            self.gl.enable_vertex_attrib_array(0);

            // Color attribute
            self.gl.vertex_attrib_pointer_f32(
                1,
                4,
                glow::FLOAT,
                false,
                stride,
                2 * std::mem::size_of::<f32>() as i32,
            );
            self.gl.enable_vertex_attrib_array(1);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);

            self.gl.disable(glow::SCISSOR_TEST);
            self.read_pixels();
        }
    }

    fn draw_triangle_shaded_zbuffer(
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
        scissor: &ScissorBox,
    ) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

            // Enable depth testing
            if self.zbuffer_enabled {
                self.gl.enable(glow::DEPTH_TEST);
                self.gl.depth_func(glow::LESS);
            }

            // Enable scissor test
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                scissor.x_min as i32,
                (self.height - scissor.y_max) as i32,
                (scissor.x_max - scissor.x_min) as i32,
                (scissor.y_max - scissor.y_min) as i32,
            );

            // Use Gouraud shader
            self.gl.use_program(Some(self.gouraud_program));
            self.current_program = ShaderProgram::Gouraud;

            // Convert to NDC, depth, and RGBA
            let (nx0, ny0) = self.screen_to_ndc(x0, y0);
            let (nx1, ny1) = self.screen_to_ndc(x1, y1);
            let (nx2, ny2) = self.screen_to_ndc(x2, y2);
            let d0 = Self::zbuffer_to_depth(z0);
            let d1 = Self::zbuffer_to_depth(z1);
            let d2 = Self::zbuffer_to_depth(z2);
            let rgba0 = Self::argb_to_rgba(c0);
            let rgba1 = Self::argb_to_rgba(c1);
            let rgba2 = Self::argb_to_rgba(c2);

            #[rustfmt::skip]
            let vertices: [f32; 21] = [
                nx0, ny0, rgba0[0], rgba0[1], rgba0[2], rgba0[3], d0,
                nx1, ny1, rgba1[0], rgba1[1], rgba1[2], rgba1[3], d1,
                nx2, ny2, rgba2[0], rgba2[1], rgba2[2], rgba2[3], d2,
            ];

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STREAM_DRAW,
            );

            let stride = 7 * std::mem::size_of::<f32>() as i32;

            // Position attribute
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            self.gl.enable_vertex_attrib_array(0);

            // Color attribute
            self.gl.vertex_attrib_pointer_f32(
                1,
                4,
                glow::FLOAT,
                false,
                stride,
                2 * std::mem::size_of::<f32>() as i32,
            );
            self.gl.enable_vertex_attrib_array(1);

            // Depth attribute
            self.gl.vertex_attrib_pointer_f32(
                2,
                1,
                glow::FLOAT,
                false,
                stride,
                6 * std::mem::size_of::<f32>() as i32,
            );
            self.gl.enable_vertex_attrib_array(2);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);

            self.gl.disable(glow::DEPTH_TEST);
            self.gl.disable(glow::SCISSOR_TEST);
            self.read_pixels();
        }
    }

    fn draw_triangle_textured(
        &mut self,
        _x0: i32,
        _y0: i32,
        _s0: f32,
        _t0: f32,
        _x1: i32,
        _y1: i32,
        _s1: f32,
        _t1: f32,
        _x2: i32,
        _y2: i32,
        _s2: f32,
        _t2: f32,
        _texture: &dyn Fn(f32, f32) -> u32,
        _scissor: &ScissorBox,
    ) {
        // TODO: Implement OpenGL textured triangle rendering
        // For now, this is a stub - textured rendering not yet supported in OpenGL backend
    }

    fn draw_triangle_textured_zbuffer(
        &mut self,
        _x0: i32,
        _y0: i32,
        _z0: u16,
        _s0: f32,
        _t0: f32,
        _x1: i32,
        _y1: i32,
        _z1: u16,
        _s1: f32,
        _t1: f32,
        _x2: i32,
        _y2: i32,
        _z2: u16,
        _s2: f32,
        _t2: f32,
        _texture: &dyn Fn(f32, f32) -> u32,
        _scissor: &ScissorBox,
    ) {
        // TODO: Implement OpenGL textured triangle rendering with Z-buffer
        // For now, this is a stub - textured rendering not yet supported in OpenGL backend
    }

    fn clear_zbuffer(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.clear(glow::DEPTH_BUFFER_BIT);
        }
    }

    fn set_zbuffer_enabled(&mut self, enabled: bool) {
        self.zbuffer_enabled = enabled;
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.init(width, height);
    }

    fn reset(&mut self) {
        self.clear(0xFF000000);
        self.clear_zbuffer();
    }

    fn name(&self) -> &str {
        "OpenGL RDP Renderer"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true
    }
}

/// Helper function to compile a shader
#[cfg(feature = "opengl")]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
unsafe fn compile_shader(
    gl: &SendContext,
    shader_type: u32,
    source: &str,
) -> Result<glow::Shader, String> {
    let shader = gl
        .create_shader(shader_type)
        .map_err(|e| format!("Failed to create shader: {}", e))?;

    gl.shader_source(shader, source);
    gl.compile_shader(shader);

    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        return Err(format!("Shader compilation failed: {}", log));
    }

    Ok(shader)
}

/// Create flat shading program
#[cfg(feature = "opengl")]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
fn create_flat_program(gl: &SendContext) -> Result<glow::Program, String> {
    unsafe {
        let vertex_shader =
            compile_shader(gl, glow::VERTEX_SHADER, include_str!("shaders/vertex.glsl"))?;

        let fragment_shader = compile_shader(
            gl,
            glow::FRAGMENT_SHADER,
            include_str!("shaders/fragment_flat.glsl"),
        )?;

        let program = gl
            .create_program()
            .map_err(|e| format!("Failed to create program: {}", e))?;

        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("Program linking failed: {}", log));
        }

        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);

        Ok(program)
    }
}

/// Create Gouraud shading program
#[cfg(feature = "opengl")]
#[allow(dead_code)] // Stub implementation - will be used when OpenGL renderer is fully integrated
fn create_gouraud_program(gl: &SendContext) -> Result<glow::Program, String> {
    unsafe {
        let vertex_shader =
            compile_shader(gl, glow::VERTEX_SHADER, include_str!("shaders/vertex.glsl"))?;

        let fragment_shader = compile_shader(
            gl,
            glow::FRAGMENT_SHADER,
            include_str!("shaders/fragment_gouraud.glsl"),
        )?;

        let program = gl
            .create_program()
            .map_err(|e| format!("Failed to create program: {}", e))?;

        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("Program linking failed: {}", log));
        }

        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);

        Ok(program)
    }
}

/// Implement Drop to clean up OpenGL resources
#[cfg(feature = "opengl")]
impl Drop for OpenGLRdpRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.color_texture);
            self.gl.delete_renderbuffer(self.depth_renderbuffer);
            self.gl.delete_program(self.flat_program);
            self.gl.delete_program(self.gouraud_program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
        }
    }
}

#[cfg(test)]
#[cfg(feature = "opengl")]
mod tests {
    #[allow(unused_imports)] // Test module for documentation purposes
    use super::*;

    #[test]
    fn test_opengl_renderer_creation_requires_context() {
        // This test documents that the OpenGL renderer now requires a GL context
        // We can't easily test actual OpenGL functionality in unit tests without
        // a headless GL context, so this is primarily documentation

        // Note: In the real application, the GL context comes from SDL2 frontend
        // The renderer is created like:
        // let gl = glow::Context::from_loader_function(...);
        // let renderer = OpenGLRdpRenderer::new(gl, 320, 240)?;
    }
}
