## ToDo

This file tracks unimplemented features, stubs, and simplified implementations across the project. When adding TODOs, categorize them by priority:

- **Critical**: Blocking issues, security vulnerabilities, or crashes
- **High**: Major functionality gaps affecting compatibility or user experience
- **Medium**: Important features or optimizations
- **Low**: Nice-to-have features, minor improvements, or polish

### Critical

**All critical items have been resolved!** The following items were previously marked as critical and have been completed:

#### SNES - Bus/Memory (Completed)
- [x] **CPU Halt During DMA**: Implement proper DMA CPU freeze - `crates/systems/snes/src/bus.rs:1507`, `crates/systems/snes/src/lib.rs:349-398`
  - **FIXED**: DMA now sets pending_dma_cycles which halts CPU execution
  - CPU execution loops check for pending DMA and consume cycles instead of executing instructions
  - Hardware-accurate timing where CPU is frozen during DMA transfers
- [x] **Cycle-Accurate DMA Timing**: Fix DMA cycle counting - `crates/systems/snes/src/bus.rs:604-665`
  - **VERIFIED**: Current implementation is hardware-accurate
  - 8 master cycles per byte transferred (correct for all DMA, even with FastROM carts)
  - 8 cycles overhead per channel
  - DMA always uses SlowROM speed regardless of MEMSEL setting

#### SNES Audio/DSP (Completed)
- [x] **Gaussian Interpolation**: Replace linear interpolation with Gaussian filter - `crates/core/src/apu/dsp.rs:60-129,429-465`
  - **FIXED**: Implemented hardware-accurate 4-point Gaussian interpolation
  - Added 512-entry Gaussian coefficient table matching SNES hardware
  - Uses pitch counter bits 4-11 to index filter coefficients
  - Significantly improves audio quality and hardware accuracy
- [x] **ADSR Envelope**: Implement full envelope with proper curves - `crates/core/src/apu/dsp.rs:253-295`
  - **FIXED**: Implemented hardware-accurate ADSR envelope with exponential curves (completed in this PR)
  - Added rate counter table with 32 entries for proper timing
  - Attack: Linear increase with hardware-accurate rate table
  - Decay: Exponential decrease to sustain level
  - Sustain: Exponential decrease towards zero (sustain rate)
  - Release: Fast exponential decrease (rate 31)
  - Reference: https://snes.nesdev.org/wiki/DSP_envelopes
- [x] **GAIN Modes**: Implement all 5 GAIN modes - `crates/core/src/apu/dsp.rs:254-258`
  - **FIXED**: Implemented all 5 GAIN modes (completed in this PR)
  - Direct mode (bit 7=0): bits 6-0 directly set envelope level
  - Linear decrease (bits 7-5=100): constant rate decrease
  - Exponential decrease (bits 7-5=101): exponential decay curve
  - Linear increase (bits 7-5=110): constant rate increase
  - Bent line increase (bits 7-5=111): two-slope increase (faster at higher values)
  - GAIN format: bit 7=mode (0=direct, 1=increase/decrease), bits 6-5=curve type, bits 4-0=rate
  - Reference: https://snes.nesdev.org/wiki/S-DSP_registers

#### ColecoVision (Completed)
- [x] **Z80 ROM Execution**: Debug why test ROM doesn't execute properly through BIOS - `crates/systems/colecovision/src/lib.rs`
  - **FIXED**: Created minimal test BIOS that properly jumps to cartridge ROM
  - Test ROM now executes correctly through BIOS initialization
  - Added `smoke_test_colecovision_with_rom_execution()` to verify full ROM execution
  - Test BIOS available at `test_roms/colecovision/test_bios.rom`

### High

#### SNES - Bus/Memory  
- [ ] **FastROM Timing**: Implement FastROM memory access timing - `crates/systems/snes/src/bus.rs:1482-1486`
  - Current: MEMSEL register ($420D) written but not applied to memory access timing
  - Needed: CPU memory accesses should be faster (6 cycles vs 8) for ROM in banks $80+ when MEMSEL bit 0 is set
  - Impact: Performance optimization for games using FastROM (affects CPU execution speed, not correctness)
  - Complexity: Requires CPU core modifications to vary cycle timing by memory region
  - Implementation approaches:
    1. Modify Memory65c816 trait to return (value, cycles) pairs
    2. Add per-instruction cycle adjustment based on accessed address ranges
    3. Track memory accesses in CPU core and adjust timing post-execution
  - Note: This is an optimization, not a correctness issue - all games work without it, just run slightly slower than hardware

