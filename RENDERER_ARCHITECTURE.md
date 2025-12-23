# Renderer Architecture

## Overview

This document describes the unified renderer architecture pattern used across all emulated systems in hemulator. The pattern provides consistency, future-proofing, and enables both software (CPU) and hardware (GPU) rendering implementations.

## Core Pattern

All systems with graphics capabilities follow the same architectural approach:

```
┌─────────────────────────────────────────────────────────────┐
│                  System (State Management)                  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  • Registers (PPUCTRL, PPUMASK, etc.)                 │  │
│  │  • Memory (VRAM, palette RAM, CHR, etc.)              │  │
│  │  • Timing (cycle counting, frame synchronization)     │  │
│  │  • Game Logic (mappers, interrupts, controllers)      │  │
│  │                                                        │  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │   renderer: Box<dyn Renderer>                    │ │  │
│  │  │                                                  │ │  │
│  │  │   ┌────────────────────────────────────────────┐ │ │  │
│  │  │   │  Renderer Trait (emu_core::renderer)      │ │ │  │
│  │  │   │  • get_frame() -> &Frame                  │ │ │  │
│  │  │   │  • clear(color: u32)                      │ │ │  │
│  │  │   │  • reset()                                │ │ │  │
│  │  │   │  • resize(width, height)                  │ │ │  │
│  │  │   │  • name() -> &str                         │ │ │  │
│  │  │   │  • is_hardware_accelerated() -> bool      │ │ │  │
│  │  │   └────────────────────────────────────────────┘ │ │  │
│  │  │                                                  │ │  │
│  │  │   Implementations:                               │ │  │
│  │  │   ┌────────────────────────────────────────────┐ │ │  │
│  │  │   │  Software Renderer (CPU-based)            │ │ │  │
│  │  │   │  • Always available                       │ │ │  │
│  │  │   │  • Maximum compatibility                  │ │ │  │
│  │  │   │  • Accurate emulation                     │ │ │  │
│  │  │   └────────────────────────────────────────────┘ │ │  │
│  │  │   ┌────────────────────────────────────────────┐ │ │  │
│  │  │   │  Hardware Renderer (GPU-accelerated)      │ │ │  │
│  │  │   │  • OpenGL / Vulkan / Metal / DirectX      │ │ │  │
│  │  │   │  • Better performance                     │ │ │  │
│  │  │   │  • Optional feature                       │ │ │  │
│  │  │   └────────────────────────────────────────────┘ │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## System-Specific Implementations

### 1. N64 System - `RdpRenderer` Trait

**Status**: ✅ Complete (following pattern)

**Location**: `crates/systems/n64/src/rdp_renderer.rs`

**Pattern Extensions**:
- **Core Methods**: Follows `Renderer` pattern (get_frame, clear, reset, resize, name, is_hardware_accelerated)
- **3D Extensions**: Triangle rasterization (flat, shaded, textured, with/without Z-buffer)
- **RDP-Specific**: Z-buffer operations, scissor clipping, texture sampling

**Implementations**:
- ✅ `SoftwareRdpRenderer`: CPU-based 3D rasterization (always available)
- ✅ `OpenGLRdpRenderer`: GPU-accelerated (feature-gated, not yet integrated)

**Architecture**:
```
RDP (state) -> RdpRenderer trait -> {Software, OpenGL} renderers
                    ↓
         (follows Renderer pattern)
```

### 2. PC System - `VideoAdapter` Trait

**Status**: ✅ Complete (following pattern)

**Location**: `crates/systems/pc/src/video_adapter.rs`

**Pattern Extensions**:
- **Core Methods**: Follows `Renderer` pattern (get_frame, reset, name, is_hardware_accelerated, resize)
- **PC Extensions**: VRAM rendering, text/graphics modes, multiple resolutions
- **Adapter-Specific**: `render(vram, pixels)`, `fb_width()`, `fb_height()`, `init()`

**Implementations**:
- ✅ `SoftwareCgaAdapter`: CGA text mode (80x25, 8x16 font)
- ✅ `CgaGraphicsAdapter`: CGA graphics modes (320x200 4-color, 640x200 2-color)
- ✅ `SoftwareEgaAdapter`: EGA all modes (text + graphics)
- ✅ `SoftwareVgaAdapter`: VGA all modes (text + Mode 13h + 640x480x16)
- 🔲 `HardwareCgaAdapter`: OpenGL stub
- 🔲 `HardwareEgaAdapter`: OpenGL stub
- 🔲 `HardwareVgaAdapter`: OpenGL stub

**Architecture**:
```
PcSystem (state) -> VideoAdapter trait -> {Software, Hardware} adapters
                         ↓
              (follows Renderer pattern)
```

### 3. Frontend - `VideoProcessor` Trait

**Status**: ✅ Complete (similar pattern, post-processing focus)

**Location**: `crates/frontend/gui/src/video_processor/mod.rs`

**Pattern Similarity**:
- **Core Methods**: Similar to `Renderer` (init, resize, name, is_hardware_accelerated)
- **Processing-Specific**: `process_frame(buffer, filter)` instead of rendering

**Implementations**:
- ✅ `SoftwareProcessor`: CPU-based CRT filters
- ✅ `OpenGLProcessor`: GPU-accelerated shaders

**Architecture**:
```
System Renderer -> Frame -> VideoProcessor -> Post-Processed Frame -> Display
                                 ↓
                      (similar to Renderer pattern)
