## ToDo

This file tracks unimplemented features, stubs, and simplified implementations across the project. When adding TODOs, categorize them by priority:

- **Critical**: Blocking issues, security vulnerabilities, or crashes
- **High**: Major functionality gaps affecting compatibility or user experience
- **Medium**: Important features or optimizations
- **Low**: Nice-to-have features, minor improvements, or polish

### Critical

_None currently_

### High

#### SNES - Bus/Memory
- [ ] **CPU Halt During DMA**: Implement proper DMA CPU freeze - `crates/systems/snes/src/bus.rs:1419`
  - Current: DMA executes immediately (comment: "would halt the CPU")
  - Needed: Freeze CPU during DMA transfers for hardware-accurate timing
  - Impact: Game timing may be wrong if code relies on CPU being frozen
- [ ] **Cycle-Accurate DMA Timing**: Fix DMA cycle counting - `crates/systems/snes/src/bus.rs:440-520`
  - Current: Fixed cycles per transfer (8-16) instead of actual hardware timing
  - Needed: Account for address bus speed differences
  - Impact: Not cycle-accurate for timing-sensitive code
- [ ] **FastROM Timing**: Implement FastROM memory access timing - `crates/systems/snes/src/bus.rs:1388-1391`
  - Current: MEMSEL register ($420D) written but ignored
  - Needed: Faster access times for FastROM regions
  - Impact: Performance optimization for games using FastROM

