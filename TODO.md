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
- [ ] **Interlace Mode**: Implement interlaced display modes - `crates/systems/snes/src/ppu.rs:1118-1122`
  - Current: $2133 SETINI register is a stub (stored but ignored)
  - Needed: Interlaced rendering support
  - Impact: Some games may rely on interlaced display
- [ ] **H/V Counter Reading**: Implement beam position tracking - `crates/systems/snes/src/ppu.rs:1214-1218`
  - Current: $213C/$213D (OPHCT/OPVCT) always return 0
  - Needed: Return actual H/V counter values
  - Impact: Games scanning beam position won't work properly

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
- [ ] **PC Speaker Audio**: Connect PIT channel 2 to audio output - `crates/systems/pc/`
  - PIT tracks frequency/state but audio generation not connected to frontend
  - Impact: No sound output from DOS programs

#### ColecoVision
- [ ] **TMS9918A Sprite Collision Detection**: Fix sprite collision test - `crates/systems/colecovision/src/lib.rs:53`
  - Current: Test ignored after TMS9918A refactor
  - Needed: Update sprite collision detection to work with refactored VDP
  - Impact: Sprite collision flag not properly tested
- [ ] **TMS9918A Sprite Overflow**: Fix sprite overflow test - `crates/systems/colecovision/src/lib.rs:107`
  - Current: Test ignored after TMS9918A refactor
  - Needed: Update sprite overflow detection to work with refactored VDP
  - Impact: Sprite overflow flag not properly tested

### Medium

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
- [ ] Make links in about tab work
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
- [ ] **RDP Counters**: Implement RDP performance counters - `crates/systems/n64/src/rdp.rs`
  - DPC_CLOCK (clock counter) - returns 0
  - DPC_BUFBUSY (buffer busy counter) - returns 0
  - DPC_PIPEBUSY (pipe busy counter) - returns 0
  - DPC_TMEM (TMEM counter) - returns 0
  - Impact: Performance monitoring not available

#### MIPS R4300i
- [ ] **Arithmetic Overflow Traps**: Implement overflow exception handling - `crates/core/src/cpu_mips_r4300i.rs`
  - ADD/ADDI/SUB should trap on overflow (currently ignored)
  - DADD/DADDI/DSUB should trap on overflow (currently ignored)
  - Impact: Some overflow-checking code may not work correctly
- [ ] **Branch Delay Slot Nullification**: Implement nullify delay slot - `crates/core/src/cpu_mips_r4300i.rs`
  - Branch likely instructions have ND (nullify delay) bit not implemented
  - Impact: Minor timing differences in certain branch patterns

#### Atari 2600
- [ ] **Player/Missile Sizing**: Implement NUSIZ register - `crates/systems/atari2600/src/lib.rs`
  - Current: Only default 1x size supported
  - Needed: Multiple player copies, sizing, and missile width control
  - Impact: Games using sprite sizing/duplication may render incorrectly

#### Chores
- [ ] Update dependencies (cargo)