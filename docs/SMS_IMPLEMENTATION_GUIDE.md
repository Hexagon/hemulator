# Sega Master System Implementation Guide

**Target System**: Sega Master System / Game Gear  
**Prerequisite Reading**: [NEXT_EMULATOR_RECOMMENDATION.md](NEXT_EMULATOR_RECOMMENDATION.md)

This guide provides practical implementation steps for adding Sega Master System emulation to Hemulator.

## Quick Reference

### Hardware Specifications

| Component | Specification |
|-----------|--------------|
| **CPU** | Zilog Z80A @ 3.58 MHz (NTSC) / 3.55 MHz (PAL) |
| **RAM** | 8 KB main RAM |
| **VRAM** | 16 KB video RAM |
| **VDP** | Sega 315-5124 (SMS 1), 315-5246 (SMS 2) |
| **PSG** | Texas Instruments SN76489 (Sega variant SN76496) |
| **Resolution** | 256×192 pixels @ 60Hz (NTSC) / 50Hz (PAL) |
| **Colors** | 64 colors (6-bit RGB), 32 simultaneous |
| **Sprites** | 64 total, 8 per scanline |
| **ROM Sizes** | Typically 8KB to 512KB, up to 1MB with mappers |

## Implementation Order

Follow this order to minimize dependencies and enable incremental testing:

### 1. Complete Z80 CPU (`crates/core/src/cpu_z80.rs`)

**Current State**: Stub with registers and basic structure  
**Required**: Full instruction set implementation

**Steps:**

1. **Study existing CPU implementations**:
   - Reference: `crates/core/src/cpu_6502.rs` (similar structure)
   - Reference: `crates/core/src/cpu_lr35902.rs` (Z80 derivative)
   
2. **Implement base instructions** (non-prefixed opcodes):
   - Load/Store: `LD r,r'`, `LD r,n`, `LD r,(HL)`, etc.
   - Arithmetic: `ADD`, `ADC`, `SUB`, `SBC`, `INC`, `DEC`
   - Logic: `AND`, `OR`, `XOR`, `CP`
   - Bit operations: `BIT`, `SET`, `RES`
   - Jumps: `JP`, `JR`, `CALL`, `RET`
   - Stack: `PUSH`, `POP`

3. **Implement prefixed instructions**:
   - `0xCB` prefix: Bit operations
   - `0xED` prefix: Extended instructions
   - `0xDD` prefix: IX index register operations
   - `0xFD` prefix: IY index register operations

4. **Implement interrupt system**:
   - IM 0, IM 1, IM 2 modes
   - NMI handling
   - IFF1/IFF2 flags
   - SMS primarily uses IM 1 (jump to $0038)

5. **Add cycle counting**:
   - Each instruction has specific cycle count
   - Essential for accurate emulation timing

**Testing**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_z80_ld_r_n() {
        let mem = TestMemory::new();
        let mut cpu = CpuZ80::new(mem);
        // Load 0x42 into register A
        cpu.memory.write(0x0000, 0x3E); // LD A, n
        cpu.memory.write(0x0001, 0x42);
        cpu.step();
        assert_eq!(cpu.a, 0x42);
    }
}
```

**References**:
- [Z80 User Manual - Zilog](http://www.zilog.com/docs/z80/um0080.pdf)
- [Z80 Instruction Set](http://www.z80.info/z80oplist.txt)
- Internal: `docs/references/cpu_z80.md`

### 2. Create SN76489 PSG (`crates/core/src/apu/sn76489.rs`)

**Architecture**: 3 square wave channels + 1 noise channel

**Steps**:

1. **Define PSG state**:
```rust
pub struct Sn76489Psg {
    // Tone generators (3 channels)
    tone_channels: [PulseChannel; 3],
    
    // Noise generator
    noise_channel: NoiseChannel,
    
    // Volume registers (4-bit, 0=max, 15=min/mute)
    volumes: [u8; 4],
    
