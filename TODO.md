## ToDo

This file tracks unimplemented features, stubs, and simplified implementations across the project. When adding TODOs, categorize them by priority:

- **Critical**: Blocking issues, security vulnerabilities, or crashes
- **High**: Major functionality gaps affecting compatibility or user experience
- **Medium**: Important features or optimizations
- **Low**: Nice-to-have features, minor improvements, or polish

### Critical

_None currently_

### High

#### SNES
- [ ] **DSP-1 Coprocessor**: Complete missing commands (Attitude/Target/Rotate) - `crates/systems/snes/src/coprocessors/dsp1.rs:231,246,326,338`
  - Attitude (0x08): Only partial rotation matrix implementation (FIXME at line 231, TODO at line 246)
  - Target (0x20): Not implemented - returns zeros (FIXME at line 326)
  - Rotate (0x24): Not implemented - returns zeros (TODO at line 338)
  - Impact: Games using these commands may malfunction (Pilotwings, Super Mario Kart)

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
- [ ] **Z80 ROM Execution**: Debug why test ROM doesn't execute properly through BIOS - `crates/systems/colecovision/src/lib.rs`
  - Current: Smoke tests use manual VDP initialization instead of ROM execution
  - Test ROM exists (`test_roms/colecovision/test.col`) but doesn't render when executed
  - Impact: Full system integration not validated via ROM execution

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

#### Chores
- [ ] Update dependencies (cargo)