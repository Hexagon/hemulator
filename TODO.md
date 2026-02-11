## ToDo

This file tracks unimplemented features, stubs, and simplified implementations across the project. When adding TODOs, categorize them by priority:

- **Critical**: Blocking issues, security vulnerabilities, or crashes
- **High**: Major functionality gaps affecting compatibility or user experience
- **Medium**: Important features or optimizations
- **Low**: Nice-to-have features, minor improvements, or polish

### Critical

### High

#### NES (Nintendo Entertainment System)
- [ ] **Mapper 1 (MMC1) Game Compatibility Issues**: Debug and fix Mapper 1 games that hang or don't display - `crates/systems/nes/src/mappers/mmc1.rs`
  - Current: Comprehensive MMC1 implementation with 11 passing tests, but some games hang or show gray screen
  - Reported issues: #307 (Rad Racer hangs), #324 (Mike Tyson's Punch-Out gray screen), #363 (Rad Racer rendering)
  - Possible causes: Consecutive write filtering (line 126), bank switching edge cases, or integration with NES bus
  - Needed: Test with actual Mapper 1 ROMs, enable debug logging, compare with known-good emulator
  - Impact: Affects ~30% of NES library including Zelda, Mega Man, Final Fantasy, Metroid
  - Note: Implementation appears correct based on code review, issue may be in edge cases or timing

#### GBA (Game Boy Advance)
- [ ] **Audio (APU)**: Implement APU audio - `crates/systems/gba/src/lib.rs`
  - Current: CPU/PPU/DMA/timers/debugger/save states implemented; audio missing
  - Needed: APU channels, mixer, and audio output
  - Impact: No sound support

### Medium

#### Atari 2600
- [ ] **Paddle Controllers GUI Integration**: Add mouse/analog input support for paddles - `crates/frontend/gui/`
  - Current: Hardware emulation complete (INPT4/INPT5 with charge/discharge timing), GUI integration needed
  - Needed: Map mouse X/Y or analog stick to paddle pot values
  - Impact: Cannot play paddle games (Breakout, Kaboom!, Circus Atari) without controller input
  - Quality: Hardware-ready, just needs frontend work
- [ ] **Optional HMOVE Comb Artifacts Mode**: Add cycle-accurate HMOVE visualization - `crates/systems/atari2600/src/tia.rs`
  - Current: HMOVE applies motion instantly (no visible comb)
  - Needed: Add optional 6-clock delay and visible comb artifacts for accuracy enthusiasts (off by default)
  - Impact: Very low - only affects cycle-accurate demos; all games work correctly
  - Quality: Nice-to-have for hardware perfectionism

#### Game Boy (DMG)
- [ ] **Sprite Per-Scanline Limit**: Restore hardware-accurate 10-sprite limit for DMG - `crates/systems/gb/src/ppu.rs`
  - Current: DMG limit relaxed to 40 sprites per scanline for compatibility
  - Needed: Accurate OAM selection (10 sprites) with proper timing so games don't flicker
  - Impact: Fixes sprite flicker hacks while keeping correct hardware behavior

#### Debugger/Tracing System

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
- [ ] **Controller Register Test Failures**: Fix failing controller tests - `crates/systems/snes/src/bus.rs:1641-1722`
  - Current: Tests test_controller_registers and test_dual_controllers fail
  - Issue: Controller data reads returning 0 instead of expected button states
  - Expected: test_controller_registers should read 0x80 (B button), test_dual_controllers should read 0xAA
  - Actual: Reads return 0x00
  - Impact: Controller input reading may not be working correctly
  - Note: This is a pre-existing test failure, not introduced by recent changes
  - Reference: SNES Dev Manual "Controller Reading"
- [ ] **APU Upload Protocol Test Failure**: Fix failing test_apu_upload_protocol - `crates/systems/snes/src/lib.rs:981-1041`
  - Current: Test times out waiting for APU ready signal after 9 frames
  - Issue: APU upload protocol not completing handshake (ports show 01 02 03 00 instead of ready)
  - Impact: APU data upload may not be working correctly
  - Note: This is a pre-existing test failure, not introduced by recent changes
  - Reference: SNES Dev Manual "APU Communication"
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

