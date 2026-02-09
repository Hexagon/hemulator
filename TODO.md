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

#### NES (Nintendo Entertainment System) - Completed
- [x] **APU Non-Linear Mixing**: Implement hardware-accurate mixer impedance curves - `crates/systems/nes/src/apu.rs:609-880`
  - **FIXED**: Implemented hardware-accurate non-linear mixing formulas
  - Formula: pulse_out = 95.88 / (8128 / (pulse1 + pulse2) + 100)
  - Formula: tnd_out = 159.79 / (1 / (triangle/8227 + noise/12241 + dmc/22638) + 100)
  - Added high-pass filter to remove DC offset from mixer output
  - Impact: Audio quality now matches NES hardware mixing behavior
  - Reference: https://www.nesdev.org/wiki/APU_Mixer
- [x] **Tile Viewer CHR Data Clone Overhead**: Optimize CHR data access in tile viewer - `crates/systems/nes/src/lib.rs:167,343`
  - **FIXED**: Replaced `Vec<u8>` with `Rc<Vec<u8>>` for CHR data in TileViewerData
  - Eliminated ~240KB/sec memory allocation overhead at 30fps viewer refresh
  - CHR data now reference-counted instead of fully copied on each call
  - Impact: Significantly reduced memory allocation overhead in GUI inspector

**All other high priority items have been resolved!** The following items were previously marked as high and have been completed:

#### SMS (Sega Master System) - Completed
- [x] **VDP Default Initialization**: Fix VDP reset to enable Mode 4 by default - `crates/systems/sms/src/vdp.rs:1035-1051`
  - **FIXED**: VDP reset now sets Register 0 to 0x04 to enable Mode 4 by default
  - Previous issue: VDP reset set all registers to 0, leaving system in TMS mode with display blanked
  - Root cause: Register 0 bit 2 (M4) = 0 → system started in TMS mode instead of Mode 4
  - Solution: Register 0 now defaults to 0x04 on reset, matching real SMS hardware behavior
  - Impact: Real SMS ROMs now work without requiring explicit VDP Mode 4 initialization
  - Real SMS hardware defaults to Mode 4 enabled; games without BIOS expect this default state


### Medium

#### Game Boy (DMG)
- [ ] **Sprite Per-Scanline Limit**: Restore hardware-accurate 10-sprite limit for DMG - `crates/systems/gb/src/ppu.rs`
  - Current: DMG limit relaxed to 40 sprites per scanline for compatibility
  - Needed: Accurate OAM selection (10 sprites) with proper timing so games don't flicker
  - Impact: Fixes sprite flicker hacks while keeping correct hardware behavior

#### Debugger/Tracing System

- [x] **Read/Write Breakpoint Support**: Add memory access breakpoints to CLI - `crates/core/src/breakpoints.rs`, `crates/frontend/gui/src/main.rs`
  - **COMPLETED**: Added --read-breakpoint and --write-breakpoint CLI flags
  - Extended impl_breakpoint_methods! macro to support add_read_breakpoint and add_write_breakpoint
  - All systems with breakpoint_manager now support read/write breakpoints via CLI
  - Note: Actual memory access checking deferred (requires bus-level integration)
  - Impact: Users can now set read/write breakpoints, though checking requires future implementation
- [x] **GUI Breakpoint Management**: Add breakpoint UI to debug tab - `crates/frontend/gui/src/egui_ui/inspector_tabs.rs`
  - **COMPLETED**: Added collapsible breakpoint panel in debug tab
  - UI shows active breakpoints with address, type (Execute/Read/Write), and remove button
  - Address input field with type selector dropdown (Execute/Read/Write)
  - Add button to create new breakpoints
  - Status bar feedback for add/remove operations
  - Integrated with existing breakpoint manager
  - Supports all systems with breakpoint_manager (NES, GB, Atari2600, CHIP-8, SNES, N64, PC)
- [ ] **Performance Profiling Mode**: Add hotspot tracking - `crates/core/src/instruction_tracer.rs`
  - Current: Instruction tracer only records execution history
  - Needed: Track instruction frequency, execution time per address
  - Impact: Cannot identify performance bottlenecks in emulated code
  - Use case: Finding slow loops, optimizing emulated game code
- [ ] **Memory Watchpoints**: Add memory change detection in GUI - `crates/frontend/gui/src/egui_ui/inspector_tabs.rs`
  - Current: Memory viewer shows static snapshot only
  - Needed: Highlight changed memory addresses, watch specific ranges
  - Impact: Hard to track memory changes during debugging
  - Features: Color-coded changes, watch list, change history
- [ ] **Conditional Breakpoints**: Add expression-based breakpoints - `crates/core/src/breakpoints.rs`
  - Current: Breakpoints trigger on address only
  - Needed: Break when register/memory equals value (e.g., "PC=0x8000 && A=0x42")
  - Impact: Too many false breakpoint hits without conditions
  - Requires: Expression parser, state evaluation

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

**SG-1000 items completed in this PR:**

