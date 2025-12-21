# N64 RDP Renderer Architecture

## Status: Software Renderer Complete, OpenGL Stub Available

## Problem Statement

The N64 RDP (Reality Display Processor) currently performs all rendering in software (CPU-based rasterization). This works but can be slow for complex 3D scenes. The question is: **Where should we separate OpenGL and software processing in the N64 emulator?**

## Solution: Pluggable RDP Renderer Backend

We've implemented a pluggable renderer architecture similar to the frontend's `VideoProcessor` trait.

### Architecture Overview

```
┌─────────────────────────────────────────────┐
│           N64System                         │
│  ┌───────────────────────────────────────┐  │
│  │  RDP (State Management)               │  │
│  │  • Registers (DPC_START, DPC_END)     │  │
│  │  • TMEM (4KB texture memory)          │  │
│  │  • Tile descriptors (8 tiles)         │  │
│  │  • Scissor box                        │  │
│  │  • Fill color                         │  │
│  │  • Display list processing           │  │
│  │                                       │  │
│  │  ┌─────────────────────────────────┐ │  │
│  │  │ renderer: Box<dyn RdpRenderer>  │ │  │
│  │  │                                 │ │  │
│  │  │  ┌──────────────────────────┐  │ │  │
│  │  │  │ RdpRenderer Trait        │  │ │  │
│  │  │  │ • get_frame()            │  │ │  │
│  │  │  │ • clear()                │  │ │  │
│  │  │  │ • fill_rect()            │  │ │  │
│  │  │  │ • set_pixel()            │  │ │  │
│  │  │  │ • draw_triangle()        │  │ │  │
│  │  │  │ • draw_triangle_zbuffer()│  │ │  │
│  │  │  │ • draw_triangle_shaded() │  │ │  │
│  │  │  │ • clear_zbuffer()        │  │ │  │
│  │  │  │ • set_zbuffer_enabled()  │  │ │  │
│  │  │  │ • reset()                │  │ │  │
│  │  │  └──────────────────────────┘  │ │  │
│  │  │                                 │ │  │
│  │  │  Implementations:               │ │  │
│  │  │  ┌──────────────────────────┐  │ │  │
│  │  │  │ SoftwareRdpRenderer      │  │ │  │
│  │  │  │ • CPU rasterization      │  │ │  │
│  │  │  │ • Framebuffer (ARGB)     │  │ │  │
│  │  │  │ • Z-buffer (16-bit)      │  │ │  │
│  │  │  └──────────────────────────┘  │ │  │
│  │  │  ┌──────────────────────────┐  │ │  │
│  │  │  │ OpenGLRdpRenderer        │  │ │  │
│  │  │  │ (future implementation)  │  │ │  │
│  │  │  │ • GPU rasterization      │  │ │  │
│  │  │  │ • OpenGL framebuffer     │  │ │  │
│  │  │  │ • Hardware depth testing │  │ │  │
│  │  │  └──────────────────────────┘  │ │  │
│  │  └─────────────────────────────────┘ │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Implementation Details

### RdpRenderer Trait (`rdp_renderer.rs`)

```rust
pub trait RdpRenderer: Send {
    fn get_frame(&self) -> &Frame;
    fn clear(&mut self, color: u32);
    fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32, scissor: &ScissorBox);
    fn set_pixel(&mut self, x: u32, y: u32, color: u32);
    fn draw_triangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, scissor: &ScissorBox);
    fn draw_triangle_zbuffer(&mut self, x0: i32, y0: i32, z0: u16, x1: i32, y1: i32, z1: u16, x2: i32, y2: i32, z2: u16, color: u32, scissor: &ScissorBox);
    fn draw_triangle_shaded(&mut self, x0: i32, y0: i32, c0: u32, x1: i32, y1: i32, c1: u32, x2: i32, y2: i32, c2: u32, scissor: &ScissorBox);
    fn draw_triangle_shaded_zbuffer(&mut self, x0: i32, y0: i32, z0: u16, c0: u32, x1: i32, y1: i32, z1: u16, c1: u32, x2: i32, y2: i32, z2: u16, c2: u32, scissor: &ScissorBox);
    fn clear_zbuffer(&mut self);
    fn set_zbuffer_enabled(&mut self, enabled: bool);
    fn reset(&mut self);
}
```

### SoftwareRdpRenderer (`rdp_renderer_software.rs`)

- **Framebuffer**: ARGB8888 format (Vec<u32>)
- **Z-buffer**: 16-bit depth buffer using `emu_core::graphics::ZBuffer`
- **Rasterization**: Scanline-based triangle rasterization
- **Color interpolation**: Uses `emu_core::graphics::ColorOps::lerp()`
- **Tests**: 6 comprehensive unit tests

### RDP State Management (`rdp.rs`)

The RDP struct retains:
- Display list processing logic
- Register management (DPC_START, DPC_END, DPC_STATUS, etc.)
- TMEM and tile descriptor management
- Scissor box state
- Fill color state

The RDP delegates to renderer:
- All drawing operations (triangles, rectangles, pixels)
- Framebuffer access
- Z-buffer operations

## Design Rationale

### Why separate at the RDP level?

1. **Matches existing pattern**: The frontend already has `VideoProcessor` trait for post-processing (CRT filters). This creates consistency.

2. **Clean separation of concerns**:
   - **RDP**: N64-specific state (registers, TMEM, display lists)
   - **Renderer**: Generic rendering operations (triangles, Z-buffer)

3. **Minimal external impact**: The N64System and other code don't need changes - the RDP interface remains the same.

4. **Future extensibility**: Easy to add OpenGL renderer without changing RDP logic.

### Why NOT separate at higher/lower levels?

**Not at N64System level**: Would require duplicating N64-specific logic (bus, memory map) for each renderer.

**Not at lower level (per-operation)**: Would create too fine-grained abstraction with excessive overhead.

**Not at VideoProcessor level**: Post-processing filters are different from core rendering. The RDP needs to output a frame that's already rendered.

## Benefits

1. **Performance**: Future OpenGL renderer can use GPU for triangle rasterization and depth testing
2. **Maintainability**: Rendering code is isolated and testable
3. **Flexibility**: Easy to switch renderers or add new ones
4. **Testability**: Each renderer can have its own unit tests

## Testing

- **70 N64 tests pass** with software renderer (69 original + 1 OpenGL stub test)
- **6 renderer-specific tests** in SoftwareRdpRenderer
- **1 OpenGL stub test** verifies error handling without GL context
- **All pre-commit checks pass**: fmt, clippy, build, test
- **OpenGL feature flag works**: `cargo build --features opengl` compiles successfully

## Current Implementation Status

### SoftwareRdpRenderer (Complete)
- ✅ Fully functional CPU-based rasterization
- ✅ All triangle rendering modes (flat, shaded, Z-buffered)
- ✅ 6 comprehensive unit tests
- ✅ Production-ready for all use cases

### OpenGLRdpRenderer (Complete)
- ✅ Feature flag support (`--features opengl`)
- ✅ Full trait implementation with OpenGL 3.3 Core
- ✅ FBO (Framebuffer Object) for offscreen rendering
- ✅ Shader programs for flat and Gouraud shading
- ✅ Hardware depth testing (Z-buffer)
- ✅ All triangle rendering modes (flat, shaded, Z-buffered, combined)
- ✅ Fill operations (clear, rectangles)
- ✅ Scissor test support
- ✅ Pixel readback for Frame compatibility
- ✅ Compiles successfully with and without feature flag
- ✅ All 70 N64 tests pass
- ⏸️ **Integration pending**: Requires GL context from frontend

## Future Work

### OpenGLRdpRenderer Integration

**Current Status**: Fully implemented but not yet integrated into system creation.

**Blocking Issue**: GL context availability
- OpenGL context comes from SDL2 frontend
- N64System doesn't have direct access to GL context
- Need to pass context at system creation or via builder pattern

**Integration Options**:

**Option 1: Builder Pattern**
```rust
// In N64System
pub struct N64SystemBuilder {
    gl_context: Option<glow::Context>,
}

