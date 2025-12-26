# PC Emulation - IBM PC/XT

This crate implements IBM PC/XT emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The PC emulator is **experimental** with CGA/EGA/VGA graphics support and basic BIOS.

### What Works

- ✅ **CPU (8086)** - Complete instruction set from `emu_core::cpu_8086`
- ✅ **CPU Model Selection** - Support for 8086, 8088, 80186, 80188, 80286
- ✅ **Memory** - 640KB RAM, 128KB VRAM, 256KB ROM
- ✅ **BIOS** - Minimal custom BIOS built from assembly
- ✅ **Video Adapters** - CGA, EGA, VGA with multiple modes and runtime switching
- ✅ **Disk Controller** - Full INT 13h disk I/O (read, write, get params, reset)
- ✅ **Boot Sector Loading** - Loads from floppy/hard drive with boot priority
- ✅ **Keyboard** - Full passthrough with host modifier
- ✅ **INT 16h Integration** - Keyboard BIOS services connected to controller
- ✅ **Mount System** - Multi-slot disk image mounting with validation
- ✅ **Persistent Disk State** - Disk images are modified in-place (writes persist to files)

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

- ⏳ **Audio**: PC speaker not implemented
- ⏳ **Timer**: PIT (Programmable Interval Timer) not implemented
- ⏳ **Serial/Parallel**: No COM/LPT port support
- ⏳ **INT 10h**: Video BIOS services (set mode, cursor control, etc.) are stubs
- ⏳ **INT 16h**: Keyboard services AH=02h (shift flags) is a stub
- ⏳ **INT 21h**: DOS API functions are mostly stubs

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
- Easy mode switching at runtime via `set_video_adapter()`
- Pluggable rendering backends
- Clean separation of state and rendering
- Future GPU acceleration support

**API Examples**:
```rust
use emu_pc::{PcSystem, SoftwareCgaAdapter, SoftwareEgaAdapter, SoftwareVgaAdapter};

let mut sys = PcSystem::new();

// Switch video adapters at runtime
sys.set_video_adapter(Box::new(SoftwareEgaAdapter::new()));
assert_eq!(sys.video_adapter_name(), "Software EGA Adapter");

// Check framebuffer dimensions
let (width, height) = sys.framebuffer_dimensions();
assert_eq!((width, height), (640, 350)); // EGA resolution
```

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

- **136 total tests** (9 new tests added):
  - CPU tests (8086 instruction set, INT 13h, INT 16h keyboard integration)
  - Video adapter tests (CGA, EGA, VGA modes)
  - **Video adapter switching tests** (runtime adapter changes)
  - Bus tests (memory access)
  - Disk controller tests
  - Keyboard tests (including peek functionality)
  - **Mount validation tests** (disk size validation)
  - System integration tests

- **Test Boot Sectors**: Various test ROMs in `test_roms/pc/` (see test_roms/pc/README.md)

## Usage Example

```rust
use emu_pc::{PcSystem, SoftwareVgaAdapter, BootPriority};
use emu_core::System;

// Create system with specific CPU model
let mut pc = PcSystem::with_cpu_model(emu_pc::PcCpuModel::Intel80286);

// Switch to VGA adapter
pc.set_video_adapter(Box::new(SoftwareVgaAdapter::new()));

// Load disk image
let disk_data = std::fs::read("boot.img")?;
pc.mount("FloppyA", &disk_data)?;

// Set boot priority
pc.set_boot_priority(BootPriority::FloppyFirst);

// Run the system - disk writes happen in-memory
let frame = pc.step_frame()?;

// Note: PC systems don't use save states like ROM-based consoles
// Disk state changes are in-memory on the mounted disk image
// To persist changes, you would need to write the disk image back to disk
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
- **No save states**: PC systems don't use save states like ROM-based consoles
  - System state is preserved in the disk images themselves
  - Disk writes are performed in-memory on the mounted disk image
  - To persist changes, the disk image would need to be written back to the file system
  - This is fundamentally different from NES/GB where ROM is read-only and state is separate
- INT 10h (Video BIOS) is partially implemented (teletype, cursor control work; mode switching is stub)
- INT 16h (Keyboard) read/check functions work; shift flags is stub
- INT 21h (DOS API) is partially implemented (character I/O works; file operations are stubs)
- Frame-based timing (not cycle-accurate)
- No PC speaker audio
- No PIT timer
- No serial/parallel ports

## Performance

- **Target**: ~60 FPS
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

**Short Term**:
- Expand INT 10h video services (more functions)
- Expand INT 16h keyboard services (actual key reading)
- Expand INT 21h DOS API (file I/O, etc.)
- Additional video modes

**Medium Term**:
- PC speaker audio
- PIT timer
- More complete DOS compatibility

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
