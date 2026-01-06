# Hemulator Icon Assets

This directory contains the application icon in various formats.

## Files

- **icon.ico** - Multi-resolution Windows icon file (69KB)
  - Contains 5 standard Windows icon sizes: 16x16, 32x32, 48x48, 64x64, 256x256
  - Automatically embedded into Windows executables via `crates/frontend/gui/build.rs`
  - Used for both x86_64 and i686 Windows release builds

- **icon_32.png** - 32x32 PNG source icon
- **icon_256.png** - 256x256 PNG source icon

## Updating the Icon

To regenerate `icon.ico` from the PNG sources:

```bash
# Install required tools (Ubuntu/Debian)
sudo apt-get install icoutils imagemagick

# Generate intermediate sizes
convert icon_256.png -resize 16x16 /tmp/icon_16.png
convert icon_256.png -resize 48x48 /tmp/icon_48.png
convert icon_256.png -resize 64x64 /tmp/icon_64.png

# Create multi-resolution .ico file
icotool -c -o icon.ico \
  /tmp/icon_16.png \
  icon_32.png \
  /tmp/icon_48.png \
  /tmp/icon_64.png \
  icon_256.png

# Verify the result
icotool -l icon.ico
```

The build system will automatically embed this icon into the Windows executable during compilation.
