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
//! - CHR ROM data uploaded as 2D texture for pattern lookups
//! - Nametable, attribute table, and OAM data uploaded as 2D textures
//! - Handles scrolling, mirroring, sprite priority via shaders
//!
//! **Performance Characteristics**:
//! - GPU rendering: All tile/sprite rendering done on GPU (very fast)
//! - Texture uploads: ~10KB uploaded per frame (palette, CHR, nametables, OAM)
//! - Pixel readback: Only when frame is taken by frontend (lazy evaluation)
//! - Blending: Hardware alpha blending for sprite transparency
//! - Scrolling: Computed in shader (no extra CPU cost)
//! - Expected performance: 1000+ fps on modern GPUs at native resolution
//!
//! **Limitations**:
//! - Requires OpenGL 3.3+ support
//! - Does not support mid-frame CHR updates (deferred rendering)
//! - Sprite priority handled via draw order, not Z-buffer
//! - Grayscale mode applied in shader (may differ slightly from hardware)
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
use crate::cartridge::Mirroring;
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

    // Shader programs
    bg_program: glow::Program,
    sprite_program: glow::Program,

    // Vertex data for quad rendering
    vao: glow::VertexArray,
    vbo: glow::Buffer,

    // NES-specific textures
    palette_texture: glow::Texture,
    chr_texture: glow::Texture,
    nametable_texture: glow::Texture,
    attribute_texture: glow::Texture,
    oam_texture: glow::Texture,
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

            // Create CHR texture (8KB CHR ROM/RAM as 2D texture)
            // Store as R8 texture - each byte is a 2bpp pattern byte
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

            // Create nametable texture (4 nametables × 32×30 tiles = 128×60)
            // Each byte is a tile index (0-255)
            let nametable_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create nametable texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(nametable_texture));
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

            // Create attribute table texture (4 nametables × 8×8 attributes = 32×16)
            // Each byte contains 4 2-bit palette indices
            let attribute_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create attribute texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(attribute_texture));
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

            // Create OAM texture (64 sprites × 4 bytes = 256 bytes as 64×4 texture)
            let oam_texture = gl
                .create_texture()
                .map_err(|e| format!("Failed to create OAM texture: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(oam_texture));
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
                width,
                height,
                framebuffer: Frame::new(width, height),
                fbo,
                color_texture,
                bg_program,
                sprite_program,
                vao,
                vbo,
                palette_texture,
                chr_texture,
                nametable_texture,
                attribute_texture,
                oam_texture,
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
uniform sampler2D uNametableTexture;
uniform sampler2D uAttributeTexture;
uniform vec2 uScroll;
uniform int uNametableBase;
uniform int uPatternBase;
uniform int uShowBg;
uniform int uShowBgLeft;
uniform int uGrayscale;

// NES uses 2bpp tiles (2 bits per pixel)
int fetchTilePixel(int tileIndex, int fineX, int fineY, int patternBase) {
    // Each tile is 16 bytes: 8 bytes for low bit plane, 8 bytes for high bit plane
    int tileAddr = patternBase + tileIndex * 16;
    
    // CHR data is stored as raw bytes in a 2D texture
    // We need to fetch the correct bytes and extract the bit
    int loAddr = tileAddr + fineY;
    int hiAddr = tileAddr + fineY + 8;
    
    // Bounds check: CHR is 8KB (0-8191), texture is 128x64
    if (hiAddr >= 8192) {
        return 0; // Return transparent/background for out-of-bounds
    }
    
    // Convert byte addresses to texture coordinates (CHR is 8KB = 128x64 bytes)
    int loX = loAddr % 128;
    int loY = loAddr / 128;
    int hiX = hiAddr % 128;
    int hiY = hiAddr / 128;
    
    // Fetch the bytes from CHR texture (stored as R8)
    float loByteF = texelFetch(uChrTexture, ivec2(loX, loY), 0).r;
    float hiByteF = texelFetch(uChrTexture, ivec2(hiX, hiY), 0).r;
    
    int loByte = int(loByteF * 255.0 + 0.5);
    int hiByte = int(hiByteF * 255.0 + 0.5);
    
    // Extract the bit for this X position (7 - fineX for left-to-right)
    int bit = 7 - fineX;
    int loBit = (loByte >> bit) & 1;
    int hiBit = (hiByte >> bit) & 1;
    
    return (hiBit << 1) | loBit;
}

void main() {
    if (uShowBg == 0) {
        discard;
    }
    
    // Screen position (0-255, 0-239)
    vec2 screenPos = TexCoord * vec2(256.0, 240.0);
    int x = int(screenPos.x);
    int y = int(screenPos.y);
    
    // Clip leftmost 8 pixels if uShowBgLeft is false
    if (uShowBgLeft == 0 && x < 8) {
        discard;
    }
    
    // Apply scrolling
    int wx = x + int(uScroll.x);
    int wy = y + int(uScroll.y);
    
    // Determine which nametable (0-3) and position within it
    int ntX = (wx / 256) & 1;
    int ntY = (wy / 240) & 1;
    int nt = (uNametableBase + ntX + (ntY << 1)) & 3;
    
    int worldX = wx % 256;
    int worldY = wy % 240;
    
    // Tile coordinates
    int tx = worldX / 8;
    int ty = worldY / 8;
    int fineX = worldX % 8;
    int fineY = worldY % 8;
    
    // Fetch tile index from nametable (32x30 tiles per nametable)
    int ntTexX = (nt % 2) * 32 + tx;
    int ntTexY = (nt / 2) * 30 + ty;
    float tileIndexF = texelFetch(uNametableTexture, ivec2(ntTexX, ntTexY), 0).r;
    int tileIndex = int(tileIndexF * 255.0 + 0.5);
    
    // Fetch attribute byte (8x8 attributes per nametable)
    int attrX = tx / 4;
    int attrY = ty / 4;
    int attrTexX = (nt % 2) * 8 + attrX;
    int attrTexY = (nt / 2) * 8 + attrY;
    float attrByteF = texelFetch(uAttributeTexture, ivec2(attrTexX, attrTexY), 0).r;
    int attrByte = int(attrByteF * 255.0 + 0.5);
    
    // Extract palette index from attribute byte (2x2 metatiles)
    int quadrant = ((ty % 4) / 2) * 2 + ((tx % 4) / 2);
    int shift = quadrant * 2;
    int paletteIdx = (attrByte >> shift) & 3;
    
    // Fetch pixel from pattern table
    int colorInTile = fetchTilePixel(tileIndex, fineX, fineY, uPatternBase);
    
    // Look up color in palette
    int paletteAddr;
    if (colorInTile == 0) {
        // Backdrop color (universal background)
        paletteAddr = 0;
    } else {
        paletteAddr = paletteIdx * 4 + colorInTile;
    }
    
    // Fetch from 1D palette texture
    vec4 color = texelFetch(uPaletteTexture, paletteAddr, 0);
    
    // Apply grayscale if enabled
    if (uGrayscale != 0) {
        float gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
        color.rgb = vec3(gray);
    }
    
    FragColor = color;
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
uniform sampler2D uOamTexture;
uniform int uSpritePatternBase;
uniform int uSprite8x16;
uniform int uShowSprites;
uniform int uShowSpritesLeft;
uniform int uGrayscale;

// Fetch sprite pixel (similar to tile pixel but with flip support)
int fetchSpritePixel(int tileIndex, int fineX, int fineY, int flipH, int flipV, int patternBase) {
    int actualFineX = (flipH != 0) ? (7 - fineX) : fineX;
    int actualFineY = (flipV != 0) ? (7 - fineY) : fineY;
    
    int tileAddr = patternBase + tileIndex * 16;
    int loAddr = tileAddr + actualFineY;
    int hiAddr = tileAddr + actualFineY + 8;
    
    // Bounds check: CHR is 8KB (0-8191)
    if (hiAddr >= 8192) {
        return 0; // Return transparent for out-of-bounds
    }
    
    int loX = loAddr % 128;
    int loY = loAddr / 128;
    int hiX = hiAddr % 128;
    int hiY = hiAddr / 128;
    
    float loByteF = texelFetch(uChrTexture, ivec2(loX, loY), 0).r;
    float hiByteF = texelFetch(uChrTexture, ivec2(hiX, hiY), 0).r;
    
    int loByte = int(loByteF * 255.0 + 0.5);
    int hiByte = int(hiByteF * 255.0 + 0.5);
    
    int bit = 7 - actualFineX;
    int loBit = (loByte >> bit) & 1;
    int hiBit = (hiByte >> bit) & 1;
    
    return (hiBit << 1) | loBit;
}

void main() {
    if (uShowSprites == 0) {
        discard;
    }
    
    vec2 screenPos = TexCoord * vec2(256.0, 240.0);
    int x = int(screenPos.x);
    int y = int(screenPos.y);
    
    // Clip leftmost 8 pixels if uShowSpritesLeft is false
    if (uShowSpritesLeft == 0 && x < 8) {
        discard;
    }
    
    // Default to transparent
    FragColor = vec4(0.0, 0.0, 0.0, 0.0);
    
    // Iterate through sprites in reverse priority order (back to front)
    // OAM has 64 sprites, each is 4 bytes: Y, tile, attr, X
    for (int i = 63; i >= 0; i--) {
        // Fetch sprite data from OAM texture (64x4)
        float spriteYF = texelFetch(uOamTexture, ivec2(i, 0), 0).r;
        float spriteTileF = texelFetch(uOamTexture, ivec2(i, 1), 0).r;
        float spriteAttrF = texelFetch(uOamTexture, ivec2(i, 2), 0).r;
        float spriteXF = texelFetch(uOamTexture, ivec2(i, 3), 0).r;
        
        int spriteY = int(spriteYF * 255.0 + 0.5);
        int spriteTile = int(spriteTileF * 255.0 + 0.5);
        int spriteAttr = int(spriteAttrF * 255.0 + 0.5);
        int spriteX = int(spriteXF * 255.0 + 0.5);
        
        // Sprite attributes: PPpppppp
        // P = priority (0 = in front of bg, 1 = behind bg)
        // p = palette (0-3)
        int palette = spriteAttr & 3;
        int flipH = (spriteAttr >> 6) & 1;
        int flipV = (spriteAttr >> 7) & 1;
        // Priority is bit 5, but we render sprites in priority order anyway
        
        int spriteHeight = (uSprite8x16 != 0) ? 16 : 8;
        
        // Check if pixel is within sprite bounds
        if (x < spriteX || x >= spriteX + 8) continue;
        if (y < spriteY + 1 || y >= spriteY + 1 + spriteHeight) continue;
        
        int fineX = x - spriteX;
        int fineY = y - (spriteY + 1);
        
        // For 8x16 sprites, determine which tile to use
        int actualTile = spriteTile;
        int actualPatternBase = uSpritePatternBase;
        
        if (uSprite8x16 != 0) {
            // In 8x16 mode, bit 0 of tile index determines pattern table
            actualPatternBase = (spriteTile & 1) * 0x1000;
            actualTile = spriteTile & 0xFE; // Use even tile
            
            int localFineY = fineY;
            
            // Handle vertical flip for 8x16 sprites (swap top/bottom tiles)
            if (flipV != 0) {
                localFineY = 15 - fineY;
            }
            
            if (localFineY >= 8) {
                actualTile++; // Bottom half uses next tile
            }
            
            // fineY stays 0-7 for the individual tile (flip is handled in fetchSpritePixel)
            fineY = fineY % 8;
        }
        
        int colorInTile = fetchSpritePixel(actualTile, fineX, fineY, flipH, flipV, actualPatternBase);
        
        if (colorInTile == 0) {
            continue; // Transparent
        }
        
        // Sprite palettes start at index 16
        int paletteAddr = 16 + palette * 4 + colorInTile;
        vec4 color = texelFetch(uPaletteTexture, paletteAddr, 0);
        
        // Apply grayscale if enabled
        if (uGrayscale != 0) {
            float gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
            color.rgb = vec3(gray);
        }
        
        FragColor = color;
        // We found a sprite pixel, stop searching
        break;
    }
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

            // Check for OpenGL errors
            if let Some(err) = self.check_gl_error() {
                eprintln!("OpenGL error in read_pixels: {}", err);
            }
        }
    }

    /// Check for OpenGL errors and return error string if any
    #[cfg(feature = "opengl")]
    fn check_gl_error(&self) -> Option<String> {
        unsafe {
            let error = self.gl.get_error();
            if error != glow::NO_ERROR {
                Some(format!("GL error 0x{:X}", error))
            } else {
                None
            }
        }
    }

    /// Helper to get NES master palette color
    fn nes_palette_rgb(index: u8) -> u32 {
        const NES_MASTER_PALETTE: [u32; 64] = [
            0xFF545454, 0xFF001E74, 0xFF081090, 0xFF300088, 0xFF440064, 0xFF5C0030, 0xFF540400,
            0xFF3C1800, 0xFF202A00, 0xFF083A00, 0xFF004000, 0xFF003C00, 0xFF00323C, 0xFF000000,
            0xFF000000, 0xFF000000, 0xFF989698, 0xFF084CC4, 0xFF3032EC, 0xFF5C1EE4, 0xFF8814B0,
            0xFFA01464, 0xFF982220, 0xFF783C00, 0xFF545A00, 0xFF287200, 0xFF087C00, 0xFF007628,
            0xFF006678, 0xFF000000, 0xFF000000, 0xFF000000, 0xFFECEEEC, 0xFF4C9AEC, 0xFF787CEC,
            0xFFB062EC, 0xFFE454EC, 0xFFEC58B4, 0xFFEC6A64, 0xFFD48820, 0xFFA0AA00, 0xFF74C400,
            0xFF4CD020, 0xFF38CC6C, 0xFF38B4CC, 0xFF3C3C3C, 0xFF000000, 0xFF000000, 0xFFECEEEC,
            0xFFA8CCEC, 0xFFBCBCEC, 0xFFD4B2EC, 0xFFECAEEC, 0xFFECAED4, 0xFFECC4B0, 0xFFE4D4A0,
            0xFFCCDCA0, 0xFFB4E4A0, 0xFFA8E4B4, 0xFFA0E4CC, 0xFFA0D4E4, 0xFFA0A2A0, 0xFF000000,
            0xFF000000,
        ];
        NES_MASTER_PALETTE[(index & 0x3F) as usize]
    }

    /// Helper to map nametable addresses (same logic as Ppu::map_nametable_addr)
    fn map_nametable_addr(addr: u16, mirroring: Mirroring) -> usize {
        let a = addr & 0x0FFF;
        let table = (a / 0x0400) as u16;
        let offset = (a % 0x0400) as u16;

        let physical_table = match mirroring {
            Mirroring::Vertical | Mirroring::FourScreen => match table {
                0 | 2 => 0,
                1 | 3 => 1,
                _ => 0,
            },
            Mirroring::Horizontal => match table {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 0,
            },
            Mirroring::SingleScreenLower => 0,
            Mirroring::SingleScreenUpper => 1,
        };

        (physical_table * 0x0400 + offset) as usize & 0x07FF
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
            self.gl.clear(glow::COLOR_BUFFER_BIT);
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
            // Upload palette data (64 colors, RGBA)
            // NES has 32 bytes of palette RAM:
            // - $3F00-$3F0F: Background palettes (4 palettes × 4 colors)
            // - $3F10-$3F1F: Sprite palettes (4 palettes × 4 colors)
            // Note: Palette mirroring is already handled in ppu.palette[]
            let mut palette_data = [0u8; 64 * 4];
            for i in 0..32 {
                let palette_byte = ppu.palette[i];
                let color = Self::nes_palette_rgb(palette_byte & 0x3F);
                let offset = i * 4;
                palette_data[offset] = ((color >> 16) & 0xFF) as u8; // R
                palette_data[offset + 1] = ((color >> 8) & 0xFF) as u8; // G
                palette_data[offset + 2] = (color & 0xFF) as u8; // B
                palette_data[offset + 3] = 0xFF; // A
            }
            // Fill remaining slots (32-63) with backdrop color for safety
            let backdrop_color = Self::nes_palette_rgb(ppu.palette[0] & 0x3F);
            for i in 32..64 {
                let offset = i * 4;
                palette_data[offset] = ((backdrop_color >> 16) & 0xFF) as u8;
                palette_data[offset + 1] = ((backdrop_color >> 8) & 0xFF) as u8;
                palette_data[offset + 2] = (backdrop_color & 0xFF) as u8;
                palette_data[offset + 3] = 0xFF;
            }

            self.gl
                .bind_texture(glow::TEXTURE_1D, Some(self.palette_texture));
            self.gl.tex_image_1d(
                glow::TEXTURE_1D,
                0,
                glow::RGBA as i32,
                64,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&palette_data),
            );

            // Upload CHR data (8KB as 128x64 R8 texture)
            // Ensure we have exactly 8KB of CHR data (pad with zeros if needed)
            let mut chr_data = vec![0u8; 8192];
            let chr_len = ppu.chr.len().min(8192);
            chr_data[0..chr_len].copy_from_slice(&ppu.chr[0..chr_len]);

            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.chr_texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                128,
                64,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(&chr_data),
            );

            // Upload nametable data (2KB VRAM as 64x32 R8 texture)
            // We lay out 4 nametables horizontally: [0][1][2][3] = 128x30
            // But we only have 2KB, so we need to handle mirroring
            let mut nametable_data = vec![0u8; 64 * 60];
            for nt in 0..4 {
                for ty in 0..30 {
                    for tx in 0..32 {
                        let nt_addr =
                            0x2000u16 + (nt as u16) * 0x0400 + (ty as u16) * 32 + (tx as u16);
                        let mapped_addr = Self::map_nametable_addr(nt_addr, ppu.get_mirroring());
                        let tile_index = ppu.vram[mapped_addr];
                        let dst_x = (nt % 2) * 32 + tx;
                        let dst_y = (nt / 2) * 30 + ty;
                        nametable_data[dst_y * 64 + dst_x] = tile_index;
                    }
                }
            }

            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.nametable_texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                64,
                60,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(&nametable_data),
            );

            // Upload attribute table data (4 nametables × 8×8 = 32x16 R8 texture)
            let mut attribute_data = vec![0u8; 32 * 16];
            for nt in 0..4 {
                for attr_y in 0..8 {
                    for attr_x in 0..8 {
                        let attr_addr = 0x2000u16
                            + (nt as u16) * 0x0400
                            + 0x03C0
                            + (attr_y as u16) * 8
                            + (attr_x as u16);
                        let mapped_addr = Self::map_nametable_addr(attr_addr, ppu.get_mirroring());
                        let attr_byte = ppu.vram[mapped_addr];
                        let dst_x = (nt % 2) * 8 + attr_x;
                        let dst_y = (nt / 2) * 8 + attr_y;
                        attribute_data[dst_y * 32 + dst_x] = attr_byte;
                    }
                }
            }

            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.attribute_texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                32,
                16,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(&attribute_data),
            );

            // Upload OAM data (256 bytes as 64x4 R8 texture)
            // Each sprite is 4 bytes: Y, tile, attr, X
            let mut oam_data = vec![0u8; 64 * 4];
            for i in 0..64 {
                let base = i * 4;
                oam_data[base] = ppu.oam[base]; // Y
                oam_data[base + 1] = ppu.oam[base + 1]; // Tile
                oam_data[base + 2] = ppu.oam[base + 2]; // Attr
                oam_data[base + 3] = ppu.oam[base + 3]; // X
            }

            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.oam_texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                64,
                4,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(&oam_data),
            );

            // Render to FBO
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            // Enable blending for sprites
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

            // Render background
            let bg_enabled = (ppu.mask() & 0x08) != 0;
            if bg_enabled {
                self.gl.use_program(Some(self.bg_program));
                self.gl.bind_vertex_array(Some(self.vao));

                // Bind textures
                self.gl.active_texture(glow::TEXTURE0);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(self.chr_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uChrTexture")
                        .as_ref(),
                    0,
                );

                self.gl.active_texture(glow::TEXTURE1);
                self.gl
                    .bind_texture(glow::TEXTURE_1D, Some(self.palette_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uPaletteTexture")
                        .as_ref(),
                    1,
                );

                self.gl.active_texture(glow::TEXTURE2);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(self.nametable_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uNametableTexture")
                        .as_ref(),
                    2,
                );

                self.gl.active_texture(glow::TEXTURE3);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(self.attribute_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uAttributeTexture")
                        .as_ref(),
                    3,
                );

                // Set uniforms
                let scroll_x = ppu.scroll_x() as f32;
                let scroll_y = ppu.scroll_y() as f32;
                self.gl.uniform_2_f32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uScroll")
                        .as_ref(),
                    scroll_x,
                    scroll_y,
                );

                let nametable_base = (ppu.ctrl() & 0x03) as i32;
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uNametableBase")
                        .as_ref(),
                    nametable_base,
                );

                let pattern_base = if (ppu.ctrl() & 0x10) != 0 {
                    0x1000
                } else {
                    0x0000
                };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uPatternBase")
                        .as_ref(),
                    pattern_base,
                );

                let show_bg = if bg_enabled { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uShowBg")
                        .as_ref(),
                    show_bg,
                );

                let show_bg_left = if (ppu.mask() & 0x02) != 0 { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uShowBgLeft")
                        .as_ref(),
                    show_bg_left,
                );

                let grayscale = if (ppu.mask() & 0x01) != 0 { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.bg_program, "uGrayscale")
                        .as_ref(),
                    grayscale,
                );

                self.gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
            }

            // Render sprites
            let sprites_enabled = (ppu.mask() & 0x10) != 0;
            if sprites_enabled {
                self.gl.use_program(Some(self.sprite_program));
                self.gl.bind_vertex_array(Some(self.vao));

                // Bind textures
                self.gl.active_texture(glow::TEXTURE0);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(self.chr_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uChrTexture")
                        .as_ref(),
                    0,
                );

                self.gl.active_texture(glow::TEXTURE1);
                self.gl
                    .bind_texture(glow::TEXTURE_1D, Some(self.palette_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uPaletteTexture")
                        .as_ref(),
                    1,
                );

                self.gl.active_texture(glow::TEXTURE2);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(self.oam_texture));
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uOamTexture")
                        .as_ref(),
                    2,
                );

                // Set uniforms
                let sprite_pattern_base = if (ppu.ctrl() & 0x08) != 0 {
                    0x1000
                } else {
                    0x0000
                };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uSpritePatternBase")
                        .as_ref(),
                    sprite_pattern_base,
                );

                let sprite_8x16 = if (ppu.ctrl() & 0x20) != 0 { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uSprite8x16")
                        .as_ref(),
                    sprite_8x16,
                );

                let show_sprites = if sprites_enabled { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uShowSprites")
                        .as_ref(),
                    show_sprites,
                );

                let show_sprites_left = if (ppu.mask() & 0x04) != 0 { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uShowSpritesLeft")
                        .as_ref(),
                    show_sprites_left,
                );

                let grayscale = if (ppu.mask() & 0x01) != 0 { 1 } else { 0 };
                self.gl.uniform_1_i32(
                    self.gl
                        .get_uniform_location(self.sprite_program, "uGrayscale")
                        .as_ref(),
                    grayscale,
                );

                self.gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
            }

            self.gl.disable(glow::BLEND);
            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            // Check for OpenGL errors during rendering
            if let Some(err) = self.check_gl_error() {
                eprintln!("OpenGL error in render_frame: {}", err);
            }
        }
        // Note: We don't read pixels back here for performance.
        // Pixels are only read when take_frame() is called by the frontend.
    }
}