    // Frequency registers (10-bit for tones)
    frequencies: [u16; 3],
    
    // Noise control
    noise_control: u8,
    
    // Sample rate
    sample_rate: u32,
    clock_rate: u32,
}
```

2. **Implement register writes**:
```rust
pub fn write(&mut self, data: u8) {
    if data & 0x80 != 0 {
        // Latch/data byte
        let channel = (data >> 5) & 0x03;
        let is_volume = (data >> 4) & 0x01;
        
        if is_volume != 0 {
            self.volumes[channel as usize] = data & 0x0F;
        } else {
            // Frequency data (low 4 bits)
            // ...
        }
    } else {
        // Data byte (6 bits)
        // ...
    }
}
```

3. **Implement audio generation**:
```rust
pub fn sample(&mut self) -> f32 {
    let mut output = 0.0;
    
    // Mix tone channels
    for i in 0..3 {
        output += self.tone_channels[i].sample() 
                * volume_to_amplitude(self.volumes[i]);
    }
    
    // Mix noise channel
    output += self.noise_channel.sample() 
            * volume_to_amplitude(self.volumes[3]);
    
    output / 4.0 // Average
}
```

4. **Implement AudioChip trait**:
```rust
impl AudioChip for Sn76489Psg {
    fn write_register(&mut self, addr: u16, val: u8) {
        // SMS writes to PSG at I/O port 0x7F
        if addr == 0x7F {
            self.write(val);
        }
    }
    
    fn sample(&mut self) -> f32 {
        self.sample()
    }
    
    fn reset(&mut self) {
        // Reset all channels
    }
}
```

**Testing**:
```rust
#[test]
fn test_psg_tone_output() {
    let mut psg = Sn76489Psg::new(44100, 3579545);
    
    // Set channel 0 to 440 Hz (A note)
    // Frequency = Clock / (32 * Register)
    let register_value = 3579545 / (32 * 440);
    
    psg.write(0x80 | 0x00); // Latch tone 0, data type
    psg.write(register_value as u8); // Low 4 bits
    psg.write((register_value >> 4) as u8); // High 6 bits
    
    psg.write(0x90); // Latch tone 0, volume = 0 (max)
    
    let sample = psg.sample();
    assert!(sample != 0.0);
}
```

**References**:
- [SN76489 - Wikipedia](https://en.wikipedia.org/wiki/Texas_Instruments_SN76489)
- [VGMPF SN76489 Documentation](https://www.vgmpf.com/Wiki/index.php?title=SN76489)

### 3. Implement VDP (`crates/systems/sms/src/vdp.rs`)

**Architecture**: Tilemap-based graphics with sprite overlay

**Steps**:

1. **Define VDP state**:
```rust
pub struct Vdp {
    // Video RAM (16KB)
    vram: [u8; 0x4000],
    
    // Color RAM (32 bytes for palette)
    cram: [u8; 0x20],
    
    // VDP registers (11 registers)
    registers: [u8; 11],
    
    // Internal state
    address_register: u16,
    code_register: u8,
    read_buffer: u8,
    write_latch: bool,
    
    // Rendering
    frame_buffer: Vec<u32>,
    
    // Interrupts
    frame_interrupt: bool,
    line_interrupt: bool,
    line_counter: u8,
}
```

2. **Implement VDP control port** (port 0xBF):
```rust
pub fn write_control(&mut self, data: u8) {
    if !self.write_latch {
        // First byte
        self.address_register = (self.address_register & 0x3F00) 
                              | data as u16;
        self.write_latch = true;
    } else {
        // Second byte
        self.address_register = (self.address_register & 0x00FF) 
                              | ((data as u16 & 0x3F) << 8);
        self.code_register = (data >> 6) & 0x03;
        self.write_latch = false;
        
        // Check if register write
        if self.code_register == 0x02 {
            let reg = data & 0x0F;
            if reg < 11 {
                self.registers[reg as usize] = 
                    (self.address_register & 0xFF) as u8;
            }
        }
    }
}
```

3. **Implement VDP data port** (port 0xBE):
```rust
pub fn write_data(&mut self, data: u8) {
    self.write_latch = false;
    self.read_buffer = data;
    
    match self.code_register {
        0x03 => {
            // CRAM write
            self.cram[(self.address_register & 0x1F) as usize] = data;
        }
        _ => {
            // VRAM write
            self.vram[(self.address_register & 0x3FFF) as usize] = data;
        }
    }
    
    self.address_register = self.address_register.wrapping_add(1);
}

