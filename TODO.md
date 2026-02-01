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

#### UI
- [ ] Make links in about tab work
- [ ] Check that áll systems has the enhanced debugger state

#### SNES
- [ ] **Upload Protocol Test**: Investigate SPC700 index echoing issue - `crates/systems/snes/src/lib.rs`
  - Test currently ignored - SPC700 not echoing indices during upload
- [ ] **Sample Interpolation**: Improve SPC700 sample quality - `crates/core/src/apu/spc700.rs`
  - Current: Basic interpolation
  - Needed: Proper sample interpolation for better audio quality

#### Chores
- [ ] Update dependencies (cargo)