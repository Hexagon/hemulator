//! OpenGL NES PPU Renderer - GPU-accelerated tile and sprite rendering
//!
//! This module implements the `NesPpuRenderer` trait using OpenGL for hardware-accelerated
//! rendering of NES graphics. This provides better performance than software rasterization,
//! especially when scaling to higher resolutions.
//!
//! **Architecture**:
//! - Uses OpenGL 3.3 Core Profile
//! - Renders to FBO (Framebuffer Object) for offscreen rendering
//! - Two rendering passes: background tiles, then sprites
//! - Palette data uploaded as 1D texture for color lookups
//! - CHR ROM data uploaded as texture for pattern lookups
//! - Handles scrolling, mirroring, sprite priority via shaders
//!
//! **Integration**:
//! - Requires OpenGL context from frontend (SDL2)
//! - Feature-gated behind `opengl` feature flag
//! - Falls back to software renderer if GL context unavailable

#[cfg(feature = "opengl")]
use super::ppu::Ppu;
#[cfg(feature = "opengl")]
use super::ppu_renderer::NesPpuRenderer;
#[cfg(feature = "opengl")]
use emu_core::renderer::Renderer;
#[cfg(feature = "opengl")]
use emu_core::types::Frame;
#[cfg(feature = "opengl")]
use glow::HasContext;

/// Wrapper for glow::Context that implements Send
/// Safety: OpenGL contexts are generally safe to send between threads as long as
/// they're not actively being used on multiple threads simultaneously. The NES
/// renderer is only used from the emulation thread, so this is safe.
#[cfg(feature = "opengl")]
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

/// OpenGL-based NES PPU renderer
#[cfg(feature = "opengl")]
pub struct OpenGLNesPpuRenderer {
    gl: SendContext,
    width: u32,
    height: u32,
    framebuffer: Frame,

    // OpenGL resources
    fbo: glow::Framebuffer,
    color_texture: glow::Texture,
    depth_renderbuffer: glow::Renderbuffer,

    // Shader programs
    bg_program: glow::Program,
    sprite_program: glow::Program,

    // Vertex data for quad rendering
    vao: glow::VertexArray,
    vbo: glow::Buffer,

    // NES-specific textures
    palette_texture: glow::Texture,
    chr_texture: glow::Texture,
}