#### SNES
- [ ] **Zero-Page Frame Counter Workaround**: Investigate NMI handler behavior - `crates/systems/snes/src/bus.rs:271-280`
  - Current: Manually incrementing $003F each frame as compatibility hack
  - Issue: Some ROMs (Bart's Nightmare) have NMI handlers that don't properly update frame counters
  - NMI handler at $00:870F starts with `PHP; REP #$30` but doesn't increment $3F
  - Workaround: Bus increments $3F during tick_frame() 
  - Needed: Determine why ROM's NMI handler doesn't work, compare with bsnes/higan
  - Impact: May affect other games expecting proper NMI handler behavior
- [ ] **DSP-1 Coprocessor**: Complete missing commands (Attitude/Target/Rotate) - `crates/systems/snes/src/coprocessors/dsp1.rs`
  - Attitude (0x08): Only partial rotation matrix implementation
  - Target (0x20): Not implemented - returns zeros
  - Rotate (0x24): Not implemented - returns zeros
  - Impact: Games using these commands may malfunction (Pilotwings, Super Mario Kart)

#### SNES Audio (DSP)
- [ ] **Gaussian Interpolation**: Replace linear interpolation with Gaussian filter - `crates/core/src/apu/dsp.rs`
  - Current: Linear interpolation (basic quality)
  - Needed: Gaussian filter for hardware-accurate audio
- [ ] **ADSR Envelope**: Implement full envelope with proper curves - `crates/core/src/apu/dsp.rs`
  - Current: Simplified envelope rates (not cycle-accurate)
  - Needed: Full ADSR implementation matching hardware timing
- [ ] **GAIN Modes**: Implement direct, linear increase/decrease, exponential - `crates/core/src/apu/dsp.rs`
  - Current: Stub implementation
  - Needed: All GAIN envelope modes

#### PC/DOS
- [ ] **PC Speaker Audio**: Connect PIT channel 2 to audio output - `crates/systems/pc/`
  - PIT tracks frequency/state but audio generation not connected to frontend
  - Impact: No sound output from DOS programs

#### Game Boy / Game Boy Color
- [x] **STAT Interrupt Blocking**: Implement edge-triggered STAT interrupts - `crates/systems/gb/src/ppu.rs` **COMPLETED**
  - Proper rising-edge detection for STAT interrupt line ✅
  - Multiple sources (Mode 0/1/2, LYC=LY) ORed together ✅
  - Interrupt only fires on low→high transition ✅
  - Reference: Pan Docs, SameBoy issue #91 ✅
  - Impact: Fixes timing-sensitive games like Worms Armageddon
- [x] **Boot ROM Integration**: Post-boot state application - `crates/systems/gb/src/` **COMPLETED**
  - Built-in DMG and CGB boot ROMs created ✅
  - External boot ROM loading support ✅
  - Post-boot hardware state definitions ✅
  - Integration with GbSystem completed ✅
  - Automatically applies post-boot state on reset ✅
  - CPU registers (A, F, B, C, D, E, H, L, SP, PC) initialized ✅
  - I/O registers (PPU, APU, Timer, Interrupts) initialized ✅
  - Impact: Proper hardware initialization for edge cases

### Medium

#### ColecoVision
- [x] **Z80 ROM Execution**: Debug why test ROM doesn't execute properly through BIOS - `crates/systems/colecovision/src/lib.rs`
  - **FIXED**: Created minimal test BIOS that properly jumps to cartridge ROM
  - Test ROM now executes correctly through BIOS initialization
  - Added `smoke_test_colecovision_with_rom_execution()` to verify full ROM execution
  - Test BIOS available at `test_roms/colecovision/test_bios.rom`

#### SG-1000
- [ ] **Test ROM**: Create basic test ROM for smoke testing - `test_roms/sg1000/`
  - Current: No test ROM exists (README mentions z80asm/SDCC for creating test ROMs)
  - Needed: Assembly-based test ROM demonstrating VDP and PSG functionality
  - Follow pattern from other systems (test_roms/README.md)
  - Impact: No automated verification of ROM loading and execution

#### SNES
- [x] ~~**Mosaic Effect**: Implement $2106 register - `crates/systems/snes/src/ppu.rs`~~ **COMPLETED**
  - Fully implemented with per-layer enable and configurable size (1x1 to 16x16)
  - Applied to all background layers and Mode 7
- [x] ~~**Hardware Multiply/Divide**: Implement $4202-$4206 registers - `crates/systems/snes/src/bus.rs`~~ **COMPLETED**
  - Full 8-bit multiplication with 16-bit result
  - Full 16-bit division with quotient and remainder
  - Proper divide-by-zero handling

#### Atari 2600
- [x] **Exotic Banking Schemes**: DPC, FE, 3F, E0 mappers implemented - `crates/systems/atari2600/src/cartridge.rs`
  - DPC (Pitfall II): Display Processor Chip ✅
  - FE (Decathlon): Write-based bank switching ✅
  - 3F (Espial): RAM-based banking ✅
  - E0 (Parker Bros): Multiple simultaneous banks ✅
  - Impact: Enables compatibility with specific commercial games (Pitfall II, Decathlon, Espial, Parker Bros titles)
- [x] **Paddle Controllers**: INPT0-INPT3 analog input implemented in TIA - `crates/systems/atari2600/src/tia.rs`
  - Hardware simulation complete with capacitor charging timing circuits ✅
  - Public API `set_paddle_position()` available in `Atari2600System` ✅
  - Impact: Hardware support complete, GUI integration needed for paddle games (Breakout, Kaboom!, Warlords)
  - Note: TODO was incorrectly listed as `riot.rs` but paddles are part of TIA hardware

#### PC/DOS
- [ ] **INT 21h DOS API**: Expand file I/O and DOS functions - `crates/systems/pc/src/cpu.rs`
  - Current: Character I/O works, file operations are stubs
  - Impact: DOS program compatibility
- [ ] **32-bit Support (80386+)**: Implement full 32-bit operations - `crates/core/src/cpu_8086.rs`
  - Register extension (EAX, EBX, etc.)
  - 32-bit addressing with SIB byte
  - 32-bit operand support
  - Extended instructions (MOVZX, MOVSX, SHLD/SHRD)

#### SNES Refactoring
- [ ] **Refactor PPU Rendering**: Use helper methods to reduce code duplication - `crates/systems/snes/src/ppu.rs`
  - `get_tile_color()` helper already exists
  - Apply to background and sprite rendering functions

### Low

#### SNES - Enhancement Chips
- [ ] **DSP-1 Full Hardware Accuracy**: Implement Parameter command and shared projection state - `crates/systems/snes/src/coprocessors/dsp1.rs`
  - Current: Simplified implementations of Attitude, Target, Rotate commands
  - Needed: Parameter command (0x02) to set up shared projection matrices and camera parameters
  - Impact: Current implementation sufficient for basic compatibility; full accuracy needed for advanced DSP-1 features
  - Note: Target and Attitude commands use simplified transformations without shared state
  - Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (parameter, target, attitude functions)

#### SNES
- [ ] **Upload Protocol Test**: Investigate SPC700 index echoing issue - `crates/systems/snes/src/lib.rs`
  - Test currently ignored - SPC700 not echoing indices during upload
- [ ] **Sample Interpolation**: Improve SPC700 sample quality - `crates/core/src/apu/spc700.rs`
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
- [ ] **RSP Microcode Commands**: Implement stubbed F3DEX commands - `crates/systems/n64/src/rsp_hle.rs`
  - G_MOVEWORD (0xDB) - stub at line 732
  - G_MOVEMEM (0xDC) - stub at line 857
  - G_SETOTHERMODE_L (0xE2) - stub at line 878
  - G_SETOTHERMODE_H (0xE3) - stub at line 895
  - Impact: Some games may not render correctly without these commands
- [ ] **Audio Microcode**: Implement RSP audio task processing - `crates/systems/n64/src/rsp_hle.rs:336`
  - Current: "Audio tasks not yet implemented"
  - Impact: No audio output from games
- [ ] **Save System**: Implement EEPROM/Flash/Controller Pak support - `crates/systems/n64/src/pif.rs`
  - Current: No save data persistence
  - Needed: EEPROM (4Kbit/16Kbit), Flash RAM, Controller Pak
  - Impact: Games cannot save progress
- [ ] **RDP SET_OTHER_MODES**: Implement rendering mode configuration - `crates/systems/n64/src/rdp.rs:1160`
  - Current: Logged as stub and ignored
  - Needed: Proper blend/combine mode application
  - Impact: Advanced graphics effects not working
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