#### SG-1000 (Completed)
- [x] **Test ROM**: Create basic test ROM for smoke testing - `test_roms/sg1000/`
  - **COMPLETED**: Created Python-based test ROM generator following SMS/ColecoVision pattern
  - Test ROM displays checkerboard pattern using TMS9918A VDP in Graphics I mode
  - Added smoke test to `crates/systems/sg1000/src/lib.rs`
  - Built ROM: `test_roms/sg1000/test.sg` (32KB)
  - Impact: Automated verification of ROM loading and basic system functionality
- [x] **Controller API Refinement**: Add type-safe controller methods - `crates/systems/sg1000/src/system.rs`, `crates/systems/sg1000/src/bus.rs`
  - **COMPLETED**: Added explicit `set_controller1(state: u8)` and `set_controller2(state: u8)` methods
  - Follows ColecoVision pattern for consistency
  - Kept generic `set_controller(port, state)` method for backward compatibility
  - Impact: Better API design with type safety
#### NES (Nintendo Entertainment System)
- [ ] **Sprite 0 Hit Timing on Odd Frames**: Verify X position calculation during odd-frame skip - `crates/systems/nes/src/lib.rs:620-627,655-656`, `crates/systems/nes/src/ppu.rs:1235-1249`
  - Current: Sprite 0 hit X position is calculated during render_scanline() and triggered in tick() at dot = X + 2
  - Analysis: The current implementation appears correct - sprite 0 hit is pixel-based (0-255), not dot-based
  - Odd-frame skip affects when rendering happens (dot 0 vs dot 1) but not the sprite 0 hit X position
  - The hit trigger is based on PPU dot counter reaching hit_x + 2, which is independent of rendering timing
  - Status: No actual bug identified; current implementation handles odd/even frames correctly
  - Action: Needs real-world testing with games that use tight sprite 0 hit timing (e.g., split-screen effects)
  - Reference: https://www.nesdev.org/wiki/PPU_frame_timing#Odd_frames
- [x] **MMC3A Mapper Support**: Implement MMC3A variant IRQ behavior - `crates/systems/nes/src/mappers/mmc3.rs:35-36`
  - **FIXED**: Implemented MMC3A variant detection via iNES 2.0 submapper field
  - Submapper 1 = MMC3A (old IRQ behavior), others = MMC3B/C (new IRQ behavior)
  - MMC3A: IRQ triggers when counter==0, even after reload
  - MMC3B/C: IRQ triggers only when counter decrements to 0
  - All 255 NES tests passing
  - Reference: https://www.nesdev.org/wiki/MMC3#IRQ_Specifics
- [ ] **PPU A12 Edge Timing**: Refine A12 callback timing during rendering - `crates/systems/nes/src/ppu.rs:980,1103-1106`
  - Current: `suppress_a12` flag toggles A12 callbacks off during `render_scanline()`, re-enables in CHR fetch
  - Issue: May cause subtle timing issues for mappers relying on A12 edge detection at specific dot cycles
  - Impact: MMC3 IRQ timing could be slightly inaccurate during mid-scanline rendering
  - Note: Current implementation works for 99%+ of games, only affects edge cases
- [ ] **Synthetic A12 Edge Generation**: Review mapper A12 edge synthesis - `crates/systems/nes/src/bus.rs:109-114`
  - Current: `clock_mapper_a12_rising_edge()` synthesizes A12 transitions by forcing false→true
  - Issue: May not match actual PPU cycle-accurate edges
  - Impact: MMC3 games with complex scanline IRQ patterns may have inaccurate behavior
  - Note: Works for standard games, may affect advanced homebrew or edge cases
- [x] **First Frame Register Protection**: Validate PPUADDR/PPUSCROLL write protection - `crates/systems/nes/src/ppu.rs:539`
  - **VERIFIED**: Implementation is correct and comprehensive
  - Protected registers during first frame: PPUCTRL ($2000), PPUMASK ($2001), PPUSCROLL ($2005), PPUADDR ($2006)
  - PPUDATA ($2007) is read-protected, returns 0x00
  - Protection released at end of first VBlank (hardware-accurate)
  - Added comprehensive test: `test_nes_first_frame_register_protection` 
  - All 255 NES tests passing
  - Reference: problemkaputt.de everynes.htm - PPU Reset section
- [ ] **Mapper State Inspection**: Expose mapper registers in debugger - `crates/systems/nes/src/debugger.rs`
  - Current: Mapper number/name exposed but detailed state not available
  - Needed: Bank select registers, IRQ counter state, CHR latch state
  - Impact: Debugging mapper-related issues requires manual memory inspection
  - Use case: Investigating MMC3 IRQ glitches, verifying bank switching behavior

#### PC/DOS - Hardware Accuracy
- [ ] **INT 21h DOS API**: Expand file I/O and DOS functions - `crates/systems/pc/src/cpu.rs`
  - Current: Character I/O works, file operations are stubs
  - Impact: DOS program compatibility
