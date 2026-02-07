## ToDo

This file tracks unimplemented features, stubs, and simplified implementations across the project. When adding TODOs, categorize them by priority:

- **Critical**: Blocking issues, security vulnerabilities, or crashes
- **High**: Major functionality gaps affecting compatibility or user experience
- **Medium**: Important features or optimizations
- **Low**: Nice-to-have features, minor improvements, or polish

### Critical

#### SNES - Enhancement Chips
- [ ] **DSP-1 Coprocessor**: Complete missing commands (Attitude/Target/Rotate) - `crates/systems/snes/src/coprocessors/dsp1.rs:231,246,326,338`
  - Attitude (0x08): Only partial rotation matrix implementation (FIXME at line 231, TODO at line 246)
  - Target (0x20): Not implemented - returns zeros (FIXME at line 326)
  - Rotate (0x24): Not implemented - returns zeros (TODO at line 338)
  - Impact: Games using these commands may malfunction (Pilotwings, Super Mario Kart)

#### SNES - PPU Advanced Features
- [x] **H/V Counter Reading**: Implement beam position tracking - `crates/systems/snes/src/ppu.rs:1214-1218`
  - Added h_counter and v_counter fields to track beam position (0-339 H, 0-261 V)
  - Implemented $2137 (SLHV) register to latch current counter values
  - Implemented $213C (OPHCT) register read with low/high byte toggle
  - Implemented $213D (OPVCT) register read with low/high byte toggle
  - Added update_counters() method to PPU
  - Integrated counter updates in SNES step_frame loop during active display and HBlank
  - Impact: Games using beam position for raster effects now work properly
- [ ] **Interlace Mode**: Implement interlaced display modes - `crates/systems/snes/src/ppu.rs:1118-1122`
  - Current: $2133 SETINI register is a stub (stored but ignored)
  - Needed: Interlaced rendering support
  - Impact: Some games may rely on interlaced display

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

#### SNES Audio (DSP)
- [ ] **Gaussian Interpolation**: Replace linear interpolation with Gaussian filter - `crates/core/src/apu/dsp.rs:368`
  - Current: Linear interpolation (basic quality)
  - Needed: Gaussian filter for hardware-accurate audio
  - Code comment: "TODO: Implement Gaussian interpolation for hardware accuracy"
- [ ] **ADSR Envelope**: Implement full envelope with proper curves - `crates/core/src/apu/dsp.rs:187`
  - Current: Simplified envelope rates (not cycle-accurate)
  - Needed: Full ADSR implementation matching hardware timing
  - Code comment: "TODO: Implement full ADSR envelope"
- [ ] **GAIN Modes**: Implement direct, linear increase/decrease, exponential - `crates/core/src/apu/dsp.rs:182`
  - Current: Stub implementation
  - Needed: All GAIN envelope modes
  - Code comment: "TODO: Implement GAIN modes (direct, linear increase/decrease, exponential)"

#### PC/DOS
- [x] **PC Speaker Audio**: Connect PIT channel 2 to audio output - `crates/systems/pc/`
  - Implemented get_audio_samples() method in PcSystem
  - Generates square wave audio based on PIT channel 2 frequency and output state
  - Respects speaker gate enable/disable from port 0x61
  - Added speaker_gate_enabled() getter method to PcBus
  - Connected to frontend audio system
  - Impact: PC speaker audio now functional for DOS programs

#### ColecoVision  
- [ ] **Z80 ROM Execution**: Debug why test ROM doesn't execute properly through BIOS - `crates/systems/colecovision/src/lib.rs`
  - Current: Smoke tests use manual VDP initialization instead of ROM execution
  - Test ROM exists (`test_roms/colecovision/test.col`) but doesn't render when executed
  - Impact: Full system integration not validated via ROM execution

### Medium

#### SG-1000
- [ ] **Test ROM**: Create basic test ROM for smoke testing - `test_roms/sg1000/`
  - Current: No test ROM exists (README mentions z80asm/SDCC for creating test ROMs)
  - Needed: Assembly-based test ROM demonstrating VDP and PSG functionality
  - Follow pattern from other systems (test_roms/README.md)
  - Impact: No automated verification of ROM loading and execution
- [ ] **I/O Port Mirroring Tests**: Add unit tests for port mirroring behavior - `crates/systems/sg1000/src/bus.rs`
  - Current: I/O port mirroring implemented but not tested
  - PSG: All ports 0x40-0x7F mirror to same PSG
  - VDP Data: All even ports 0x80-0xFF
  - VDP Control: All odd ports 0x80-0xFF
  - Controllers: Ports 0xC0-0xFF (even=controller1, odd=controller2)
  - Impact: Verify hardware-accurate port decoding

#### ColecoVision
- [ ] **Test ROM**: Create basic test ROM for smoke testing - `test_roms/colecovision/`
  - Current: Test ROM exists but doesn't execute properly through BIOS
  - Smoke tests use manual VDP initialization instead of ROM execution
  - Impact: Full system integration not validated via ROM execution

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

#### COMPLETED ✅
- [x] **SNES H/V Timer IRQ**: Fully implemented (2026-02-05) - `crates/systems/snes/src/bus.rs:155-163`, `crates/systems/snes/src/lib.rs:347-387`
  - All 4 timer modes (off, H-only, V-only, HV) implemented
  - IRQ flag register ($4211) with read-and-clear behavior
  - Mode selection via $4200 bits 5-4
  - Comprehensive test coverage (4 new tests, all passing)
  - Impact: Essential for timing-sensitive games using raster effects
  - Reference: https://sneslab.net/wiki/H/V_Count_Timer

#### UI
- [x] **Make links in about tab work** - `crates/frontend/gui/src/window_backend/sdl2_egui_backend.rs:206-212`
  - Implemented egui OutputCommand::OpenUrl handling in end_frame()
  - Hyperlinks now properly open in default browser via open crate
  - Fixed: Previously platform_output.commands were ignored
  - Impact: All hyperlinks in About tab now functional
- [ ] Check that áll systems has the enhanced debugger state

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

#### Chores
- [ ] Update dependencies (cargo)