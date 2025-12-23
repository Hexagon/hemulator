# agents.md

Purpose: guidance for automated agents and maintainers about CI, formatting, and safety.

- **Keep track of known limitations**: Document known limitations and missing features in MANUAL.md under each system's "Known Limitations" section. When making changes related to a system, review and update its limitations list if any are fixed.
- **Project structure**: workspace with `crates/core`, `crates/systems/*`, and `crates/frontend/gui`.
  - **Binary**: The GUI crate builds as `hemu` (not `emu_gui`)
  - **CLI removed**: There is no CLI frontend, only the GUI
  - **Core architecture**: Reusable CPU implementations in `crates/core/` (e.g., `cpu_6502`)
- **Agent tasks**:
  - Run `cargo fmt` and `cargo clippy` on PRs.
  - Build the workspace (`cargo build --workspace`).
  - Run unit/integration tests (`cargo test`).
  - Optionally run benchmarks in a separate job.
- **Pre-commit checks** (REQUIRED before committing any code):
  1. **Formatting**: `cargo fmt --all -- --check` - Must pass with no diff
  2. **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings` - Must pass with no warnings
  3. **Build**: `cargo build --workspace` - Must compile successfully
  4. **Tests**: `cargo test --workspace` - All tests must pass
  - Run these checks in order and fix any issues before committing
  - If any check fails, fix the issues and re-run all checks
  - These same checks run in CI, so ensuring they pass locally prevents CI failures
- **Implementation philosophy**:
  - **Always prefer full, tested implementations** of each module/component, even if all parts aren't immediately used
  - **Especially important** when other not-yet-implemented systems will use the features
  - Example: Implement complete APU with all channels (pulse, triangle, noise, DMC) even if only pulse is currently used, because future systems will need the other channels
  - Incomplete implementations create technical debt and require refactoring later
  - Full implementations with comprehensive tests ensure robustness and reusability
- **Permissions & safety**:
  - Agents must not add or distribute ROMs or other copyrighted game data.
  - Agents may run tests that do not require ROMs; for ROM-based tests, maintainers must provide legal test ROMs off-repo.
  - **Exception**: Simple test ROMs created from scratch for smoke testing are allowed and required.
- **Test ROM requirements**:
  - **Every system MUST have a basic test ROM** in `test_roms/<system>/` for smoke testing.
  - Test ROMs must be minimal, created from scratch (not copyrighted), and built from assembly source.
  - Each test ROM directory must include:
    - Assembly source code (`.s`, `.asm`)
    - Build script (`build.sh`)
    - Built ROM file for CI/testing
  - Test ROMs should produce deterministic, verifiable output (e.g., known pixel pattern).
  - If implementing a new system, create a test ROM before adding smoke tests.
  - **Building test ROMs**:
    - NES: Use `cc65` (ca65 assembler, ld65 linker)
    - Game Boy: Use `rgbds` (rgbasm assembler, rgblink linker, rgbfix for header)
    - Atari 2600: Use `dasm` assembler
    - Install on Ubuntu: `sudo apt-get install cc65 dasm libpng-dev && git clone https://github.com/gbdev/rgbds.git && cd rgbds && make && sudo make install`
  - See `test_roms/README.md` for detailed instructions and specifications.
- **Smoke tests**:
  - Each system crate must include a smoke test using its test ROM.
  - Smoke tests verify basic functionality: ROM loading, execution, and frame rendering.
  - Tests should check frame dimensions and pixel data for expected patterns.
  - See existing smoke tests in `crates/systems/*/src/lib.rs` for examples.
- **Cross-platform notes**:
  - Frontend uses `minifb` and `rodio` which are cross-platform; CI should include at least Linux and Windows runners.
  - For macOS specifics, `rodio` may require additional CI setup; document platform checks in CI config.
- **When to notify maintainers**:
  - Failing build or tests, or lint errors.
  - Long-running benchmark jobs exceeding expected time.

## Architecture

### Core Module (`crates/core/`)

Contains reusable CPU implementations and common traits:

- **`cpu_6502`**: Complete MOS 6502 CPU implementation
  - Generic `Memory6502` trait for memory access
  - Full instruction set with all addressing modes
  - Comprehensive test coverage (12 unit tests)
  - Can be used by any system: NES, Atari 2600, Apple II, Commodore 64, etc.
  - Implementation includes:
    - All official 6502 opcodes
    - Accurate cycle counting
    - Hardware interrupt support (NMI, IRQ)
    - Page-wrap bug emulation (JMP indirect)
    - Stack operations
    - Status flags (N, V, B, D, I, Z, C)
  - `ArrayMemory` helper for testing and simple use cases

- **`cpu_8080`**: Intel 8080 CPU implementation
  - Generic `Memory8080` trait for memory access
  - Foundation for Z80 and Game Boy CPUs
  - Stub implementation with basic opcodes (LXI, MOV, XCHG, etc.)
  - I/O port support (IN/OUT instructions)
  - Flag register (S, Z, AC, P, C)
  - Can be used for: Space Invaders, CP/M systems, early arcade games

- **`cpu_z80`**: Zilog Z80 CPU implementation
  - Generic `MemoryZ80` trait for memory access
  - Extends 8080 with shadow registers and index registers
  - Stub implementation with Z80-specific features:
    - Shadow register set (AF', BC', DE', HL')
    - Index registers (IX, IY)
    - Interrupt vector (I) and memory refresh (R) registers
    - Multiple interrupt modes (IM 0, 1, 2)
  - Can be used for: Sega Master System, Game Gear, ZX Spectrum, MSX

- **`cpu_lr35902`**: Sharp LR35902 CPU implementation (Game Boy)
  - Generic `MemoryLr35902` trait for memory access
  - Z80-like CPU with Game Boy-specific modifications
  - Stub implementation with GB-specific features:
    - 8-bit registers: A, F, B, C, D, E, H, L (no shadow registers)
    - 16-bit registers: SP, PC
    - Flags: Z (Zero), N (Subtract), H (Half Carry), C (Carry)
    - IME (Interrupt Master Enable) flag
    - HALT and STOP instructions
    - Starts at PC=0x0100 (after boot ROM)
  - Used by: Game Boy, Game Boy Color, Game Boy Advance (in GB mode)

- **`cpu_65c816`**: WDC 65C816 CPU implementation (SNES)
  - Generic `Memory65c816` trait for memory access
  - 16-bit extension of the 6502
  - **256/256 opcodes implemented (100% COMPLETE!)**
  - Comprehensive test coverage (29 unit tests)
  - Can be used by: SNES, Apple IIGS, etc.
  - Implementation includes:
    - 16-bit accumulator (C) and index registers (X, Y)
    - 8/16-bit mode switching via status flags (m, x)
    - 24-bit address space (16MB via DBR, PBR)
    - Emulation mode for 6502 compatibility
    - Direct page register (D)
    - Stack pointer (S)
    - **ALL 256 opcodes** including BRK (software interrupt)
    - **Arithmetic instructions**: ADC, SBC with all addressing modes including long and stack-relative
    - **Logical instructions**: AND, ORA, EOR with all addressing modes including long and stack-relative
    - **Shift/rotate instructions**: ASL, LSR, ROL, ROR (accumulator AND memory modes: dp, abs, dp,X, abs,X)
    - **Load/store instructions**: LDA, LDX, LDY, STA, STX, STY with ALL addressing modes including dp,X, long, and stack-relative
    - **Transfer instructions**: TAX, TAY, TXA, TYA, TSX, TXS, TCD, TDC, TCS, TSC, TXY, TYX
    - **Increment/decrement**: INC, DEC (accumulator and memory with dp, abs, dp,X, abs,X modes), INX, INY, DEX, DEY
    - **Compare instructions**: CMP, CPX, CPY with all addressing modes including long and stack-relative
    - **BIT instruction**: Complete with all addressing modes (immediate, dp, abs, dp,X, abs,X)
    - **Branch instructions**: BCC, BCS, BEQ, BNE, BMI, BPL, BVC, BVS, BRA, BRL
    - **Stack instructions**: PHA, PLA, PHP, PLP, PHX, PLX, PHY, PLY, PHD, PLD, PHB, PLB, PHK, PEA, PEI, PER
    - **Jump instructions**: JMP (absolute, long, indirect variants), JSR (absolute, long, indirect,X), JSL, RTS, RTI, RTL
    - **65C816-specific**: XCE, REP, SEP, XBA, STZ, TSB, TRB, MVN, MVP, COP, WAI, STP, WDM, BRK
    - **Mode control**: XCE (emulation toggle), REP (reset status bits), SEP (set status bits)
    - **Status flag instructions**: CLC, SEC, CLI, SEI, CLV, CLD, SED
    - **Status flags**: N, V, m, x, D, I, Z, C, e
  - **Production-ready**: Complete implementation with no known limitations
  - `ArrayMemory` helper for testing (16MB address space)