pub fn read_data(&mut self) -> u8 {
    self.write_latch = false;
    let value = self.read_buffer;
    self.read_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
    self.address_register = self.address_register.wrapping_add(1);
    value
}
```

4. **Implement background rendering**:
```rust
fn render_background(&mut self, line: u8) {
    let name_table_addr = ((self.registers[2] as u16) & 0x0E) << 10;
    
    // Calculate scroll offsets
    let scroll_x = self.registers[8];
    let scroll_y = if line >= 16 && (self.registers[0] & 0x40) != 0 {
        0 // Vertical scroll lock for top 2 rows
    } else {
        self.registers[9]
    };
    
    let y = line.wrapping_add(scroll_y);
    let tile_row = (y >> 3) as u16;
    
    for x in 0..256 {
        let adj_x = x.wrapping_sub(scroll_x);
        let tile_col = (adj_x >> 3) as u16;
        
        // Read name table entry
        let name_addr = name_table_addr + (tile_row * 32 + tile_col) * 2;
        let tile_data = self.vram[name_addr as usize] as u16 
                      | ((self.vram[(name_addr + 1) as usize] as u16) << 8);
        
        let tile_index = tile_data & 0x1FF;
        let palette_bit = (tile_data >> 11) & 1;
        let priority = (tile_data >> 12) & 1;
        let h_flip = (tile_data >> 9) & 1;
        let v_flip = (tile_data >> 10) & 1;
        
        // Render tile pixel
        // ...
    }
}
```

5. **Implement sprite rendering**:
```rust
fn render_sprites(&mut self, line: u8) {
    let sprite_attr_table = ((self.registers[5] as u16) & 0x7E) << 7;
    let sprite_size = if (self.registers[1] & 0x02) != 0 { 16 } else { 8 };
    
    let mut sprites_on_line = 0;
    
    for i in 0..64 {
        let y = self.vram[(sprite_attr_table + i) as usize];
        
        // Check if sprite is on this line
        if y == 0xD0 { break; } // End marker
        
        let y_pos = y.wrapping_add(1);
        if line >= y_pos && line < y_pos + sprite_size {
            sprites_on_line += 1;
            if sprites_on_line > 8 {
                // Sprite overflow flag
                break;
            }
            
            // Render sprite
            // ...
        }
    }
}
```

6. **Implement Renderer trait**:
```rust
impl Renderer for Vdp {
    fn get_frame(&self) -> &[u32] {
        &self.frame_buffer
    }
    
    fn clear(&mut self) {
        self.frame_buffer.fill(0);
    }
    
    fn reset(&mut self) {
        self.vram.fill(0);
        self.cram.fill(0);
        self.registers.fill(0);
        self.clear();
    }
    
    fn resize(&mut self, _width: u32, _height: u32) {
        // SMS has fixed resolution
    }
    
    fn name(&self) -> &str {
        "SMS VDP"
    }
}
```

**Testing**:
```rust
#[test]
fn test_vdp_register_write() {
    let mut vdp = Vdp::new();
    
    // Write to register 0
    vdp.write_control(0x00); // Low byte
    vdp.write_control(0x80); // High byte (register write, reg 0)
    
    assert_eq!(vdp.registers[0], 0x00);
}
```

**References**:
- [Charles MacDonald's VDP Documentation](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt)
- [SMS Power! VDP Documentation](https://www.smspower.org/Development/VDP)

### 4. Build System Integration (`crates/systems/sms/`)

**Steps**:

1. **Create system structure**:
```rust
pub struct SmsSystem {
    cpu: CpuZ80<SmsMemory>,
    vdp: Vdp,
    psg: Sn76489Psg,
    
