# Atari 2600 PAL Support Analysis

## Current Implementation Status

The current Atari 2600 emulator implementation **only supports NTSC** with no PAL support.

### NTSC Hardcoded Values

| Component | Current (NTSC Only) | PAL Requirement |
|-----------|---------------------|-----------------|
| **Scanlines per frame** | 262 | 312 |
| **Frame rate** | 60 Hz | 50 Hz |
| **Color palette** | 128 colors (NTSC) | 104 colors (PAL) |
| **CPU clock** | ~1.19 MHz | ~1.19 MHz (same) |
| **Color clock** | 3.579545 MHz | 3.546894 MHz |
| **Visible lines** | ~192 | ~228 |

### Where NTSC is Hardcoded

1. **`crates/systems/atari2600/src/tia.rs`**:
   - Line 12: `Total Scanlines: 262 (NTSC)`
   - Line 395: `scanline_states: vec![ScanlineState::default(); 262]`
   - Line 889: `if self.scanline >= 262 {`
   - Line 1473: `fn ntsc_to_rgb(ntsc: u8) -> u32` - NTSC palette function
   - Multiple references to 262 scanlines throughout

2. **`crates/systems/atari2600/src/tia_renderer.rs`**:
   - Line 29: `const TOTAL_SCANLINES: u16 = 262;`
   - Line 31: Comment mentions "0-261, total 262 scanlines"
   - Line 120, 131: Uses 262 for wraparound calculations

3. **`crates/systems/atari2600/src/lib.rs`**:
   - Line 85: `NTSC: ~1.19 MHz CPU, 262 scanlines/frame`
   - Line 267: `Atari 2600 frames are 262 scanlines (NTSC).`
   - Line 377: `(visible_start + 191) % 262` - assumes NTSC visible lines

4. **Documentation**:
   - All docs reference NTSC only
   - No mention of PAL anywhere

## Impact of Missing PAL Support

### 1. Game Compatibility
- **PAL-only games**: Will not run correctly
- **PAL versions**: Different timing, slower gameplay (50 Hz vs 60 Hz)
- **Color differences**: PAL games will have wrong colors

### 2. User Experience
- European/Australian users need PAL support
- Many games have PAL variants with different content
- PAL games run ~17% slower than NTSC (50 Hz vs 60 Hz)

### 3. Accuracy
- PAL ROMs detected incorrectly as NTSC
- Frame timing incorrect for PAL games
- Color palette incorrect for PAL games

## Implementation Plan

### Phase 1: Add Video Mode Enum (Week 1)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoMode {
    NTSC,
    PAL,
    // SECAM could be added later if needed
}

impl VideoMode {
    pub fn scanlines_per_frame(&self) -> u16 {
        match self {
            VideoMode::NTSC => 262,
            VideoMode::PAL => 312,
        }
    }
    
    pub fn frame_rate(&self) -> f64 {
        match self {
            VideoMode::NTSC => 60.0,
            VideoMode::PAL => 50.0,
        }
    }
    
    pub fn color_clock_hz(&self) -> f64 {
        match self {
            VideoMode::NTSC => 3_579_545.0,
            VideoMode::PAL => 3_546_894.0,
        }
    }
    
    pub fn visible_scanlines(&self) -> u16 {
        match self {
            VideoMode::NTSC => 192,
            VideoMode::PAL => 228,
        }
    }
}
```

### Phase 2: Update TIA for Dynamic Scanline Count (Week 1-2)

```rust
pub struct Tia {
    // ... existing fields ...
    
    video_mode: VideoMode,
    
    // Replace fixed 262 with dynamic
    // scanline_states: Vec<ScanlineState>,  // Already allocated, just use differently
}

impl Tia {
    pub fn new(video_mode: VideoMode) -> Self {
        let total_scanlines = video_mode.scanlines_per_frame() as usize;
        Self {
            // ... existing init ...
            video_mode,
            scanline_states: vec![ScanlineState::default(); total_scanlines],
        }
    }
    