- **`cpu_mips_r4300i`**: MIPS R4300i CPU implementation (N64)
  - Generic `MemoryMips` trait for memory access
  - 64-bit MIPS III RISC processor
  - Complete instruction set implementation
  - Comprehensive test coverage (47 unit tests)
  - Used by: Nintendo 64
  - Implementation includes:
    - 32 general-purpose 64-bit registers (R0-R31, R0 always zero)
    - HI/LO registers for multiply/divide
    - CP0 coprocessor registers (system control)
    - Floating-point registers (FPR) and FCR31 control register
    - **Complete R-type instructions (SPECIAL opcode 0x00)**:
      - Shift operations: SLL, SRL, SRA, SLLV, SRLV, SRAV
      - 64-bit shifts: DSLL, DSRL, DSRA, DSLLV, DSRLV, DSRAV, DSLL32, DSRL32, DSRA32
      - Jump operations: JR, JALR
      - Move operations: MFHI, MTHI, MFLO, MTLO
      - Multiply/Divide: MULT, MULTU, DIV, DIVU
      - 64-bit multiply/divide: DMULT, DMULTU, DDIV, DDIVU
      - Arithmetic: ADD, ADDU, SUB, SUBU, DADD, DADDU, DSUB, DSUBU
      - Logical: AND, OR, XOR, NOR
      - Compare: SLT, SLTU
    - **Complete I-type instructions**:
      - Arithmetic immediate: ADDI, ADDIU, SLTI, SLTIU, DADDI, DADDIU
      - Logical immediate: ANDI, ORI, XORI
      - Load operations: LB, LBU, LH, LHU, LW, LWU, LD, LWL, LWR, LDL, LDR
      - Store operations: SB, SH, SW, SD, SWL, SWR, SDL, SDR
      - Branch operations: BEQ, BNE, BLEZ, BGTZ, BEQL, BNEL, BLEZL, BGTZL
    - **REGIMM instructions (opcode 0x01)**:
      - BLTZ, BGEZ, BLTZL, BGEZL, BLTZAL, BGEZAL, BLTZALL, BGEZALL
    - **J-type instructions**:
      - J (Jump), JAL (Jump and Link)
    - **COP0 (Coprocessor 0) instructions**:
      - MFC0, MTC0 (move to/from CP0 registers)
      - TLB instructions: TLBR, TLBWI, TLBWR, TLBP (basic stubs)
      - ERET (Exception Return)
    - **COP1 (FPU) instructions**:
      - MFC1, DMFC1, CFC1 (move from FPU)
      - MTC1, DMTC1, CTC1 (move to FPU)
      - Floating-point arithmetic: ADD.fmt, SUB.fmt, MUL.fmt, DIV.fmt
      - Floating-point operations: SQRT.fmt, ABS.fmt, MOV.fmt, NEG.fmt
      - Floating-point conversion: CVT.S, CVT.D, CVT.W, CVT.L
      - Floating-point compare: C.cond.fmt (sets condition bit)
      - Floating-point branch: BC1F, BC1T
    - CACHE instruction (NOP for basic emulation)
    - Big-endian memory access
  - Ready for use in Nintendo 64 emulation and other MIPS III systems
  - `ArrayMemory` helper for testing (8MB)

- **`cpu_8086`**: Intel 8086 CPU implementation with core instruction set
  - Generic `Memory8086` trait for memory access
  - Segment-based memory addressing (CS, DS, ES, SS)
  - Comprehensive test coverage (22 unit tests)
  - Can be used by any system: IBM PC, PC XT, etc.
  - Implementation includes:
    - All general-purpose registers (AX, BX, CX, DX, SI, DI, BP, SP)
    - Segment registers (CS, DS, ES, SS)
    - Instruction pointer (IP) and FLAGS register
    - Core instructions: MOV (immediate), arithmetic (ADD, SUB, CMP, INC, DEC), logical (AND, OR, XOR)
    - Control flow: JMP (short), conditional jumps (JZ, JNZ, JC, JNC)
    - Stack operations (PUSH, POP)
    - Flag manipulation (CLC, STC, CLI, STI, CLD, STD)
    - Accurate cycle counting
    - Parity, zero, sign, carry, and overflow flags
  - Ready for extension with additional instructions (ModR/M, multiply/divide, shifts, string operations, etc.)
  - `ArrayMemory` helper for testing and simple use cases

- **`apu`**: Reusable audio processing unit components
  - **Core Components** (building blocks for various systems):
    - `PulseChannel`: Square wave generator with duty cycle control (NES, Game Boy)
    - `TriangleChannel`: Triangle wave generator (32-step) (NES)
    - `WaveChannel`: Programmable waveform playback (Game Boy, custom waveform systems)
    - `NoiseChannel`: Pseudo-random noise with LFSR (NES, Game Boy)
    - `PolynomialCounter`: TIA-style waveform generation (Atari 2600)
    - `Envelope`: Volume envelope generator with decay (NES, Game Boy)
    - `LengthCounter`: Automatic note duration control (NES, Game Boy)
    - `SweepUnit`: Frequency sweep for pitch modulation (Game Boy)
    - `FrameCounter`: Timing controller for envelope/length/sweep units (NES, Game Boy)
  - **Audio Chip Implementations**:
    - `Rp2a03Apu`: NES NTSC audio chip (1.789773 MHz)
    - `Rp2a07Apu`: NES PAL audio chip (1.662607 MHz)
  - **AudioChip trait**: Common interface for pluggable audio systems
    - Allows different chips to be swapped (C64 SID, Atari 2600 TIA, ColecoVision SN76489, etc.)
    - Provides standard methods: `write_register`, `read_register`, `clock`, `reset`, `timing`
  - **Timing Support**:
    - `TimingMode` enum for NTSC/PAL configuration
    - CPU clock frequencies: NTSC 1.789773 MHz, PAL 1.662607 MHz
    - Frame rates: NTSC ~60.1 Hz, PAL ~50.0 Hz
    - Frame counter rates: NTSC 240 Hz, PAL 200 Hz
  - **Component Reusability**:
    - **For Game Boy APU**: Use `PulseChannel` (with `SweepUnit` for channel 1), `WaveChannel`, `NoiseChannel`, `Envelope`, `LengthCounter`
    - **For Atari 2600 TIA**: Use `PolynomialCounter` for both audio channels
    - **For future systems**: Mix and match components as needed (e.g., SN76489 can use `NoiseChannel`)
  - Comprehensive unit tests (48+ tests covering all components)