    // Memory
    rom: Vec<u8>,
    ram: [u8; 0x2000], // 8KB
    
    // I/O
    io_control: u8,
    
    // Timing
    cycles: u64,
}
```

2. **Implement memory map**:
```rust
impl MemoryZ80 for SmsMemory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => {
                // ROM (up to 48KB direct mapped)
                self.rom.get(addr as usize).copied().unwrap_or(0xFF)
            }
            0xC000..=0xDFFF => {
                // RAM (8KB mirrored)
                self.ram[(addr & 0x1FFF) as usize]
            }
            0xE000..=0xFFFF => {
                // RAM mirror
                self.ram[(addr & 0x1FFF) as usize]
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }
    
    fn io_read(&mut self, port: u8) -> u8 {
        match port {
            0x7E | 0x7F => {
                // VDP vertical counter
                self.vdp.read_vcounter()
            }
            0xBE => self.vdp.read_data(),
            0xBF => self.vdp.read_status(),
            0xDC | 0xDD => {
                // Controller ports
                self.read_controller(port)
            }
            _ => 0xFF,
        }
    }
    
    fn io_write(&mut self, port: u8, val: u8) {
        match port {
            0x7E | 0x7F => self.psg.write(val),
            0xBE => self.vdp.write_data(val),
            0xBF => self.vdp.write_control(val),
            0x3E => {
                // Memory control (banking)
                self.memory_control = val;
            }
            _ => {}
        }
    }
}
```

3. **Implement System trait**:
```rust
impl System for SmsSystem {
    fn reset(&mut self) {
        self.cpu.reset();
        self.vdp.reset();
        self.psg.reset();
        self.cycles = 0;
    }
    
    fn step_frame(&mut self) -> (Frame, Vec<f32>) {
        let target_cycles = 59659; // ~3.58 MHz / 60 Hz
        
        while self.cycles < target_cycles {
            // Execute CPU instruction
            let cpu_cycles = self.cpu.step() as u64;
            self.cycles += cpu_cycles;
            
            // Update VDP (3 pixels per CPU cycle)
            for _ in 0..(cpu_cycles * 3) {
                self.vdp.step();
            }
            
            // Generate audio samples
            // ...
        }
        
        self.cycles -= target_cycles;
        
        let frame = Frame::from_buffer(
            self.vdp.get_frame(),
            256,
            192,
        );
        
        (frame, self.audio_buffer.drain(..).collect())
    }
    
    fn name(&self) -> &str {
        "Sega Master System"
    }
}
```

### 5. ROM Loading and Detection

**ROM Format**:
- Most ROMs are headerless raw binaries
- Some have 512-byte header (TMR SEGA format)
- Typical sizes: 8KB, 16KB, 32KB, 48KB, 64KB, 128KB, 256KB, 512KB

**Detection**:
```rust
pub fn detect_sms_rom(data: &[u8]) -> bool {
    // Check for TMR SEGA header
    if data.len() >= 512 + 0x7FF0 {
        let header_offset = if data.len() >= 512 + 0x7FF0 + 16 {
            512 // Has header
        } else {
            0 // Headerless
        };
        
        let sig_offset = header_offset + 0x7FF0;
        if sig_offset + 16 <= data.len() {
            let signature = &data[sig_offset..sig_offset + 8];
            if signature == b"TMR SEGA" {
                return true;
            }
        }
    }
    
    // Check size (common SMS ROM sizes)
    matches!(data.len(), 
        8192 | 16384 | 32768 | 49152 | 65536 | 
        131072 | 262144 | 524288)
}
```

### 6. Banking Support

SMS uses simple paging for ROMs > 48KB:

```rust
impl SmsMemory {
    fn update_banking(&mut self) {
        // Paging registers at 0xFFFC, 0xFFFD, 0xFFFE
        let frame_0 = self.ram[0x1FFC] as usize;
        let frame_1 = self.ram[0x1FFD] as usize;
        let frame_2 = self.ram[0x1FFE] as usize;
        
        // Map 16KB banks
        self.rom_bank_0 = frame_0 % self.num_banks;
        self.rom_bank_1 = frame_1 % self.num_banks;
        self.rom_bank_2 = frame_2 % self.num_banks;
    }
}
```

## Testing Strategy

### 1. Unit Tests
- CPU instruction tests (per opcode)
- PSG channel tests (frequency, volume)
- VDP register tests

### 2. Test ROM
Create minimal test ROM in `test_roms/sms/`:

```assembly
; test.asm - SMS test ROM
.org $0000

    di              ; Disable interrupts
    ld sp, $DFF0    ; Set stack pointer
    
    ; Initialize VDP
    ld hl, vdp_init
    ld b, 11
    ld c, $BF