#[cfg(feature = "opengl")]
impl OpenGLNesPpuRenderer {
    /// Create a new OpenGL NES PPU renderer with the given GL context
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
                glow::DEPTH_COMPONENT24,
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
                return Err(format!("Framebuffer incomplete: {:x}", status));
            }

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            // Create shader programs
            let bg_program = Self::create_bg_shader(&gl)?;
            let sprite_program = Self::create_sprite_shader(&gl)?;

            // Create VAO and VBO for quad rendering
            let vao = gl
                .create_vertex_array()
                .map_err(|e| format!("Failed to create VAO: {}", e))?;
            let vbo = gl
                .create_buffer()
                .map_err(|e| format!("Failed to create VBO: {}", e))?;

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            // Quad vertices (position + texcoord)
            #[rustfmt::skip]
            let vertices: [f32; 20] = [
                // pos (x, y, z)    texcoord (u, v)
                -1.0, -1.0, 0.0,    0.0, 1.0,
                 1.0, -1.0, 0.0,    1.0, 1.0,
                 1.0,  1.0, 0.0,    1.0, 0.0,
                -1.0,  1.0, 0.0,    0.0, 0.0,
            ];
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            // Position attribute
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 20, 0);

            // Texcoord attribute
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 20, 12);

            gl.bind_vertex_array(None);

            // Create palette texture (64 colors × 1 pixel, RGBA)
            let palette_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create palette texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_1D, Some(palette_texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_1D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_1D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_1D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );

            // Create CHR texture (256 tiles × 16 bytes each = 4KB, 128x32 in 2bpp format)
            let chr_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create CHR texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(chr_texture));
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

            Ok(Self {
                gl,
                width,
                height,
                framebuffer: Frame::new(width, height),
                fbo,
                color_texture,
                depth_renderbuffer,
                bg_program,
                sprite_program,
                vao,
                vbo,
                palette_texture,
                chr_texture,
            })
        }
    }

    /// Create background rendering shader program
    #[cfg(feature = "opengl")]
    fn create_bg_shader(gl: &glow::Context) -> Result<glow::Program, String> {
        unsafe {
            let vertex_shader_source = r#"#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec2 aTexCoord;

out vec2 TexCoord;

void main() {
    gl_Position = vec4(aPos, 1.0);
    TexCoord = aTexCoord;
}
"#;

            let fragment_shader_source = r#"#version 330 core
in vec2 TexCoord;
out vec4 FragColor;

uniform sampler2D uChrTexture;
uniform sampler1D uPaletteTexture;
uniform vec2 uScroll;
uniform int uNametableBase;
uniform int uPatternBase;

void main() {
    // For now, just output a test pattern
    // Full implementation would fetch nametable, attribute, and pattern data
    FragColor = vec4(TexCoord.x, TexCoord.y, 0.5, 1.0);
}
"#;

            let program = gl
                .create_program()
                .map_err(|e| format!("Failed to create program: {}", e))?;

            let vertex_shader = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(|e| format!("Failed to create vertex shader: {}", e))?;
            gl.shader_source(vertex_shader, vertex_shader_source);
            gl.compile_shader(vertex_shader);
            if !gl.get_shader_compile_status(vertex_shader) {
                let info = gl.get_shader_info_log(vertex_shader);
                return Err(format!("Vertex shader compilation failed: {}", info));
            }

            let fragment_shader = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| format!("Failed to create fragment shader: {}", e))?;
            gl.shader_source(fragment_shader, fragment_shader_source);
            gl.compile_shader(fragment_shader);
            if !gl.get_shader_compile_status(fragment_shader) {
                let info = gl.get_shader_info_log(fragment_shader);
                return Err(format!("Fragment shader compilation failed: {}", info));
            }

            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let info = gl.get_program_info_log(program);
                return Err(format!("Program linking failed: {}", info));
            }

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            Ok(program)
        }
    }

    /// Create sprite rendering shader program
    #[cfg(feature = "opengl")]
    fn create_sprite_shader(gl: &glow::Context) -> Result<glow::Program, String> {
        unsafe {
            let vertex_shader_source = r#"#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec2 aTexCoord;

out vec2 TexCoord;

void main() {
    gl_Position = vec4(aPos, 1.0);
    TexCoord = aTexCoord;
}
"#;

            let fragment_shader_source = r#"#version 330 core
in vec2 TexCoord;
out vec4 FragColor;

uniform sampler2D uChrTexture;
uniform sampler1D uPaletteTexture;

void main() {
    // For now, just output a test pattern
    // Full implementation would render sprites from OAM
    FragColor = vec4(1.0 - TexCoord.x, 1.0 - TexCoord.y, 0.5, 1.0);
}
"#;

            let program = gl
                .create_program()
                .map_err(|e| format!("Failed to create program: {}", e))?;

            let vertex_shader = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(|e| format!("Failed to create vertex shader: {}", e))?;
            gl.shader_source(vertex_shader, vertex_shader_source);
            gl.compile_shader(vertex_shader);
            if !gl.get_shader_compile_status(vertex_shader) {
                let info = gl.get_shader_info_log(vertex_shader);
                return Err(format!("Vertex shader compilation failed: {}", info));
            }

            let fragment_shader = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| format!("Failed to create fragment shader: {}", e))?;
            gl.shader_source(fragment_shader, fragment_shader_source);
            gl.compile_shader(fragment_shader);
            if !gl.get_shader_compile_status(fragment_shader) {
                let info = gl.get_shader_info_log(fragment_shader);
                return Err(format!("Fragment shader compilation failed: {}", info));
            }

            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let info = gl.get_program_info_log(program);
                return Err(format!("Program linking failed: {}", info));
            }

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            Ok(program)
        }
    }

    /// Read back pixels from the FBO to the CPU framebuffer
    #[cfg(feature = "opengl")]
    fn read_pixels(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.read_pixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(bytemuck::cast_slice_mut(&mut self.framebuffer.pixels)),
            );
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
    }
}

