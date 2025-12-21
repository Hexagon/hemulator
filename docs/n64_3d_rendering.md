# N64 3D Rendering Implementation

This document describes the 3D rendering capabilities implemented for the Nintendo 64 emulator.

## Overview

The N64's Reality Display Processor (RDP) has been enhanced with proper 3D triangle rasterization support, including:
- Flat-shaded triangles (solid color)
- Gouraud-shaded triangles (smooth color interpolation)
- Z-buffer depth testing for hidden surface removal
- Scissor clipping for efficient rendering

## Architecture

### RDP Graphics Pipeline

The N64 graphics pipeline consists of two main components:

1. **RSP (Reality Signal Processor)**: Geometry processing, vertex transforms, lighting (not yet implemented)
2. **RDP (Reality Display Processor)**: Rasterization, texturing, blending (partially implemented)

### Current Implementation

Our RDP implementation focuses on the rasterization stage:

```
Vertices → Triangle Setup → Edge Walking → Per-Pixel Operations → Framebuffer
                                              ├─ Color Interpolation
                                              ├─ Z-Buffer Test
                                              └─ Scissor Clip
```

## Triangle Rasterization

### Scanline Algorithm

The implementation uses a scanline-based edge walking algorithm:

1. **Vertex Sorting**: Sort vertices by Y-coordinate (top to bottom)
2. **Edge Walking**: Split triangle into top and bottom halves
3. **Scanline Interpolation**: For each scanline:
   - Interpolate X coordinates along triangle edges
   - Interpolate attributes (color, depth) along edges
   - Rasterize pixels between edge pairs
4. **Per-Pixel Operations**: For each pixel:
   - Interpolate color and depth across scanline
   - Apply scissor clipping
   - Perform Z-buffer test
   - Write to framebuffer

### Triangle Functions

#### `draw_triangle(x0, y0, x1, y1, x2, y2, color)`
- Renders a flat-shaded triangle (single solid color)
- No depth testing
- Fast rendering for 2D graphics

#### `draw_triangle_zbuffer(x0, y0, z0, x1, y1, z1, x2, y2, z2, color)`
- Renders a flat-shaded triangle with depth testing
- Interpolates Z values per-pixel
- Updates Z-buffer for occluded surfaces
- Depth range: 0 (near) to 0xFFFF (far)

#### `draw_triangle_shaded(x0, y0, c0, x1, y1, c1, x2, y2, c2)`
- Renders a Gouraud-shaded triangle
- Interpolates color per-vertex → per-edge → per-pixel
- Smooth color gradients
- No depth testing

#### `draw_triangle_shaded_zbuffer(x0, y0, z0, c0, x1, y1, z1, c1, x2, y2, z2, c2)`
- Full 3D rendering with both shading and depth
- Interpolates both color and depth per-pixel
- Proper hidden surface removal
- Suitable for complex 3D scenes

## Z-Buffer Implementation

### Data Structure

- **Storage**: `Vec<u16>` - one depth value per pixel
- **Format**: 16-bit unsigned integer
  - 0x0000 = nearest (close to camera)
  - 0xFFFF = farthest (far plane)
- **Size**: `width × height` entries

### Depth Testing

```rust
fn zbuffer_test(&mut self, x: u32, y: u32, depth: u16) -> bool {
    if !self.zbuffer_enabled {
        return true; // Always pass if disabled
    }
    
    let idx = (y * self.width + x) as usize;
    
    // Closer (smaller) values pass
    if depth < self.zbuffer[idx] {
        self.zbuffer[idx] = depth; // Update Z-buffer
        true
    } else {
        false // Occluded
    }
}
```

### Operations

- `clear_zbuffer()`: Reset all depths to 0xFFFF (far plane)
- `set_zbuffer_enabled(enabled)`: Enable/disable depth testing

## Color Interpolation

### Gouraud Shading

Colors are linearly interpolated in ARGB color space:

```rust
fn lerp_color(c0: u32, c1: u32, t: f32) -> u32 {
    let a0 = ((c0 >> 24) & 0xFF) as f32;
    let r0 = ((c0 >> 16) & 0xFF) as f32;
    let g0 = ((c0 >> 8) & 0xFF) as f32;
    let b0 = (c0 & 0xFF) as f32;
    
    // ... same for c1 ...
    
    let a = (a0 + (a1 - a0) * t).round() as u32;
    let r = (r0 + (r1 - r0) * t).round() as u32;
    let g = (g0 + (g1 - g0) * t).round() as u32;
    let b = (b0 + (b1 - b0) * t).round() as u32;
    
    (a << 24) | (r << 16) | (g << 8) | b
}
```

### Color Format

- **Internal Format**: ARGB (0xAARRGGBB)
  - Alpha: bits 24-31
  - Red: bits 16-23
  - Green: bits 8-15
  - Blue: bits 0-7

## Scissor Clipping

Scissor rectangle defines the renderable region:

```rust
struct ScissorBox {
    x_min: u32,
    y_min: u32,
    x_max: u32,
    y_max: u32,
}
```

Pixels outside the scissor box are skipped during rasterization.

## Performance Characteristics

### Time Complexity

- **Triangle Setup**: O(1) - constant time for sorting and edge computation
- **Scanline Iteration**: O(h) - height of triangle in pixels
- **Per-Scanline**: O(w) - width of scanline
- **Overall**: O(h × w) - proportional to triangle area

### Optimization Opportunities

1. **SIMD**: Color interpolation can be vectorized
2. **Edge Functions**: Barycentric coordinates for parallel evaluation
3. **Tile-based Rendering**: Process 8×8 or 16×16 tiles
4. **Early-Z Rejection**: Skip pixels that fail depth test early

## Example Usage

### Simple 3D Scene

```rust
// Enable Z-buffer
rdp.set_zbuffer_enabled(true);
rdp.clear_zbuffer();

// Draw back face (far)
rdp.draw_triangle_zbuffer(
    160, 80, 0xC000,  // Top
    220, 180, 0xC000, // Right
    100, 180, 0xC000, // Left
    0xFFFF0000,       // Red
);

// Draw front face (near) with shading
rdp.draw_triangle_shaded_zbuffer(
    160, 80, 0x6000, 0xFF0000FF,  // Top: Blue
    220, 180, 0x6000, 0xFF00FFFF, // Right: Cyan
    240, 140, 0x6000, 0xFFFF00FF, // Side: Magenta
);
```

## Limitations

### Current

- **No texture mapping**: Only flat and Gouraud shading
- **No perspective correction**: Linear interpolation only
- **No anti-aliasing**: Aliased edges
- **No blending**: No alpha blending or fog
- **Display list integration**: Triangle commands not wired to command processor

### Future Work

1. **Texture Mapping**: Sample textures from TMEM
2. **Perspective Correction**: Interpolate 1/z, u/z, v/z
3. **Triangle Commands**: Wire opcodes 0x08-0x0F to display list
4. **Anti-aliasing**: Edge coverage calculation
5. **Blending**: Alpha blending and fog effects

## Testing

### Test Coverage

- **Z-buffer Tests** (11 tests):
  - Initialization, clear, enable/disable
  - Depth testing (pass/fail)
  - Depth updates
  - Occlusion (near/far triangles)

- **Triangle Rendering Tests** (4 tests):
  - Flat shading
  - Gouraud shading
  - Z-buffered rendering
  - Combined shading + Z-buffer

- **Integration Tests**:
  - Color interpolation accuracy
  - Scissor clipping
  - 3D scene demo (pyramid with multiple faces)

### Demo Test

The `test_n64_3d_rendering_demo` creates a simple 3D pyramid:
- 3 triangles at different depths
- Multiple colors (Red, Green, Blue)
- Demonstrates proper occlusion
- Validates color interpolation

## References

- [N64 RDP Command Summary](http://hcs64.com/files/RDP_COMMANDS.pdf)
- [N64 Graphics Documentation](https://n64squid.com/homebrew/n64-sdk/graphics/)
- [Real-Time Rendering Techniques](https://www.realtimerendering.com/)

## Conclusion

This implementation provides a solid foundation for 3D rendering on the N64 emulator. While not cycle-accurate, it correctly implements the core rasterization algorithms needed for basic 3D graphics, including depth testing and smooth shading. Future enhancements will add texture mapping and other advanced features to approach hardware-accurate rendering.