impl N64SystemBuilder {
    pub fn with_gl_context(mut self, gl: glow::Context) -> Self {
        self.gl_context = Some(gl);
        self
    }
    
    pub fn build(self) -> N64System {
        let bus = if let Some(gl) = self.gl_context {
            N64Bus::with_gl_renderer(gl)
        } else {
            N64Bus::new()  // Software renderer
        };
        // ...
    }
}
```

**Option 2: Settings-Based** (Recommended)
```rust
// In settings.rs
pub struct Settings {
    pub n64_renderer: String, // "software" | "opengl"
    // ...
}

// In main.rs or frontend
let renderer: Box<dyn RdpRenderer> = if settings.n64_renderer == "opengl" && gl_available {
    Box::new(OpenGLRdpRenderer::new(gl_context, width, height)?)
} else {
    Box::new(SoftwareRdpRenderer::new(width, height))
};
```

**Option 3: Dynamic Renderer Factory**
```rust
// In RDP
impl Rdp {
    pub fn new_with_renderer(renderer: Box<dyn RdpRenderer>) -> Self {
        // Use provided renderer instead of default
    }
}

// In frontend
let renderer = create_renderer(&settings, gl_context);
let rdp = Rdp::new_with_renderer(renderer);
```

When implementing the integration:

1. **Add renderer selection to settings**:
   ```rust
   // In settings.rs
   pub n64_renderer: String, // Default: "software"
   ```

2. **Modify N64System creation** to accept renderer preference:
   ```rust
   // Option: Add to N64Bus or N64System::new()
   pub fn new_with_renderer(renderer_type: &str, gl_context: Option<glow::Context>) -> Self
   ```

3. **Update frontend** to pass GL context when creating N64System:
   ```rust
   // In GUI main loop
   if system_is_n64 && settings.n64_renderer == "opengl" {
       let gl = get_gl_context();  // From SDL2 window
       system = N64System::new_with_gl(gl);
   }
   ```

4. **Add UI for renderer selection**:
   - Settings menu option
   - Function key toggle (e.g., F11 for renderer switching)
   - Display current renderer in status bar

### Settings Integration (Future)

```rust
// In settings.rs
pub struct Settings {
    pub n64_renderer: String, // "software" | "opengl"
    // ...
}