```

### 4. PPU-Based Systems (Future)

**Status**: 🔲 To Be Migrated

**Systems**: NES, Game Boy, SNES, Atari 2600

**Current State**:
- ❌ No trait abstraction
- ❌ Direct PPU/TIA implementations
- ❌ Rendering tightly coupled to system
- ❌ No pluggable renderer architecture

**Migration Path**:

**Phase 1: Documentation Alignment** (Current)
- ✅ Reference `Renderer` pattern in documentation
- ✅ No code changes required

**Phase 2: Optional Refactoring** (Future)
- Create renderer wrappers for existing implementations
- Maintain backward compatibility
- Example structure:
  ```rust
  // In crates/systems/nes/src/ppu_renderer.rs
  pub struct NesPpuRenderer {
      framebuffer: Frame,
      // Rendering state (not registers!)
  }
  
  impl Renderer for NesPpuRenderer {
      fn get_frame(&self) -> &Frame { ... }
      fn clear(&mut self, color: u32) { ... }
      fn reset(&mut self) { ... }
      fn resize(&mut self, width: u32, height: u32) { ... }
      fn name(&self) -> &str { "NES Software Renderer" }
  }
  
  impl NesPpuRenderer {
      // System-specific methods
      pub fn render_scanline(&mut self, ...) { ... }
      pub fn render_frame(&mut self, ppu_state) { ... }
  }
  ```

**Phase 3: Full Migration** (Long-term)
- Adopt pluggable renderers
- Benefits: Hardware acceleration, consistent architecture, easier testing

## Core Renderer Trait

**Location**: `crates/core/src/renderer.rs`

```rust
pub trait Renderer: Send {
    /// Get the current framebuffer (read-only)
    fn get_frame(&self) -> &Frame;

    /// Clear the framebuffer with a solid color (ARGB8888)
    fn clear(&mut self, color: u32);

    /// Reset the renderer to its initial state
    fn reset(&mut self);

    /// Resize the renderer to new dimensions
    fn resize(&mut self, width: u32, height: u32);

    /// Get the name of this renderer (for debugging/UI)
    fn name(&self) -> &str;

    /// Check if this renderer uses hardware acceleration
    fn is_hardware_accelerated(&self) -> bool {
        false // Default: software renderer
    }
}
```

## Benefits of the Unified Pattern

### 1. Consistency
- All systems use the same core interface
- Easy to understand and navigate codebase
- Predictable method names and signatures

### 2. Future-Proofing
- Easy to add new rendering backends (Vulkan, Metal, DirectX, WebGPU)
- No changes to system code when adding new renderer
- Settings-based renderer selection

### 3. Testability
- Renderers can be tested independently
- Mock renderers for system tests
- Comprehensive unit tests for each implementation

### 4. Performance
- Optional GPU acceleration without modifying core emulation
- Renderer selection based on user preference or hardware capabilities
- Graceful fallback to software renderer

### 5. Separation of Concerns
- **System/State Management**: Registers, memory, timing, game logic
- **Renderer**: Drawing operations, framebuffer management
- Clear boundaries, easier to maintain

## Implementation Guidelines

### For New Systems

1. **Choose your approach**:
   - Simple 2D: Use `Renderer` trait directly or reference the pattern
   - Complex 3D/modes: Create system-specific trait following pattern

2. **Always provide software renderer first**:
   - Maximum compatibility
   - Reference implementation
   - Testing baseline

3. **Hardware renderer is optional**:
   - Add when performance matters
   - Feature-gated compilation
   - Settings-based selection

4. **Follow naming conventions**:
   - Core methods: `get_frame()`, `clear()`, `reset()`, `resize()`, `name()`
   - System extensions clearly documented

### For Existing Systems

1. **Document alignment** (no code changes)
2. **Optional refactoring** when adding features
3. **Full migration** only when beneficial

## Comparison with Existing Patterns

| Aspect | Renderer (Core) | RdpRenderer (N64) | VideoAdapter (PC) | VideoProcessor (Frontend) |
|--------|----------------|-------------------|-------------------|---------------------------|
| **Purpose** | Base pattern | 3D rendering | Text/graphics modes | Post-processing |
| **Location** | `emu_core` | `emu_n64` | `emu_pc` | `emu_gui` |
| **Status** | ✅ Complete | ✅ Complete | ✅ Complete | ✅ Complete |
| **Software** | Reference | ✅ Available | ✅ Available | ✅ Available |
| **Hardware** | Reference | ✅ OpenGL (stub) | 🔲 OpenGL (stub) | ✅ OpenGL |
| **Extensions** | None | 3D operations | PC modes | CRT filters |

## Future Work

1. **N64**: Integrate OpenGL renderer with frontend GL context
2. **PC**: Implement hardware adapters for better performance
3. **PPU Systems**: Create optional renderer wrappers
4. **Documentation**: Add examples for each system type
5. **Testing**: Renderer comparison tests (software vs hardware output)
6. **Settings**: Unified renderer selection UI

## Related Documentation

- [N64 Renderer Architecture](N64_RENDERER_ARCHITECTURE.md): Detailed N64-specific docs
- [AGENTS.md](AGENTS.md): Implementation guidelines for all systems
- [CONTRIBUTING.md](CONTRIBUTING.md): Development workflow and standards

## Conclusion

The unified renderer architecture provides a consistent, future-proof pattern for all graphics rendering in hemulator. By following this pattern, all systems benefit from:

- **Consistency**: Same interface across all systems
- **Flexibility**: Easy to add new rendering backends
- **Performance**: Optional GPU acceleration
- **Maintainability**: Clear separation of concerns
- **Testability**: Independent renderer testing

Systems can adopt this pattern incrementally, with no requirement to refactor existing working code unless adding new features or optimizations.