- [ ] **BIOS Interrupt Stubs**: Complete stub implementations - `crates/systems/pc/src/cpu.rs`
  - INT 05h (Print Screen) - stub only (cpu.rs:820)
  - INT 09h (Keyboard IRQ) - partial stub (cpu.rs:900-920)
  - INT 1Ah AH=01h/03h/05h (RTC set) - read-only stubs (cpu.rs:1040-1080)
  - INT 14h, 17h, 18h, 19h, 1Bh, 1Ch, 2Ah - stub implementations
  - INT 2Fh (Multiplex) - mostly unimplemented (cpu.rs:1600+)
  - Impact: Many BIOS services non-functional, breaks some DOS programs
- [ ] **INT 08h Chaining**: Review INT 1Ch chain skipping logic - `crates/systems/pc/src/cpu.rs:900-920`
  - Current: Skips calling INT 1Ch if it points to default BIOS stub (F000:0040) as optimization
  - Risk: Could break programs expecting chaining behavior
  - Impact: Potential compatibility issue with DOS programs that rely on INT 1Ch
- [ ] **PIT Timer IRQ Generation**: Connect PIT to actual IRQ0 generation - `crates/systems/pc/src/pit.rs`
  - Current: PIT tracks state but doesn't trigger interrupts; INT 08h simulated elsewhere
  - Needed: PIT counter decrement should trigger IRQ0 when reaching zero
  - Impact: Timer behavior may not match hardware timing accurately
- [ ] **Disk CHS Parameter Validation**: Add CHS bounds checking - `crates/systems/pc/src/disk.rs:117`
  - Current: Only validates final offset, not cylinder/head limits
  - Needed: Validate cylinder < max_cylinders, head < max_heads against drive geometry
  - Impact: Invalid disk operations may succeed when they should fail
- [ ] **Disk Geometry Detection**: Parse BIOS Parameter Block instead of hardcoding - `crates/systems/pc/src/disk.rs:65`
  - Current: Assumes floppy is always 1.44MB (18 sectors/track)
  - Needed: Read BPB from boot sector to detect 360KB, 720KB, 1.2MB, 1.44MB formats
  - Impact: Cannot correctly read non-1.44MB floppy images
- [ ] **Video Memory Banking**: Implement EGA/VGA extended memory banking - `crates/systems/pc/src/bus.rs`
  - Current: Only 128KB video memory allocated (0xA0000-0xBFFFF)
  - Needed: Support for memory plane banking in EGA/VGA modes
  - Impact: Advanced EGA/VGA modes may not work correctly
- [ ] **VGA Register Documentation**: Document VGA register bit layouts - `crates/systems/pc/src/bus.rs`
  - Graphics controller registers (bus.rs:100) - no bit documentation
  - Sequencer registers (bus.rs:97) - no description
  - Attribute controller (bus.rs:101-102) - no bit layout
  - DAC state machine (bus.rs:87-88) - transitions not explained
  - Impact: Makes code maintenance and debugging difficult
- [ ] **32-bit Support (80386+)**: Implement full 32-bit operations - `crates/core/src/cpu_8086.rs`
  - Register extension (EAX, EBX, etc.)
  - 32-bit addressing with SIB byte
  - 32-bit operand support
  - Extended instructions (MOVZX, MOVSX, SHLD/SHRD)
  - Impact: Cannot run 32-bit protected mode DOS extenders
- [ ] **Protected Mode Instructions**: Complete stubbed 80286+ instructions - `crates/core/src/cpu_8086.rs`
  - INVLPG (Invalidate TLB Entry) - stub at line 3484: "No TLB implementation"
  - LAR (Load Access Rights) - stub at line 3506: "Set ZF=0 (invalid selector)"
  - LSL (Load Segment Limit) - stub at line 3528: "Set ZF=0 (invalid selector)"
  - VERR (Verify Segment for Reading) - stub at line 3590: "Set ZF=0 (segment not readable)"
  - VERW (Verify Segment for Writing) - stub at line 3599: "Set ZF=0 (segment not writable)"
  - SHLD (Double Precision Shift Left) - stub at lines 3881, 3893
  - SHRD (Double Precision Shift Right) - stub at lines 3907, 3919
  - Impact: Protected mode DOS extenders and DPMI applications won't work

#### PC/DOS - Timing Accuracy
- [ ] **Cycle-Accurate Timing**: Implement proper CPU cycle counting - `crates/systems/pc/src/bus.rs:265-266`
  - Current: Hardcoded 80,000 cycles/frame; I/O timing generic (10-8 cycles)
  - Needed: Scale cycles with actual CPU model frequency; instruction-accurate timing
  - Impact: Frame timing approximate; some timing-sensitive code may not work
- [ ] **VGA Retrace Timing**: Use hardware-accurate retrace windows - `crates/systems/pc/src/bus.rs:252-277`
  - Current: Generic 5% retrace simulation
  - Needed: Real CRT timing specifications (horizontal/vertical blank periods)
  - Impact: Games relying on precise retrace timing may glitch

#### PC/DOS - Performance
- [ ] **Video Text Rendering Optimization**: Implement dirty region tracking - `crates/systems/pc/src/video_adapter_software.rs:113-200`
  - Current: Full screen rerender every frame with per-character pixel loops
  - Needed: Track dirty regions, cache character glyphs, skip unchanged areas
  - Impact: High CPU usage for text mode rendering; inefficient for large text updates
