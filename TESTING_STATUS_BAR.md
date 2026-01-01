# Status Bar Testing Guide

This document describes how to manually test the status bar improvements.

## Changes Made

1. **Rendering Backend Indicator**: Status bar now shows "Software" or "OpenGL"
2. **CPU Frequency Display**: Shows target CPU frequency in MHz for each system
3. **Instruction Pointer (IP) Fix**: IP now updates correctly for all systems

## Test Cases

### 1. Test NES System

```bash
./target/release/hemu test_roms/nes/test.nes
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `1.8MHz` - NES CPU frequency (1.79 MHz)
- `60fps` - Frame rate
- `$XXXX` - 4-digit hex IP (updates every frame)
- `XXK` - Cycle count

### 2. Test Game Boy System

```bash
./target/release/hemu test_roms/gb/test.gb
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `4.2MHz` - Game Boy CPU frequency (4.19 MHz)
- `60fps` - Frame rate
- `$XXXX` - 4-digit hex IP (updates every frame)

### 3. Test PC System

```bash
./target/release/hemu test_roms/pc/basic_boot/basic_boot.com
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `4.8MHz` - PC CPU frequency (4.77 MHz for 8086)
- `60fps` - Frame rate
- `$XXXXX` - 5-6 digit hex IP (CS:IP linear address, updates every frame)

### 4. Test Atari 2600 System

```bash
./target/release/hemu test_roms/atari2600/test.bin
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `1.2MHz` - Atari 2600 CPU frequency (1.19 MHz)
- `60fps` - Frame rate
- IP may not be shown (not exposed by Atari 2600 system)

### 5. Test SNES System

```bash
./target/release/hemu test_roms/snes/test.sfc
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `3.6MHz` - SNES CPU frequency (3.58 MHz)
- `60fps` - Frame rate
- `$XXXXXX` - 6-digit hex IP (PBR:PC 24-bit address, updates every frame)

### 6. Test N64 System

```bash
./target/release/hemu test_roms/n64/test.z64
```

**Expected Status Bar Display (right side):**
- `Software` - Rendering backend
- `93.8MHz` - N64 CPU frequency (93.75 MHz)
- `60fps` - Frame rate
- `$XXXXXXXX` - 8-digit hex IP (PC address, updates every frame)

### 7. Test OpenGL Backend (if available)

If OpenGL backend is configured:

```bash
# Edit config.json to set "video_backend": "opengl"
./target/release/hemu test_roms/nes/test.nes
```

**Expected Status Bar Display (right side):**
- `OpenGL` - Rendering backend (instead of Software)
- Rest should be the same as Software backend

## Verification Points

1. **Rendering Backend**: Check that it shows either "Software" or "OpenGL"
2. **CPU Frequency**: Verify the frequency matches the expected value for each system
3. **IP Updates**: The IP value should change frequently (every few frames)
4. **IP Format**: 
   - NES/GB: 4 hex digits ($XXXX)
   - PC: 5-6 hex digits ($XXXXX)
   - SNES: 6 hex digits ($XXXXXX)
   - N64: 8 hex digits ($XXXXXXXX)
5. **Layout**: All elements should fit on the status bar without overlapping

## Known Limitations

- Actual CPU frequency is not yet implemented (only target frequency is shown)
- Atari 2600 does not expose IP in its debug info
