---
title: "Supported Systems"
parent: "User Manual"
nav_order: 5
---

# Supported Systems

This emulator supports 10 different retro gaming systems. **NES emulation is fully working** with ~90% game coverage. Other systems are in various stages of development.

| System | Status | What Works | What's Missing | Recommended For |
|--------|--------|------------|----------------|-----------------|
| **NES** | ✅ Fully Working | Everything | - | Playing NES games |
| **Atari 2600** | 🚧 In Development | TIA, RIOT, cartridge formats | Rendering issues, stability | Testing/development |
| **CHIP-8** | ✅ Fully Working | Complete CHIP-8/Super-CHIP/XO-CHIP | - | Playing CHIP-8 programs |
| **Game Boy** | ✅ Fully Functional (GB) / 🚧 GBC WIP | Core features, MBC0/1/2/3/5 | Some edge cases, GBC features incomplete | Playing GB games |
| **SMS** | 🚧 In Development | Z80 CPU, VDP, PSG, ROM banking | Not producing image, test ROM | Development only |
| **ColecoVision** | 🚧 In Development | Z80 CPU, TMS9918A VDP, SN76489 PSG | Not producing image, audio output, BIOS required | Development only |
| **SG-1000** | ⚠️ Experimental | Z80 CPU, TMS9918A VDP, SN76489 PSG | Audio output, test ROM | Development/testing |
| **SNES** | ✅ Functional | CPU, all PPU modes 0-7, sprites, DMA/HDMA | Audio (DSP), some enhancement chips | Playing most games (silent) |
| **N64** | 🚧 In Development | 3D rendering, CPU | Full graphics, audio, games | Development/testing |
| **PC/DOS** | 🧪 Experimental | Multi-slot mounts, disk controller, custom BIOS, CGA/EGA/VGA | Full disk I/O, boot | Development/testing |

### NES (Nintendo Entertainment System)

**Status**: ✅ Fully Working  
**Coverage**: ~90% of all NES games (14 mappers supported)

The emulator supports the following NES mappers:
- **Mapper 0 (NROM)** - Simple games (~10% of games)
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, The Legend of Zelda (~28% of games)
- **Mapper 2 (UxROM)** - Mega Man, Castlevania, Contra (~11% of games)
- **Mapper 3 (CNROM)** - Gradius, Paperboy (~6.4% of games)
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3, Mega Man 3-6 (~24% of games)
- **Mapper 7 (AxROM)** - Battletoads, Marble Madness (~3.1% of games)
- **Mapper 9 (MMC2/PxROM)** - Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Fire Emblem (Japan)
- **Mapper 11 (Color Dreams)** - Color Dreams and Wisdom Tree games (~1.3% of games)
- **Mapper 34 (BNROM)** - Deadly Towers, homebrew titles
- **Mapper 66 (GxROM)** - SMB + Duck Hunt, Doraemon (~1.2% of games)
- **Mapper 71 (Camerica)** - Fire Hawk, Micro Machines (~0.6% of games)
- **Mapper 79 (NINA-03/06)** - AVE games like Dudes with Attitude, Pyramid
- **Mapper 206 (Namco 118)** - Dragon Spirit, Famista (~1.8% of games)

**ROM Format**: iNES (.nes files) - automatically detected

**Features**:
- Full PPU (video) and APU (audio) emulation
- Save states (F5/F6)
- NTSC and PAL timing modes (auto-detected)
- Controller support with customizable key mappings

**Known Limitations**:
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games but may not handle edge cases requiring precise PPU timing
- **Unsupported Mappers**: Games using mappers beyond the supported 14 will not work (affects ~10% of games)

### Atari 2600

**Status**: 🚧 In Development  
**Coverage**: Most common cartridge formats (2K, 4K, 8K, 12K, 16K, 32K)

The emulator supports the following cartridge banking schemes:
- **2K ROM** - No banking, simple games like Combat
- **4K ROM** - No banking, common format for early games like Pac-Man
- **8K (F8)** - 2x 4KB banks, games like Asteroids, Missile Command
- **12K (FA)** - 3x 4KB banks, used by some CBS games
- **16K (F6)** - 4x 4KB banks, games like Donkey Kong, Crystal Castles
- **32K (F4)** - 8x 4KB banks, later and larger games

**ROM Format**: Raw binary (.a26, .bin files) - automatically detected by size

**Features**:
- TIA (Television Interface Adapter) video emulation with playfield rendering
- **Cycle-Accurate Rendering**: Renders each pixel as generated, handles mid-scanline register changes, supports "racing the beam" techniques
- **Player/Missile Sizing (NUSIZ)**: Full support for sprite sizing (1x, 2x, 4x) and duplication modes
- **Ball Sizing**: Full support for ball sizing (1, 2, 4, or 8 pixels) via CTRLPF bits 4-5
- **Collision Detection**: All 8 collision registers implemented with pixel-perfect detection
- **Delayed Graphics (VDELP0/VDELP1/VDELBL)**: Player and ball graphics can be delayed by one scanline
- **Reset Missile to Player (RESMP0/RESMP1)**: Missiles can be locked to player positions
- TIA audio emulation with 2 channels (polynomial waveform synthesis)
- RIOT (6532) chip emulation for RAM, I/O, and timers
- Save states (F5/F6)
- Joystick controls mapped to keyboard (same as NES controls)
- 160x192 resolution

**Known Limitations**:
- **Paddle Controllers**: INPT0-INPT3 always return 0 - paddle games (Breakout, Kaboom!, Warlords) are unplayable
- **Banking**: Standard schemes supported (2K, 4K, F8, FA, F6, F4); exotic formats not implemented (DPC for Pitfall II, FE for Decathlon, 3F, E0)