- [ ] **Disk Logging Performance**: Move environment variable check outside hot path - `crates/systems/pc/src/disk.rs:82,103,127`
  - Current: `std::env::var("EMU_LOG_BUS")` called on every disk read/write
  - Needed: Use static log level initialized once at startup
  - Impact: Unnecessary overhead on every disk operation
- [ ] **Font Data Pre-rendering**: Pre-bake font glyphs to pixel patterns - `crates/systems/pc/src/font.rs`
  - Current: Font arrays stored as `[u8]` requiring pixel extraction on every render
  - Needed: Pre-computed pixel patterns for common resolutions/zoom levels
  - Impact: Faster character rendering, reduced CPU overhead
- [ ] **Keyboard Buffer Implementation**: Use fixed ring buffer instead of VecDeque - `crates/systems/pc/src/keyboard.rs:11`
  - Current: VecDeque with allocation overhead
  - Needed: Fixed 16-byte ring buffer matching hardware
  - Impact: Reduced allocations, better cache locality

#### PC/DOS - Documentation
- [ ] **Magic Number Documentation**: Add comments for hardcoded values - `crates/systems/pc/src/cpu.rs`, `crates/systems/pc/src/bus.rs`
  - 0xB8000 (video memory) - used without comment (cpu.rs:526)
  - 0x400 (BIOS Data Area) - offset hardcoded in multiple places (cpu.rs:103-110)
  - Port addresses: 0x40-0x43 (PIT), 0x60/0x64 (keyboard), 0x3C0-0x3C9 (VGA) - mostly uncommented
  - Boot sector signature 0xAA55 - no reference to standard (bus.rs:395)
  - Impact: Makes code harder to understand and maintain
- [ ] **INT 10h Documentation**: Add high-level function overview - `crates/systems/pc/src/cpu.rs:412+`
  - Current: ~1000+ lines with no module-level description
  - Needed: Summary of supported video BIOS functions and modes
  - Impact: Hard to understand what's implemented without reading all code
- [ ] **I/O Port Documentation**: Document port ranges in io_read/io_write - `crates/systems/pc/src/bus.rs:694-1000`
  - Current: 300+ lines with scattered port documentation
  - Needed: Port map table at top of each function
  - Impact: Difficult to find which ports are implemented
- [ ] **Hardware Reference Citations**: Add IBM PC Technical Reference links
  - No references to IBM PC documentation
  - No 8086 CPU instruction set references
  - No VGA BIOS Programmer's Reference notes
  - No BIOS INT specification documents
  - Impact: Hard to verify hardware accuracy without original documentation

### Low

#### Debugger/Tracing System

- [ ] **JSON Debug Dump Export**: Add JSON format option for debug dumps - `crates/frontend/gui/src/main.rs:1993-2179`
  - Current: Debug dumps only in human-readable text format
  - Needed: --debug-dump-format json flag for machine-parseable output
  - Impact: Cannot easily parse debug dumps programmatically
  - Use case: Automated testing, diff comparison, external analysis tools
- [ ] **Interactive Debugger Mode**: Add step/continue/inspect commands - `crates/frontend/gui/src/main.rs`
  - Current: Emulator runs continuously or dumps at breakpoint then exits
  - Needed: Interactive mode with commands: step, continue, inspect, modify
  - Impact: Cannot debug interactively without restarting
  - Requires: Command parser, execution control, state modification
- [ ] **CPU Performance Counters**: Add instructions per second tracking - `crates/core/src/debug.rs`
  - Current: Only cycle count available
  - Needed: Track instructions/second, cycles/instruction, frame timing
  - Impact: Cannot measure emulation performance metrics
  - Use case: Performance tuning, accuracy verification
- [ ] **Trace Compression**: Add compression for long trace sessions - `crates/core/src/instruction_tracer.rs`
  - Current: Full instruction data stored per entry (~32 bytes)
  - Needed: Run-length encoding for repeated instructions, delta compression
  - Impact: Large trace buffers consume excessive memory
  - Benefit: 2-5x memory reduction for typical code patterns
- [ ] **Debugger Scripting Support**: Add Lua/JavaScript scripting - `crates/core/src/debug.rs`
  - Current: Debugging requires code changes or recompilation
  - Needed: Script engine for custom breakpoint logic, automated testing
  - Impact: Cannot automate complex debugging scenarios
  - Use case: Automated regression testing, custom analysis tools

#### Game Boy / Game Boy Color - Performance

- [ ] **Per-Scanline Sprite Vector Allocation**: Use fixed array instead of Vec - `crates/systems/gb/src/ppu.rs:887`
  - Current: `Vec::new()` allocated 144 times per frame (once per scanline)
  - Needed: Use fixed array `[(u8, u8); 40]` or reuse preallocated buffer
  - Impact: ~5-10% frame rendering overhead from allocator pressure
  - Max 40 sprites total, fixed size known at compile time
- [ ] **Background Color Index Buffer**: Optimize per-frame array allocation - `crates/systems/gb/src/ppu.rs:614-873`
  - Current: `bg_color_indices` array created fresh each frame (23KB)
  - Needed: Reuse buffer across frames or make it a struct field
  - Impact: Memory allocator overhead every frame