### Medium

#### SG-1000
- [ ] **Test ROM**: Create basic test ROM for smoke testing - `test_roms/sg1000/`
  - Current: No test ROM exists (README mentions z80asm/SDCC for creating test ROMs)
  - Needed: Assembly-based test ROM demonstrating VDP and PSG functionality
  - Follow pattern from other systems (test_roms/README.md)
  - Impact: No automated verification of ROM loading and execution

#### PC/DOS
- [ ] **INT 21h DOS API**: Expand file I/O and DOS functions - `crates/systems/pc/src/cpu.rs`
  - Current: Character I/O works, file operations are stubs
  - Impact: DOS program compatibility
- [ ] **32-bit Support (80386+)**: Implement full 32-bit operations - `crates/core/src/cpu_8086.rs`
  - Register extension (EAX, EBX, etc.)
  - 32-bit addressing with SIB byte
  - 32-bit operand support
  - Extended instructions (MOVZX, MOVSX, SHLD/SHRD)
- [ ] **Protected Mode Instructions**: Complete stubbed 80286+ instructions - `crates/core/src/cpu_8086.rs`
  - INVLPG (Invalidate TLB Entry) - stub at line 3484: "No TLB implementation"
  - LAR (Load Access Rights) - stub at line 3506: "Set ZF=0 (invalid selector)"
  - LSL (Load Segment Limit) - stub at line 3528: "Set ZF=0 (invalid selector)"
  - VERR (Verify Segment for Reading) - stub at line 3590: "Set ZF=0 (segment not readable)"
  - VERW (Verify Segment for Writing) - stub at line 3599: "Set ZF=0 (segment not writable)"
  - SHLD (Double Precision Shift Left) - stub at lines 3881, 3893
  - SHRD (Double Precision Shift Right) - stub at lines 3907, 3919
  - Impact: Protected mode DOS extenders and DPMI applications

### Low

#### SNES - Enhancement Chips
- [ ] **DSP-1 Full Hardware Accuracy**: Implement Parameter command and shared projection state - `crates/systems/snes/src/coprocessors/dsp1.rs`
  - Current: Simplified implementations of Attitude, Target, Rotate commands
  - Needed: Parameter command (0x02) to set up shared projection matrices and camera parameters
  - Impact: Current implementation sufficient for basic compatibility; full accuracy needed for advanced DSP-1 features
  - Note: Target and Attitude commands use simplified transformations without shared state
  - Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (parameter, target, attitude functions)

#### SNES
- [ ] **Upload Protocol Test**: Investigate SPC700 index echoing issue - `crates/systems/snes/src/lib.rs:832`, `crates/systems/snes/src/bus.rs:1605`
  - Tests currently ignored - SPC700 not echoing indices during upload
  - Affects: `test_apu_upload_protocol` and `test_apu_ports_echo`
- [ ] **Sprite Rendering**: Fix sprite rendering issues - `crates/systems/snes/src/lib.rs:970`
  - Test ignored: `test_sprite_overflow_rom` - "SNES sprite rendering not fully implemented yet - sprites not showing up"
  - Impact: Sprite overflow detection not working
- [ ] **Sample Interpolation**: Improve SPC700 sample quality - `crates/core/src/apu/spc700.rs:638`
  - Current: Basic interpolation
  - Needed: Proper sample interpolation for better audio quality
  - Code comment: "TODO: Implement proper sample interpolation"
- [ ] **PPU Refactoring**: Use helper methods to reduce code duplication - `crates/systems/snes/src/ppu.rs:2157,2185`
  - Helper methods `get_tile_color()` exist but not yet applied to all rendering functions
  - Impact: Code maintainability and consistency
- [ ] **Hardware Registers $2000-$5FFF**: Implement stubbed hardware register range - `crates/systems/snes/src/bus.rs:1466`
  - Current: Stub that ignores writes to this range
  - Needed: Proper handling of expansion/hardware registers
  - Impact: Some hardware features may not work

