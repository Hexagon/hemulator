# CRT Filters Visual Guide

This document describes the visual effects of each CRT filter implemented in Hemulator.

## Filter Types

### 1. None (Default)
- **Description**: Raw pixel output with no processing
- **Use Case**: When you want sharp, unfiltered pixels
- **Performance**: No overhead

### 2. Scanlines
- **Description**: Simulates the horizontal raster scan lines visible on CRT displays
- **Implementation**: 
  - Darkens every other horizontal line (odd rows)
  - Reduces brightness to 60% on affected rows
  - Even rows remain at full brightness
- **Visual Effect**: Creates horizontal dark lines across the screen
- **Use Case**: For a classic CRT TV look
- **Example**:
  ```
  Row 0: ████████████████ (Full brightness)
  Row 1: ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ (60% brightness - scanline)
  Row 2: ████████████████ (Full brightness)
  Row 3: ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ (60% brightness - scanline)
  ```

### 3. Phosphor
- **Description**: Simulates the phosphor glow and color bleeding of CRT screens
- **Implementation**:
  - Blends each pixel with its horizontal neighbors
  - 15% blend ratio with left neighbor (if exists)
  - 15% blend ratio with right neighbor (if exists)
  - Creates soft horizontal glow
- **Visual Effect**: Softens edges and creates a subtle glow between pixels
- **Use Case**: For a softer, more authentic CRT appearance without harsh scanlines
- **Example**: Sharp edges become blurred horizontally, colors bleed slightly into adjacent pixels

### 4. CRT Monitor (Full Effect)
- **Description**: Combines multiple CRT characteristics for the most authentic look
- **Implementation**:
  1. First applies phosphor effect (horizontal color bleeding)
  2. Then applies scanlines with 70% darkness (less aggressive than scanlines-only)
  3. Boosts brightness on non-scanline rows by 5% for contrast
- **Visual Effect**: 
  - Horizontal color bleeding from phosphor
  - Visible but not harsh scanlines
  - Enhanced contrast between scanlines and active rows
- **Use Case**: For the most authentic CRT monitor simulation
- **Performance**: Most intensive filter (processes buffer twice)

## Technical Details

### Color Processing
- All filters work in RGB color space (0xRRGGBB format)
- Filters use floating-point math for blending, then convert back to u8
- Uses `saturating_add` for brightness adjustments to prevent overflow

### Performance Characteristics
- **None**: Zero overhead (no processing)
- **Scanlines**: O(n) single pass, simple arithmetic
- **Phosphor**: O(n) single pass, with neighbor lookups and blending
- **CRT Monitor**: O(2n) two passes (phosphor + enhanced scanlines)

Where n = width × height (typically 256 × 240 = 61,440 pixels for NES)

### Filter Application
- Filters are applied after frame rendering but before display
- Filters do NOT affect overlays (help, debug, slot selector)
- Filters modify the buffer in-place for efficiency
- Selected filter persists across sessions via config.json

## Usage

Press **F11** to cycle through filters in this order:
1. None
2. Scanlines
3. Phosphor
4. CRT Monitor
5. (back to None)

The current filter name is printed to console when changed.

## Configuration

The filter selection is stored in `config.json`:

```json
{
  "crt_filter": "None"
}
```

Valid values: `"None"`, `"Scanlines"`, `"Phosphor"`, `"CrtMonitor"`

## Implementation Notes

- Filters are pure software (no GPU shaders required)
- Works with minifb's software renderer
- Cross-platform (Windows, Linux, macOS)
- No external dependencies beyond standard library
- Comprehensive unit tests for each filter effect