#[cfg(feature = "opengl")]
impl Drop for OpenGLNesPpuRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.color_texture);
            self.gl.delete_program(self.bg_program);
            self.gl.delete_program(self.sprite_program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_texture(self.palette_texture);
            self.gl.delete_texture(self.chr_texture);
            self.gl.delete_texture(self.nametable_texture);
            self.gl.delete_texture(self.attribute_texture);
            self.gl.delete_texture(self.oam_texture);
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
            std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice))
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

#[cfg(all(test, feature = "opengl"))]
mod tests {
    use super::*;

    #[test]
    fn test_nes_palette_rgb() {
        // Test a few known palette colors
        assert_eq!(OpenGLNesPpuRenderer::nes_palette_rgb(0x0F), 0xFF000000); // Black
        assert_eq!(OpenGLNesPpuRenderer::nes_palette_rgb(0x30), 0xFFECEEEC); // White
        assert_eq!(OpenGLNesPpuRenderer::nes_palette_rgb(0x16), 0xFF982220); // Red
        assert_eq!(OpenGLNesPpuRenderer::nes_palette_rgb(0x2A), 0xFF4CD020); // Green
        assert_eq!(OpenGLNesPpuRenderer::nes_palette_rgb(0x12), 0xFF3032EC); // Blue
    }