// In N64System::new() or RDP::new()
let renderer: Box<dyn RdpRenderer> = match settings.n64_renderer.as_str() {
    "opengl" if gl_context_available => {
        OpenGLRdpRenderer::new_with_context(gl_context, width, height)
            .unwrap_or_else(|_| SoftwareRdpRenderer::new(width, height))
    }
    _ => Box::new(SoftwareRdpRenderer::new(width, height)),
};
```

### Alternative Approach: Hybrid Rendering

Instead of full OpenGL renderer, consider:
- Keep software rasterization for accuracy
- Use OpenGL for post-processing (upscaling, filtering)
- Leverage existing `VideoProcessor` trait in frontend
- This avoids GL context dependency in N64 core

## Comparison with VideoProcessor

Both follow the same pattern but serve different purposes:

| Aspect | RdpRenderer | VideoProcessor |
|--------|-------------|----------------|
| **Purpose** | Core 3D rendering | Post-processing effects |
| **Location** | N64 crate | GUI frontend |
| **Operations** | Triangle rasterization, Z-buffer | CRT filters, scaling |
| **Input** | Display list commands | Finished frame |
| **Output** | Rendered frame | Filtered frame |
| **Implementations** | Software (complete), OpenGL (stub) | Software, OpenGL |

## Conclusion

The pluggable RDP renderer architecture provides a clean separation between N64-specific state management and generic rendering operations. This allows for future GPU acceleration while maintaining the existing software renderer for compatibility. The implementation follows established patterns in the codebase and requires no changes to external code.