**Hardware-Accurate Behaviors** (appear as visual issues but are authentic to original hardware):
- **Sprite Flicker** (e.g., Pac-Man ghosts, Space Invaders): The Atari 2600 only has 2 sprite registers. Games needing more sprites use "sprite multiplexing" - cycling through sprites on alternate frames. On original CRT TVs, phosphor persistence blended the flicker naturally. On modern displays, this appears as pronounced flashing. This affects ALL Atari 2600 emulators.
  - **Solution**: Use the **Phosphor Persistence** display filter (View → Properties → Display Filter → Phosphor Persistence) to reduce flicker
  - This filter blends frames to simulate CRT phosphor decay, significantly improving visual quality for flicker-heavy games
  - See the [Atari 2600 README](../../../crates/systems/atari2600/README.md#hardware-accurate-behaviors-not-bugs) for detailed explanation

**Recent Fixes**:
- **Vertical Stability**: Fixed vertical jumping issue by caching the visible window start position across frames
- **Frame Consistency**: Enhanced test ROMs to catch timing-related rendering issues
- **Bus Address Mapping**: Fixed address range 0x40-0x7F to write to both TIA and RIOT RAM (hardware-accurate dual-write behavior)
- **Ball Sizing**: Implemented variable ball sizing (1, 2, 4, or 8 pixels) via CTRLPF bits 4-5
- **Delayed Ball Graphics**: Implemented VDELBL register for ball animation
- **Reset Missile to Player**: Implemented RESMP0/RESMP1 registers for synchronized missile/player movement

**Controls**: The Atari 2600 joystick is mapped to the same keyboard layout as NES:
- Arrow keys = Joystick directions
- Z = Fire button
- Enter = Game Reset (console switch)
- Left Shift = Game Select (console switch)

### CHIP-8

**Status**: ✅ Fully Working  
**Coverage**: Complete CHIP-8, CHIP-8 Hires, Super-CHIP, XO-CHIP, and Mega-CHIP specifications (all opcodes and features implemented)

**Program Format**: .ch8 or .c8 files - automatically detected by file size (< 3.5KB for CHIP-8/Super-CHIP, < 64KB for XO-CHIP)

**Features**:
- **CHIP-8 Core**: All 35 original opcodes, 64x32 monochrome display, 4KB memory
- **CHIP-8 Hires**: VIP 2-page mode with 64x64 resolution for Cosmac VIP/Telmac 1800 ROMs
- **Super-CHIP Extensions**: 128x64 high-resolution mode, 4-direction scrolling, 16x16 sprites, large font, flag registers
- **XO-CHIP Extensions**: 4-color display with dual bit planes, 64KB extended memory, audio patterns, plane selection
- **Mega-CHIP Extensions**: Ultra-high 256x192 resolution, advanced graphics capabilities
- Built-in hexadecimal fonts (5x8 standard, 10x16 large)
- 16-key hexadecimal keypad
- Sound and delay timers (60Hz)
- 16-level stack for subroutines
- Save states with mode and resolution preservation (State → Save State menu)
- Runs at ~700 instructions per second (original CHIP-8 speed)

**Keyboard Layout**: The 16-key CHIP-8 hex keypad is mapped to QWERTY:
```
CHIP-8 Keypad:     Keyboard Mapping:
1 2 3 C            1 2 3 4
4 5 6 D      →     Q W E R
7 8 9 E            A S D F
A 0 B F            Z X C V
```

**Mode Selection**:
- Programs automatically run in CHIP-8 mode by default
- CHIP-8 Hires mode is automatically detected when opcode 0x1260 is at PC=0x200
- Super-CHIP and XO-CHIP programs can switch modes using specific opcodes
- Mega-CHIP mode auto-activates when 00FF opcode is used from basic CHIP-8
- Manual mode selection available via API (for developers)
- Current mode displayed in debug panel for easy verification

**CHIP-8 Hires Features** (fully implemented):
- 64x64 pixel high-resolution mode for original VIP hardware
- Automatic mode detection via opcode 0x1260 at program start
- Special clear screen opcode 0x0230 for 64x64 display
- Compatible with ROMs designed for Cosmac VIP and Telmac 1800

**Super-CHIP Features** (fully implemented):
- High-resolution 128x64 display (switchable with 00FE/00FF)
- Scrolling: down (00CN), up (00DN), right (00FB), left (00FC)
- Large 16x16 font sprites for digits 0-9 (FX30)
- 16x16 sprite drawing in high-res mode (DXY0)
- RPL flag register save/load (FX75/FX85)

**XO-CHIP Features** (fully implemented):
- 4-color display using dual bit planes (black, green, red, yellow)
- Extended 64KB memory addressing
- Plane selection for multi-color graphics (FN01)
- Audio pattern buffer (F002) and pitch control (FX3A)
- Extended I register loading (F000 NNNN)
- Range save/load opcodes (5XY2/5XY3)

**Mega-CHIP Features** (fully implemented):
- Ultra-high resolution 256x192 display mode
- Automatic mode upgrade when 00FF opcode used from basic CHIP-8
- Full backward compatibility with all standard CHIP-8 opcodes
- Larger display buffer for advanced graphics and homebrew games
- Save state support with mode preservation

**Known Limitations**:
- **Audio**: Audio pattern buffer and pitch are tracked but no actual tone generation
- **Random Number**: Uses simple pseudo-random generator (not cryptographically secure)
- **Timing**: Fixed instruction count per frame (not cycle-accurate)

**Technical Details**:
- 16 8-bit general-purpose registers (V0-VF, where VF is used as flag)
- Programs loaded at address 0x200
- XOR-based sprite drawing with collision detection
- Deterministic behavior for save states and replay
- Variable memory: 4KB (CHIP-8/Super-CHIP) or 64KB (XO-CHIP)
- Variable resolution: 64x32 (low-res), 64x64 (hires), 128x64 (high-res), or 256x192 (mega)
- Debug panel shows current mode and active resolution for easy verification

**Recommended Programs**:
- Classic CHIP-8 games (Pong, Tetris, Space Invaders, Breakout)
- VIP 2-page hires ROMs for original Cosmac VIP/Telmac 1800 hardware
- Super-CHIP games with enhanced graphics
- XO-CHIP programs with color and audio
- Mega-CHIP homebrew with advanced graphics
- Homebrew programs and test ROMs

For more technical information, see [crates/systems/chip8/README.md](../crates/systems/chip8/README.md).

### Game Boy / Game Boy Color

**Status**: ✅ Fully Functional  
**Coverage**: ~99% of Game Boy games supported (MBC0, MBC1, MBC2, MBC3, MBC5, HuC1 implemented)

**ROM Format**: GB/GBC (.gb, .gbc files) - automatically detected

**Features**:
- Full PPU (Picture Processing Unit) rendering: background, window, sprites
- Resolution: 160x144 pixels
- Sprite support: 40 sprites with 8x8/8x16 modes, flipping, priority, 10-per-scanline limit
- **Game Boy Color (CGB) Support**:
  - Automatic CGB mode detection
  - 15-bit RGB color palettes (8 BG + 8 OBJ palettes)
  - DMG-compatible default palette initialization for early CGB games (e.g., Pokemon Yellow)
  - VRAM banking (2 banks of 8KB)
  - WRAM banking (8 banks of 4KB, 32KB total) via SVBK register
  - Tile attributes (palette selection, VRAM banking, flipping)
  - Backward compatible with DMG games
- **MBC (Memory Bank Controller) Support**:
  - MBC0: No mapper (32KB ROMs)
  - MBC1: Most common mapper (~70% of games, up to 2MB ROM, 32KB RAM)
  - MBC2: Built-in RAM mapper (~1% of games, up to 256KB ROM, 512×4 bits built-in RAM)
  - MBC3: With battery saves and **working RTC** (~15% of games, up to 2MB ROM, 32KB RAM)
  - MBC5: Advanced mapper (~10% of games, up to 8MB ROM, 128KB RAM)
  - HuC1: Hudson Soft mapper (<1% of games, up to 1MB ROM, 32KB RAM, IR sensor support)
- Joypad input with matrix selection
- Timer registers (DIV, TIMA, TMA, TAC) with interrupt support
- VBlank and Timer interrupts
- **Audio**: Full APU with 4 sound channels (Pulse 1/2, Wave, Noise)
- Audio integrated with frontend (44.1 kHz stereo output)
- Save states (State → Save State menu)
- Frame-based timing (~59.73 Hz)

**Known Limitations**:
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games
- **Speed Switching**: CGB speed switching is supported, but timing is not affected (emulation runs at same speed regardless)
- **Link Cable**: Serial transfer implemented with loopback mode, but no external link cable support - multiplayer/trading won't work between instances
- **Infrared Port**: RP register (0xFF56) accessible but IR hardware not emulated - IR features won't communicate externally
- **Unimplemented Mappers** (rare, <3% of games): MBC6, MBC7, HuC3, MMM01, TAMA5

**Controls**: Game Boy buttons are mapped to the same keyboard layout as NES:
- Arrow keys = D-pad
- Z = A button
- X = B button
- Enter = Start
- Left Shift = Select

### SMS (Sega Master System)

**Status**: 🚧 In Development (not producing image)  
**Coverage**: Complete hardware emulation - Z80 CPU, VDP, PSG fully implemented

**ROM Format**: SMS (.sms files) - automatically detected via TMR SEGA header or file size

**Features**:
- **Z80 CPU**: Complete instruction set (~586 opcodes)
  - All base instructions (8080-compatible)
  - Extended instruction sets (CB, DD, ED, FD prefixes)
  - Bit operations, indexed addressing, block transfers
  - Interrupt modes (IM 0, IM 1, IM 2)
  - Shadow registers and special operations
- **VDP (Video Display Processor)**: Full tilemap-based rendering
  - 256×192 resolution (standard SMS display)
  - 64 sprites with 8 sprites per scanline limit
  - Hardware sprite collision detection
  - Background and sprite rendering
  - VRAM (16KB) and CRAM (32 bytes color RAM)
  - I/O ports: 0xBE (data), 0xBF (control)
- **SN76489 PSG (Programmable Sound Generator)**: Full audio emulation
  - 3 tone channels (square wave generators)
  - 1 noise channel with 16-bit LFSR (Sega variant)
  - Proper volume curve and attenuation
  - I/O port: 0x7F (write-only)
- **Memory System**:
  - ROM banking for cartridges >48KB
  - 8KB work RAM (mirrored in upper address space)
  - Banking registers at 0xFFFC-0xFFFE
  - I/O port mapping (VDP, PSG, controllers)
- **Controller Support**: 2 controller ports
  - I/O ports: 0xDC (port A/B), 0xDD (port B/misc)
  - Full button mapping (Up, Down, Left, Right, Button 1, Button 2)
- **Save States** (F5/F6): Complete state serialization
  - CPU state (all Z80 registers and flags)
  - VDP state (VRAM, CRAM, registers)
  - PSG audio state (all channels)
  - Memory state (RAM, ROM banking)
- **PAL/NTSC Support**: Automatic timing detection
  - Detects region from ROM header (TMR SEGA)
  - NTSC: 60 Hz, 3.579545 MHz, 262 scanlines
  - PAL: 50 Hz, 3.546894 MHz, 313 scanlines
  - Frame-based timing with proper cycle counts

**Known Limitations**:
- **Not Producing Image**: Currently not generating visible output - under active development
- **Test ROM**: No smoke test ROM yet - testing with commercial ROMs needed
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games

**Controls**: SMS controller mapped to same keyboard layout as NES:
- Arrow keys = D-pad
- Z = Button 1
- X = Button 2
- Enter = Pause button (console)
- Left Shift = Not used (SMS has no Select button)

**ROM Loading**: 
- ROMs with TMR SEGA header (offset 0x7FF0) are auto-detected as SMS
- Headerless ROMs of exactly 48KB are detected as SMS
- Larger ROMs (64KB+) default to SNES unless header is present

**Technical Details**:
- Z80 clock speed: ~3.58 MHz (NTSC colorburst frequency)
- VDP frame rate: ~60 Hz (59.922 Hz actual)
- Cycles per frame: 59659 (Z80 cycles)
- Display resolution: 256×192 pixels
- Color depth: 6-bit RGB (64 total colors: 2 bits per channel)
- Audio sample rate: 44100 Hz (mixed from PSG channels)

**Recommended Test Games**:
- Commercial SMS ROMs with TMR SEGA header
- Games using standard ROM banking (most cartridge games)
- Games not requiring special peripherals (Light Phaser, paddle controllers)

### ColecoVision

**Status**: 🚧 In Development (not producing image)  
**Coverage**: Complete hardware emulation - Z80 CPU, TMS9918A VDP, SN76489 PSG fully implemented

**ROM Format**: COL (.col files) - automatically detected

**Features**:
- **Z80 CPU**: Complete instruction set implementation (reused from `emu_core`)
  - All base instructions (8080-compatible)
  - Extended instruction sets (CB, DD, ED, FD prefixes)
  - Interrupt modes (IM 0, IM 1, IM 2)
  - 3.579545 MHz clock speed (NTSC)
- **TMS9918A VDP (Video Display Processor)**: Full tilemap-based rendering
  - 256×192 resolution (standard display)
  - 16-color palette
  - 16 KB VRAM
  - 4 graphics modes:
    - Graphics I: 256×192, 8×8 tiles, 2 colors per 8 patterns
    - Graphics II: 256×192, 8×8 tiles, 2 colors per pattern row (most common)
    - Text: 40×24 characters, 6×8 font
    - Multicolor: 64×48 blocks, 4×4 pixels per block
  - 32 hardware sprites:
    - 8×8 or 16×16 pixels
    - 1× or 2× magnification
    - 4 sprites per scanline limit
    - Sprite collision detection
  - Frame interrupts at 60 Hz
- **SN76489 PSG (Programmable Sound Generator)**: Full audio implementation
  - 3 square wave tone generators (10-bit frequency)
  - 1 noise generator (white/periodic noise)
  - 4-bit volume control per channel
  - Reuses the SN76489 implementation from `emu_core`
- **Memory System**:
  - 8 KB BIOS ROM (required, must be provided separately)
  - 1 KB work RAM (mirrored in upper address space)
  - Up to 32 KB cartridge ROM
  - I/O port mapping (VDP, PSG, controllers)
- **Controller Support**: 2 controller ports
  - Standard joystick with fire buttons
  - Full button mapping (Up, Down, Left, Right, Fire 1, Fire 2)
- **Save States** (F5/F6): Complete state serialization
  - CPU state (all Z80 registers and flags)
  - VDP state (VRAM, registers, internal state)
  - PSG audio state (all channels)
  - Memory state (RAM, BIOS, cartridge)

**Known Limitations**:
- **Not Producing Image**: Currently not generating visible output - under active development
- **Audio Output**: PSG implemented but not yet connected to audio pipeline - silent gameplay
- **BIOS Required**: Must provide 8 KB BIOS ROM separately (not included)
- **Test ROM**: No smoke test ROM yet - testing with commercial ROMs needed
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games
- **No Expansion Modules**: Super Game Module and other expansions not supported
- **Controllers**: Limited to standard joystick (no Super Action Controllers, spinners, etc.)

**Controls**: The ColecoVision controller has a joystick, 2 fire buttons, and a 12-key numeric keypad.

**Player 1:**
- Arrow keys = Joystick (Up/Down/Left/Right)
- Left Shift = Fire Button A (left side)
- Enter = Fire Button B (right side)
- 1,2,3,Q,W,E,A,S,D,Z,X,C = Numeric keypad (keys 1-9, *, 0, #)

**Player 2:**
- I/J/K/L = Joystick (I=Up, K=Down, J=Left, L=Right)
- Right Shift = Fire Button A (left side)
- P = Fire Button B (right side)
- 7,8,9,U,I,O,H,J,K,N,M,Comma = Numeric keypad (keys 1-9, *, 0, #)

See the [Controls](controls.html#colecovision-controller-input) page for detailed keypad layout.

**ROM Loading**: 
- ROMs with .col extension are auto-detected as ColecoVision
- BIOS must be mounted separately via mount points system
- Cartridges up to 32 KB supported

**Technical Details**:
- Z80 clock speed: 3.579545 MHz (NTSC colorburst frequency)
- VDP frame rate: 60 Hz
- Display resolution: 256×192 pixels
- Color depth: 4-bit RGB (16 total colors)
- Audio sample rate: 44100 Hz (when connected)

**Recent Improvements** (January 2026):
- Sprite collision detection now uses dedicated sprite buffer
- Sprite overflow handling properly persists across scanlines
- Corrected sprite Y position offset (-1)
- Added array bounds checking for sprite table accesses
- PSG state now properly resets on system reset
- Fixed sprite enable logic

For detailed technical information, see [crates/systems/colecovision/README.md](../../../crates/systems/colecovision/README.md).

### SG-1000

**Status**: ⚠️ Experimental  
**Coverage**: Complete hardware emulation - Z80 CPU, TMS9918A VDP, SN76489 PSG fully implemented

**ROM Format**: SG (.sg, .sc files) - automatically detected

**Features**:
- **Z80 CPU**: Complete instruction set (shared with ColecoVision and SMS)
  - 3.579545 MHz clock speed (NTSC)
  - Reuses the Z80 CPU implementation from `emu_core`
- **TMS9918A VDP**: Same graphics chip as ColecoVision
  - 256×192 resolution
  - 16-color palette
  - 16 KB VRAM
  - 4 graphics modes (Graphics I/II, Text, Multicolor)
  - 32 hardware sprites with collision detection
  - Frame interrupts at 60 Hz
- **SN76489 PSG**: Same sound chip as ColecoVision and SMS
  - 3 square wave tone generators
  - 1 noise generator
  - Reuses the SN76489 implementation from `emu_core`
- **Memory System**:
  - No BIOS ROM (boots directly from cartridge)
  - Up to 48 KB cartridge ROM
  - 1 KB work RAM (mirrored in upper address space)
  - Different memory map than ColecoVision (cartridge at 0x0000)
- **Controller Support**: 2 controller ports
  - Standard joystick with fire buttons
  - Full button mapping
- **Save States** (F5/F6): Complete state serialization
  - CPU state (all Z80 registers)
  - VDP state (VRAM, registers)
  - PSG audio state (all channels)
  - Memory state (RAM contents)

**Hardware Differences from ColecoVision**:
- **No BIOS**: SG-1000 boots directly from cartridge (ColecoVision requires BIOS)
- **Memory Map**: Cartridge at 0x0000 instead of 0x8000
- **I/O Ports**: PSG at 0x7F instead of 0xA0
- **Same Components**: Identical Z80, TMS9918A, and SN76489 chips

**Known Limitations**:
- **Audio Output**: PSG implemented but not yet connected to audio pipeline - silent gameplay
- **Test ROM**: No smoke test ROM yet - testing with commercial ROMs needed
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games
- **No SC-3000**: Keyboard computer variant not supported
- **Controllers**: Limited to standard joystick

**Controls**: SG-1000 controller mapped to same keyboard layout as NES:
- Arrow keys = D-pad
- Z = Fire 1
- X = Fire 2
- Enter = Not used
- Left Shift = Not used

**ROM Loading**: 
- ROMs with .sg or .sc extension are auto-detected as SG-1000
- No BIOS required (unlike ColecoVision)
- Cartridges up to 48 KB supported

**Technical Details**:
- Z80 clock speed: 3.579545 MHz (NTSC colorburst frequency)
- VDP frame rate: 60 Hz
- Display resolution: 256×192 pixels
- Color depth: 4-bit RGB (16 total colors)
- Audio sample rate: 44100 Hz (when connected)

For detailed technical information, see [crates/systems/sg1000/README.md](../../../crates/systems/sg1000/README.md).

### SNES (Super Nintendo Entertainment System)

**Status**: ✅ Functional (Graphics Working!)  
**Coverage**: Complete CPU, comprehensive PPU with all modes 0-7, full DMA/HDMA

**ROM Format**: SMC/SFC (.smc, .sfc files) - automatically detected

**Features**:
- ✅ **65C816 CPU** - Complete 16-bit CPU (256/256 opcodes, 100% complete)
- ✅ **PPU Rendering** - All modes 0-7 fully implemented:
  - Mode 0: 4 BG layers, 2bpp (most flexible)
  - Mode 1: 2 BG layers 4bpp + 1 BG layer 2bpp (most common in games)
  - Mode 2-6: Advanced features (offset-per-tile, hi-res 512px)
  - Mode 7: Full rotation/scaling with matrix transformation
- ✅ **Sprites** - 128 sprites, 4bpp, hardware-accurate overflow limits
- ✅ **Advanced Graphics**:
  - Window masking (2 independent windows with combination logic)
  - Color math (sub-screen blending, add/subtract/half modes)
  - Mode 7 rotation and scaling
  - Offset-per-tile scrolling (Modes 2, 4, 6)
  - Hi-res 512px modes (Modes 5-6)
- ✅ **DMA/HDMA** - Complete 8-channel implementation, all transfer modes
- ✅ **SPC700 APU CPU** - Full audio processor implementation:
  - Complete SPC700 instruction set
  - 64KB audio RAM (ARAM)
  - IPL boot ROM with upload protocol
  - Communication ports ($2140-$2143) working
  - Games can upload and execute audio drivers
  - ❌ DSP not implemented (no sound output)
- ✅ **Cartridge Support**:
  - LoROM, HiROM, and ExHiROM auto-detection
  - SRAM save support
  - Enhancement chips: DSP-1 (partial), SuperFX/SuperFX2 (core features)
- ✅ **Controller** - Full 12-button support with auto-joypad read
- ✅ **Save States** - F5-F9 for save, Shift+F5-F9 for load

**Known Limitations**:
- **Audio**: SPC700 CPU fully functional but DSP not implemented - **silent gameplay**
  - Games can upload audio drivers and communicate with APU
  - No 8-voice synthesis, ADPCM playback, or echo effects
- **Enhancement Chips**: 
  - DSP-1 partially implemented (missing some math operations)
  - SuperFX/SuperFX2 core functionality implemented
  - SA-1, S-DD1, CX4, and others not yet implemented
- **Graphics Edge Cases**:
  - Frame-based rendering: mid-frame register changes take effect on next frame
  - Most games work correctly, advanced techniques may not render exactly as hardware
- **Mosaic Effects**: Not implemented ($2106 register stubbed)
- **Timing**: Frame-based rendering (not cycle-accurate), NTSC only

**Recommended For**:
- Playing most SNES games (silently)
- Games using Modes 0-1 (vast majority of commercial titles)
- Testing enhancement chip games (DSP-1, SuperFX)
- Not recommended if audio is essential

**See Also**: [SNES Implementation README](../../../crates/systems/snes/README.md) for technical details

### N64 (Nintendo 64)

**Status**: 🚧 In Development (3D rendering works, limited game support)  
**Coverage**: Very limited - Core components functional, working towards game compatibility

**ROM Format**: Z64/N64/V64 (.z64, .n64, .v64 files) - automatically detected with byte-order conversion

**Features**:
- MIPS R4300i CPU core with complete instruction set
- Memory bus (4MB RDRAM + PIF + SP memory + RDP/VI registers)
- **PIF (Peripheral Interface)** - Controller support
  - 4 controller ports with full button mapping
  - All 14 buttons supported: A, B, Z, Start, D-pad (4), L, R, C-buttons (4)
  - Analog stick with full range (-128 to 127 on X/Y axes)
  - Controller command protocol for game communication
- **RSP (Reality Signal Processor)** - High-Level Emulation
  - Microcode detection (F3DEX/F3DEX2/Audio)
  - Vertex buffer management (32 vertices)
  - **Full matrix transformation pipeline**:
    - 4x4 projection and modelview matrices
    - Matrix loading from RDRAM (16.16 fixed-point format)
    - Matrix multiplication (LOAD and MUL modes)
    - Complete vertex transformation: modelview → projection → perspective divide → viewport
  - **F3DEX display list commands**:
    - G_VTX (0x01) - Load vertices
    - G_TRI1 (0x05), G_TRI2 (0x06), G_QUAD (0x07) - Triangle/quad rendering
    - G_MTX (0xDA) - Load transformation matrices
    - G_GEOMETRYMODE (0xD9) - Set rendering flags
    - G_DL (0xDE) - Display list branching (nested display lists)
    - G_ENDDL (0xDF) - End display list
    - RDP passthrough (0xE0-0xFF) - Embedded RDP commands
  - Task execution framework for graphics microcode
- RDP (Reality Display Processor) with OpenGL hardware-accelerated rendering
  - **GPU-accelerated**: Uses OpenGL 3.3 Core Profile for high performance
  - **Full feature set**: 3D triangles, Z-buffer, texture mapping, scissor clipping
  - **Hardware depth testing**: Leverages GPU Z-buffer for efficient occlusion
  - **3D triangle rasterization** with flat, Gouraud shading, and texture mapping
  - **Z-buffer (depth buffer)** for hidden surface removal
  - **Scissor clipping** for efficient rendering
  - **Texture mapping** with UV coordinate interpolation
  - OpenGL renderer provides optimal performance for N64 3D graphics
- VI (Video Interface) with display configuration registers
- ROM loading with automatic byte-order detection and conversion
- Save states (F5/F6)
- Resolution: 320x240 pixels (configurable)

**3D Rendering Capabilities**:
- **Triangle Rendering**:
  - Flat-shaded triangles (solid color)
  - Gouraud-shaded triangles (per-vertex color interpolation)
  - **Textured triangles** (with UV coordinate interpolation)
  - Z-buffered triangles (depth testing for proper occlusion)
  - Combined shading + Z-buffer rendering
  - Combined texture + Z-buffer rendering
- **Z-Buffer**:
  - 16-bit depth buffer (0 = near, 0xFFFF = far)
  - Per-pixel depth testing
  - Automatic depth buffer updates
  - Can be enabled/disabled per triangle
- **Rasterization Features**:
  - Scanline-based edge walking
  - Barycentric coordinate interpolation
  - Per-pixel color and depth interpolation
  - **Per-pixel texture coordinate interpolation**
  - Scissor rectangle clipping
- **Texture Mapping**:
  - 4KB TMEM (Texture Memory) for texture storage
  - 8 tile descriptors for texture configuration
  - RGBA16 (5-5-5-1) and RGBA32 (8-8-8-8) format support
  - Texture wrapping and clamping modes
  - LOAD_BLOCK and LOAD_TILE commands for texture loading

**Controller Mapping**:
For N64 games, the standard controller mappings apply with these button equivalents:
- **A** = Z key (Player 1) / U key (Player 2)
- **B** = X key (Player 1) / O key (Player 2)
- **Start** = Enter (Player 1) / P (Player 2)
- **D-pad** = Arrow keys (Player 1) / I/J/K/L (Player 2)
- **L/R** = (Not yet mapped - will be added in future update)
- **C-buttons** = (Not yet mapped - will be added in future update)
- **Analog stick** = (Not yet mapped - will be added in future update)

*Note: Controller mappings can be customized in `config.json`. Full analog stick and shoulder button support coming soon.*

**Known Limitations**:
- **Renderer**: Uses OpenGL 3.3 for hardware-accelerated GPU rendering
  - Requires OpenGL 3.3+ compatible graphics hardware
  - OpenGL 3.3+ is required for N64 emulation (no software fallback)
- **Graphics**: RDP implementation supports basic display list commands
  - **Working commands**:
    - FILL_RECTANGLE - solid color rectangles
    - SET_FILL_COLOR - set fill color for rectangles
    - SET_SCISSOR - clipping rectangle support (fully working)
    - SET_TILE - configure tile descriptors for textures (fully implemented)
    - SET_TEXTURE_IMAGE - set texture source address (fully implemented)
    - SYNC commands (SYNC_FULL, SYNC_PIPE, SYNC_TILE, SYNC_LOAD)
    - SET_COLOR_IMAGE - accepted but uses internal framebuffer
  - **Triangle commands** (opcodes 0x08-0x0F):
    - 0x08: Non-shaded triangle (fully implemented)
    - 0x09: Non-shaded triangle with Z-buffer (fully implemented)
    - 0x0A: Textured triangle (fully implemented)
    - 0x0B: Textured triangle with Z-buffer (fully implemented)
    - 0x0C: Shaded triangle (fully implemented)
    - 0x0D: Shaded triangle with Z-buffer (fully implemented)
  - **Stub implementations** (accept but don't fully process):
    - TEXTURE_RECTANGLE - currently renders as solid rectangle (needs advanced sampling)
    - SET_OTHER_MODES - rendering mode configuration (ignored)
  - **TMEM (Texture Memory)**: ✅ Fully implemented
    - 4KB TMEM buffer with texture loading via LOAD_BLOCK and LOAD_TILE
    - Tile descriptors (8 tiles) fully configured via SET_TILE
    - Texture image address tracking via SET_TEXTURE_IMAGE
    - Texture sampling for RGBA16 and RGBA32 formats
    - **Textured triangle rendering fully integrated**
  - **Not implemented**: 
    - Advanced texture formats (CI, IA, I) - render as white
    - Anti-aliasing and blending
    - Perspective-correct texture mapping
    - Most advanced rendering commands
    - Performance counters (DPC_CLOCK, DPC_BUFBUSY, DPC_PIPEBUSY, DPC_TMEM) return zeros
  - Can render 3D textured graphics with depth testing
  - Full game graphics require perspective-correct mapping, additional RDP features, and more complete RSP emulation
- **VI (Video Interface)**: Registers implemented but not fully integrated
  - All VI registers accessible (STATUS, ORIGIN, WIDTH, timing, scaling)
  - Not yet used for actual display output (uses RDP internal framebuffer)
  - Scanline tracking and interrupt support in place but not active
- **RSP**: High-Level Emulation with partial F3DEX display list processing
  - ✅ **Implemented**:
    - Microcode detection (F3DEX, F3DEX2, Audio)
    - Vertex buffer management (32 vertices)
    - Full matrix transformation pipeline (projection, modelview, viewport)
    - **10-level matrix stack** with push/pop operations (G_MTX, G_POPMTX)
    - Display list parsing with command execution
    - Matrix loading (G_MTX) with LOAD/MUL/PUSH modes
    - Geometry mode control (G_GEOMETRYMODE)
    - Triangle rendering commands (G_TRI1, G_TRI2, G_QUAD)
    - Display list branching (G_DL) for nested lists
    - **Conditional branching (G_BRANCH_Z)** for Z-buffer-based culling
    - RDP command passthrough (0xE0-0xFF range)
    - Vertex transformation with perspective projection
    - RDP triangle command generation
  - ⚠️ **Stubbed/Missing**:
    - Audio microcode tasks (not implemented - no audio output)
    - G_MOVEWORD, G_MOVEMEM, G_SETOTHERMODE_L/H (logged but ignored)
    - Semaphore register (always returns 0)
    - Signal bits (SIG0-SIG7) not implemented
    - No instruction-level execution (HLE only, no LLE)
    - No lighting calculations
    - No texture coordinate generation
    - Some advanced F3DEX2 commands missing
- **Audio**: Audio interface implemented but RSP audio microcode not supported - silent gameplay
- **Save System**: No EEPROM, Flash, or Controller Pak support - games cannot save progress
- **Input**: Controller infrastructure complete, needs frontend integration
  - All 14 buttons defined and working (A, B, Z, Start, D-pad, L, R, C-buttons)
  - Analog stick support implemented (-128 to 127 range)
  - PIF command protocol functional
  - Frontend keyboard/gamepad mapping not yet connected
- **Memory**: Basic memory map only - no TLB, cache, or accurate timing
- **Timing**: Frame-based implementation (50,000 cycles/frame vs hardware's 1,562,500) - not cycle-accurate
- **CPU Edge Cases**: 
  - Overflow traps not implemented (uses wrapping arithmetic)
  - Memory alignment not validated (assumes proper alignment)
  - Cache is direct-mapped only (no full coherency)
- **Status**: Core infrastructure in place (CPU, RDP, RSP HLE with partial F3DEX support, PIF). RSP supports full matrix stack operations and conditional branching. **Textured triangle rendering fully implemented** with TMEM texture loading and sampling. Next steps: complete RSP commands, audio microcode, save system, perspective-correct mapping, lighting, frontend controller integration. Test ROMs can run and render transformed 3D graphics with textures.

### PC/DOS (IBM PC/XT)

**Status**: 🧪 Experimental (Modular architecture with disk support)  
**Coverage**: Basic emulation - Custom BIOS, multi-slot mount system

**File Formats**: 
- Executable: COM/EXE (.com, .exe files)
- BIOS: Binary ROM (.bin, .rom files)
- Floppy disks: Disk images (.img, .ima files)
- Hard drives: Disk images (.img, .vhd files)

**Features**:
- **8086/80186/80286/80386 CPU core** with comprehensive instruction set
  - **8086/8088**: All base instructions (MOV, arithmetic, logical, control flow, stack, flags)
  - **80186/80188**: PUSHA/POPA, BOUND, PUSH immediate, IMUL immediate, INS/OUTS, ENTER/LEAVE
  - **80286**: Protected mode instruction stubs (LMSW, LAR, LSL, CLTS)
  - **80386**: Full 32-bit register support with operand size override (0x66 prefix)
    - **32-bit Registers**: EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP, EIP, EFLAGS
    - **32-bit Addressing**: SIB (Scale-Index-Base) byte support for complex addressing modes
    - **32-bit Instructions**: MOV, ADD, SUB, AND, OR, XOR, TEST, CMP with 32-bit operands
    - MOVSX/MOVZX, BSF/BSR, BT/BTS/BTR/BTC, SETcc (bit manipulation and conditional operations)
  - CPU model selection support for running software with different instruction set requirements
  - Maintains full backward compatibility - 16-bit operations work on low 16 bits of 32-bit registers
  - See `../AGENTS.md` for full instruction set details
- **Memory bus** (configurable conventional + extended memory, 128KB VRAM, 256KB ROM)
  - **Conventional memory**: 256KB-640KB (PC/XT compatible, visible to all software)
  - **Extended memory**: Above 640KB (accessible via XMS, for protected mode or extended software)
  - **Total memory**: Configurable from 256KB (minimum) to much larger (e.g., 16MB+)
  - INT 12h reports conventional memory (max 640KB)
  - INT 15h AH=88h reports extended memory (above 1MB equivalent)
- **Custom BIOS** with POST screen
  - 64KB BIOS ROM with traditional PC BIOS POST (Power-On Self-Test) screen
  - Displays on boot: BIOS version, CPU type, memory test, disk drives, boot priority
  - Updates dynamically when disks are mounted/unmounted
  - Shows helpful instructions: F3 to mount disks, F12 to reset, F8 to save VM
  - INT 13h disk services (FULLY IMPLEMENTED - all standard and extended functions including FAT32 support)
    - Standard functions: Reset (00h), Get Status (01h), Read (02h), Write (03h), Verify (04h), Format (05h), Get Drive Parameters (08h)
    - Extended functions: Get Disk Type (15h), Disk Change Status (16h), Check Extensions (41h)
    - **Extended INT 13h (EDD) for FAT32/large disks**: Extended Read LBA (42h), Extended Write LBA (43h), Extended Verify (44h), Get Extended Drive Parameters (48h)
    - Complete CHS (Cylinder/Head/Sector) to LBA translation
    - LBA (Logical Block Addressing) support for drives >8GB
    - Full read/write access to all mounted disk images
    - **Supports DOS with FAT12, FAT16, and FAT32 filesystems** via real DOS running in emulator
  - Source: `test_roms/pc/bios.asm`
  - Build script: `test_roms/pc/build.sh` (requires NASM)
  - Replaceable via BIOS mount point
- **Modular mount point system**:
  1. **BIOS** (Slot 1) - Custom or replacement BIOS ROM (`.bin`, `.rom`)
  2. **Floppy A** - Floppy disk drive A: (`.img`, `.ima`)
  3. **Floppy B** - Floppy disk drive B: (`.img`, `.ima`)
  4. **Hard Drive C** - Hard disk drive C: (`.img`, `.vhd`)
- **Disk controller** with INT 13h support (fully implemented)
  - Floppy geometry: 1.44MB format (80 cylinders, 18 sectors, 2 heads)
  - Hard drive geometry: 10MB format (306 cylinders, 17 sectors, 4 heads)
  - LBA (Logical Block Address) calculation
  - Read/write operations to disk images (fully functional)
  - Boot sector loading with boot priority (floppy first, hard drive first, etc.)
- **CGA video** (640x400 text mode)
- **Keyboard input** with full passthrough
- **Virtual Machine State Saving**: PC systems use File → Save Project to save VM configuration
  - Instead of save states, PC mode saves the current VM configuration to a `.hemu` project file
  - Includes all mounted disk images, BIOS, boot priority settings, CPU model, memory size, and video mode
  - Use File → Save Project to open a save dialog and choose where to save the VM file
  - Load the VM file later via File → Open Project to restore all mount points and configuration
  - Disk state is preserved in the disk image files themselves (as in a real PC)

**Virtual PC Configuration (.hemu files)**:

The `.hemu` project file format allows you to configure all aspects of the virtual PC. All fields except `version` and `system` are optional and will use defaults if not specified.

Example configuration file:
```json
{
  "version": 1,
  "system": "pc",
  "mounts": {
    "FloppyA": "dos622_boot.img",
    "HardDrive": "freedos.img"
  },
  "boot_priority": "FloppyFirst",
  "cpu_model": "Intel8086",
  "memory_kb": 640,
  "video_mode": "CGA"
}
```

**Configuration Options**:

- **`cpu_model`** (optional, default: "Intel8086")
  - Valid values: `"Intel8086"`, `"Intel8088"`, `"Intel80186"`, `"Intel80188"`, `"Intel80286"`, `"Intel80386"`
  - Controls which CPU instruction set is available
  - Intel8086/8088: Original IBM PC/XT instruction set
  - Intel80186/80188: Adds PUSHA/POPA, BOUND, IMUL immediate, etc.
  - Intel80286: Adds protected mode instruction stubs
  - Intel80386: Adds 32-bit operations (MOVSX, MOVZX, BSF, BSR, etc.)

- **`memory_kb`** (optional, default: 640)
  - Specifies total system memory in KB
  - If memory_kb <= 640: All memory is conventional (PC/XT compatible)
  - If memory_kb > 640: 640KB conventional + remainder as extended memory
  - Minimum: 256KB (conventional only)
  - No maximum for total memory (extended memory can be very large)
  - Common configurations:
    - 256KB: Early PC/XT (all conventional)
    - 512KB: Typical PC/XT (all conventional)
    - 640KB: Maximum conventional memory (DOS limit)
    - 1024KB (1MB): 640KB conventional + 384KB extended
    - 16384KB (16MB): 640KB conventional + 15.75MB extended
  - INT 12h BIOS call reports conventional memory (max 640KB)
  - INT 15h BIOS call (AH=88h) reports extended memory
  - Most DOS software requires at least 512KB conventional memory

- **`video_mode`** (optional, default: "CGA")
  - Valid values: `"CGA"`, `"EGA"`, `"VGA"`
  - **CGA** (Color Graphics Adapter):
    - Text mode: 80x25 characters (640x400 pixels)
    - Graphics modes: 320x200 4-color, 640x200 2-color
    - 16-color fixed palette
  - **EGA** (Enhanced Graphics Adapter):
    - Text mode: 80x25 characters (640x350 pixels, 8x14 font)
    - Graphics modes: 640x350 16-color, 320x200 16-color
    - 64-color palette (6-bit RGB), 16 active colors
  - **VGA** (Video Graphics Array):
    - Text mode: 80x25 characters (720x400 pixels, 9x16 font)
    - Graphics modes: 320x200 256-color (Mode 13h), 640x480 16-color
    - 256-color palette (18-bit RGB)

- **`boot_priority`** (optional, default: "FloppyFirst")
  - Valid values: `"FloppyFirst"`, `"HardDriveFirst"`, `"FloppyOnly"`, `"HardDriveOnly"`
  - Controls the boot device order
  - FloppyFirst: Try floppy A first, then hard drive C (default)
  - HardDriveFirst: Try hard drive C first, then floppy A
  - FloppyOnly: Only boot from floppy A
  - HardDriveOnly: Only boot from hard drive C

**Creating .hemu Files**:

1. **Manual Creation**: Create a text file with the JSON structure above
2. **Save from GUI**: Use File → Save Project while running a PC system to save current configuration
3. **Edit Existing**: Open any .hemu file in a text editor and modify the settings

**Loading .hemu Files**:

1. Use File → Open Project in the emulator
2. Select your `.hemu` file
3. All disks will be mounted and configuration will be applied
4. System will reset and boot with the configured settings


**Mount Point Usage**:

There are two ways to mount disk images and BIOS:

1. **GUI Method** (File → Mount Points):
   - Use File → Mount Points to open mount point selector
   - Select the desired slot (BIOS, FloppyA, FloppyB, or HardDrive)
   - Choose the file to mount

2. **Command-Line Method** (Recommended for quick loading):
   - Use `--slot1` through `--slot4` to load files directly
   - See "Advanced Command-Line Options" section for examples
   - Example: `./hemu --slot2 boot.img --slot4 hdd.img`

**Creating Disk Images**:
- Use `--create-blank-disk <path> <format>` to create blank disks
- See "Advanced Command-Line Options" section for supported formats
- Example: `./hemu --create-blank-disk floppy.img 1.44m`

**Keyboard Input**:
- All keyboard keys are passed through to the emulated PC
- Use **Right Ctrl** as host modifier to access emulator function keys (F1-F12)
- See "PC/DOS Keyboard Input" section for details

**Known Limitations**:
- **BIOS Interrupts**: 
  - INT 10h (Video): Extensive implementation with teletype, cursor control, scrolling, character I/O (video mode switching acknowledged but not functional)
  - INT 13h (Disk): **FULLY IMPLEMENTED** ✅ - All standard and extended functions work
    - AH=00h (Reset), AH=01h (Get Status), AH=02h (Read), AH=03h (Write)
    - AH=04h (Verify), AH=05h (Format), AH=08h (Get Params)
    - AH=15h (Get Disk Type), AH=16h (Change Status), AH=41h (Check Extensions)
    - **Supports count=0 reads/writes** (used by DOS to check disk readiness) ✅
  - INT 15h (Extended Services): **Core functions implemented** ✅
    - AH=88h (Get Extended Memory), AH=C0h (Get System Configuration) ✅
    - AH=E801h/E820h (Extended Memory Detection) ✅
    - AH=41h (Wait on External Event) - returns "not supported" ✅
  - INT 16h (Keyboard): All functions work - read keystroke, check keystroke, and get shift flags
  - INT 1Ah (Time/Date Services): **Time/Date and PCI BIOS functions implemented** ✅
    - AH=00h-05h (Time/Date): Read/Set system clock, RTC time/date ✅
    - AH=B1h (PCI BIOS): Returns "not present" for PC/XT (no PCI bus) ✅
      - AL=01h (Installation Check), AL=02h (Find Device), AL=03h (Find Class)
      - AL=08h-0Dh (Read/Write Configuration Space) all return appropriate errors
      - This allows CD-ROM drivers and other PCI-aware software to gracefully handle absence of PCI
  - INT 21h (DOS): **Use Real DOS for File Operations** 
    - Character I/O fully functional (AH=01h, 02h, 06h, 07h, 08h, 09h, 0Ah, 0Bh)
    - File I/O stubs present - fallback handler supports device names (CON, NUL, PRN, AUX, etc.) for standalone programs
    - **Device Support**: INT 21h AH=3Dh recognizes DOS device names (CON, NUL, PRN, AUX, COM1-4, LPT1-3) ✅
      - Returns appropriate standard file handles (0=stdin, 1=stdout, 3=stdaux, 4=stdprn)
      - Allows DOS CON driver initialization to succeed
    - **For filesystem access**: Boot real DOS from a disk image - DOS will handle FAT12/FAT16 filesystems
    - System functions (INT 21h AH=25h, 35h, 4Ch) are functional
  - INT 29h (Fast Console Output): **Implemented** ✅
    - Used by DOS for fast character output
    - Redirects to INT 10h teletype output for display
  - INT 2Fh (Multiplex): **Installation checks implemented** ✅
    - AH=11h (Network Redirector Check) - returns "not installed" ✅
    - AH=16h (DPMI), AH=43h (XMS) - installation checks functional
- **DOS Compatibility**: **Improved** ✅
  - **MS-DOS 3.3**: Now boots successfully with INT 15h AH=C0h support
  - **FreeDOS**: Boots successfully with reduced stub warnings (INT 2Fh AH=11h)
  - **MS-DOS 6.21**: Boots successfully with INT 13h count=0 support
  - DOS can detect system configuration and extended memory properly
  - Network redirector checks return proper "not installed" status
  - **CD-ROM Drivers**: PCI BIOS support (INT 1Ah AH=B1h) allows CD-ROM drivers to gracefully handle absence of CD-ROM hardware
    - Drivers like BANANA will detect "no PCI bus" and skip initialization without halting the boot process
- **DOS Filesystem Support**:
  - ✅ **Full filesystem support available via real DOS - FAT12, FAT16, and FAT32**
  - Mount a DOS boot disk (.img file with DOS installed)
  - DOS boots and provides INT 21h file services using its own FAT12/FAT16/FAT32 code
  - INT 13h provides complete sector-level access to all mounted disk images (both CHS and LBA)
  - **Extended INT 13h (EDD) support enables FAT32 and large disk support (>8GB)**
  - DOS can read, write, create, delete files on mounted floppy and hard drive images
  - **Supports large drives**: LBA addressing allows drives up to 2TB (limited by 32-bit LBA)
  - **How to use**:
    1. Create or download a DOS boot disk image with FAT32 support (FreeDOS 1.2+, MS-DOS 7.1+, Windows 98 DOS)
    2. Mount it via `--slot2 dos_boot.img` or press F3 to mount to FloppyA
    3. Mount data disks (including large FAT32 drives) to FloppyB or HardDrive as needed
    4. Boot the system - DOS will load from the boot disk
    5. Use DOS commands (DIR, COPY, etc.) to access files on all mounted disks
    6. **FAT32 drives work if DOS supports FAT32** (FreeDOS, MS-DOS 7.x, Windows 95 OSR2+)
  - **Standalone COM/EXE programs**: Can run directly without DOS but have limited file I/O
- **Display**: CGA, EGA, and VGA adapters implemented with multiple modes
  - **CGA Support** (Color Graphics Adapter):
    - Text mode: 80x25 characters (640x400 pixels)
    - Graphics modes: 320x200 4-color, 640x200 2-color
    - 16-color fixed palette
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - **EGA Support** (Enhanced Graphics Adapter):
    - Text mode: 80x25 characters (640x350 pixels, 8x14 font)
    - Graphics modes: 640x350 16-color, 320x200 16-color
    - 64-color palette (6-bit RGB), 16 active colors
    - Planar memory organization (4 bit planes)
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - **VGA Support** (Video Graphics Array):
    - Text mode: 80x25 characters (720x400 pixels, 9x16 font)
    - Graphics modes: 320x200 256-color (Mode 13h), 640x480 16-color
    - 256-color palette (18-bit RGB: 6 bits per channel)
    - Mode 13h uses linear addressing (1 byte per pixel)
    - 640x480x16 uses planar memory (4 bit planes)
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - Future: Additional palettes, more VGA modes
- **Input**: Keyboard passthrough works with INT 16h integration
  - Keyboard controller implemented with scancode buffer
  - INT 16h keyboard services now read from keyboard controller
  - AH=00h (read keystroke) and AH=01h (check keystroke) functional
  - No mouse support
  - No serial/parallel port emulation
- **No audio**: PC speaker tone generation not connected (PIT channel 2 tracks frequency but audio output not implemented)
- **Timing**: Frame-based execution with PIT timer (INT 08h) - not cycle-accurate