- **`graphics`**: Reusable graphics utilities and components
  - **ZBuffer (Depth Buffer)**: 16-bit depth buffer for hidden surface removal
    - Purpose: Stores depth information for each pixel to enable proper occlusion in 3D rendering
    - Key features:
      - Configurable resolution (width x height)
      - 16-bit depth values (0x0000 = near, 0xFFFF = far)
      - Enable/disable depth testing
      - Efficient `test_and_update()` method for read-modify-write operations
      - `clear()` to reset to far plane
      - `resize()` for dynamic resolution changes
    - Used by: N64 RDP (3D triangle rendering)
    - Can be used by: PlayStation, Saturn, or any 3D system
    - Comprehensive unit tests (10 tests)
  - **ColorOps**: Color manipulation utilities for ARGB8888 format
    - Purpose: Common color operations to reduce code duplication
    - Key functions:
      - `lerp()`: Linear interpolation between two colors (for Gouraud shading)
      - `adjust_brightness()`: Scale RGB channels by a factor (for lighting effects)
      - Component extraction: `red()`, `green()`, `blue()`, `alpha()`
      - Color construction: `from_argb()`, `from_rgb()`
    - Used by: N64 RDP (triangle rasterization, color interpolation)
    - Can be used by: Any system with color blending or interpolation needs
    - Comprehensive unit tests (5 tests)
  - **Design Philosophy**:
    - Provides performance-critical primitives with inline optimization
    - Stateless utility functions for easy reuse
    - Format-agnostic where possible (but optimized for ARGB8888)
    - Modular design allows systems to use only what they need

- **`ppu`**: Reusable video/graphics processing components
  - **Core Components** (building blocks for tile-based systems):
    - `IndexedPalette`: Generic indexed palette trait for color lookup systems
    - `RamPalette`: Simple RAM-based palette storage
    - `TileDecoder`: Trait for decoding tile/pattern data into pixel indices
    - `Nes2BppDecoder`: NES/Famicom 2bpp planar tile format
    - `GameBoy2BppDecoder`: Game Boy 2bpp interleaved tile format
    - `TileFormat`: Enum for different tile encoding formats
  - **Design Philosophy**:
    - Provides reusable primitives, not complete PPU implementations
    - Each system has unique register layouts, memory maps, and rendering pipelines
    - Systems like NES, Game Boy, SNES, Genesis share common concepts (tiles, palettes, sprites)
    - Core components reduce code duplication while allowing system-specific customization
  - **Future Formats**: SNES 4/8bpp, Genesis 4bpp linear (currently unimplemented)
  - Comprehensive unit tests (10+ tests)

- **`types`**: Common data structures (Frame, AudioSample)
- **`Cpu` trait**: Generic CPU interface
- **`System` trait**: High-level system interface

### System Modules (`crates/systems/`)

System-specific implementations that use core components. Current implementation status:

- ✅ **NES** - Fully working (~90% game coverage)
- ✅ **Atari 2600** - Fully working (complete TIA/RIOT/audio)
- ⚠️ **Game Boy** - Functional (graphics/input work, missing audio/timer)
- 🚧 **SNES** - Basic (CPU only, minimal PPU)
- 🚧 **N64** - In development (3D rendering functional)
- 🧪 **PC** - Experimental (COM/EXE loading only)

Detailed implementation notes:

- **NES (`emu_nes`)**: 
  - Uses `cpu_6502` from core with NES-specific bus implementation
  - `NesCpu` wraps `Cpu6502<NesMemory>` to provide NES-specific interface
  - `NesMemory` enum implements `Memory6502` trait for both simple array and full NES bus
  - NES bus includes: PPU, APU, controllers, mappers, RAM, WRAM
  - **PPU (Picture Processing Unit)**:
    - System-specific implementation in `crates/systems/nes/ppu.rs`
    - 2C02 PPU with 64-color master palette (NES-specific RGB values)
    - 32-byte palette RAM with NES-specific mirroring rules
    - 8KB CHR/pattern memory (ROM or RAM depending on cartridge)
    - 2KB internal VRAM for nametables with cartridge mirroring support
    - 256-byte OAM (Object Attribute Memory) for sprites
    - Background rendering with attribute table palette selection
    - Sprite rendering with 8x8 and 8x16 modes, priority, and flipping
    - Sprite 0 hit detection (basic implementation)
    - Sprite overflow detection (PPUSTATUS bit 5) with per-scanline evaluation
    - Scrolling support with nametable switching
    - **Timing Model**: Frame-based rendering (not cycle-accurate)
      - Renders complete 256x240 frames on-demand via `render_frame()`
      - Scanlines rendered incrementally via `render_scanline()` for mapper CHR switching
      - Per-scanline sprite evaluation to set sprite overflow flag
      - VBlank flag management for NMI generation
      - Suitable for most games; some games requiring precise PPU timing may not work perfectly
    - Could potentially use core `ppu` components (palette, tile decoder) in future refactoring
  - **PAL/NTSC Support**:
    - Auto-detection from iNES/NES 2.0 ROM headers
    - Timing-aware CPU cycles per frame (NTSC: ~29780, PAL: ~33247)
    - Timing-aware VBlank cycles (NTSC: 2500, PAL: 2798)
    - APU configured to match ROM timing mode
    - **PPU Timing Differences** (informational - not implemented in current frame-based model):
      - NTSC: 262 scanlines/frame, 341 PPU cycles/scanline, ~60.1 Hz
      - PAL: 312 scanlines/frame, 341 PPU cycles/scanline, ~50.0 Hz
      - Current implementation abstracts these differences at system level
  - **Mappers**:
    - System-specific implementations in `crates/systems/nes/mappers/`
    - Supported mappers: NROM (0), MMC1 (1), UxROM (2), CNROM (3), MMC3 (4), AxROM (7), MMC2 (9), MMC4 (10), ColorDreams (11), BNROM (34), GxROM (66), Camerica (71), NINA-03/06 (79), Namco 118 (206)
    - **Coverage**: ~90%+ of all NES games (based on nescartdb statistics)
    - **MMC2/MMC4 Latch Switching**: Fully implemented via CHR read callbacks
      - Hardware switches CHR banks when PPU reads from specific addresses ($0FD8, $0FE8, $1FD8-$1FDF, $1FE8-$1FEF)
      - PPU notifies mapper of CHR reads during frame rendering
      - Mapper tracks latch state changes and applies CHR bank updates after each frame
      - Frame-based rendering means updates happen per-frame, not mid-scanline
      - Games like Punch-Out!! and Fire Emblem should work correctly with per-frame latch switching
  - All existing tests pass (130 NES tests total: APU with sweep units, mapper, PPU, and system tests)