- [ ] **Palette Lookup Optimization**: Cache RGB palette conversions - `crates/systems/gb/src/ppu.rs:852-854`
  - Current: Index calculation and palette lookup per pixel (23,040 times/frame)
  - Needed: Precompute RGB palette array (8 palettes × 4 colors = 32 entries)
  - Impact: Reduced arithmetic in tight pixel loop
- [x] **Tight Loop Division Operations**: Replace division with bit shifts - `crates/systems/gb/src/ppu.rs:636-638`
  - **FIXED**: Replaced all division and modulo operations by 8 with bit operations
  - `x / 8` → `x >> 3` (right shift by 3)
  - `x % 8` → `x & 7` (bitwise AND with 7)
  - Impact: Minor performance improvement in tight pixel rendering loops

#### Game Boy / Game Boy Color - Documentation

- [x] **PPU Magic Numbers**: Document inline color constants - `crates/systems/gb/src/ppu.rs` (DMG palette constants)
  - **FIXED**: Added detailed comments explaining ARGB8888 format (0xAARRGGBB) and DMG grayscale mapping
  - White: 0xFFFFFFFF (lightest), Light gray: 0xFFAAAAAA (2/3 brightness), Dark gray: 0xFF555555 (1/3 brightness), Black: 0xFF000000 (darkest)
  - Impact: Improved code readability and maintainability
- [x] **LCDC Register Reference**: Add Pan Docs cross-reference - `crates/systems/gb/src/ppu.rs:242-254`
  - Completed: LCDC bits now include a Pan Docs URL comment referencing the LCDC section
  - Impact: Easier verification of hardware accuracy

- [x] **Signed Tile Addressing**: Explain 2's complement behavior - `crates/systems/gb/src/ppu.rs:809`
  - Completed: `calculate_signed_tile_address()` is documented with signed/unsigned tile indexing behavior
  - Impact: Understanding of tile data addressing
#### NES (Nintendo Entertainment System)
- [ ] **Duplicate APU Frame Counter State**: Refactor duplicated frame counter tracking - `crates/systems/nes/src/apu.rs:203-206`
  - Current: `frame_counter_cycles` and `irq_frame_counter_cycles` track same information separately
  - Code comment: "duplicated to avoid rewriting audio generation"
  - Impact: Unnecessary memory duplication; maintenance burden
  - Solution: Unify frame counter state or document rationale for separation
- [x] **Duplicate Variable Extraction in PPU**: Remove redundant scroll variable extraction - `crates/systems/nes/src/ppu.rs`
  - **FIXED**: Removed duplicate scroll variable extraction in the NES PPU scroll calculation code
  - Variables `coarse_x`, `coarse_y`, `nt_x`, `nt_y`, `fine_y` are now extracted only once
  - Impact: Cleaner code without redundant calculations
- [x] **PC Histogram Allocation**: Only allocate when instruction tracing enabled - `crates/systems/nes/src/lib.rs`
  - **FIXED**: PC histogram now only allocated when `instruction_tracer.is_enabled()` returns true
  - Eliminates ~60KB/sec allocation overhead when tracing is disabled
  - Impact: Reduced memory allocation overhead in normal operation
- [ ] **Sprite Evaluation Optimization**: Early exit sprite iteration at 8-sprite limit - `crates/systems/nes/src/ppu.rs:1230-1314`
  - Current: Sprite evaluation iterates all 64 sprites per scanline even when only 8 rendered
  - Issue: Early break at `sprites_on_scanline > 8` happens too late in loop
  - Impact: Wastes CPU cycles checking sprites 9-64 when sprite limit already reached
  - Solution: Break immediately when 8 sprites collected (preserve overflow detection logic)
- [ ] **RefCell Borrow & Callback Overhead in Rendering**: Batch CHR callbacks to reduce overhead - `crates/systems/nes/src/ppu.rs:1103-1106`
  - Current: `chr_read_callback.borrow_mut()` is performed for each callback invocation; when using `chr_fetch_fast()`, callbacks are invoked explicitly per tile/sprite rather than per individual CHR fetch
  - Impact: Repeated `RefCell` borrows and callback calls in the render hot path add overhead across a scanline/frame
  - Solution: Reduce callback invocation frequency (e.g., batch work per tile/sprite group) or hold a mutable borrow over a larger scope (such as a scanline) to avoid repeated `borrow_mut()` calls
  - Note: Low priority - `chr_fetch_fast()` already limits callback frequency, but further batching may yield minor performance gains
- [ ] **Weak Pointer Upgrade Overhead**: Review mapper A12 callback performance - `crates/systems/nes/src/bus.rs:88,112-114`
  - Current: `.upgrade()` is called from PPU CHR/A12 callback closures and runs once per callback invocation (potentially many times per scanline/frame depending on CHR reads/A12 transitions)
  - Impact: Additional weak-to-strong pointer upgrades on each relevant callback; expected to be minimal on modern CPUs
  - Solution: Profile actual callback frequency and optimize (e.g., cache strong references or restructure callbacks) only if a measurable impact is observed
  - Note: Very low priority - likely negligible performance impact in practice
