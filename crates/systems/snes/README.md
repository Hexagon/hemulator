# SNES Emulation - Super Nintendo Entertainment System

This crate implements Super Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## References

This implementation follows specifications from the **SNESdev Wiki** and **Super Famicom Wiki**:
- **Main Wiki**: https://snes.nesdev.org/wiki/SNESdev_Wiki
- **65C816 CPU**: https://snes.nesdev.org/wiki/65c816_reference
- **PPU Registers**: https://snes.nesdev.org/wiki/PPU_registers
- **CPU Registers**: https://snes.nesdev.org/wiki/CPU_registers
- **Memory Map**: https://snes.nesdev.org/wiki/Memory_map
- **DMA & HDMA**: https://wiki.superfamicom.org/dma-and-hdma (primary reference)
- **Timing**: https://snes.nesdev.org/wiki/Timing

## Current Status

The SNES emulator supports comprehensive gameplay with complete CPU, full DMA/HDMA, LoROM, HiROM, and ExHiROM cartridge support, SPC700 APU processor, and complete PPU rendering for all modes 0-7. All background modes now support their advanced features including Mode 7 matrix transformation (rotation/scaling), offset-per-tile rendering (Modes 2, 4, 6), and true hi-res 512px rendering (Modes 5-6). Audio processor (SPC700) is fully implemented and DSP (sound generation) now supports BRR sample playback, enabling actual audio output from games!

### What Works