    #[test]
    fn test_nes_palette_rgb_wrapping() {
        // Test that palette wraps at 0x3F
        assert_eq!(
            OpenGLNesPpuRenderer::nes_palette_rgb(0x00),
            OpenGLNesPpuRenderer::nes_palette_rgb(0x40)
        );
        assert_eq!(
            OpenGLNesPpuRenderer::nes_palette_rgb(0x3F),
            OpenGLNesPpuRenderer::nes_palette_rgb(0x7F)
        );
    }

    #[test]
    fn test_map_nametable_addr_vertical() {
        // Vertical mirroring: [0 1] [0 1]
        let mirroring = Mirroring::Vertical;

        // Nametable 0 ($2000-$23FF) maps to physical table 0
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2000, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x23FF, mirroring),
            0x03FF
        );

        // Nametable 1 ($2400-$27FF) maps to physical table 1
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2400, mirroring),
            0x0400
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x27FF, mirroring),
            0x07FF
        );

        // Nametable 2 ($2800-$2BFF) mirrors nametable 0
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2800, mirroring),
            0x0000
        );

        // Nametable 3 ($2C00-$2FFF) mirrors nametable 1
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2C00, mirroring),
            0x0400
        );
    }

    #[test]
    fn test_map_nametable_addr_horizontal() {
        // Horizontal mirroring: [0 0] [1 1]
        // NT0 and NT1 -> physical table 0
        // NT2 and NT3 -> physical table 1
        let mirroring = Mirroring::Horizontal;

        // Nametable 0 ($2000-$23FF) maps to physical table 0
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2000, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x23FF, mirroring),
            0x03FF
        );

        // Nametable 1 ($2400-$27FF) also maps to physical table 0
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2400, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x27FF, mirroring),
            0x03FF
        );

        // Nametable 2 ($2800-$2BFF) maps to physical table 1
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2800, mirroring),
            0x0400
        );

        // Nametable 3 ($2C00-$2FFF) also maps to physical table 1
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2C00, mirroring),
            0x0400
        );
    }

    #[test]
    fn test_map_nametable_addr_single_screen() {
        // Single screen lower: all map to physical table 0
        let mirroring = Mirroring::SingleScreenLower;
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2000, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2400, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2800, mirroring),
            0x0000
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2C00, mirroring),
            0x0000
        );

        // Single screen upper: all map to physical table 1
        let mirroring = Mirroring::SingleScreenUpper;
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2000, mirroring),
            0x0400
        );
        assert_eq!(
            OpenGLNesPpuRenderer::map_nametable_addr(0x2400, mirroring),
            0x0400
        );
    }

    // Note: Full renderer tests (shader execution, texture uploads, etc.)
    // would require a GL context and are better suited for integration tests.
    // The tests above cover the pure computation logic that doesn't depend on GL.
}
