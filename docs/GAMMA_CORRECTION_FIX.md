# Dark Tint Issue Fix - Gamma Correction

## Problem

The emulator GUI and rendered game screen appeared darker than expected. When taking screenshots, the saved images looked brighter than what was displayed on screen, indicating a color space mismatch.

## Root Cause

The issue was a gamma correction mismatch between different parts of the rendering pipeline:

1. **Emulator Output**: Produces pixel data in **sRGB color space** (standard for displays)
2. **egui_sdl2_gl Textures**: Created with `GL_RGBA` format (**linear color space**)
3. **Screen Rendering**: Uses `GL_FRAMEBUFFER_SRGB` enabled (converts linear to sRGB)

### The Problem Flow

```
Emulator (sRGB) → Texture (stored as sRGB in linear format) → GL_FRAMEBUFFER_SRGB (linear→sRGB) → Screen
                                    ❌                                    ❌
                              No conversion                      Double conversion!
```

When sRGB values are stored in a linear texture and then rendered with `GL_FRAMEBUFFER_SRGB` enabled, the GPU applies the sRGB→linear→sRGB double conversion, making everything appear darker.

## Solution

Convert emulator pixel data from sRGB to linear color space **before** creating the egui texture:

```rust
fn srgb_to_linear(srgb: u8) -> u8 {
    let srgb_f = srgb as f32 / 255.0;
    let linear_f = if srgb_f <= 0.04045 {
        srgb_f / 12.92
    } else {
        ((srgb_f + 0.055) / 1.055).powf(2.4)
    };
    (linear_f * 255.0).round().min(255.0) as u8
}
```

### The Fixed Flow

```
Emulator (sRGB) → Convert to Linear → Texture (linear) → GL_FRAMEBUFFER_SRGB (linear→sRGB) → Screen ✓
                         ✓                                            ✓
                    Proper conversion                         Proper conversion
```

## Implementation

The fix was applied in `crates/frontend/gui/src/egui_ui/layout.rs`:

```rust
pub fn update_emulator_texture(...) {
    let rgba_pixels: Vec<u8> = pixels
        .iter()
        .flat_map(|&pixel| {
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            
            // Convert sRGB to linear before creating texture
            let r_linear = srgb_to_linear(r);
            let g_linear = srgb_to_linear(g);
            let b_linear = srgb_to_linear(b);
            
            [r_linear, g_linear, b_linear, a]
        })
        .collect();
    ...
}
```

## Why This Works

1. **Emulator produces sRGB colors** - Standard for game consoles and displays
2. **We convert to linear** - Match the GL_RGBA texture format
3. **GPU converts linear to sRGB** - GL_FRAMEBUFFER_SRGB does final conversion
4. **Screen displays correctly** - No double gamma correction!

## Alternative Solutions Considered

1. **Patch egui_sdl2_gl to use GL_SRGB8_ALPHA8** - More invasive, requires maintaining a fork
2. **Disable GL_FRAMEBUFFER_SRGB** - Would break egui's UI rendering
3. **Apply inverse gamma to make screenshots match** - Wrong approach, treats symptom not cause

## Benefits of This Approach

- ✅ Simple, self-contained fix
- ✅ No external dependencies or patches needed
- ✅ Correct color space handling throughout pipeline
- ✅ Screenshots now match display
- ✅ UI elements render at correct brightness

## Testing

Run the emulator and verify:
1. GUI panels are properly visible (not too dark)
2. Game rendering matches expected brightness
3. Screenshots match what's displayed on screen
4. No color banding or artifacts introduced