- **Game Boy (`emu_gb`)**: Functional implementation with PPU, joypad, and rendering
  - Uses `cpu_lr35902` from core with GB-specific memory bus
  - `GbSystem` integrates CPU with `GbBus` memory implementation
  - **Memory Bus** (`GbBus`):
    - 8KB Work RAM (WRAM) at $C000-$DFFF
    - 127 bytes High RAM (HRAM) at $FF80-$FFFE
    - Cartridge ROM support (32KB+ with banking)
    - Cartridge RAM support (size auto-detected from ROM header: 0KB, 8KB, 32KB, 64KB, 128KB)
    - I/O registers: Joypad ($FF00), Interrupt Flag ($FF0F), PPU registers, Boot ROM disable ($FF50)
    - Interrupt Enable ($FFFF) register
    - VRAM and OAM access delegated to PPU
  - **PPU (Picture Processing Unit)**:
    - System-specific implementation in `crates/systems/gb/ppu.rs`
    - Resolution: 160x144 pixels (DMG mode)
    - 8KB VRAM for tiles and tilemaps
    - 160-byte OAM for 40 sprites
    - **Tile System**:
      - 8x8 pixel tiles, 2 bits per pixel (4 colors)
      - Two tile data areas: $8000-$8FFF (unsigned), $8800-$97FF (signed)
      - Two tilemap areas: $9800-$9BFF, $9C00-$9FFF
    - **Rendering Features**:
      - Background layer with scrolling (SCX, SCY registers)
      - Window layer with independent positioning (WX, WY)
      - 40 sprites (8x8 or 8x16 pixels)
      - Sprite flipping (horizontal and vertical)
      - Sprite priority (above/behind background)
      - Palette support (BGP, OBP0, OBP1 for DMG)
    - **Timing Model**: Frame-based rendering (not cycle-accurate)
      - Renders complete 160x144 frames on-demand
      - Scanline counter (LY) updated during execution
      - LYC=LY coincidence detection
      - V-Blank detection when LY reaches 144
      - Suitable for homebrew and simple games
  - **Joypad Input**:
    - Matrix-based input system at $FF00
    - Button mode: Start, Select, B, A
    - Direction mode: Down, Up, Left, Right
    - Proper mode selection via bits 4-5
  - **Cartridge Support**:
    - ROM loading with header parsing
    - RAM size auto-detection from header
    - **Mappers (Memory Bank Controllers)**:
      - MBC0 (no mapper): 32KB ROM, no banking
      - MBC1: Most common mapper (~70% of games)
        - Up to 2MB ROM (128 banks)
        - Up to 32KB RAM (4 banks)
        - ROM/RAM banking modes
        - RAM enable control
      - MBC3: Popular for games with saves
        - Up to 2MB ROM (128 banks)
        - Up to 32KB RAM (4 banks)
        - RTC (Real-Time Clock) registers (stubbed - clock doesn't tick)
        - RAM enable control
      - MBC5: Advanced mapper for large ROMs
        - Up to 8MB ROM (512 banks)
        - Up to 128KB RAM (16 banks)
        - 9-bit ROM banking
        - RAM enable control
    - **Coverage**: Approximately 95%+ of Game Boy games supported
    - **Not yet implemented**: MBC2 (rare, ~1% of games with built-in 512×4 bits RAM)
  - **APU (Audio Processing Unit)**:
    - System-specific implementation in `crates/systems/gb/apu.rs`
    - Uses reusable components from `core/apu`
    - **4 Sound Channels**:
      1. **Pulse 1 (NR10-NR14)**: Square wave with sweep
         - Duty cycle: 12.5%, 25%, 50%, 75%
         - Frequency sweep (increase/decrease over time)
         - Envelope generator for volume control
         - Length counter for automatic duration
      2. **Pulse 2 (NR21-NR24)**: Square wave without sweep
         - Same as Pulse 1 but no sweep unit
      3. **Wave (NR30-NR34)**: Custom waveform
         - 32 x 4-bit samples in wave RAM ($FF30-$FF3F)
         - Volume control: mute, 100%, 50%, 25%
         - No envelope generator
         - Length counter
      4. **Noise (NR41-NR44)**: Pseudo-random noise
         - 7-bit or 15-bit LFSR modes
         - Envelope generator for volume control
         - Length counter
    - **Frame Sequencer**: 512 Hz timing controller
      - Clocks length counters at 256 Hz (every other step)
      - Clocks sweep at 128 Hz (steps 2 and 6)
      - Clocks envelopes at 64 Hz (step 7)
    - **Master Controls (NR50-NR52)**:
      - Volume control (left/right channels)
      - Sound panning per channel
      - Global power on/off
    - **Audio Output**: Ready for integration with frontend
      - 44.1 kHz sample rate
      - Mixes all 4 channels
      - Method `generate_samples()` available
    - **Registers**: $FF10-$FF26 (control), $FF30-$FF3F (wave RAM)
  - **Timing**:
    - 4.194304 MHz CPU clock
    - ~59.73 Hz frame rate
    - ~70,224 cycles per frame
    - 456 cycles per scanline
  - **Features**:
    - Full save state support (CPU registers, timing)
    - Cartridge mount/unmount
    - System reset
    - Controller input handling
    - Audio synthesis (not yet integrated with frontend)
  - **Known Limitations**:
    - DMG (original Game Boy) mode only - no Game Boy Color support
    - MBC2 mapper not implemented (rare, ~1% of games)
    - Audio output not yet connected to frontend (requires GUI audio integration)
    - No timer registers
    - No serial/link cable support
    - Frame-based timing (not cycle-accurate)
    - RTC in MBC3 doesn't actually count time (registers are accessible but static)
  - All tests pass (68 tests: 13 PPU, 7 APU, 7 system, 41 mapper tests)

- **Atari 2600 (`emu_atari2600`)**: 
  - Uses `cpu_6502` from core with Atari 2600-specific bus implementation (6507 variant)
  - `Atari2600Cpu` wraps `Cpu6502<Atari2600Bus>` to provide system-specific interface
  - Atari 2600 bus includes: TIA (video/audio), RIOT (RAM/I/O/timer), cartridge
  - **TIA (Television Interface Adapter)**:
    - System-specific implementation in `crates/systems/atari2600/tia.rs`
    - Resolution: 160x192 visible pixels (NTSC)
    - 128-color NTSC palette with proper hue/luminance mapping
    - **Graphics Objects**:
      - Playfield: 40-bit bitmap (20 bits × 2 halves) with mirror/repeat modes
        - Each bit controls 4 pixels (20 bits × 4 pixels = 80 pixels per half)
        - Left half: pixels 0-79, right half: pixels 80-159
        - PF0 uses bits 4-7, PF1 uses bits 0-7, PF2 uses bits 0-7
      - 2 Players (sprites): 8-pixel wide with reflection support
      - 2 Missiles: 1-pixel wide, share color with players
      - 1 Ball: 1-pixel wide, uses playfield color
    - Priority ordering: Playfield/Player/Missile/Ball/Background (configurable)
    - **Timing Model**: Frame-based rendering (not cycle-accurate)
      - Renders complete 160x192 frames on-demand
      - TIA state updated during CPU execution
      - Scanline states latched at scanline boundaries for accurate rendering
      - Suitable for most games; timing-critical effects may not work perfectly
    - **Audio Synthesis**: Complete implementation using `PolynomialCounter` from `core/apu`
      - 2 audio channels with polynomial waveform generation
      - Each channel has:
        - AUDC (4 bits): Waveform type selector (0-15 different waveforms)
        - AUDF (5 bits): Frequency divider (0-31)
        - AUDV (4 bits): Volume (0-15)
      - Waveform types include pure tones, buzzy sounds, white noise, and combinations
      - Polynomial counters (4-bit and 5-bit) create waveforms via LFSR feedback
      - Audio output integrated with frontend (44.1 kHz sampling)
      - See `crates/core/src/apu/polynomial.rs` for implementation details
    - **Known Limitations**:
      - Player/missile sizing (NUSIZ) stored but not applied
      - Horizontal motion (HMxx) stored but not applied (HMOVE strobe is supported)
      - Collision detection registers exist but return 0
      - Delayed graphics registers not implemented
  - **RIOT (6532 chip)**:
    - System-specific implementation in `crates/systems/atari2600/riot.rs`
    - 128 bytes of RAM with proper mirroring at $00-$7F, $80-$FF, $100-$17F
    - Programmable interval timer (1, 8, 64, 1024 clock intervals)
    - Timer underflow detection and interrupt flag
    - **IMPORTANT**: Reading TIMINT/INSTAT clears the interrupt flag (hardware side effect)
      - Critical for commercial ROMs that use timer-based frame synchronization
      - Games set timer → do work → wait for TIMINT flag → repeat
      - Flag auto-clears on read to enable detection of next expiration
    - I/O ports for joystick (SWCHA) and console switches (SWCHB)
    - Data direction registers (SWACNT, SWBCNT)
  - **Cartridge Banking**:
    - System-specific implementation in `crates/systems/atari2600/cartridge.rs`
    - Supports 2K, 4K (no banking), 8K (F8), 12K (FA), 16K (F6), 32K (F4)
    - Auto-detection based on ROM size
    - Bank switching via memory reads/writes to specific addresses
    - **Schemes**:
      - F8 (8K): 2 banks, switch at $1FF8-$1FF9
      - FA (12K): 3 banks, switch at $1FF8-$1FFA
      - F6 (16K): 4 banks, switch at $1FF6-$1FF9
      - F4 (32K): 8 banks, switch at $1FF4-$1FFB
  - **Timing**:
    - CPU: ~1.19 MHz (NTSC)
    - ~19,912 cycles per frame (~60 Hz)
    - 262 scanlines/frame, ~76 cycles/scanline
  - **Features**:
    - Full save state support (CPU, TIA, RIOT, cartridge banking)
    - Comprehensive test coverage (45 tests including checkerboard pattern validation)
    - Proper NTSC color palette
    - Accurate playfield rendering (4 pixels per bit)
    - Player/missile/ball rendering with priority
    - RIOT timer interrupt flag properly clears on read (fixes commercial ROM compatibility)
    - Scanline state latching for accurate VBLANK detection
  - **Test ROMs**:
    - `test.bin`: Basic smoke test with playfield pattern
    - `checkerboard.bin`: Validates alternating playfield patterns (vertical checkerboard)
    - `test_timer.bin`: Validates RIOT timer and color cycling
  - All existing tests pass (45 tests total: 14 TIA, 7 RIOT, 6 cartridge, 10 system, 4 bus, 2 CPU, 2 integration)

- **PC (`emu_pc`)**: Experimental IBM PC/XT emulation
  - Uses `cpu_8086` from core with PC-specific bus implementation
  - `PcCpu` wraps `Cpu8086<PcBus>` to provide PC-specific interface
  - PC bus includes: 640KB RAM, 128KB VRAM, 256KB ROM area
  - **Memory Map**:
    - 0x00000-0x9FFFF: Conventional memory (640KB)
    - 0xA0000-0xBFFFF: Video memory (128KB)
    - 0xC0000-0xFFFFF: ROM area (256KB, includes BIOS)
    - 0xF0000-0xFFFFF: BIOS ROM (64KB)
  - **BIOS**:
    - Minimal BIOS stub for booting DOS executables
    - Entry point at 0xFFFF:0x0000 (physical 0xFFFF0)
    - Initializes segments and stack
    - Jumps to loaded program at 0x0000:0x0100 (COM file convention)
  - **Executable Support**:
    - COM files: Loaded at 0x0100, limited to 64KB - 256 bytes
    - EXE files: MZ header detected but full parsing not yet implemented
  - **Timing**:
    - 4.77 MHz CPU clock (IBM PC standard)
    - ~79,500 cycles per frame at 60 Hz
  - **Display**:
    - 640x400 frame buffer (text mode 80x25 equivalent)
    - Currently renders black screen (video hardware not implemented)
  - All tests pass (22 tests total)

- **SNES (`emu_snes`)**: Basic implementation (functional PPU Mode 0)
  - Uses `cpu_65c816` from core with SNES-specific bus implementation
  - `SnesCpu` wraps `Cpu65c816<SnesBus>` to provide SNES-specific interface
  - SNES bus includes: 128KB WRAM, cartridge ROM/RAM, hardware registers
  - **CPU (65C816)**:
    - System-specific implementation uses core `cpu_65c816`
    - 16-bit processor with 8/16-bit mode switching
    - Registers: C (accumulator), X, Y, S (stack), D (direct page), DBR, PBR
    - Emulation mode for 6502 compatibility
    - Native mode for 16-bit operations
  - **Memory Bus** (`SnesBus`):
    - 128KB WRAM at banks $7E-$7F (full) and mirrors in $00-$3F, $80-$BF
    - Hardware registers at $2000-$5FFF (PPU registers functional)
    - Cartridge ROM mapped at $8000-$FFFF in various banks
    - LoROM mapping: ROM at banks $00-$7D, $80-$FF
  - **Cartridge Support**:
    - System-specific implementation in `crates/systems/snes/cartridge.rs`
    - SMC header detection (512 bytes) and automatic removal
    - LoROM mapping (basic)
    - 32KB SRAM support
  - **Timing**:
    - CPU: ~3.58 MHz (NTSC)
    - ~89,342 cycles per frame (~60 Hz)
  - **Features**:
    - Full save state support (CPU registers)
    - Cartridge mount/unmount
    - System reset
  - **Known Limitations**:
    - **PPU**: Mode 0 only (other modes 1-7 not implemented)
      - No sprites (OAM)
      - No scrolling (BG offset registers not implemented)
      - No windows or masks
      - No HDMA effects, mosaic, or color math
      - Only 32x32 tilemap size supported (64x32, 32x64, 64x64 not implemented)
    - **APU (SPC700)**: Not implemented - no audio
    - Controller support not implemented
    - Only LoROM mapping - no HiROM, ExHiROM
    - No enhancement chips (SuperFX, DSP, SA-1, etc.)
    - Frame-based timing (not cycle-accurate)
  - **PPU (Picture Processing Unit)**:
    - Implementation in `crates/systems/snes/ppu.rs`
    - VRAM access via registers $2116-$2119 (word-addressed, 64KB)
    - CGRAM (palette) access via $2121-$2122 (256 colors, 15-bit BGR format)
    - Screen enable/disable via $2100 (force blank + brightness control)
    - **Mode 0 Support** (4 BG layers, 2bpp each):
      - BG mode register ($2105 - BGMODE)
      - BG tilemap address registers ($2107-$210A - BG1SC-BG4SC)
      - BG CHR address registers ($210B-$210C - BG12NBA, BG34NBA)
      - Main screen designation ($212C - TM) for layer enable/disable
      - Proper tile rendering with 8x8 tiles, 2-bit color (4 colors per tile)
      - Tilemap attribute support: horizontal/vertical flip, palette selection (8 palettes × 4 colors)
      - Layer priority rendering (BG4 → BG3 → BG2 → BG1)
      - Transparent pixel handling (color 0 is transparent)
    - **NOT implemented**: Modes 1-7, sprites, scrolling, windows, effects
  - All tests pass (17 tests: 5 cartridge, 7 PPU, 5 system)

- **N64 (`emu_n64`)**: Basic implementation with RDP graphics
  - Uses `cpu_mips_r4300i` from core with N64-specific bus implementation
  - `N64Cpu` wraps `CpuMips<N64Bus>` to provide N64-specific interface
  - N64 bus includes: 4MB RDRAM, PIF RAM/ROM, SP memory, cartridge ROM, RDP registers
  - **CPU (MIPS R4300i)**:
    - System-specific implementation uses core `cpu_mips_r4300i`
    - 64-bit MIPS III processor with complete instruction set
    - 32 general-purpose registers, HI/LO, CP0 coprocessor
    - Full instruction set including:
      - All R-type arithmetic, logical, shift operations
      - All I-type load/store, immediate, branch operations
      - J-type jump instructions
      - 64-bit doubleword operations (DADD, DSUB, DMULT, DDIV, LD, SD, etc.)
      - Floating-point operations (FPU/COP1)
      - Coprocessor 0 operations (CP0)
  - **Memory Bus** (`N64Bus`):
    - 4MB RDRAM at 0x00000000-0x003FFFFF
    - 8KB SP DMEM/IMEM at 0x04000000-0x04001FFF (RSP memory)
    - RDP Command registers at 0x04100000-0x0410001F
    - 2KB PIF RAM at 0x1FC00000-0x1FC007FF (boot ROM area)
    - Cartridge ROM at 0x10000000-0x1FBFFFFF
    - Simple address translation (unmapped addresses)
  - **RDP (Reality Display Processor)**:
    - System-specific implementation in `crates/systems/n64/src/rdp.rs`
    - **Pluggable renderer architecture** via `RdpRenderer` trait:
      - `SoftwareRdpRenderer`: CPU-based rasterization (always available)
      - `OpenGLRdpRenderer`: GPU-accelerated rasterization (feature-gated, complete but not integrated)
    - **Uses modular components from `emu_core::graphics`**:
      - `ZBuffer`: Depth buffer for hidden surface removal
      - `ColorOps`: Color interpolation and brightness adjustment
    - Framebuffer support with configurable resolution (default 320x240)
    - **3D Triangle Rasterization** (both renderers):
      - Flat-shaded triangles (solid color)
      - Gouraud-shaded triangles (per-vertex color interpolation using `ColorOps::lerp`)
      - **Textured triangles** (with UV coordinate interpolation)
      - Z-buffered rendering (uses modular `ZBuffer` component)
      - Combined shading + Z-buffer support
      - Combined texture + Z-buffer support
      - Scanline-based edge walking algorithm (software)
      - GPU-accelerated rasterization via shaders (OpenGL)
      - Barycentric coordinate interpolation for attributes
    - **OpenGL Renderer Details** (when enabled with `--features opengl`):
      - OpenGL 3.3 Core Profile
      - FBO (Framebuffer Object) for offscreen rendering
      - Shader programs: `vertex.glsl`, `fragment_flat.glsl`, `fragment_gouraud.glsl`
      - Hardware depth testing for Z-buffer
      - Scissor test support
      - Pixel readback for Frame compatibility
      - **Not yet integrated**: Requires GL context from frontend (SDL2)
    - **Display List Commands** (wired to processor):
      - 0x08: Non-shaded triangle
      - 0x09: Non-shaded triangle with Z-buffer
      - 0x0A: Textured triangle (custom command for demo)
      - 0x0B: Textured triangle with Z-buffer (custom command for demo)
      - 0x0C: Shaded triangle (with Gouraud shading)
      - 0x0D: Shaded triangle with Z-buffer
      - 0x36: Fill rectangle
      - 0x37: Set fill color
      - 0x29: Sync full
    - **Rasterization Features**:
      - Scissor rectangle clipping (fully functional)
      - Per-pixel color interpolation (via `ColorOps`)
      - Per-pixel depth interpolation
      - **Per-pixel texture coordinate interpolation**
      - Edge function-based rasterization
    - Basic fill operations (clear, fill rectangle, set pixel)
    - Memory-mapped register interface (DPC_START, DPC_END, DPC_STATUS, etc.)
    - Display list command processing for fill and triangle operations
    - Color format support (RGBA5551, RGBA8888, internally uses ARGB)
    - **Texture Support**:
      - 4KB TMEM (Texture Memory) for texture storage
      - 8 tile descriptors for texture configuration
      - LOAD_BLOCK and LOAD_TILE commands for texture loading
      - SET_TEXTURE_IMAGE to specify texture source
      - SET_TILE to configure tile format (RGBA16, RGBA32)
      - Texture sampling with wrapping/clamping support
      - **Ready for textured triangle rendering**
    - **Timing Model**: Frame-based rendering (not cycle-accurate)
      - Maintains framebuffer for frame generation
      - Registers accessible via memory-mapped I/O
      - Suitable for 3D textured rendering with depth testing
  - **Cartridge Support**:
    - System-specific implementation in `crates/systems/n64/cartridge.rs`
    - Automatic byte-order detection (Z64/N64/V64 formats)
    - Conversion to big-endian for emulation
    - Magic byte validation: 0x80371240
  - **Timing**:
    - CPU: 93.75 MHz
    - ~1,562,500 cycles per frame (~60 Hz)
  - **Features**:
    - Full save state support (CPU registers, GPRs)
    - Cartridge mount/unmount
    - System reset
    - RDP framebuffer rendering with 3D triangle support and Z-buffer
  - **Known Limitations**:
    - **RSP (Reality Signal Processor)**: 
      - High-Level Emulation (HLE) implemented
      - Microcode detection working (F3DEX, F3DEX2, Audio)
      - Vertex buffer and transformation infrastructure complete
      - Display list parsing fully implemented (F3DEX commands)
      - Full matrix transformation pipeline with 10-level stack (G_MTX, G_POPMTX)
      - Conditional branching support (G_BRANCH_Z)
      - Vertex transformation and RDP command generation working
      - Triangle rendering commands (G_TRI1, G_TRI2, G_QUAD) operational
    - **RDP Graphics**: 
      - OpenGL renderer available with `--features opengl` but not yet integrated
      - **Textured triangle rendering fully implemented**:
        - Texture sampling for RGBA16/RGBA32 formats
        - UV coordinate interpolation across triangle surfaces
        - Combined texture + Z-buffer rendering
        - RDP commands 0x0A and 0x0B for textured triangles
      - **Texture mapping (TMEM loading) fully implemented**:
        - LOAD_BLOCK and LOAD_TILE commands working
        - 4KB TMEM with proper tile descriptor management
        - Texture wrapping and clamping support
      - No perspective-correct rasterization
      - No anti-aliasing or blending
      - No sub-pixel accuracy
      - No edge AA or coverage calculation
    - **System**:
      - Can render transformed 3D graphics with vertex colors and textures
      - Controller support implemented but needs frontend integration
        - All 14 buttons defined (A, B, Z, Start, D-pad, L, R, C-buttons)
        - Analog stick support complete (-128 to 127 range)
        - PIF command protocol functional
        - Frontend keyboard/gamepad mapping not yet connected
      - No TLB, cache, or accurate memory timing
      - Exception handling not fully implemented (no traps on overflow)
      - Frame-based timing (not cycle-accurate)
      - Audio not implemented - silent gameplay
  - All tests pass (118 tests passing, 1 ignored: cartridge, RDP with 3D textured rendering, display list commands, texture loading/sampling, RSP HLE with matrix stack and branching, PIF/controllers, VI, system integration)


### Frontend (`crates/frontend/gui`)

GUI frontend using minifb and rodio.

#### Video Processing System

The frontend includes a modular video processing architecture that supports multiple rendering backends through the `VideoProcessor` trait.

**Architecture Overview:**
- **Location**: `crates/frontend/gui/src/video_processor/`
- **Abstraction**: `VideoProcessor` trait defines common interface for all backends
- **Backends**:
  1. **SoftwareProcessor** (default): CPU-based rendering, always available
  2. **OpenGLProcessor** (optional): GPU-accelerated rendering with shader support

**VideoProcessor Trait:**
```rust
pub trait VideoProcessor {
    fn init(&mut self, width: usize, height: usize) -> VideoResult<()>;
    fn process_frame(&mut self, buffer: &[u32], width: usize, height: usize, filter: CrtFilter) -> VideoResult<Vec<u32>>;
    fn resize(&mut self, width: usize, height: usize) -> VideoResult<()>;
    fn name(&self) -> &str;
    fn is_hardware_accelerated(&self) -> bool;
}
```

**Software Backend (`mod.rs`):**
- Uses existing CPU-based CRT filter implementation from `crt_filter.rs`
- Clones input buffer and applies filters in-place
- No GPU dependencies, maximum compatibility
- Default choice for all builds

**OpenGL Backend (`opengl.rs`):**
- Requires feature flag: `--features opengl`
- Dependencies: `glow`, `glutin`, `glutin-winit`, `raw-window-handle`, `bytemuck`
- Shader-based CRT effects for better performance
- Dynamic shader compilation and switching based on active filter
- GLSL shaders stored in `src/shaders/`:
  - `vertex.glsl`: Fullscreen quad vertex shader
  - `fragment_none.glsl`: Passthrough (no filter)
  - `fragment_scanlines.glsl`: Scanline effect (darkens every other row)
  - `fragment_phosphor.glsl`: Horizontal phosphor glow
  - `fragment_crt.glsl`: Combined scanlines + phosphor + brightness boost

**Implementation Details:**
1. **Texture Management**: Converts ARGB to RGBA, uploads to GPU texture
2. **Shader Compilation**: Lazy compilation on filter change to save resources
3. **Uniform Handling**: Sets `uResolution` and `uTexture` uniforms
4. **Resource Cleanup**: Proper cleanup of GL resources in Drop implementation

**CRT Filter Effects (both backends):**
- **None**: Direct pixel passthrough
- **Scanlines**: Darkens every other scanline to 60% brightness
- **Phosphor**: Horizontal color bleeding (15% neighbor contribution)
- **CRT Monitor**: Combines scanlines (70% brightness) with phosphor, plus 5% brightness boost on bright lines

**Integration Points:**
- Settings: `video_backend` field in `config.json` ("software" or "opengl")
- Main loop: Process frame buffer before sending to minifb window
- Filter switching: F11 key cycles through CRT filters

**Testing:**
- Software backend: Comprehensive unit tests (creation, init, processing, filters)
- OpenGL backend: Compile-time tests via feature flags
- Both backends tested for build compatibility

**Future Enhancements:**
- Direct rendering to window (bypass minifb buffer copy)
- Additional shader effects (curvature, bloom, color correction)
- Runtime backend switching without restart
- Custom shader loading from configuration

## Audio Implementation Guidelines

When implementing audio for a new system or enhancing existing audio:

### Component Selection

1. **Identify the audio hardware**: Research the system's audio chip specifications
   - Number of channels
   - Waveform types (pulse/square, triangle, noise, wave, custom)
   - Frequency control mechanism
   - Volume/envelope control
   - Special features (sweep, programmable waveforms, etc.)

2. **Select reusable components** from `crates/core/src/apu/`:
   - **PulseChannel**: For square/pulse waves (NES, Game Boy, potentially others)
   - **TriangleChannel**: For triangle waves (NES-specific, 32-step quantized)
   - **WaveChannel**: For programmable waveforms (Game Boy wave channel)
   - **NoiseChannel**: For LFSR-based noise (NES, Game Boy, can be adapted)
   - **PolynomialCounter**: For complex waveform generation (Atari 2600 TIA)
   - **Envelope**: For volume decay/fade effects
   - **LengthCounter**: For automatic note duration
   - **SweepUnit**: For frequency modulation/sweep effects

3. **Create system-specific wrappers**: Most systems need a wrapper that:
   - Maps hardware registers to component parameters
   - Manages multiple channels
   - Handles mixing and output
   - Implements the `AudioChip` trait for integration

### Implementation Steps

1. **Create APU structure** in system crate (e.g., `crates/systems/gb/src/apu.rs`)
2. **Instantiate core components** for each channel
3. **Implement register I/O** to map system registers to component state
4. **Implement clock method** to generate audio samples
5. **Mix channels** and output to audio buffer
6. **Write comprehensive tests** for each register and channel interaction

### Testing Strategy

- Test each channel independently
- Test register writes affect correct parameters
- Test volume/envelope behavior
- Test frequency control accuracy
- Test interaction between components (e.g., length counter muting)
- Add integration tests with known waveforms

### Example: Game Boy APU Implementation

```rust
// In crates/systems/gb/src/apu.rs
use emu_core::apu::{PulseChannel, WaveChannel, NoiseChannel, SweepUnit, Envelope, LengthCounter};

pub struct GbApu {
    pulse1: PulseChannel,
    pulse1_sweep: SweepUnit,
    pulse2: PulseChannel,
    wave: WaveChannel,
    noise: NoiseChannel,
    // ... frame sequencer, mixing, etc.
}
```

### Common Pitfalls

- **Timing errors**: Ensure correct clock rate and sample generation rate
- **Register mapping**: Verify bit positions match hardware specifications
- **Mixing levels**: Balance channel volumes to avoid clipping or distortion
- **State management**: Save/restore APU state for save states
- **Test coverage**: Write tests before implementing to verify correctness

## Documentation Structure

All documentation is maintained in the root-level markdown files. **Do NOT create files in a `docs/` directory**.

- **README.md**: Developer-focused documentation
  - Building instructions and quick start
  - Project architecture overview
  - NES mapper support details
  - Supported ROM formats
  - Development and testing guidelines
  - Target audience: Contributors and developers setting up the project

- **MANUAL.md**: End-user manual
  - Included in all release packages
  - Getting started and controls
  - Configuration and settings
  - System-specific information (features, limitations, controls)
  - Troubleshooting and system requirements
  - **Update when**: Adding user-facing features, changing controls, or fixing system limitations
  - **Contains**: "Known Limitations" sections for each system - update these when fixing issues
  - Target audience: End users running the emulator

- **CONTRIBUTING.md**: Contribution guidelines
  - Pre-commit check requirements (fmt, clippy, build, test)
  - Development workflow and code quality standards
  - Debug environment variables
  - Areas for contribution
  - Target audience: External contributors

- **AGENTS.md**: This file - guidance for automated agents and CI
  - Project structure and architecture details
  - Implementation philosophy and best practices
  - Test ROM requirements and smoke testing
  - Audio/PPU implementation guidelines
  - Settings system and release packaging
  - Debug environment variables (comprehensive reference)
  - Target audience: Automated agents, CI systems, and maintainers

### Documentation Routing

When adding new documentation, use this routing guide:

| Content Type | File | Examples |
|--------------|------|----------|
| **User-facing features** | MANUAL.md | Controls, settings, filters, save states, troubleshooting |
| **System limitations (user impact)** | MANUAL.md | "Game X won't work because Y", "Feature Z not supported" |
| **Developer setup** | README.md | Build commands, quick start, project overview |
| **Architecture details** | AGENTS.md | System modules, CPU implementations, mapper internals |
| **Implementation guidelines** | AGENTS.md | How to add mappers, audio implementation patterns |
| **Contribution workflow** | CONTRIBUTING.md | Pre-commit checks, coding standards, PR process |
| **Debug tooling** | CONTRIBUTING.md | Basic debug variable usage for contributors |
| **Debug tooling (comprehensive)** | AGENTS.md | All debug variables with full context for agents |
| **CI/Agent instructions** | AGENTS.md | Test requirements, build artifacts, automation guidelines |

**Never create a `docs/` directory**. All documentation belongs in the four root-level files listed above.

## Release Packaging

When building release artifacts:
- **Include**: Executable (`hemu` or `hemu.exe`), `LICENSE`, `MANUAL.md`
- **Exclude**: All other files (source code, build artifacts, config files, saves)
- **Platforms**: Windows (.exe), Linux (binary + .deb package)
- **Architectures**: Both 64-bit (x86_64/amd64) and 32-bit (i686/i386)
- **Naming**: 
  - Windows 64-bit: `hemu-{version}-windows-x86_64.zip` containing `hemu.exe`, `LICENSE`, `MANUAL.md`
  - Windows 32-bit: `hemu-{version}-windows-i686.zip` containing `hemu.exe`, `LICENSE`, `MANUAL.md`
  - Linux 64-bit binary: `hemu-{version}-linux-x86_64.tar.gz` containing `hemu`, `LICENSE`, `MANUAL.md`
  - Linux 32-bit binary: `hemu-{version}-linux-i686.tar.gz` containing `hemu`, `LICENSE`, `MANUAL.md`
  - Debian package 64-bit: `hemu_{version}_amd64.deb` with proper packaging structure
  - Debian package 32-bit: `hemu_{version}_i386.deb` with proper packaging structure

## Settings System

The GUI frontend includes a comprehensive settings system stored in `config.json` in the executable directory.

### Settings Structure
- **Keyboard mappings**: Customizable button mappings for emulated controllers
  - Default: Z (A), X (B), LeftShift (Select), Enter (Start), Arrow keys (D-pad)
  - Settings automatically persist to disk on any change
- **Window size**: Actual window dimensions (width and height in pixels)
  - Automatically saved when window is resized
  - Default: 512x480 (2x scale of native 256x240 resolution)
- **Last ROM path**: Automatically remembered for quick restarts
- **Location**: `./config.json` (relative to executable, not working directory)

### ROM Loading

ROMs are auto-detected based on their format:
- **NES**: iNES format (header starts with `NES\x1A`)
- **Atari 2600**: Raw binary format, detected by size (2048, 4096, 8192, 12288, 16384, or 32768 bytes)
- **Game Boy**: GB/GBC format (Nintendo logo at offset 0x104)
- **PC/DOS**: MZ header for EXE files, or small binary files (16-65280 bytes) for COM files
- Unsupported formats show clear error messages

ROM loading workflow:
1. User opens ROM via F3 key or command-line argument
2. System detects ROM format automatically
3. Appropriate emulator core is selected (NES fully implemented, Atari 2600 core functional, GB is skeleton)
4. ROM hash is calculated for save state management
5. Last ROM path is saved to settings for auto-load on next start

### Save States

Save states are stored in `/saves/<rom_hash>/states.json` relative to the executable:
- **5 slots per game**: F5-F9 to save, Shift+F5-F9 to load
- **ROM hash-based organization**: Each ROM's states are in a separate directory
- **Base64 encoding**: State data is base64-encoded JSON
- **Automatic directory creation**: Save directories are created as needed
- **Instant persistence**: States are written immediately to disk

### Function Keys

- **F1**: Toggle help overlay (shows all controls)
- **F2**: Speed selector (0x, 0.5x, 1x, 2x, 4x)
- **F3**: Open ROM file dialog
- **F4**: Take screenshot (saved to screenshots/<system-name>/YYYYMMDDHHMMSSRRR.png)
- **F5**: Save state (opens slot selector)
- **F6**: Load state (opens slot selector)
- **F10**: Toggle debug overlay (shows ROM info, mapper, PAL/NTSC, FPS)
- **F11**: Cycle CRT filters (None → Scanlines → Phosphor → CRT Monitor → None)
- **F12**: Reset system
- **ESC**: Exit emulator

### CRT Filters

The GUI includes software-based CRT filters that can be toggled with F11:
- **None**: Raw pixel output
- **Scanlines**: Horizontal dark lines on odd rows (60% brightness)
- **Phosphor**: Horizontal color bleeding (15% blend with neighbors)
- **CRT Monitor**: Combined scanlines (70% darkness) + phosphor + brightness boost on even rows

Filters are applied to the frame buffer before display and do not affect overlays (help, debug, slot selector).
The selected filter persists in config.json.

### Default Screen

When no ROM is loaded or ROM fails to load, a default splash screen is displayed:
- Shows "HEMULATOR" logo
- Instructions: "Press F3 to open a ROM" and "Press F1 for help"
- Clean dark blue background with cyan/white text

## Debug Environment Variables

The emulator supports several environment variables for debugging. Set them to `1`, `true`, or `TRUE` to enable, or `0` (or any other value) to disable.

### Core (6502 CPU)

- **`EMU_LOG_UNKNOWN_OPS`**: Log unknown/unimplemented 6502 opcodes to stderr
  - Useful for finding missing CPU instruction implementations
  - Applies to: NES, Atari 2600, and any other 6502-based systems
  - Example: `$env:EMU_LOG_UNKNOWN_OPS=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_LOG_BRK`**: Log BRK instruction execution with PC and status register
  - Shows when BRK is executed and where it jumps to (IRQ vector)
  - Helpful for debugging unexpected BRK loops or interrupt issues
  - Applies to: NES, Atari 2600, and any other 6502-based systems
  - Example: `$env:EMU_LOG_BRK=1; cargo run --release -- roms/nes/game.nes`

### NES-Specific

- **`EMU_LOG_PPU_WRITES`**: Log all PPU register writes
  - Shows when games write to PPU registers ($2000-$2007)
  - Useful for debugging graphics/rendering issues
  - Example: `$env:EMU_LOG_PPU_WRITES=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_LOG_IRQ`**: Log when IRQ interrupts are fired
  - Shows when mapper or APU IRQs are pending and triggered
  - Useful for debugging IRQ timing issues (e.g., MMC3 scanline counter)
  - Example: `$env:EMU_LOG_IRQ=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_TRACE_PC`**: Log program counter hotspots every 60 frames
  - Shows the top 3 most frequently executed addresses
  - Useful for performance profiling and finding infinite loops
  - Lower volume than full PC tracing
  - Example: `$env:EMU_TRACE_PC=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_TRACE_NES`**: Comprehensive NES system trace every 60 frames
  - Logs frame index, PC, CPU steps/cycles, IRQ/NMI counts, MMC3 A12 edges, PPU registers, and interrupt vectors
  - Useful for debugging complex system-level issues
  - High-level overview of NES state over time
  - Example: `$env:EMU_TRACE_NES=1; cargo run --release -- roms/nes/game.nes`

### Usage Examples

**PowerShell usage** (Windows):
```powershell
# Enable logs
$env:EMU_LOG_BRK=1; $env:EMU_LOG_IRQ=1; cargo run --release -- roms/nes/excitebike.nes

# Disable logs (set to 0 or unset)
$env:EMU_LOG_BRK=0; cargo run --release -- roms/nes/excitebike.nes
```

**Bash usage** (Linux/macOS):
```bash
# Enable logs
EMU_LOG_BRK=1 EMU_LOG_IRQ=1 cargo run --release -- roms/nes/excitebike.nes

# Disable logs (set to 0 or just don't set the variable)
EMU_LOG_BRK=0 cargo run --release -- roms/nes/excitebike.nes
```

**Note**: All environment variables accept `1`, `true`, or `TRUE` to enable. Any other value (including `0`) or an unset variable will disable the log.