#### NES (Nintendo Entertainment System)
- [ ] **Sprite 0 Hit Timing on Odd Frames**: Verify X position calculation during odd-frame skip - `crates/systems/nes/src/lib.rs:620-627,655-656`, `crates/systems/nes/src/ppu.rs:1235-1249`
  - Current: Sprite 0 hit X position is calculated during render_scanline() and triggered in tick() at dot = X + 2
  - Analysis: The current implementation appears correct - sprite 0 hit is pixel-based (0-255), not dot-based
  - Odd-frame skip affects when rendering happens (dot 0 vs dot 1) but not the sprite 0 hit X position
  - The hit trigger is based on PPU dot counter reaching hit_x + 2, which is independent of rendering timing
  - Status: No actual bug identified; current implementation handles odd/even frames correctly
  - Action: Needs real-world testing with games that use tight sprite 0 hit timing (e.g., split-screen effects)
  - Reference: https://www.nesdev.org/wiki/PPU_frame_timing#Odd_frames
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
- [ ] **Font Data Pre-rendering**: Pre-bake font glyphs to pixel patterns - `crates/systems/pc/src/font.rs`
  - Current: Font arrays stored as `[u8]` requiring pixel extraction on every render
  - Needed: Pre-computed pixel patterns for common resolutions/zoom levels
  - Impact: Faster character rendering, reduced CPU overhead

### Low

#### GBA (Game Boy Advance) - Low Priority

- [ ] **Per-Sprite Mosaic**: Implement per-sprite mosaic flag - `crates/systems/gba/src/ppu.rs:1023`
  - Current: OBJ mosaic uses global setting only; attr0 mosaic bit ignored
  - Needed: Track and apply mosaic per-sprite based on attr0 bit 12
  - Impact: Sprites incorrectly share global mosaic settings
- [ ] **HBlank OAM Access Restriction**: Restrict OAM access during HBlank - `crates/systems/gba/src/ppu.rs:91`
  - Current: OAM accessible at all times
  - Needed: Hardware restricts OAM writes during HBlank period
  - Impact: Games relying on this restriction may have rendering glitches
- [ ] **LDM/STM User Bank Enforcement**: Handle S bit in LDM/STM - `crates/core/src/cpu_arm7tdmi.rs:1922`
  - Current: S bit (force user banks) not enforced in LDM/STM instructions
  - Needed: Load/store from user mode registers in privileged modes when S=1
  - Impact: Very rare edge case, most games don't use this feature

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

#### N64 Testing

- [ ] **Ignored Tests Require OpenGL Context**: 15+ N64 tests marked with #[ignore] - `crates/systems/n64/src/lib.rs`, `crates/frontend/gui/tests/logging_integration.rs`
  - Current: Tests disabled because they require OpenGL context initialization
  - Tests affected: test_n64_initialization, test_n64_reset, test_n64_cpu_basic, test_n64_pif_boot, test_n64_rdp_commands, test_n64_memory_map, test_n64_sp_dma, and 8 more
  - Impact: Core N64 functionality untested in CI
  - Possible solutions:
    - Use headless/offscreen OpenGL context for tests (e.g., glutin with EGL)
    - Mock the renderer interface for unit tests
    - Run tests only on platforms with GL support
  - Note: Tests work when run manually with graphics context available

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

#### Game Boy / Game Boy Color - Documentation


#### NES (Nintendo Entertainment System)
- [ ] **Duplicate APU Frame Counter State**: Refactor duplicated frame counter tracking - `crates/systems/nes/src/apu.rs:203-206`
  - Current: `frame_counter_cycles` and `irq_frame_counter_cycles` track same information separately
  - Code comment: "duplicated to avoid rewriting audio generation"
  - Impact: Unnecessary memory duplication; maintenance burden
  - Solution: Unify frame counter state or document rationale for separation
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