- [x] **Unreachable Panic Branch**: Use `unreachable!()` for impossible flag names - `crates/systems/nes/src/debugger.rs`
  - **FIXED**: Replaced `panic!()` with `unreachable!()` macro in test code
  - All flag names are pre-defined, so the default case is truly unreachable
  - Impact: Better code clarity and potential compiler optimizations

#### NES APU - Completed
- [x] **Sweep Unit**: Implement pulse channel sweep units - `crates/systems/nes/src/apu.rs:56-151`
  - **FIXED**: Fully implemented NES-specific sweep units for both pulse channels
  - Sweep period, shift count, negate flag, and timer all working
  - Pulse 1 uses one's complement negation, Pulse 2 uses two's complement (hardware-accurate)
  - Proper muting when frequency is too low (<8) or too high (>0x7FF)
  - 10 comprehensive tests passing (sweep_register_write, sweep_ones_complement_vs_twos_complement, etc.)
  - Impact: Audio pitch effects now work correctly (portamento, pitch bends)
  - Reference: https://www.nesdev.org/wiki/APU_Sweep
- [x] **DMC Channel**: Implement Delta Modulation Channel - `crates/core/src/apu/dmc.rs`, `crates/systems/nes/src/apu.rs:194,417-446`
  - **FIXED**: Full DMC channel implementation with sample playback, DMA, and IRQ
  - 16 different sample rates supported (NTSC and PAL tables)
  - Loop support and IRQ generation on sample completion
  - DMA implementation in `crates/systems/nes/src/lib.rs:723-727` for memory reads
  - Proper integration with NES mixer (included in non-linear mixing formula)
  - 3 comprehensive tests passing (dmc_basic_operation, dmc_memory_read_request, dmc_irq_generation)
  - Impact: Drum/sample sounds now work in all games
  - Reference: https://www.nesdev.org/wiki/APU_DMC
- [x] **Frame Counter**: Implement APU frame counter - `crates/systems/nes/src/apu.rs:200-203,477-510,564-587,617-735`
  - **FIXED**: Full frame counter implementation with 4-step and 5-step modes
  - Proper envelope clocking at quarter-frame rate (~240 Hz NTSC, ~200 Hz PAL)
  - Proper length counter and sweep unit clocking at half-frame rate (~120 Hz NTSC, ~100 Hz PAL)
  - IRQ generation in 4-step mode with IRQ inhibit flag support
  - Cycle-accurate $4017 write handling (immediate clock in 5-step mode)
  - Impact: Envelope and length counter timing now hardware-accurate
  - Reference: https://www.nesdev.org/wiki/APU_Frame_Counter

#### NES Mappers
- [ ] **MMC5 PCM Audio**: Implement MMC5 PCM playback - `crates/systems/nes/src/mappers/mmc5.rs:41`
  - Current: MMC5 mapper implemented but PCM audio features not available
  - Needed: PCM sample playback and mixing with APU channels
  - Impact: Missing audio features in MMC5 games (e.g., Castlevania III)
  - Note: Advanced feature, low priority as basic mapper functionality works

#### SNES - Enhancement Chips
- [ ] **DSP-1 Full Hardware Accuracy**: Implement Parameter command and shared projection state - `crates/systems/snes/src/coprocessors/dsp1.rs`
  - Current: Simplified implementations of Attitude, Target, Rotate commands
  - Needed: Parameter command (0x02) to set up shared projection matrices and camera parameters
  - Impact: Current implementation sufficient for basic compatibility; full accuracy needed for advanced DSP-1 features
  - Note: Target and Attitude commands use simplified transformations without shared state
  - Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (parameter, target, attitude functions)

#### SNES - Audio
- [x] **SPC700 DSP Audio Generation**: Implement audio output - `crates/frontend/gui/src/main.rs:600`
  - **FIXED**: Wired up SPC700 DSP audio generation to frontend
  - Changed from returning silence to calling `sys.get_audio_samples(count)`
  - SNES games now have audio (SPC700 DSP implementation was already complete in core)

#### SNES - I/O
- [ ] **JOY3/JOY4 Controller Ports**: Implement multitap support - `crates/systems/snes/src/bus.rs:1108-1111`
  - Current: Registers $421C-$421F (JOY3L/H, JOY4L/H) return 0
  - Needed: Full multitap implementation for 3-4 player games
  - Impact: 3-4 player games cannot use additional controllers
  - Note: Low priority - most games use only 2 controllers
  - Status: Documented in code comments as a known limitation

#### SNES - PPU
- [x] **Unused Helper Methods**: Refactor or document dead code - `crates/systems/snes/src/ppu.rs:2301,2329`
  - **FIXED**: Removed unused helper methods
  - Removed `parse_tilemap_entry()` and `calculate_16x16_tile_info()` as they were never used
  - Improves code maintainability by eliminating dead code

#### SNES
- [ ] **Upload Protocol Test**: Investigate SPC700 index echoing issue - `crates/systems/snes/src/lib.rs:832`
  - Tests currently ignored - SPC700 not echoing indices during upload
  - Affects: `test_apu_upload_protocol` and `test_apu_ports_echo`
  - Note: This requires deep APU debugging and is not critical for general functionality
  - Status: Improved test comments to clarify this is a known issue, not blocking
