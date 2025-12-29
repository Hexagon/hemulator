//! OpenGL-based video processor with shader support
//!
//! This module provides hardware-accelerated video processing using OpenGL.
//! It supports shader-based CRT filters and scaling for better performance.

use super::VideoProcessor;
use super::VideoResult;
use crate::display_filter::DisplayFilter;

use glow::HasContext;

/// OpenGL-based video processor
pub struct OpenGLProcessor {
    gl: glow::Context,
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    texture: glow::Texture,
    width: usize,
    height: usize,
    current_filter: DisplayFilter,
}

impl OpenGLProcessor {
    /// Create a new OpenGL processor with the given context
    pub fn new(gl: glow::Context) -> VideoResult<Self> {
        unsafe {
            // Create shader program
            let vertex_shader = compile_shader(
                &gl,
                glow::VERTEX_SHADER,
                include_str!("../shaders/vertex.glsl"),
            )?;

            let fragment_shader = compile_shader(
                &gl,
                glow::FRAGMENT_SHADER,
                include_str!("../shaders/fragment_none.glsl"),
            )?;

            let program = gl.create_program()?;
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                return Err(format!("Failed to link shader program: {}", log).into());
            }

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            // Create vertex array and buffer for fullscreen quad
            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao));

            let vbo = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            // Fullscreen quad vertices (position + texcoord)
            #[rustfmt::skip]
            let vertices: [f32; 24] = [
                // pos         // tex
                -1.0, -1.0,    0.0, 1.0,
                 1.0, -1.0,    1.0, 1.0,
                 1.0,  1.0,    1.0, 0.0,
                -1.0, -1.0,    0.0, 1.0,
                 1.0,  1.0,    1.0, 0.0,
                -1.0,  1.0,    0.0, 0.0,
            ];

            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            // Position attribute
            gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                4 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(0);

            // Texcoord attribute
            gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                4 * std::mem::size_of::<f32>() as i32,
                2 * std::mem::size_of::<f32>() as i32,
            );
            gl.enable_vertex_attrib_array(1);

            // Create texture
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
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
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            Ok(Self {
                gl,
                program,
                vao,
                vbo,
                texture,
                width: 0,
                height: 0,
                current_filter: DisplayFilter::None,
            })
        }
    }

    /// Switch shader program based on filter
    unsafe fn update_shader_for_filter(&mut self, filter: DisplayFilter) -> VideoResult<()> {
        if self.current_filter == filter {
            return Ok(()); // No need to recompile
        }

        // Delete old program
        self.gl.delete_program(self.program);

        // Create new program with appropriate fragment shader
        let vertex_shader = compile_shader(
            &self.gl,
            glow::VERTEX_SHADER,
            include_str!("../shaders/vertex.glsl"),
        )?;

        let fragment_source = match filter {
            DisplayFilter::None => include_str!("../shaders/fragment_none.glsl"),
            DisplayFilter::SonyTrinitron => include_str!("../shaders/fragment_sony_trinitron.glsl"),
            DisplayFilter::Ibm5151 => include_str!("../shaders/fragment_ibm5151.glsl"),
            DisplayFilter::Commodore1702 => include_str!("../shaders/fragment_commodore1702.glsl"),
            DisplayFilter::SharpLcd => include_str!("../shaders/fragment_sharp_lcd.glsl"),
            DisplayFilter::RcaVictor => include_str!("../shaders/fragment_rca_victor.glsl"),
        };

        let fragment_shader = compile_shader(&self.gl, glow::FRAGMENT_SHADER, fragment_source)?;

        self.program = self.gl.create_program()?;
        self.gl.attach_shader(self.program, vertex_shader);
        self.gl.attach_shader(self.program, fragment_shader);
        self.gl.link_program(self.program);

        if !self.gl.get_program_link_status(self.program) {
            let log = self.gl.get_program_info_log(self.program);
            return Err(format!("Failed to link shader program: {}", log).into());
        }

        self.gl.delete_shader(vertex_shader);
        self.gl.delete_shader(fragment_shader);
        self.current_filter = filter;

        Ok(())
    }

    /// Get a reference to the GL context (for egui integration)
    pub fn gl_context(&self) -> &glow::Context {
        &self.gl
    }
}

impl VideoProcessor for OpenGLProcessor {
    fn init(&mut self, width: usize, height: usize) -> VideoResult<()> {
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn process_frame(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
        filter: DisplayFilter,
    ) -> VideoResult<Vec<u32>> {
        unsafe {
            // Update shader if filter changed
            self.update_shader_for_filter(filter)?;

            // Upload texture
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));

            // Convert ARGB to RGBA for OpenGL
            let mut rgba_buffer = Vec::with_capacity(buffer.len() * 4);
            for pixel in buffer {
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                let a = ((pixel >> 24) & 0xFF) as u8;
                rgba_buffer.push(r);
                rgba_buffer.push(g);
                rgba_buffer.push(b);
                rgba_buffer.push(a);
            }

            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&rgba_buffer),
            );

            // Render to default framebuffer
            self.gl.use_program(Some(self.program));
            self.gl.bind_vertex_array(Some(self.vao));

            // Set uniforms
            if let Some(loc) = self.gl.get_uniform_location(self.program, "uResolution") {
                self.gl
                    .uniform_2_f32(Some(&loc), width as f32, height as f32);
            }

            if let Some(loc) = self.gl.get_uniform_location(self.program, "uTexture") {
                self.gl.uniform_1_i32(Some(&loc), 0);
            }

            // Clear and draw
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);

            // For now, we still return the original buffer since the window expects CPU buffer
            // In a full implementation, this would render directly to window via OpenGL context
            Ok(buffer.to_vec())
        }
    }

    fn resize(&mut self, width: usize, height: usize) -> VideoResult<()> {
        self.width = width;
        self.height = height;
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "OpenGL Renderer"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true
    }
}

impl Drop for OpenGLProcessor {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_texture(self.texture);
        }
    }
}

unsafe fn compile_shader(
    gl: &glow::Context,
    shader_type: u32,
    source: &str,
) -> VideoResult<glow::Shader> {
    let shader = gl.create_shader(shader_type)?;
    gl.shader_source(shader, source);
    gl.compile_shader(shader);

    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        return Err(format!("Failed to compile shader: {}", log).into());
    }

    Ok(shader)
}