#### NES Mappers
- [ ] **MMC5 PCM Audio**: Implement MMC5 PCM playback - `crates/systems/nes/src/mappers/mmc5.rs:41`
  - Current: MMC5 mapper implemented but PCM audio features not available
  - Needed: PCM sample playback and mixing with APU channels
  - Impact: Missing audio features in MMC5 games (e.g., Castlevania III)
  - Note: Advanced feature, low priority as basic mapper functionality works

#### SNES - Enhancement Chips
- [ ] **DSP-1 Full Hardware Accuracy**: Complete projection math for Parameter/Target/Attitude - `crates/systems/snes/src/coprocessors/dsp1.rs`
  - Current: Parameter command (0x02) is accepted and cached; Attitude/Target/Rotate remain simplified
  - Needed: Use cached parameters to build projection matrices and apply accurate Target/Attitude math
  - Impact: Basic DSP-1 games may run, but advanced projection scenes can still break or hang
  - Note: Target and Attitude commands still use simplified transformations without shared state
  - Reference: bsnes/sfc/coprocessor/dsp1/dsp1emu.cpp (parameter, target, attitude functions)

#### SNES - Audio

#### SNES - I/O
- [ ] **JOY3/JOY4 Controller Ports**: Implement multitap support - `crates/systems/snes/src/bus.rs:1108-1111`
  - Current: Registers $421C-$421F (JOY3L/H, JOY4L/H) return 0
  - Needed: Full multitap implementation for 3-4 player games
  - Impact: 3-4 player games cannot use additional controllers
  - Note: Low priority - most games use only 2 controllers
  - Status: Documented in code comments as a known limitation

#### SNES - PPU

#### SNES
- [ ] **Upload Protocol Test**: Investigate SPC700 index echoing issue - `crates/systems/snes/src/lib.rs:832`
  - Tests currently ignored - SPC700 not echoing indices during upload
  - Affects: `test_apu_upload_protocol` and `test_apu_ports_echo`
  - Note: This requires deep APU debugging and is not critical for general functionality
  - Status: Improved test comments to clarify this is a known issue, not blocking

#### N64

#### N64

#### MIPS R4300i
- [ ] **Arithmetic Overflow Traps**: Implement overflow exception handling - `crates/core/src/cpu_mips_r4300i.rs`
  - ADD/ADDI/SUB should trap on overflow (currently ignored)
  - DADD/DADDI/DSUB should trap on overflow (currently ignored)
  - Impact: Some overflow-checking code may not work correctly

#### Game Boy / Game Boy Color

- [ ] **OAM DMA Transfer Test Failures**: Fix failing OAM DMA tests - `crates/systems/gb/src/lib.rs:1220-1294`
  - Current: Tests test_oam_dma_basic and test_oam_dma_full_copy fail
  - Issue: OAM memory not being populated after DMA transfer (all zeros instead of expected values)
  - Expected: OAM[0] = 0x80 (test_oam_dma_basic), OAM[0] = 0xAA (test_oam_dma_full_copy)
  - Actual: OAM[0] = 0x00 in both tests
  - Impact: OAM DMA transfer may not be working correctly
  - Note: This is a pre-existing test failure, not introduced by recent changes
  - Reference: Pan Docs "LCD OAM DMA Transfers"
- [ ] **LR35902 EI Delayed Enable Test Failure**: Fix failing test_di_ei test - `crates/core/src/cpu_lr35902.rs:1552-1568`
  - Current: Test expects EI instruction to delay IME enable by one instruction (per hardware spec)
  - Issue: Test assertion `assert!(!cpu.ime)` fails after EI instruction executes
  - Expected behavior: EI should set ime_scheduled flag, then IME becomes true after next instruction
  - Impact: Test failure indicates EI instruction may not be following hardware-accurate delayed enable
  - Note: This is a pre-existing test failure, not introduced by recent changes
  - Reference: Pan Docs "Interrupt Master Enable Flag (IME)"
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