- [x] **Sprite Rendering**: Fix sprite overflow test - `crates/systems/snes/src/lib.rs:test_sprite_overflow_rom`, `test_roms/snes/test_sprite_overflow.s`
  - **FIXED**: Test ROM was not properly uploading sprite data to VRAM
  - Fixed by adding VRAM increment mode setup ($2115 = $80) and writing to both $2118 and $2119
  - Test was also checking wrong scanline (100 instead of 101 due to Y+1 offset)
  - Test now passes - sprite overflow detection working correctly
- [x] **Sample Interpolation**: Improve SPC700 sample quality - `crates/core/src/apu/spc700.rs:629-655`
  - **FIXED**: Implemented linear interpolation between DSP output samples
  - DSP generates samples every 32 CPU cycles (32kHz)
  - Linear interpolation smoothly fills the gaps between DSP samples
  - Eliminates the previous behavior of returning 0 between samples
  - Improves audio quality and reduces clicking/popping artifacts
  - Note: This is interpolation between DSP outputs, separate from the Gaussian interpolation within the DSP for BRR sample playback

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
- [x] **Audio Microcode**: Implement RSP audio task processing - `crates/systems/n64/src/rsp_hle.rs:462-522`
  - **IMPLEMENTED**: Basic audio task structure parsing
    - Parses task structure from DMEM (input/output buffers, command list)
    - Validates RDRAM pointers and logs task information
    - Returns appropriate cycle count for audio processing
  - **REMAINING**: Full audio implementation (future work)
    - TODO: ADPCM decompression
    - TODO: Resampling and filtering
    - TODO: Multi-channel mixing
  - Impact: Audio task infrastructure in place, full audio output pending
- [x] **Save System**: Implement EEPROM support - `crates/systems/n64/src/pif.rs`
  - **IMPLEMENTED**: EEPROM (4Kbit/16Kbit) support
    - Command 0x04: Read EEPROM block (8 bytes)
    - Command 0x05: Write EEPROM block (8 bytes)
    - Block validation and error handling
    - API for persistence: set_eeprom_type(), load_eeprom(), save_eeprom()
  - **REMAINING**: Flash RAM and Controller Pak (future work)
  - Impact: Games with EEPROM saves can now persist data
- [x] **RDP SET_OTHER_MODES**: Implement rendering mode configuration - `crates/systems/n64/src/rdp.rs:1160`
  - **IMPLEMENTED**: Now stores full 64-bit othermode value
  - Extracts and logs cycle type, texture filtering, alpha compare, and z-mode
  - Impact: RDP properly tracks rendering mode settings for future use
- [x] **Texture Format Support**: Implement missing texture formats - `crates/systems/n64/src/rdp.rs:705`
  - **IMPLEMENTED**: YUV16 texture format (format=1, size=2)
    - Used for video textures with interleaved YUYV data
    - Implements ITU-R BT.601 YUV to RGB conversion
    - Properly handles even/odd texel positions for U/V sampling
  - Impact: Video textures now render correctly instead of returning white

#### N64
- [x] **RDP Performance Counters**: Implement RDP performance counters - `crates/systems/n64/src/rdp.rs:165-172`
  - **IMPLEMENTED**: All four performance counters now functional
    - DPC_CLOCK: Increments ~10 cycles per RDP command processed
    - DPC_BUFBUSY: Increments by number of commands during DMA processing
    - DPC_PIPEBUSY: Increments for triangle/fill commands (pipeline-active operations)
    - DPC_TMEM: Increments on each texture load to TMEM
  - Impact: Performance monitoring now available for debugging and profiling
- [x] **RSP Semaphore**: Implement RSP semaphore register - `crates/systems/n64/src/rsp.rs:103`
  - **IMPLEMENTED**: Full atomic semaphore implementation using Cell<u32>
    - Reading returns current value and atomically clears it (test-and-clear operation)
    - Writing sets semaphore to 1 (locked state)
    - Uses interior mutability to support atomic read-clear within immutable read context
  - Impact: CPU-RSP synchronization now functional
- [x] **RSP Signal Bits**: Implement signal bits (SIG0-SIG7) - `crates/systems/n64/src/rsp.rs:72-86`
  - **IMPLEMENTED**: All 8 signal bits (SIG0-SIG7) in SP_STATUS register
    - Each signal has clear/set bit pairs in SP_STATUS write (bits 9-24)
    - Signals properly reflected in SP_STATUS read
    - Enables RSP-CPU communication for task coordination
  - Impact: RSP-CPU communication signals fully operational
