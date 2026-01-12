# Hemulator Icon Assets

This directory contains the application icon in various formats.

## Files

- **icon.ico** - Multi-resolution Windows icon file (~33KB)
  - Contains 5 standard Windows icon sizes: 16x16, 32x32, 48x48, 64x64, 256x256
  - All sizes use 32-bit color depth for proper color accuracy
  - Automatically embedded into Windows executables via `crates/frontend/gui/build.rs`
  - Used for both x86_64 and i686 Windows release builds

- **icon_32.png** - 32x32 PNG source icon (RGBA format)
- **icon_256.png** - 256x256 PNG source icon (RGBA format)

## Updating the Icon

To regenerate `icon.ico` from the PNG sources with proper 32-bit color depth:

```bash
# Install required tools (Ubuntu/Debian)
sudo apt-get install imagemagick

# Generate intermediate sizes from the 256px source
convert icon_256.png -resize 16x16 /tmp/icon_16.png
convert icon_256.png -resize 48x48 /tmp/icon_48.png
convert icon_256.png -resize 64x64 /tmp/icon_64.png

# Create multi-resolution .ico file using ImageMagick
# Note: Use 'convert' instead of 'icotool' to maintain 32-bit color depth
convert /tmp/icon_16.png icon_32.png /tmp/icon_48.png /tmp/icon_64.png icon_256.png icon.ico

# Verify the result (all sizes should show bit-depth=32)
icotool -l icon.ico
```

**Important**: Use ImageMagick's `convert` command rather than `icotool -c`, as icotool may reduce color depth to 4-bit for some icon sizes, causing color accuracy issues.

The build system will automatically embed this icon into the Windows executable during compilation.