    pub fn clock(&mut self) {
        self.pixel += 3;
        
        if self.pixel >= 228 {
            self.pixel -= 228;
            let old_scanline = self.scanline;
            self.latch_scanline_state(old_scanline);
            self.scanline += 1;
            
            // Dynamic wraparound based on video mode
            let total_scanlines = self.video_mode.scanlines_per_frame();
            if self.scanline >= total_scanlines {
                self.scanline = 0;
            }
        }
    }
}
```

### Phase 3: Add PAL Color Palette (Week 2)

```rust
fn palette_to_rgb(value: u8, video_mode: VideoMode) -> u32 {
    match video_mode {
        VideoMode::NTSC => ntsc_to_rgb(value),
        VideoMode::PAL => pal_to_rgb(value),
    }
}

fn pal_to_rgb(pal: u8) -> u32 {
    // PAL palette table (104 colors)
    // Similar structure to NTSC palette but different color encoding
    PAL_PALETTE[(pal & 0x7F) as usize]
}

const PAL_PALETTE: [u32; 128] = [
    // PAL color values (to be researched and filled)
    // Note: PAL has only 104 valid colors, rest are duplicates/black
    // ...
];
```

### Phase 4: ROM Detection and Auto-Selection (Week 2-3)

```rust
pub fn detect_video_mode(rom: &[u8]) -> VideoMode {
    // Heuristics for detecting PAL vs NTSC:
    // 1. Check ROM header/metadata if available
    // 2. Analyze scanline count patterns in code
    // 3. Check for PAL-specific timing values
    // 4. Default to NTSC if unclear
    
    // For now, simple heuristic based on ROM patterns
    // Many PAL ROMs have specific patterns in headers
    
    // Default to NTSC
    VideoMode::NTSC
}
```

### Phase 5: Update Frame Timing (Week 3)

```rust
impl Atari2600 {
    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Use video mode to determine frame completion
        let total_scanlines = self.video_mode.scanlines_per_frame();
        
        // Detect frame completion: scanline wrapped from high to low
        let threshold = total_scanlines - 12; // Last ~12 scanlines
        if current_scanline < last_scanline 
            && last_scanline > threshold 
            && current_scanline < 10 
        {
            // Frame complete
            break;
        }
        
        // ...
    }
}
```

### Phase 6: Testing (Week 4)

1. **NTSC regression testing**: Ensure all existing tests still pass
2. **PAL-specific tests**: Add tests for 312 scanlines, 50 Hz timing
3. **Real ROM testing**: Test with known PAL and NTSC ROMs
4. **Color palette verification**: Visual comparison with reference emulators

## Alternative: Minimal PAL Support

If full PAL support is too much work, consider minimal support:

### Option A: PAL Detection with NTSC Emulation
- Detect PAL ROMs
- Run them in NTSC mode with warning
- Better than crashing or incorrect behavior

### Option B: Basic PAL Timing Only
- Support 312 scanlines
- Use NTSC palette (acceptable compromise)
- Focus on getting games to run, not perfect color accuracy

### Option C: Configuration-Based
- Add configuration option for video mode
- User manually selects NTSC or PAL
- Simpler than auto-detection

## Recommended Approach

**Start with Option C (Configuration-Based)**:
1. Add `VideoMode` enum and configuration
2. Update TIA to use dynamic scanline count
3. Update frame detection logic
4. Add basic PAL palette (can be refined later)
5. Document how to set video mode
6. Later: Add ROM detection for auto-selection

**Estimated Effort**: 2-3 weeks for basic PAL support

## Benefits of PAL Support

1. ✅ **Compatibility**: European/Australian games work correctly
2. ✅ **Accuracy**: Proper timing for PAL ROMs
3. ✅ **Completeness**: Full region support like modern emulators
4. ✅ **Future-proof**: Foundation for SECAM if needed

## Current Status: NTSC Only

**Summary**: The emulator currently has **no PAL support**. All timing, scanline counts, and color palettes are hardcoded for NTSC. Adding PAL support would require:
- Dynamic scanline count (262 → 312)
- PAL color palette implementation
- Frame rate adjustment (60 Hz → 50 Hz)
- ROM detection or configuration option
- Updated documentation

This is a **significant enhancement** that would improve compatibility but requires careful implementation and testing to avoid breaking existing NTSC functionality.