#### CPU & Memory
- ✅ **CPU (65C816)** - Complete 16-bit CPU from `emu_core::cpu_65c816`
  - 256/256 opcodes implemented (100% complete)
  - 8/16-bit mode switching (M and X flags)
  - 24-bit address space (16MB addressable)
  - Native and emulation modes
  - Reference: [65c816 Reference](https://snes.nesdev.org/wiki/65c816_reference)

- ✅ **Memory Bus** - Full SNES memory map implementation
  - 128KB WRAM ($7E0000-$7FFFFF)
  - Shadow RAM at $0000-$1FFF in banks $00-$3F and $80-$BF
  - Hardware registers ($2100-$21FF, $4000-$43FF)
  - Reference: [Memory Map](https://snes.nesdev.org/wiki/Memory_map)

- ✅ **Cartridge Loading** - LoROM, HiROM, and ExHiROM with auto-detection
  - LoROM: 32KB banks at $8000-$FFFF per bank (up to 4MB)
  - HiROM: Full 64KB banks with linear addressing (up to 4MB)
  - ExHiROM: Extended HiROM for larger ROMs (up to 8MB, e.g., Tales of Phantasia)
  - Header detection via map mode bytes ($20/$30=LoROM, $21/$31=HiROM, $25/$35=ExHiROM)
  - SMC header detection and removal
  - SRAM support for all mapping modes
  - Reference: [ROM File Formats](https://snes.nesdev.org/wiki/ROM_file_formats)

#### PPU (Picture Processing Unit)
- ✅ **Background Modes** - Complete rendering for all modes 0-7
  - **Mode 0**: ✅ Complete - 4 BG layers, 2bpp each (4 colors per tile)
  - **Mode 1**: ✅ Complete - 2 BG layers 4bpp + 1 BG layer 2bpp (most common commercial mode)
  - **Mode 2**: ✅ Complete - 2 BG layers, 4bpp each with offset-per-tile support
  - **Mode 3**: ✅ Complete - BG1 8bpp (256 colors), BG2 4bpp (16 colors)
  - **Mode 4**: ✅ Complete - BG1 8bpp, BG2 2bpp with offset-per-tile support
  - **Mode 5**: ✅ Complete - BG1 4bpp, BG2 2bpp with true 512px hi-res rendering
  - **Mode 6**: ✅ Complete - BG1 4bpp with true 512px hi-res and offset-per-tile support
  - **Mode 7**: ✅ Complete - 8bpp with full matrix transformation (rotation/scaling)
  - Reference: [Backgrounds](https://wiki.superfamicom.org/backgrounds), [PPU Overview](https://snes.nesdev.org/wiki/PPU_registers)

- ✅ **Mode 7 Matrix Transformation** - Full rotation and scaling support
  - All Mode 7 registers implemented ($211A-$2120)
  - M7SEL - Screen over modes and flip settings
  - M7A-M7D - 2x2 transformation matrix (8.8 fixed point)
  - M7X/M7Y - Center point coordinates
  - Supports wrap, transparent, and tile 0 modes
  - Horizontal and vertical flip support
  - Reference: [Mode 7](https://snes.nesdev.org/wiki/Mode_7)

- ✅ **Offset-per-tile** - Per-tile scrolling for Modes 2, 4, 6
  - Reads offset data from BG3 tilemap
  - Supports both horizontal and vertical offsets
  - Applied to BG1 and BG2 in Mode 2
  - Applied to BG1 in Modes 4 and 6
  - Reference: [Offset-per-tile](https://snes.nesdev.org/wiki/PPU_registers#BG_Scroll)

- ✅ **Hi-res (512px)** - True high-resolution rendering for Modes 5-6
  - Renders at native 512x224 resolution
  - Proper pixel doubling in horizontal direction
  - Fully compatible with all BG layers and sprites
  - Reference: [Hi-res mode](https://snes.nesdev.org/wiki/PPU_registers)

- ✅ **Sprites (OAM)** - Complete sprite system
  - 128 sprites with 4bpp (16 colors per sprite)
  - Multiple size modes (8x8, 16x16, 32x32, 64x64)
  - Priority levels (0-3)
  - **Sprite priority rotation** (bit 7 of $2103) - allows dynamic sprite ordering
  - Hardware-accurate 32 sprites/scanline limit
  - 34 tile slots/scanline limit
  - Reference: [Sprites](https://wiki.superfamicom.org/sprites), [SNESdev OAM](https://snes.nesdev.org/wiki/PPU_OAM)

- ✅ **Scrolling** - Full background scrolling support
  - Horizontal and vertical scrolling on all BG layers
  - Per-layer scroll registers ($210D-$2114)
  - Reference: [Scrolling](https://snes.nesdev.org/wiki/PPU_registers#Background_Scrolling)

- ✅ **VRAM/CGRAM/OAM Access**
  - 64KB VRAM for tiles and tilemaps
  - 512-byte CGRAM for 256 colors (15-bit BGR)
  - 544-byte OAM (512 bytes main + 32 bytes high table)
  - VRAM access protection during active display
  - Reference: [VRAM](https://snes.nesdev.org/wiki/PPU_registers#VRAM)

- ✅ **Window Masking** - Complete window system implementation
  - Two independent windows with configurable left/right boundaries
  - Per-layer window enable and inversion controls
  - Window logic operations: OR, AND, XOR, XNOR
  - Applied to all BG layers and sprites
  - Reference: [Windows](https://snes.nesdev.org/wiki/PPU_registers#Windows)

#### DMA & HDMA
- ✅ **General-Purpose DMA** - Full 8-channel support
  - Channels configured via $4300-$437F
  - Enable register $420B (MDMAEN)
  - All transfer modes (0-7) with proper B-bus patterns
  - Address modes: increment, decrement, fixed
  - Direction: A-bus ↔ B-bus (both directions)
  - Cycle-accurate timing (8 cycles per byte + overhead)
  - Reference: [DMA](https://wiki.superfamicom.org/dma-and-hdma#dma)

- ✅ **HDMA (H-blank DMA)** - Per-scanline updates
  - 8-channel HDMA support (shared channels with DMA)
  - Enable register $420C (HDMAEN)
  - Direct and indirect addressing modes
  - Per-scanline register updates
  - Line counter and repeat mode
  - Automatic table processing
  - Reference: [HDMA](https://wiki.superfamicom.org/dma-and-hdma#hdma)

#### CPU I/O Registers
- ✅ **Interrupt Control**
  - $4200 (NMITIMEN) - NMI/IRQ enable and auto-joypad
  - $4210 (RDNMI) - NMI flag with read-and-clear ⭐
  - $4211 (TIMEUP) - IRQ flag (stub)
  - $4212 (HVBJOY) - H/V-Blank and joypad status
  - Reference: [CPU Registers](https://snes.nesdev.org/wiki/CPU_registers)

- ✅ **Controller Input**
  - $4016-$4017 - Serial joypad ports (JOYSER0/1)
  - $4218-$421F - Auto-joypad read registers (JOY1L-JOY4H)
  - Full SNES controller support (12 buttons)
  - Auto-joypad read during VBlank
  - Reference: [Controllers](https://snes.nesdev.org/wiki/Input_devices)

#### APU (Audio Processing Unit)
- ✅ **SPC700 CPU** - Complete audio processor implementation
  - Full SPC700 instruction set from `emu_core::apu::Spc700`
  - 64KB audio RAM (ARAM)
  - IPL boot ROM with upload protocol
  - $2140-$2143 (APUIO0-3) - CPU ↔ SPC700 communication ports
  - Bidirectional port communication working
  - Games can upload audio drivers and communicate with APU
  - Reference: [APU](https://snes.nesdev.org/wiki/APU), [SPC700](https://snes.nesdev.org/wiki/SPC700)

- ✅ **DSP (Digital Signal Processor)** - Core functionality implemented
  - ✅ 8-voice synthesis engine with BRR sample playback
  - ✅ BRR (ADPCM) decoder with all 4 filter types
  - ✅ Sample directory and loop point support
  - ✅ Pitch control (14-bit precision)
  - ✅ ADSR envelope generator (simplified curves)
  - ✅ Voice control (key on/off, volume, pitch registers)
  - ✅ Voice mixing to stereo output
  - ✅ Master volume control
  - ✅ ENDX register (voice ended flags)
  - ⚠️ Linear interpolation (Gaussian filter not yet implemented)
  - ⚠️ Simplified envelope rates (not cycle-accurate)
  - ❌ Echo/reverb FIR filter
  - ❌ Noise generator
  - ❌ Pitch modulation
  - **Status**: Actual audio output working! Games can play samples from RAM
  - Reference: [DSP](https://snes.nesdev.org/wiki/DSP)

#### Timing
- ✅ **Frame Timing** - NTSC timing implementation
  - **Hardware Specifications** (from https://wiki.superfamicom.org/timing):
    - Master clock: 21.477272 MHz (1.89e9/88 Hz)
    - Scanline: 1364 master cycles (340 dots × 4 master cycles/dot, with dots 323 and 327 being 6 cycles)
    - Frame: 262 scanlines = 357,368 master cycles
    - Frame rate: ~60.0988 Hz
    - Special case: Scanline $F0 (240) is 1360 cycles every other frame (non-interlace)
  - **CPU Timing**:
    - IO operations: 6 master cycles
    - Memory access: 6, 8, or 12 master cycles (region and $420D dependent)
    - Effective CPU speed: ~3.58 MHz (for IO operations)
    - WRAM Refresh: 40 master cycle pause at ~536 master cycles into each scanline
  - **Implementation**:
    - Uses approximate CPU cycle counts for timing (~89,342 cycles/frame, ~341 cycles/scanline)
    - CPU cycles are abstract units returned by the 65C816 core
    - Timing is tuned for game compatibility rather than hardware accuracy
  - Reference: [Timing](https://wiki.superfamicom.org/timing), [SNESdev Timing](https://snes.nesdev.org/wiki/Timing)

- ✅ **VBlank/NMI**
  - VBlank starts at scanline 225 ($E1) or 240 ($F0) depending on $2133 bit 2
  - NMI triggers if enabled ($4200 bit 7)
  - Proper NMI flag handling ($4210 read-and-clear)
  - Reference: [NMI](https://snes.nesdev.org/wiki/NMI), [Timing](https://wiki.superfamicom.org/timing)

#### Other Features
- ✅ **Save States** - Full system state serialization
- ✅ **Logging** - Comprehensive debug logging for CPU, PPU, DMA, interrupts
- ✅ **Enhancement Chips** - DSP-1 math coprocessor partially implemented, SuperFX coprocessor implemented
  - Automatic detection from ROM header
  - Support for most DSP-1 games (Pilotwings, Super Mario Kart)
  - Implemented commands: Multiply, Inverse, Gyrate, Distance, Radius, Range, Project, Polar
  - SuperFX/SuperFX2 (GSU-1/GSU-2) graphics coprocessor implemented
    - 16-bit RISC processor with 16 general-purpose registers
    - Complete ALU instruction set (ADD, SUB, MULT, AND, OR, XOR, etc.)
    - Pixel plotting and graphics operations (PLOT, COLOR for GSU-1; RPIX for GSU-2)
    - Control flow operations (branches, loops, jumps)
    - Memory access (load/store word, register moves)
    - WITH register tracking for source/destination selection
    - GETC ROM reading with ROMBR and R14 pointer
    - Used in Star Fox, Yoshi's Island, and Doom
  - **Known Limitations**:
    - Attitude command (0x08) is incomplete - outputs only partial rotation matrix
    - Target command (0x20) not implemented - returns zeros
    - Rotate command (0x24) not implemented - returns zeros
    - Games heavily using these commands may not work correctly

### What's Missing

#### PPU Advanced Features
- ✅ **Windows** - Complete window masking implementation ($2123-$212B)
  - **Window Registers**:
    - $2123 (W12SEL): Window settings for BG1/BG2
    - $2124 (W34SEL): Window settings for BG3/BG4
    - $2125 (WOBJSEL): Window settings for sprites and color window
    - $2126-$2129 (WH0-WH3): Window boundaries (left/right positions)
    - $212A (WBGLOG): Window logic for BG layers (OR/AND/XOR/XNOR)
    - $212B (WOBJLOG): Window logic for sprites and color window
  - **Features**:
    - ✅ Two independent windows with configurable boundaries
    - ✅ Per-layer window enable and inversion
    - ✅ Window combination logic (OR/AND/XOR/XNOR)
    - ✅ Window masking for BG layers and sprites
    - ✅ Color window for clipping and color math control
  - **Window Boundaries**:
    - Inclusive on both ends: [left, right]
    - Empty window when left > right (no wraparound)
  - **Implementation Details**:
    - BG1 uses bits 0-3 of W12SEL, BG2 uses bits 4-7
    - BG3 uses bits 0-3 of W34SEL, BG4 uses bits 4-7
    - Sprites use bits 0-3 of WOBJSEL
    - Color window uses bits 4-7 of WOBJSEL
  - Reference: [Windows](https://wiki.superfamicom.org/windows)

- ✅ **Color Math** - Fully implemented with per-pixel layer tracking ($2130-$2132)
  - **Implementation Status**: Complete with sub-screen and fixed color blending
    - $2130 (CGWSEL): Color math control with color clipping and window-based math control
    - $2131 (CGADSUB): Per-layer enable (BG1-4, OBJ, backdrop) with add/subtract/half modes
    - $2132 (COLDATA): Fixed color RGB blending source
    - $212D (TS): Sub-screen layer designation for blending
  
  - **Features Implemented**:
    - ✅ Per-pixel layer tracking (BG1-4, OBJ, backdrop)
    - ✅ Selective color math based on layer source
    - ✅ Add and subtract color blending modes
    - ✅ Half-color math mode (divide result by 2)
    - ✅ Color component clamping (0-255 range)
    - ✅ Fixed color blending source
    - ✅ Sub-screen rendering and blending
    - ✅ Window-based color math clipping (CGWSEL bits 4-5)
    - ✅ **Direct color mode** (CGWSEL bit 0) - for Modes 3, 4, 7
      - Allows 2048 colors (BBGGGRRR + bgr palette bits)
      - Used by some games for enhanced color effects
      - Reference: [Backgrounds - Direct Color](https://wiki.superfamicom.org/backgrounds#direct-color-mode)
    - ✅ Color clipping to black (CGWSEL bits 6-7) - Applied BEFORE color math
      - 00 = Never clip colors
      - 01 = Clip colors outside color window
      - 10 = Clip colors inside color window
      - 11 = Always clip colors to black
    - ✅ Window-based color math control (CGWSEL bits 4-5) - Controls WHERE color math is applied
      - 00 = Enable color math everywhere
      - 01 = Enable inside color window
      - 10 = Enable outside color window
      - 11 = Disable color math everywhere
    
  - **Technical Details**:
    - Layer buffer tracks source layer for each pixel (BG1=0, BG2=1, BG3=2, BG4=3, OBJ=4, backdrop=5)
    - Both main screen and sub-screen rendered independently
    - Sub-screen layers determined by TS register ($212D)
    - **Color clipping applied first** (clips pixels to black based on color window)
    - **Color math applied second** (blends main/sub screens based on layer enables and math window)
    - Only pixels from layers enabled in CGADSUB undergo blending
    - CGWSEL prevent-math bit (bit 6) can globally disable color math
    - Blending performed in 8-bit RGB color space with proper clamping
    - Direct color: combines tile data (BBGGGRRR) with palette bits (bgr) → Red=RRRr0, Green=GGGg0, Blue=BBb00
    
  - **Impact on Game Compatibility**:
    - Games using color math now work correctly
    - Fade effects, transparency, and color tinting render properly
    - Sub-screen blending effects (transparencies, shadows) work correctly
    - Window-based effects (spotlight, fade regions, color clipping) work correctly
    - Direct color mode games now have correct color output
    
  - Reference: [Color Math](https://wiki.superfamicom.org/rendering-the-screen#color-math), [Transparency](https://wiki.superfamicom.org/transparency)

- ❌ **Mosaic** - No mosaic effect ($2106)

- ✅ **Sub-screen** - Fully implemented for color math ($212D)
  - TS register controls which layers appear on sub-screen
  - Sub-screen pixels blended with main screen via color math
  - Used for transparency, shadows, and other blending effects
  - Reference: [Transparency](https://wiki.superfamicom.org/transparency)

#### Audio
- ✅ **DSP (Digital Signal Processor)** - Core functionality implemented
  - SPC700 CPU is fully implemented and functional
  - ✅ DSP register interface and voice control
  - ✅ 8-voice synthesis with BRR sample playback
  - ✅ BRR (ADPCM) decoder with all filter types
  - ✅ Sample directory and loop point support
  - ✅ Pitch control and sample advancement
  - ✅ Simplified ADSR envelope generation
  - ✅ Audio output working - games can play sound!
  - ⚠️ Linear interpolation (Gaussian filter pending)
  - ⚠️ Simplified envelope rates (not cycle-accurate)
  - ❌ Echo/reverb FIR filter
  - **Status**: Actual audio output working! Advanced features pending
  - Reference: [DSP](https://snes.nesdev.org/wiki/DSP)

#### Enhancement Chips

The emulator now includes a framework for enhancement chip (coprocessor) support:

- ⚠️ **DSP-1** - Math coprocessor (partially implemented)
  - Used in ~20 games including Pilotwings, Super Mario Kart, Ace o Nerae!
  - Provides hardware acceleration for 3D math operations
  - Implemented operations: multiply, inverse, gyrate (2D rotation), distance, radius, range, project (3D projection), polar to cartesian
  - **Incomplete operations**:
    - Attitude (sin/cos) - only partial implementation, missing full 3x3 rotation matrix
    - Target - coordinate transformation not implemented
    - Rotate - 3D rotation not implemented
  - Both LoROM and HiROM memory mappings supported
  - Save state support included
  - Reference: [DSP-1](https://snes.nesdev.org/wiki/DSP-1), [SNESLab DSP1](https://sneslab.net/wiki/DSP1)

- ✅ **SuperFX/SuperFX2 (GSU-1/GSU-2)** - Graphics coprocessor (core functionality implemented)
  - Used in ~10 popular games including Star Fox, Yoshi's Island, Doom
  - Custom 16-bit RISC processor with 16 general-purpose registers
  - Pixel plotting operations: PLOT, COLOR (GSU-1), RPIX (GSU-2)
  - Complete ALU: ADD, SUB, MULT, AND, OR, XOR, NOT, INC, DEC
  - Control flow: branches, loops, jumps with correct PC handling
  - Memory operations: load/store word, register moves, WITH register selection
  - ROM reading: GETC with ROMBR bank and R14 pointer
  - Register mapping at $3000-$32FF in banks $00-$3F, $80-$BF
  - GSU RAM at $700000-$71FFFF (128 KB) for frame buffer
  - Save state support included
  - **Known Limitations**:
    - Instruction cache allocated but not used (no performance impact)
    - Simplified timing (not cycle-accurate)
    - Basic pixel operations (no advanced screen modes)
  - Reference: [SuperFX](https://snes.nesdev.org/wiki/Super_FX), [SnesLab SuperFX](https://sneslab.net/wiki/Super_FX)

- ❌ **Not Yet Implemented**
  - SA-1 - CPU coprocessor with additional 65C816 (Super Mario RPG, Kirby's Dream Land 3)
  - DSP-2 - Math coprocessor variant (Dungeon Master)
  - DSP-3 - Math coprocessor variant (SD Gundam GX)
  - DSP-4 - Math coprocessor variant (Top Gear 3000)
  - S-DD1 - Decompression chip (Star Ocean, Street Fighter Alpha 2)
  - CX4 - Coprocessor (Mega Man X2, Mega Man X3)
  - SPC7110 - Data decompression (Far East of Eden Zero)
  - ST010/ST011/ST018 - Various coprocessors (F1 ROC II, Hayazashi Nidan Morita Shougi)
  - OBC-1 - Coprocessor (Metal Combat: Falcon's Revenge)

### Enhancement Chip Roadmap

**Current Status**
- ⚠️ **DSP-1** - Partially implemented (missing Attitude/Target/Rotate)
- ✅ **SuperFX/SuperFX2** - Core functionality implemented (PC bugs fixed, WITH/GETC/RPIX added)

**Priority 1 - Complete Existing Chips**
1. **DSP-1 Completion** - Finish Attitude, Target, and Rotate commands
   - Requires proper 3x3 rotation matrix implementation for Attitude
   - Reference bsnes implementation for accuracy
2. **SuperFX Enhancements** - Add advanced features and optimize
   - Implement instruction cache for performance (currently allocated but unused)
   - Add cycle-accurate timing based on CLSR register
   - Implement advanced screen modes and proper SCBR/SCMR handling
   - Test extensively with commercial games (Star Fox, Yoshi's Island, Doom)

**Priority 2 - Most Common Chips (High Impact)**
3. **SA-1** - CPU coprocessor, ~30 games including Super Mario RPG
   - Add comprehensive tests for all commands

**Priority 2 - Most Common Chips (High Impact)**
2. **SuperFX** - Graphics coprocessor, ~10 popular games (Star Fox, Yoshi's Island)
3. **SA-1** - CPU coprocessor, ~30 games including Super Mario RPG

**Priority 3 - Moderately Common**
4. **S-DD1** - Decompression chip, ~5 games including Star Ocean
5. **CX4** - Used in Mega Man X2/X3

**Priority 4 - Less Common**
6. **DSP-2/3/4** - Math coprocessor variants (few games each)
7. **SPC7110** - Decompression chip (few games)
8. **OBC-1** - Rare coprocessor (1-2 games)
9. **ST010/ST011/ST018** - Very rare (1-2 games each)

**Implementation Notes:**
- Enhancement chips are detected automatically from ROM header (byte $FFD6)
- Chips are instantiated during cartridge load if detected and implemented
- Memory mapping is handled transparently by the cartridge module
- Interior mutability (RefCell) allows chip state updates during memory reads
- Save state support implemented via EnhancementChip trait
- Unimplemented chips are detected but log warnings and don't instantiate

**Known Issues:**
- DSP-1 Attitude command incomplete (missing full 3x3 rotation matrix)
- DSP-1 Target and Rotate commands not implemented
- Games heavily relying on these commands may malfunction

**References:**
- [Enhancement Chips Overview](https://snes.nesdev.org/wiki/Enhancement_chips)
- [List of SNES Enhancement Chips](https://en.wikipedia.org/wiki/List_of_Super_NES_enhancement_chips)
- [SNES Coprocessors Blog Post](https://jsgroth.dev/blog/posts/snes-coprocessors-part-1/)


#### Other Missing Features
- ❌ **IRQ** - H/V timer interrupts not implemented
- ❌ **PAL** - NTSC timing only
- ❌ **Interlace** - No interlace mode support
- ❌ **Hardware Multiply/Divide** - Registers stubbed

## Register Implementation Status

### PPU Registers ($2100-$213F)
Core PPU registers implemented:
- ✅ $2100 (INIDISP) - Screen display, force blank, brightness
- ✅ $2105 (BGMODE) - BG mode and character size
- ✅ $2107-$210A - BG tilemap address and size
- ✅ $210B-$210C - BG character data address
- ✅ $210D-$2114 - Background scrolling
- ✅ $2115-$2119 - VRAM access
- ✅ $211A-$2120 - Mode 7 registers (M7SEL, M7A-M7D, M7X, M7Y)
- ✅ $2121-$2122 - CGRAM access
- ✅ $2101-$2104 - OAM access
- ✅ $212C (TM) - Main screen layer enable
- ✅ $213F (STAT78) - PPU status and NMI flag
- ✅ $2123-$212B - Windows (complete implementation)
- ⚠️ $2130-$2132 - Color math (stubbed)

Reference: [PPU Registers](https://snes.nesdev.org/wiki/PPU_registers)

### CPU I/O Registers ($4000-$43FF)
- ✅ $4200 (NMITIMEN) - Interrupt enable
- ✅ $4210 (RDNMI) - NMI flag ⭐ Critical for proper NMI handling
- ✅ $4211 (TIMEUP) - IRQ flag (stub)
- ✅ $4212 (HVBJOY) - H/V-Blank and joypad status
- ✅ $4016-$4017 - Controller serial ports
- ✅ $4218-$421F - Auto-joypad read
- ✅ $420B (MDMAEN) - DMA enable
- ✅ $420C (HDMAEN) - HDMA enable
- ✅ $4300-$437F - DMA/HDMA channel registers
- ⚠️ $4202-$4206 - Multiply/Divide (stubbed)

Reference: [CPU Registers](https://snes.nesdev.org/wiki/CPU_registers)

## Architecture

### Component Structure

```
SnesSystem
  └── SnesCpu (wraps Cpu65c816<SnesBus>)
      └── SnesBus (implements Memory65c816)
          ├── 128KB WRAM
          ├── SPC700 APU (Full implementation)
          │   ├── SPC700 CPU core
          │   ├── 64KB Audio RAM (ARAM)
          │   ├── IPL boot ROM
          │   ├── Communication ports ($2140-$2143)
          │   └── DSP registers (stub - no audio output)
          ├── DMA Controller (8 channels)
          │   ├── General-purpose DMA
          │   ├── HDMA (H-blank DMA)
          │   └── Transfer modes 0-7
          ├── SNES PPU (All Modes 0-7)
          │   ├── 64KB VRAM
          │   ├── 256-color CGRAM (palette)
          │   ├── 4 BG layers (modes 0-1)
          │   ├── 2 BG layers (modes 2-5)
          │   ├── 1 BG layer (modes 6-7)
          │   └── 2bpp/4bpp/8bpp tile support
          └── Cartridge (LoROM/HiROM/ExHiROM auto-detect)
              ├── ROM banks (LoROM: 32KB chunks, HiROM/ExHiROM: 64KB linear)
              ├── 32KB SRAM
              └── Enhancement Chip (optional)
                  ├── DSP-1 (math coprocessor) - RefCell for interior mutability
                  └── SuperFX/SuperFX2 (graphics coprocessor) - RefCell for interior mutability
```

### Key Files
- `src/lib.rs` - System initialization and frame execution
- `src/cpu.rs` - CPU wrapper using core 65C816
- `src/bus.rs` - Memory bus with all hardware registers
- `src/ppu.rs` - Complete PPU implementation (modes 0-7)
- `src/ppu_renderer.rs` - Rendering backend
- `src/cartridge.rs` - ROM loading, mapping, and enhancement chip integration
- `src/coprocessors/mod.rs` - Enhancement chip framework and chip type detection
- `src/coprocessors/dsp1.rs` - DSP-1 math coprocessor implementation
- `src/coprocessors/superfx.rs` - SuperFX graphics coprocessor implementation

## Testing

### Test ROMs
Located in `test_roms/snes/`:
- `test.sfc` - Basic Mode 0 checkerboard pattern
- `test_priority.sfc` - Priority bit handling test
- `test_enhanced.sfc` - Enhanced rendering features
- `test_sprite_overflow.sfc` - Sprite limits test

### Unit Tests
```bash
cargo test --package emu_snes
```
- 70+ unit tests covering bus, PPU, DMA, HDMA, controllers, Mode 7, offset-per-tile, hi-res
- All tests passing

### Commercial Game Testing
Games known to work:
- ✅ **Super Mario World** - Full support with Mode 1, sprites, scrolling (no audio)
- ✅ **F-Zero** - Now works with Mode 7 rotation/scaling
- ⚠️ **Donkey Kong Country** - Graphics work (no audio)
- 🔧 **Tales of Phantasia** - ExHiROM support implemented, should work (not tested)
- ✅ **Pilotwings** - Should work with DSP-1 support (not tested)
- ✅ **Super Mario Kart** - Should work with DSP-1 support (not tested)
- 🔧 **Star Fox** - SuperFX chip implemented, should work (needs testing)
- 🔧 **Yoshi's Island** - SuperFX2 chip implemented, should work (needs testing)
- 🔧 **Doom** - SuperFX chip implemented, should work (needs testing)
- ❌ **Super Mario RPG** - Requires SA-1 chip (not yet implemented)
- ❌ **Star Ocean** - Requires S-DD1 chip (not yet implemented)
- ❌ **Mega Man X2/X3** - Requires CX4 chip (not yet implemented)

## Development

### Building
```bash
cargo build --profile release-quick
```

### Debugging
Enable logging for different subsystems:
```bash
# CPU execution trace
cargo run -- game.sfc --log-cpu trace

# Interrupt debugging
cargo run -- game.sfc --log-interrupts info

# PPU register access
cargo run -- game.sfc --log-ppu info

# DMA operations
cargo run -- game.sfc --log-bus debug
```

## Known Issues

1. **Audio Quality** - DSP implemented but with simplifications
   - ✅ BRR (ADPCM) sample decoding working
   - ✅ Sample playback and mixing functional
   - ✅ Games can play sound
   - ⚠️ Using linear interpolation instead of Gaussian filter
   - ⚠️ Simplified envelope rates (not cycle-accurate)
   - ❌ Echo/reverb effects not implemented
   - Current status: Audio works but quality may differ from hardware

2. **Timing** - Frame-based, not cycle-accurate
   - Good enough for most games
   - Some timing-sensitive effects may not work

3. **Enhancement Chips** - Most chips not yet implemented
   - DSP-1 is partially implemented with known limitations:
     - Attitude command (0x08) incomplete - missing full rotation matrix
     - Target command (0x20) not implemented
     - Rotate command (0x24) not implemented
   - SuperFX, SA-1, S-DD1, CX4, and other chips not yet implemented
   - Games requiring unimplemented chips will not work properly
   - See Enhancement Chip Roadmap section for planned implementations

## Additional Documentation

- `SNES_REGISTER_FIXES.md` - Details on NMI register implementation
- `SNES_WAI_INVESTIGATION.md` - WAI instruction debugging notes

## References & Further Reading

### Primary References
- **SNESdev Wiki**: https://snes.nesdev.org/wiki/SNESdev_Wiki
- **Super Famicom Wiki**: https://wiki.superfamicom.org
- **Anomie's Register Doc**: https://snes.nesdev.org/wiki/Anomie%27s_Doc
- **fullsnes**: https://problemkaputt.de/fullsnes.htm

### Specific Topics
- **65C816 CPU**: https://snes.nesdev.org/wiki/65c816_reference
- **PPU Registers**: https://snes.nesdev.org/wiki/PPU_registers
- **CPU Registers**: https://snes.nesdev.org/wiki/CPU_registers
- **DMA/HDMA**: https://wiki.superfamicom.org/dma-and-hdma (primary reference)
- **Memory Map**: https://snes.nesdev.org/wiki/Memory_map
- **Timing**: https://snes.nesdev.org/wiki/Timing
- **Controllers**: https://snes.nesdev.org/wiki/Input_devices
- **APU/SPC700**: https://snes.nesdev.org/wiki/SPC700
- **Enhancement Chips**: https://snes.nesdev.org/wiki/Enhancement_chips
- **Transparency Effects**: https://wiki.superfamicom.org/transparency
