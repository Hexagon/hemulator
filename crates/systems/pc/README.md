# PC Emulation - IBM PC/XT

This crate implements IBM PC/XT emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The PC emulator is **experimental** with CGA/EGA/VGA graphics support and basic BIOS.

### What Works

- ✅ **CPU (8086)** - Complete instruction set from `emu_core::cpu_8086`
- ✅ **Memory** - 640KB RAM, 128KB VRAM, 256KB ROM
- ✅ **BIOS** - Minimal custom BIOS built from assembly
- ✅ **Video Adapters** - CGA, EGA, VGA with multiple modes
- ✅ **Disk Controller** - INT 13h infrastructure
- ✅ **Keyboard** - Full passthrough with host modifier
- ✅ **Mount System** - Multi-slot disk image mounting
- ✅ **Save States** - State serialization

### Video Adapter Support

All adapters follow the modular `VideoAdapter` trait:

- **CGA (Color Graphics Adapter)**:
  - Text: 80x25 (640x400 pixels, 8x16 font)
  - Graphics: 320x200 4-color, 640x200 2-color
  - 16-color fixed palette
  
- **EGA (Enhanced Graphics Adapter)**:
  - Text: 80x25 (640x350 pixels, 8x14 font)
  - Graphics: 640x350 16-color, 320x200 16-color
  - 64-color palette (6-bit RGB), 16 active colors
  
- **VGA (Video Graphics Array)**:
  - Text: 80x25 (720x400 pixels, 9x16 font)
  - Graphics: 320x200 256-color (Mode 13h), 640x480 16-color
  - 256-color palette (18-bit RGB)

Each adapter has software (CPU) and hardware (OpenGL stub) implementations.

### What's Missing

- ⏳ **BIOS**: INT 13h disk I/O not connected
- ⏳ **Boot**: Boot sector loading infrastructure exists but not fully wired
- ⏳ **Audio**: PC speaker not implemented
- ⏳ **Timer**: PIT (Programmable Interval Timer) not implemented
- ⏳ **Serial/Parallel**: No COM/LPT port support

## Architecture

### Component Structure

```
PcSystem
  └── PcCpu (wraps Cpu8086<PcBus>)
      └── PcBus (implements Memory8086)
          ├── 640KB Conventional Memory
          ├── 128KB Video Memory
          ├── 256KB ROM Area
          │   └── 64KB BIOS ROM
          ├── Video Adapter (pluggable)
          │   ├── SoftwareCgaAdapter
          │   ├── CgaGraphicsAdapter
          │   ├── SoftwareEgaAdapter
          │   └── SoftwareVgaAdapter
          ├── Disk Controller
          │   ├── Floppy A: / B:
          │   └── Hard Drive C:
          └── Keyboard
```

### Video Adapter Architecture

**Location**: `src/video_adapter*.rs`

Follows modular renderer pattern:

```
PcSystem (state) → VideoAdapter trait → {Software, Hardware} implementations
```

**Benefits**:
- Easy mode switching at runtime
- Pluggable rendering backends
- Clean separation of state and rendering
- Future GPU acceleration support

### Memory Map

- **0x00000-0x9FFFF**: Conventional memory (640KB)
- **0xA0000-0xBFFFF**: Video memory (128KB)
- **0xC0000-0xFFFFF**: ROM area (256KB)
- **0xF0000-0xFFFFF**: BIOS ROM (64KB)

### Mount Points

1. **BIOS** (Slot 1): Custom or replacement BIOS ROM
2. **Floppy A** (Slot 2): Floppy drive A:
3. **Floppy B** (Slot 3): Floppy drive B:
4. **Hard Drive C** (Slot 4): Hard disk drive C:

## Building

```bash
# Build PC crate
cargo build --package emu_pc

# Run tests
cargo test --package emu_pc

# Run with disk image
cargo run --release -p emu_gui -- --slot2 boot.img
```

## Testing

The PC crate includes comprehensive tests:

- **121 total tests**:
  - CPU tests (8086 instruction set)
  - Video adapter tests (CGA, EGA, VGA modes)
  - Bus tests (memory access)
  - Disk controller tests
  - System integration tests

- **Test BIOS**: `test_roms/pc/bios.bin` built from assembly
- **Boot Sector**: `test_roms/pc/boot.bin` for boot testing

## Usage Example

```rust
use emu_pc::PcSystem;
use emu_core::System;

// Create system
let mut pc = PcSystem::new();

// Load disk image (optional)
let disk_data = std::fs::read("boot.img")?;
pc.mount("FloppyA", &disk_data)?;

// Run one frame
let frame = pc.step_frame()?;
```

## Keyboard Input

**Full Passthrough**: All keys sent to emulated PC

**Host Modifier** (Right Ctrl by default):
- Hold Right Ctrl + F3 to open file dialog
- Hold Right Ctrl + F4 to take screenshot
- Hold Right Ctrl + F5/F6 for save states

Without modifier, function keys go to DOS program.

## Known Limitations

See [MANUAL.md](../../../MANUAL.md#pcdos-ibm-pcxt) for user-facing limitations.

**Technical Limitations**:
- BIOS INT 13h exists but not fully connected to disk controller
- Boot sector loading infrastructure exists but not wired
- Frame-based timing (not cycle-accurate)

## Performance

- **Target**: ~60 FPS
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

**Short Term**:
- Connect disk controller to BIOS INT 13h
- Boot sector loading and execution
- Additional video modes

**Medium Term**:
- INT 10h (Video Services)
- INT 16h (Keyboard Services)
- PC speaker audio
- PIT timer

**Long Term**:
- EMS/XMS memory
- Mouse support
- Serial/parallel ports
- Protected mode (80286)

## Contributing

When adding PC features:

1. **Video Adapters**: Add to `src/video_adapter*.rs`
2. **BIOS Interrupts**: Add to `src/bios.rs` (when created)
3. **Tests**: Add unit tests for new functionality
4. **Documentation**: Update this README and [MANUAL.md](../../../MANUAL.md)

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [MANUAL.md](../../../MANUAL.md#pcdos-ibm-pcxt)
- **Contributing**: [CONTRIBUTING.md](../../../CONTRIBUTING.md)
- **OSDev Wiki**: https://wiki.osdev.org/

## License

Same as the parent Hemulator project.