init_loop:
    ld a, (hl)
    out (c), a
    inc hl
    djnz init_loop
    
    ; Main loop
main_loop:
    halt
    jr main_loop

vdp_init:
    .db $04, $80    ; Reg 0: mode control 1
    .db $00, $81    ; Reg 1: mode control 2
    ; ...more registers
```

### 3. Integration Test
```rust
#[test]
fn smoke_test_sms() {
    let rom = include_bytes!("../../test_roms/sms/test.sms");
    let mut system = SmsSystem::new(rom.to_vec());
    
    system.reset();
    let (frame, _audio) = system.step_frame();
    
    assert_eq!(frame.width, 256);
    assert_eq!(frame.height, 192);
    // Verify expected pixel pattern
}
```

## Common Pitfalls

1. **Z80 Flags**: Carefully implement all flag behaviors (especially Half-Carry)
2. **VDP Timing**: Line interrupts occur at specific scanline positions
3. **PSG Noise**: Sega variant uses 16-bit LFSR (not 15-bit like original)
4. **Banking**: Some games expect specific initial bank configuration
5. **Controller Reading**: Must handle both ports 0xDC and 0xDD

## Game Gear Differences

Once SMS works, add Game Gear support:

```rust
pub struct GameGearSystem {
    base: SmsSystem, // Reuse SMS implementation
    gg_start_button: bool,
}

impl GameGearSystem {
    fn convert_color(&self, sms_color: u8) -> u32 {
        // Game Gear uses 12-bit color (4096 colors)
        // vs SMS 6-bit (64 colors)
        let r = (sms_color & 0x03) << 2;
        let g = ((sms_color >> 2) & 0x03) << 2;
        let b = ((sms_color >> 4) & 0x03) << 2;
        
        let r32 = ((r << 4) | r) as u32;
        let g32 = ((g << 4) | g) as u32;
        let b32 = ((b << 4) | b) as u32;
        
        (r32 << 16) | (g32 << 8) | b32
    }
}
```

## Resources

### Essential Documentation
- [SMS Power! Development Portal](https://www.smspower.org/Development/)
- [Charles MacDonald's SMS Documentation](https://github.com/franckverrot/EmulationResources/tree/master/consoles/sms-gg)
- [Rodrigo Copetti's SMS Architecture](https://www.copetti.org/writings/consoles/master-system/)

### Reference Emulators
- [Gearsystem](https://github.com/drhelius/Gearsystem) - Open source, good reference
- [Emulicious](https://emulicious.net/) - Excellent debugger

### Test ROMs
- [SMS Power! Homebrew](https://www.smspower.org/Homebrew/)
- Create your own minimal test ROMs

---

**Next Steps**: Once comfortable with this guide, start with Phase 1 (Z80 CPU) and work incrementally through each phase, testing thoroughly at each step.