- [x] **Memory Alignment Validation**: Add alignment checks for load/store - `crates/core/src/cpu_mips_r4300i.rs`
  - **IMPLEMENTED**: Alignment validation for all load/store instructions
    - LH/LHU/SH: 2-byte aligned (addr & 1 == 0)
    - LW/LWU/SW: 4-byte aligned (addr & 3 == 0)
    - LD/SD: 8-byte aligned (addr & 7 == 0)
    - Logs warning on misalignment (doesn't crash)
    - Updated documentation comments
  - Impact: Unaligned access now detected and logged for debugging

#### MIPS R4300i
- [x] **Branch Delay Slot Implementation**: CRITICAL BUG FIXED - Implement proper delay slot execution - `crates/core/src/cpu_mips_r4300i.rs`
  - **FIXED**: Branch/jump instructions now properly execute delay slot before taking branch
  - **Previous bug**: PC was updated immediately, skipping delay slot execution entirely
  - **New behavior**: 
    - Branch/jump sets next_pc and in_delay_slot flag
    - Delay slot instruction executes
    - Then PC is updated to branch target
  - **Affected instructions**: All branches (BEQ, BNE, BLEZ, BGTZ, BLTZ, BGEZ, etc.), jumps (J, JAL), and jump register (JR, JALR)
  - **Impact**: This was causing wildly incorrect PC behavior and execution of random memory
  - **Return address fix**: JAL/JALR now correctly save PC+8 (instruction after delay slot) instead of PC+4
  - All CPU tests updated and passing
- [ ] **Arithmetic Overflow Traps**: Implement overflow exception handling - `crates/core/src/cpu_mips_r4300i.rs`
  - ADD/ADDI/SUB should trap on overflow (currently ignored)
  - DADD/DADDI/DSUB should trap on overflow (currently ignored)
  - Impact: Some overflow-checking code may not work correctly
- [x] **Branch Delay Slot Nullification**: Implement nullify delay slot for branch-likely instructions - `crates/core/src/cpu_mips_r4300i.rs`
  - **IMPLEMENTED**: All branch-likely instructions now skip the delay-slot instruction when the branch is not taken
    - I-type: BEQL, BNEL, BLEZL, BGTZL
    - REGIMM: BLTZL, BGEZL, BLTZALL, BGEZALL
    - FPU: BC1TL, BC1FL (BC1 with ND bit set)
  - Impact: Correct behavior for all branch-likely instructions per MIPS specification

#### Game Boy / Game Boy Color

- [ ] **Game Boy Color Speed Switching**: Implement double-speed mode - `crates/core/src/cpu_lr35902.rs:27`
  - Current: Returns false for speed switching query (DMG mode)
  - Needed: Implement CGB double-speed mode (KEY1 register)
  - Impact: Some GBC games may run at wrong speed or not work at all
  - Note: DMG games work correctly; only affects Game Boy Color titles
- [ ] **MBC3 RTC Tick Frequency**: Improve RTC accuracy from ~60 Hz to proper 1 Hz precision - `crates/systems/gb/src/mappers/mbc3.rs:47,254-300`
  - Current: RTC incremented at ~60 Hz frame rate instead of 1 Hz
  - Issue: `rtc_ticks` counter uses frame rate (60 ticks = 1 second approximation)
  - Needed: Proper 1 Hz timing using actual elapsed time or cycle counting
  - Impact: RTC time drifts slightly from real time, affects time-based gameplay in Pokemon Gold/Silver/Crystal
  - Note: Day counter overflow and carry flag ARE implemented correctly (lines 286-296)
- [ ] **MBC3 RTC Persistence**: Add RTC state save/load support - `crates/systems/gb/src/mappers/mbc3.rs`
  - Current: RTC state resets on emulator restart
  - Needed: Persist RTC values (seconds, minutes, hours, days) across sessions
  - Impact: Time-based events in Pokemon games reset when emulator closes
  - Implementation: Add RTC state to mapper's save state serialization
- [ ] **HuC1 IR Sensor Stub Documentation**: Document IR sensor limitation - `crates/systems/gb/src/mappers/huc1.rs:99-100,119-122`
  - Current: Returns hardcoded 0xC0 (no signal) without clear documentation
  - Needed: Add comment explaining IR communication not emulated
  - Impact: Affects <1% of games (Pocket Bomberman, Tamagotchi series)
  - Note: IR mode register write is silently ignored (line 99-100)

#### CHIP-8
- [ ] **Audio Beep Tone**: Implement sound timer beep - `crates/frontend/gui/src/main.rs:602`
  - Current: Returns silence (vec![0; count]) instead of beep tone
  - Needed: Generate simple beep tone when sound timer is non-zero
  - Impact: CHIP-8/Super-CHIP/XO-CHIP games have no audio feedback
  - Note: CHIP-8 has only one audio feature - a simple beep tone

#### GUI / Frontend
- [ ] **Mouse Button Support**: Implement mouse input handling - `crates/frontend/gui/src/input_mapper.rs:51`
  - Current: Mouse button events are ignored with a TODO comment
  - Needed: Map mouse buttons to emulator input system
  - Impact: Cannot use mouse for light gun games or point-and-click interfaces
  - Note: Low priority - most systems don't use mouse input

#### SG-1000
- [ ] **ROM Banking Support**: Implement memory banking for larger cartridges - `crates/systems/sg1000/src/bus.rs`
  - Current: Fixed 48KB ROM space (0x0000-0xBFFF)
  - Most SG-1000 games are under 48KB, so not critical
  - Impact: Cannot run games larger than 48KB (rare)

