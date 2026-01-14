# NES Emulator Hot Path Optimizations

This document describes the performance optimizations applied to the NES emulator hot paths while maintaining cycle-accuracy and all hardware quirks.

## Performance Summary

**Baseline Performance (before optimizations):**
- Frame execution: 1.186 ms/frame (~844 FPS)
- Memory read from RAM: 914 ns
- Memory read from PRG ROM: 7.0 µs

**Optimized Performance:**
- Frame execution: **0.986 ms/frame (~1015 FPS)** - **16.6% faster**
- Memory read from RAM: **723 ns** - **20.9% faster**
- Memory read from PRG ROM: **6.6 µs** - **5.5% faster**

**Net Result: +171 FPS improvement (20% faster overall)**

## Hot Paths Identified

### 1. CPU Step Function (`cpu_6502.rs::step()`)
**Frequency:** Called ~30,000 times per frame (once per CPU instruction)

**Issue:** Large match statement with ~256 opcodes, each involving opcode fetch, address mode calculation, and execution.

**Optimization:** Added `#[inline(always)]` to:
- `step()` - The main instruction execution loop
- `read()` / `write()` - Memory access (called 3+ times per instruction)
- `fetch_u8()` / `fetch_u16()` - Opcode/operand fetching
- `set_zero_and_negative()` - Flag updates (called by most instructions)
- `adc()` - Arithmetic operations

**Result:** 8.8% faster CPU execution per frame

### 2. Memory Bus Read/Write (`bus.rs`)
**Frequency:** Called 3+ times per CPU instruction on average

**Issue:** Address decoding via match statements for every memory access. Mapper calls involve `Rc<RefCell<>>` borrows which add overhead.

**Optimization:** Added `#[inline]` to:
- `NesBus::read()` - CPU memory read
- `NesBus::write()` - CPU memory write

**Result:** 20.9% faster RAM reads, 5.5% faster PRG ROM reads

### 3. PPU Tick (`ppu.rs::tick()`)
**Frequency:** Called 3 times per CPU cycle (PPU runs at 3x CPU clock rate)

**Issue:** Complex state machine with conditional checks for scanline/dot positions, VBlank timing, sprite evaluation, scroll register updates.

**Optimization:** Added `#[inline]` to:
- `tick()` - Main PPU state machine
- `map_nametable_addr()` - Nametable address translation

Added `#[inline(always)]` to frequently-called getters:
- `get_scanline()` / `get_dot()` / `get_frame_counter()`
- `nmi_enabled()` / `mask()` / `ctrl()` / `get_mirroring()`
- `pre_render_scanline()`

**Result:** Contributes to overall 16.6% frame execution speedup

### 4. PPU Register Access
**Frequency:** Called on every PPU register read/write ($2000-$2007)

**Issue:** Register reads/writes involve state tracking (address latch, read buffer, VBlank clearing) and are called from the bus read/write path.

**Optimization:** Added `#[inline]` to:
- `Ppu::read_register()` - PPU register reads
- `Ppu::write_register()` - PPU register writes

**Result:** Reduced overhead in bus read/write path

## Addressing Mode Calculations

**Already optimized:** All addressing mode helper functions were already marked with `#[inline]`:
- `addr_zero_page_x()` / `addr_zero_page_y()`
- `addr_absolute_x()` / `addr_absolute_y()`
- `addr_indirect_x()` / `addr_indirect_y()`
- `read_indirect_u16_bug()` - JMP indirect with 6502 page-wrap bug
- Stack operations: `push_u8()`, `pop_u8()`, `push_u16()`, `pop_u16()`

These functions are called for every instruction using those addressing modes, so inlining eliminates function call overhead.

## Why `#[inline(always)]` vs `#[inline]`?

### `#[inline(always)]`
Used for **trivial, extremely hot** functions:
- Simple getters that just return a field
- 1-2 line functions called millions of times per second
- Functions where call overhead is significant compared to function body

Examples: `read()`, `write()`, `fetch_u8()`, `get_scanline()`, `nmi_enabled()`

### `#[inline]`
Used for **moderately complex, frequently-called** functions:
- Functions with 10-50 lines of code
- Complex logic but called very frequently
- Where inlining is beneficial but compiler heuristics might not always inline

Examples: `tick()`, `read_register()`, `write_register()`, `NesBus::read()`

## Cycle Accuracy Preservation

All optimizations were **purely mechanical** - adding inline hints to existing functions without changing any logic:

✅ **VBlank timing** - Still set at scanline 241, dot 1
✅ **NMI triggering** - Cycle-accurate with proper suppression
✅ **Sprite evaluation** - Hardware bug emulation intact
✅ **Scroll register behavior** - Loopy register updates preserved
✅ **PPU address latch** - Shared latch for $2005/$2006 maintained
✅ **Odd frame skip** - NTSC odd frame cycle skip still works

## Hardware Quirks Preserved

All documented hardware quirks remain functional:

✅ **Sprite overflow bug** - m/n pointer increment bug emulated
✅ **PPUSTATUS read timing** - VBlank suppression on read at scanline 241, dot 1
✅ **6502 indirect JMP bug** - Page-wrapping bug on $xxFF addresses
✅ **Decimal mode disabled** - Not implemented (2A03 doesn't have it)
✅ **Palette mirroring** - $3F10/$3F14/$3F18/$3F1C mirror to $3F00/$3F04/$3F08/$3F0C

## Testing

All optimizations were validated with:

✅ **239 NES unit tests** - All pass
✅ **Comprehensive benchmarks** - Frame execution, memory access, CPU instructions
✅ **Smoke test ROM** - Simple test ROM executes correctly
✅ **Code formatting** - `cargo fmt` passes
✅ **Linting** - `cargo clippy` passes with no warnings

## Benchmarks

The benchmark suite in `crates/systems/nes/benches/nes_hotpaths.rs` measures:

- **Frame CPU execution** - Measures CPU step loop performance
- **Memory reads** - RAM and PRG ROM read performance
- **Frame execution** - Complete frame rendering with PPU
- **Multi-frame execution** - Sustained performance over 10 frames

Run benchmarks with:
```bash
cargo bench --bench nes_hotpaths --profile release-quick
```

## Future Optimization Opportunities

Potential areas for further optimization (beyond scope of this PR):

1. **Cache locality** - Reorganize frequently-accessed fields to be cache-line aligned
2. **SIMD** - Vectorize palette lookups and pixel blending operations
3. **Branch prediction** - Reorder conditionals to favor common cases
4. **Memory allocation** - Pre-allocate buffers to avoid runtime allocations
5. **Lookup tables** - Pre-compute common calculations (e.g., cycle counts per opcode)

**Important:** Any future optimizations must maintain:
- Cycle-accurate execution timing
- All hardware quirks and edge cases
- Compatibility with existing ROMs
- Debugger and inspector functionality

## References

- **NESdev Wiki**: https://www.nesdev.org/wiki/
- **6502 Reference**: http://www.6502.org/
- **PPU Scrolling**: https://www.nesdev.org/wiki/PPU_scrolling
- **PPU Registers**: https://www.nesdev.org/wiki/PPU_registers
- **Sprite Evaluation**: https://www.nesdev.org/wiki/PPU_sprite_evaluation