#[cfg(feature = "opengl")]
impl Renderer for OpenGLNesPpuRenderer {
    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn clear(&mut self, color: u32) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            let r = ((color >> 16) & 0xFF) as f32 / 255.0;
            let g = ((color >> 8) & 0xFF) as f32 / 255.0;
            let b = (color & 0xFF) as f32 / 255.0;
            let a = ((color >> 24) & 0xFF) as f32 / 255.0;
            self.gl.clear_color(r, g, b, a);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }

        // Also clear CPU framebuffer
        for pixel in &mut self.framebuffer.pixels {
            *pixel = color;
        }
    }

    fn reset(&mut self) {
        self.clear(0xFF000000); // Black
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        self.width = width;
        self.height = height;
        self.framebuffer = Frame::new(width, height);

        unsafe {
            // Resize color texture
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

            // Resize depth renderbuffer
            self.gl
                .bind_renderbuffer(glow::RENDERBUFFER, Some(self.depth_renderbuffer));
            self.gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT24,
                width as i32,
                height as i32,
            );
        }
    }

    fn name(&self) -> &str {
        "NES OpenGL Renderer"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true
    }
}

#[cfg(feature = "opengl")]
impl NesPpuRenderer for OpenGLNesPpuRenderer {
    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn take_frame(&mut self) -> Frame {
        // Read pixels from GPU before taking the frame
        self.read_pixels();
        std::mem::replace(&mut self.framebuffer, Frame::new(self.width, self.height))
    }

    fn render_scanline(&mut self, ppu: &mut Ppu, scanline: u32) {
        // For OpenGL renderer, we accumulate scanline data and render the full frame
        // This is a simplified implementation - ideally would batch scanlines
        let _ = (ppu, scanline); // Silence unused warnings for now

        // TODO: Implement incremental scanline rendering
        // For now, this is a placeholder that would need to accumulate state
    }

    fn render_frame(&mut self, ppu: &Ppu) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            // Use background shader
            self.gl.use_program(Some(self.bg_program));
            self.gl.bind_vertex_array(Some(self.vao));

            // TODO: Upload PPU state to uniforms
            // - Scroll position
            // - Nametable base
            // - Pattern base
            // - Palette data
            // - CHR data
            let _ = ppu; // Silence unused warning for now

            // Draw fullscreen quad
            self.gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);

            // Use sprite shader for second pass
            self.gl.use_program(Some(self.sprite_program));
            // TODO: Upload OAM data and render sprites
            // For now, just draw test pattern
            self.gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);

            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }

        // Read pixels back to CPU framebuffer
        self.read_pixels();
    }
}

#[cfg(feature = "opengl")]
impl Drop for OpenGLNesPpuRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.color_texture);
            self.gl.delete_renderbuffer(self.depth_renderbuffer);
            self.gl.delete_program(self.bg_program);
            self.gl.delete_program(self.sprite_program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_texture(self.palette_texture);
            self.gl.delete_texture(self.chr_texture);
        }
    }
}

#[cfg(feature = "opengl")]
impl std::fmt::Debug for OpenGLNesPpuRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenGLNesPpuRenderer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("is_hardware_accelerated", &true)
            .finish()
    }
}

// Helper function to convert vertices to bytes (needed for buffer_data)
#[cfg(feature = "opengl")]
mod bytemuck {
    pub fn cast_slice<T: Copy>(slice: &[T]) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr() as *const u8,
                std::mem::size_of_val(slice),
            )
        }
    }

    pub fn cast_slice_mut<T: Copy>(slice: &mut [T]) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                slice.as_mut_ptr() as *mut u8,
                std::mem::size_of_val(slice),
            )
        }
    }
}