#### N64
- [x] **RSP Microcode Commands**: Implement stubbed F3DEX commands - `crates/systems/n64/src/rsp_hle.rs`
  - **IMPLEMENTED**: G_MOVEWORD (0xDB) - Now handles G_MW_NUMLIGHT (0x02) and G_MW_SEGMENT (0x06)
    - G_MW_NUMLIGHT: Sets number of active lights for lighting calculations
    - G_MW_SEGMENT: Configures segment base addresses for segmented addressing
    - Other indices log as debug stubs (non-critical for basic rendering)
  - **PARTIALLY IMPLEMENTED**: G_MOVEMEM (0xDC) - Handles viewport and light loading
    - Viewport loading (offset 0x80) fully functional
    - Light loading (offsets 0x82-0x92) fully functional
    - Other MOVEMEM types logged as debug stubs
  - **IMPLEMENTED**: G_SETOTHERMODE_L (0xB2) - Stores lower other modes with bit masking
    - Properly applies shift/length/data to othermode_l register
    - Controls alpha compare, depth source, render mode settings
  - **IMPLEMENTED**: G_SETOTHERMODE_H (0xB3) - Stores upper other modes with bit masking
    - Properly applies shift/length/data to othermode_h register
    - Controls cycle type, texture filtering, dithering settings
  - Impact: Improved compatibility with games using these commands
- [ ] **Audio Microcode**: Implement RSP audio task processing - `crates/systems/n64/src/rsp_hle.rs:336`
  - Current: "Audio tasks not yet implemented"
  - Impact: No audio output from games
- [ ] **Save System**: Implement EEPROM/Flash/Controller Pak support - `crates/systems/n64/src/pif.rs`
  - Current: No save data persistence
  - Needed: EEPROM (4Kbit/16Kbit), Flash RAM, Controller Pak
  - Impact: Games cannot save progress
- [x] **RDP SET_OTHER_MODES**: Implement rendering mode configuration - `crates/systems/n64/src/rdp.rs:1160`
  - **IMPLEMENTED**: Now stores full 64-bit othermode value
  - Extracts and logs cycle type, texture filtering, alpha compare, and z-mode
  - Impact: RDP properly tracks rendering mode settings for future use
- [ ] **Texture Format Support**: Implement missing texture formats - `crates/systems/n64/src/rdp.rs:805`
  - Current: "Other formats not yet implemented - return white"
  - Impact: Some textures render as white instead of proper images

#### N64
- [ ] **RDP Performance Counters**: Implement RDP performance counters - `crates/systems/n64/src/rdp.rs`
  - DPC_CLOCK (clock counter) - returns 0
  - DPC_BUFBUSY (buffer busy counter) - returns 0
  - DPC_PIPEBUSY (pipe busy counter) - returns 0
  - DPC_TMEM (TMEM counter) - returns 0
  - Impact: Performance monitoring not available
- [ ] **RSP Semaphore**: Implement RSP semaphore register - `crates/systems/n64/src/rsp.rs:195`
  - Current: Always returns 0 (stub implementation)
  - Impact: Synchronization between CPU and RSP not working
- [ ] **RSP Signal Bits**: Implement signal bits (SIG0-SIG7) - `crates/systems/n64/src/rsp.rs:72-86`
  - Current: Marked as #[allow(dead_code)]
  - Impact: RSP-CPU communication signals not working
- [ ] **Memory Alignment Validation**: Add alignment checks for load/store - `crates/core/src/cpu_mips_r4300i.rs`
  - LH/SH: 2-byte aligned
  - LW/SW: 4-byte aligned
  - LD/SD: 8-byte aligned
  - Impact: Unaligned access currently not validated (most code is properly aligned)

#### MIPS R4300i
- [ ] **Arithmetic Overflow Traps**: Implement overflow exception handling - `crates/core/src/cpu_mips_r4300i.rs`
  - ADD/ADDI/SUB should trap on overflow (currently ignored)
  - DADD/DADDI/DSUB should trap on overflow (currently ignored)
  - Impact: Some overflow-checking code may not work correctly
- [ ] **Branch Delay Slot Nullification**: Implement nullify delay slot - `crates/core/src/cpu_mips_r4300i.rs`
  - Branch likely instructions have ND (nullify delay) bit not implemented
  - Impact: Minor timing differences in certain branch patterns

#### SG-1000
- [ ] **ROM Banking Support**: Implement memory banking for larger cartridges - `crates/systems/sg1000/src/bus.rs`
  - Current: Fixed 48KB ROM space (0x0000-0xBFFF)
  - Most SG-1000 games are under 48KB, so not critical
  - Impact: Cannot run games larger than 48KB (rare)
- [ ] **Controller API Refinement**: Add type-safe controller methods - `crates/systems/sg1000/src/system.rs`
  - Current: Generic `set_controller(port: u8, state: u8)` method
  - Consider: Explicit `set_controller1(state: u8)` and `set_controller2(state: u8)` methods
  - Follow ColecoVision pattern for consistency
  - Impact: Better API design and type safety

