//! SNES cartridge implementation

use crate::coprocessors::{dsp1::Dsp1, sa1::Sa1, superfx::SuperFx, ChipType, EnhancementChip};
use crate::SnesError;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::RefCell;

/// ROM mapping mode
#[derive(Debug, Clone, Copy, PartialEq)]
enum MappingMode {
    LoROM,
    HiROM,
    ExHiROM,
}

/// SNES cartridge
pub struct Cartridge {
    /// ROM data
    rom: Vec<u8>,
    /// RAM (if present)
    ram: Vec<u8>,
    /// Header offset (512 bytes if SMC header present)
    header_offset: usize,
    /// Mapping mode (LoROM or HiROM)
    mapping_mode: MappingMode,
    /// Enhancement chip type
    chip_type: ChipType,
    /// Enhancement chip instance (if present and supported)
    chip: Option<RefCell<Box<dyn EnhancementChip + Send>>>,
}

impl Cartridge {
    pub fn load(data: &[u8]) -> Result<Self, SnesError> {
        if data.len() < 0x8000 {
            log(LogCategory::Bus, LogLevel::Error, || {
                format!(
                    "SNES Cartridge: ROM too small ({} bytes, minimum 32KB)",
                    data.len()
                )
            });
            return Err(SnesError::InvalidRom(
                "ROM too small (minimum 32KB)".to_string(),
            ));
        }

        // Check for SMC header (512 bytes)
        let header_offset = if data.len() % 1024 == 512 { 512 } else { 0 };

        let rom_data = &data[header_offset..];

        // Validate minimum ROM size
        if rom_data.len() < 0x8000 {
            log(LogCategory::Bus, LogLevel::Error, || {
                format!(
                    "SNES Cartridge: ROM data too small after header ({} bytes)",
                    rom_data.len()
                )
            });
            return Err(SnesError::InvalidRom(
                "ROM data too small after header".to_string(),
            ));
        }

        // Detect mapping mode from header
        // SNES ROM header is at $7FC0 (LoROM) or $FFC0 (HiROM)
        let mapping_mode = Self::detect_mapping_mode(rom_data);

        // Detect enhancement chip from header
        let (chip_type, chip) = Self::detect_chip(rom_data, mapping_mode);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "SNES Cartridge: Loaded ROM - Size: {} KB, SMC Header: {}, Mapping: {:?}, Chip: {}",
                rom_data.len() / 1024,
                if header_offset > 0 { "Yes" } else { "No" },
                mapping_mode,
                chip_type.name()
            )
        });

        Ok(Self {
            rom: rom_data.to_vec(),
            ram: vec![0; 0x8000], // 32KB SRAM (standard size)
            header_offset,
            mapping_mode,
            chip_type,
            chip,
        })
    }

    /// Detect mapping mode by checking ROM headers
    fn detect_mapping_mode(rom: &[u8]) -> MappingMode {
        // Try LoROM header at $7FC0 (offset in ROM)
        let lorom_header_offset = 0x7FC0;
        // Try HiROM header at $FFC0 (offset in ROM)
        let hirom_header_offset = 0xFFC0;

        // Prefer the header's map mode byte when it contains a known value.
        // This avoids false positives where the alternate header location happens
        // to look valid and makes LoROM games map as HiROM (causing ROM reads to
        // go out-of-bounds and return 0).
        let lorom_map_mode = rom
            .get(lorom_header_offset + 0x15)
            .copied()
            .and_then(Self::map_mode_byte_to_mapping);
        let hirom_map_mode = rom
            .get(hirom_header_offset + 0x15)
            .copied()
            .and_then(Self::map_mode_byte_to_mapping);

        match (lorom_map_mode, hirom_map_mode) {
            (Some(MappingMode::LoROM), Some(MappingMode::HiROM)) => {
                // Ambiguous: both locations claim different mappings. Fall back to scoring.
            }
            (Some(mode), None) => return mode,
            (None, Some(mode)) => return mode,
            (Some(mode), Some(_same_mode)) => return mode,
            (None, None) => {
                // Fall back to heuristic scoring.
            }
        }

        let lorom_score = if lorom_header_offset < rom.len() {
            Self::score_header(rom, lorom_header_offset)
        } else {
            0
        };

        let hirom_score = if hirom_header_offset < rom.len() {
            Self::score_header(rom, hirom_header_offset)
        } else {
            0
        };

        // If HiROM score is higher, use HiROM, otherwise default to LoROM
        if hirom_score > lorom_score {
            MappingMode::HiROM
        } else {
            MappingMode::LoROM
        }
    }

    fn map_mode_byte_to_mapping(map_mode: u8) -> Option<MappingMode> {
        // Common values:
        // - 0x20 = LoROM
        // - 0x21 = HiROM
        // - 0x25 = ExHiROM
        // - 0x30 = LoROM + FastROM
        // - 0x31 = HiROM + FastROM
        // - 0x35 = ExHiROM + FastROM
        match map_mode {
            0x20 | 0x30 => Some(MappingMode::LoROM),
            0x21 | 0x31 => Some(MappingMode::HiROM),
            0x25 | 0x35 => Some(MappingMode::ExHiROM),
            _ => None,
        }
    }

    /// Score a potential header location (higher = more likely valid)
    fn score_header(rom: &[u8], offset: usize) -> u32 {
        if offset + 0x40 > rom.len() {
            return 0;
        }

        let mut score = 0u32;

        // Check mapper type byte at +$15 (should be reasonable value)
        let mapper_type = rom[offset + 0x15];
        if mapper_type < 0x08 {
            score += 2; // Valid mapper type
        }

        // Check ROM size byte at +$17 (should be 0x07-0x0D typically)
        let rom_size = rom[offset + 0x17];
        if (0x07..=0x0D).contains(&rom_size) {
            score += 2; // Reasonable ROM size
        }

        // Check checksum complement at +$1C-$1D and checksum at +$1E-$1F
        let checksum_comp = u16::from_le_bytes([rom[offset + 0x1C], rom[offset + 0x1D]]);
        let checksum = u16::from_le_bytes([rom[offset + 0x1E], rom[offset + 0x1F]]);
        if checksum_comp == !checksum {
            score += 4; // Valid checksum pair
        }

        // Check reset vector at +$3C-$3D (should be reasonable address)
        let reset_vector = u16::from_le_bytes([rom[offset + 0x3C], rom[offset + 0x3D]]);
        if reset_vector >= 0x8000 {
            score += 2; // Valid reset vector (in ROM area)
        }

        score
    }

    /// Detect enhancement chip from ROM header
    fn detect_chip(
        rom: &[u8],
        mapping_mode: MappingMode,
    ) -> (ChipType, Option<RefCell<Box<dyn EnhancementChip + Send>>>) {
        // Determine header offset based on mapping mode
        let header_offset = match mapping_mode {
            MappingMode::LoROM => 0x7FC0,
            MappingMode::HiROM | MappingMode::ExHiROM => 0xFFC0,
        };

        if header_offset + 0x16 >= rom.len() {
            return (ChipType::None, None);
        }

        // Read ROM type byte (offset +$16 from header)
        let rom_type = rom[header_offset + 0x16];
        let map_mode = rom[header_offset + 0x15];

        // Detect chip type from ROM header
        let chip_type = ChipType::detect(rom_type, map_mode);

        // Create chip instance if supported
        let chip: Option<RefCell<Box<dyn EnhancementChip + Send>>> = match chip_type {
            ChipType::Dsp1 => {
                log(LogCategory::Bus, LogLevel::Info, || {
                    "SNES Cartridge: DSP-1 coprocessor detected".to_string()
                });
                Some(RefCell::new(Box::new(Dsp1::new())))
            }
            ChipType::SuperFx => {
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!("SNES Cartridge: {} coprocessor detected", chip_type.name())
                });
                let mut sfx = SuperFx::new();
                sfx.set_rom(rom.to_vec());
                Some(RefCell::new(Box::new(sfx)))
            }
            ChipType::SuperFx2 => {
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!("SNES Cartridge: {} coprocessor detected", chip_type.name())
                });
                let mut sfx = SuperFx::new_superfx2();
                sfx.set_rom(rom.to_vec());
                Some(RefCell::new(Box::new(sfx)))
            }
            ChipType::Sa1 => {
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!("SNES Cartridge: {} coprocessor detected", chip_type.name())
                });
                Some(RefCell::new(Box::new(Sa1::new())))
            }
            ChipType::None => None,
            _ => {
                log(LogCategory::Bus, LogLevel::Warn, || {
                    format!(
                        "SNES Cartridge: Enhancement chip {} detected but not implemented",
                        chip_type.name()
                    )
                });
                None
            }
        };

        (chip_type, chip)
    }

    pub fn read(&self, addr: u32) -> u8 {
        // Check if this address should be handled by the enhancement chip
        // DSP-1 in LoROM: banks $30-$3F at $3000-$3FFF (DR) and $7000-$7FFF (SR)
        // DSP-1 in HiROM: banks $00-$1F at $6000-$7FFF
        if let Some(ref chip) = self.chip {
            let bank = (addr >> 16) as u8;
            let offset = (addr & 0xFFFF) as u16;

            match (self.chip_type, self.mapping_mode) {
                (ChipType::Dsp1, MappingMode::LoROM) => {
                    // DSP-1 LoROM mapping
                    if matches!(bank, 0x30..=0x3F) && (offset >= 0x3000) {
                        return chip.borrow_mut().read(addr);
                    }
                }
                (ChipType::Dsp1, MappingMode::HiROM) => {
                    // DSP-1 HiROM mapping
                    if matches!(bank, 0x00..=0x1F) && (0x6000..=0x7FFF).contains(&offset) {
                        return chip.borrow_mut().read(addr);
                    }
                }
                (ChipType::SuperFx | ChipType::SuperFx2, _) => {
                    // SuperFX register mapping: $3000-$32FF in banks $00-$3F, $80-$BF
                    if matches!(bank, 0x00..=0x3F | 0x80..=0xBF)
                        && (0x3000..=0x32FF).contains(&offset)
                    {
                        return chip.borrow_mut().read(addr);
                    }
                    // SuperFX RAM: banks $70-$71 (128 KB) or $70-$73 (256 KB)
                    if matches!(bank, 0x70..=0x73) {
                        return chip.borrow_mut().read(addr);
                    }
                    // SuperFX ROM passthrough: banks $00-$3F and $80-$BF at $8000-$FFFF
                    // SuperFX games need CPU to access ROM through SuperFX for decryption/decompression
                    if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) && offset >= 0x8000 {
                        return chip.borrow_mut().read(addr);
                    }
                    // Also banks $40-$5F for higher ROM access
                    if matches!(bank, 0x40..=0x5F) {
                        return chip.borrow_mut().read(addr);
                    }
                }
                (ChipType::Sa1, _) => {
                    // SA-1 Register space: $2200-$23FF in all banks
                    if (0x2200..=0x23FF).contains(&offset) {
                        return chip.borrow_mut().read(addr);
                    }
                    // SA-1 I-RAM: $3000-$37FF in banks $00-$1F, $80-$9F
                    if matches!(bank, 0x00..=0x1F | 0x80..=0x9F)
                        && (0x3000..=0x37FF).contains(&offset)
                    {
                        return chip.borrow_mut().read(addr);
                    }
                    // SA-1 BW-RAM: banks $40-$4F, $60-$6F
                    if matches!(bank, 0x40..=0x4F | 0x60..=0x6F) {
                        return chip.borrow_mut().read(addr);
                    }
                }
                _ => {}
            }
        }

        match self.mapping_mode {
            MappingMode::LoROM => self.read_lorom(addr),
            MappingMode::HiROM => self.read_hirom(addr),
            MappingMode::ExHiROM => self.read_exhirom(addr),
        }
    }

    fn read_lorom(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // LoROM mapping: $8000-$FFFF in each bank maps to 32KB chunks
        match bank {
            0x00..=0x7D => {
                if offset >= 0x8000 {
                    let rom_offset =
                        ((bank as usize) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    self.read_rom_mirrored(rom_offset)
                } else if matches!(bank, 0x70..=0x7D) && offset < 0x8000 {
                    // SRAM in banks $70-$7D at $0000-$7FFF
                    *self.ram.get(offset as usize).unwrap_or(&0)
                } else {
                    0
                }
            }
            0x80..=0xFF => {
                if offset >= 0x8000 {
                    let rom_offset =
                        (((bank as usize) - 0x80) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    self.read_rom_mirrored(rom_offset)
                } else if matches!(bank, 0xF0..=0xFF) && offset < 0x8000 {
                    // SRAM in banks $F0-$FF at $0000-$7FFF (mirror)
                    *self.ram.get(offset as usize).unwrap_or(&0)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn read_hirom(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // HiROM mapping: Full 64KB per bank
        match bank {
            // Banks $00-$3F: SRAM at $6000-$7FFF, ROM at $8000-$FFFF
            0x00..=0x3F => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM mirror
                    let rom_offset = ((bank as usize) << 16) | (offset as usize);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $40-$7D: Full ROM access
            0x40..=0x7D => {
                let rom_offset = ((bank as usize) << 16) | (offset as usize);
                self.read_rom_mirrored(rom_offset)
            }
            // Banks $80-$BF: Mirror of $00-$3F
            0x80..=0xBF => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM (mirror)
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM mirror
                    let rom_offset = (((bank - 0x80) as usize) << 16) | (offset as usize);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $C0-$FF: Full ROM access (primary area)
            0xC0..=0xFF => {
                let rom_offset = (((bank - 0xC0) as usize) << 16) | (offset as usize);
                self.read_rom_mirrored(rom_offset)
            }
            _ => 0,
        }
    }

    fn read_exhirom(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // ExHiROM mapping: Extended HiROM for ROMs up to 8MB
        // Banks $00-$3F:$8000-$FFFF mirror $40-$7F:$0000-$FFFF (first 4MB)
        // Banks $C0-$FF:$0000-$FFFF contain second 4MB
        match bank {
            // Banks $00-$1F: ROM at $8000-$FFFF
            0x00..=0x1F => {
                if offset >= 0x8000 {
                    // ROM (mirrors $40-$5F area)
                    // Formula: ((Bank + $40) * $10000) + (Address - $8000)
                    let rom_offset = ((bank as usize + 0x40) << 16) | (offset as usize - 0x8000);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $20-$3F: SRAM at $6000-$7FFF, ROM at $8000-$FFFF
            0x20..=0x3F => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM (mirrors $60-$7F area)
                    // Formula: ((Bank + $40) * $10000) + (Address - $8000)
                    let rom_offset = ((bank as usize + 0x40) << 16) | (offset as usize - 0x8000);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $40-$7D: Extended ROM area (first 4MB of 8MB)
            0x40..=0x7D => {
                let rom_offset = ((bank as usize) << 16) | (offset as usize);
                self.read_rom_mirrored(rom_offset)
            }
            // Banks $80-$9F: Mirror of $00-$1F with same ROM mapping
            0x80..=0x9F => {
                if offset >= 0x8000 {
                    // ROM (mirrors $40-$5F area)
                    // Formula: ((Bank - $80 + $40) * $10000) + (Address - $8000)
                    let rom_offset =
                        ((bank as usize - 0x80 + 0x40) << 16) | (offset as usize - 0x8000);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $A0-$BF: Mirror of $20-$3F with same ROM mapping
            0xA0..=0xBF => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM (mirror)
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM (mirrors $60-$7F area)
                    // Formula: ((Bank - $A0 + $60) * $10000) + (Address - $8000)
                    let rom_offset =
                        ((bank as usize - 0xA0 + 0x60) << 16) | (offset as usize - 0x8000);
                    self.read_rom_mirrored(rom_offset)
                } else {
                    0
                }
            }
            // Banks $C0-$FF: Extended ROM area (second 4MB of 8MB)
            0xC0..=0xFF => {
                let rom_offset = ((bank as usize) << 16) | (offset as usize);
                self.read_rom_mirrored(rom_offset)
            }
            _ => 0,
        }
    }

    fn read_rom_mirrored(&self, rom_offset: usize) -> u8 {
        if self.rom.is_empty() {
            return 0;
        }
        let mirrored = rom_offset % self.rom.len();
        self.rom[mirrored]
    }

    pub fn write(&mut self, addr: u32, val: u8) {
        // Check if this address should be handled by the enhancement chip
        if let Some(ref chip) = self.chip {
            let bank = (addr >> 16) as u8;
            let offset = (addr & 0xFFFF) as u16;

            match (self.chip_type, self.mapping_mode) {
                (ChipType::Dsp1, MappingMode::LoROM) => {
                    // DSP-1 LoROM mapping (data register only)
                    if matches!(bank, 0x30..=0x3F) && (0x3000..0x7000).contains(&offset) {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                }
                (ChipType::Dsp1, MappingMode::HiROM) => {
                    // DSP-1 HiROM mapping
                    if matches!(bank, 0x00..=0x1F) && (0x6000..=0x7FFF).contains(&offset) {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                }
                (ChipType::SuperFx | ChipType::SuperFx2, _) => {
                    // SuperFX register mapping: $3000-$32FF in banks $00-$3F, $80-$BF
                    if matches!(bank, 0x00..=0x3F | 0x80..=0xBF)
                        && (0x3000..=0x32FF).contains(&offset)
                    {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                    // SuperFX RAM: banks $70-$71 (128 KB) or $70-$73 (256 KB)
                    if matches!(bank, 0x70..=0x73) {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                }
                (ChipType::Sa1, _) => {
                    // SA-1 Register space: $2200-$23FF in all banks
                    if (0x2200..=0x23FF).contains(&offset) {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                    // SA-1 I-RAM: $3000-$37FF in banks $00-$1F, $80-$9F
                    if matches!(bank, 0x00..=0x1F | 0x80..=0x9F)
                        && (0x3000..=0x37FF).contains(&offset)
                    {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                    // SA-1 BW-RAM: banks $40-$4F, $60-$6F
                    if matches!(bank, 0x40..=0x4F | 0x60..=0x6F) {
                        chip.borrow_mut().write(addr, val);
                        return;
                    }
                }
                _ => {}
            }
        }

        match self.mapping_mode {
            MappingMode::LoROM => self.write_lorom(addr, val),
            MappingMode::HiROM => self.write_hirom(addr, val),
            MappingMode::ExHiROM => self.write_exhirom(addr, val),
        }
    }

    fn write_lorom(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $70-$7D, $F0-$FF at $0000-$7FFF)
        if matches!(bank, 0x70..=0x7D | 0xF0..=0xFF) && offset < 0x8000 {
            let ram_offset = offset as usize;
            if ram_offset < self.ram.len() {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SNES Cartridge: LoROM SRAM Write ${:06X} = ${:02X}",
                        addr, val
                    )
                });
                self.ram[ram_offset] = val;
            }
        }
    }

    fn write_hirom(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $20-$3F, $A0-$BF at $6000-$7FFF)
        if matches!(bank, 0x20..=0x3F | 0xA0..=0xBF) && (0x6000..0x8000).contains(&offset) {
            let ram_offset = (offset - 0x6000) as usize;
            if ram_offset < self.ram.len() {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SNES Cartridge: HiROM SRAM Write ${:06X} = ${:02X}",
                        addr, val
                    )
                });
                self.ram[ram_offset] = val;
            }
        }
    }

    fn write_exhirom(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $20-$3F, $A0-$BF at $6000-$7FFF, same as HiROM)
        if matches!(bank, 0x20..=0x3F | 0xA0..=0xBF) && (0x6000..0x8000).contains(&offset) {
            let ram_offset = (offset - 0x6000) as usize;
            if ram_offset < self.ram.len() {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SNES Cartridge: ExHiROM SRAM Write ${:06X} = ${:02X}",
                        addr, val
                    )
                });
                self.ram[ram_offset] = val;
            }
        }
    }

    pub fn rom_size(&self) -> usize {
        self.rom.len()
    }

    pub fn has_smc_header(&self) -> bool {
        self.header_offset == 512
    }

    /// Get the enhancement chip type
    #[allow(dead_code)]
    pub fn chip_type(&self) -> ChipType {
        self.chip_type
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_hirom(&self) -> bool {
        self.mapping_mode == MappingMode::HiROM
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_exhirom(&self) -> bool {
        self.mapping_mode == MappingMode::ExHiROM
    }

    /// Tick the enhancement chip (if present) for the given number of master cycles
    /// This allows coprocessors like SuperFX to run asynchronously
    pub fn tick_chip(&mut self, master_cycles: u32) {
        if let Some(ref chip) = self.chip {
            // Only SuperFX needs continuous ticking
            if matches!(self.chip_type, ChipType::SuperFx | ChipType::SuperFx2) {
                // SuperFX runs at 21.48 MHz (same as master clock)
                // but internally counts in GSU cycles which are different
                // For now, we'll just run a proportional number of cycles
                let gsu_cycles = master_cycles as u64;
                chip.borrow_mut().tick(gsu_cycles);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_too_small() {
        let data = vec![0; 1024];
        assert!(Cartridge::load(&data).is_err());
    }

    #[test]
    fn test_load_with_smc_header() {
        let mut data = vec![0; 512 + 0x8000]; // 512-byte header + 32KB ROM
                                              // SMC header
        data.iter_mut().take(512).for_each(|x| *x = 0xFF);

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.header_offset, 512);
        assert_eq!(cart.rom.len(), 0x8000);
    }

    #[test]
    fn test_load_without_header() {
        let data = vec![0; 0x8000]; // 32KB ROM, no header

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.header_offset, 0);
        assert_eq!(cart.rom.len(), 0x8000);
    }

    #[test]
    fn test_read_rom_lorom() {
        let mut data = vec![0; 0x8000];
        data[0] = 0x42; // First byte

        let cart = Cartridge::load(&data).unwrap();

        // Bank 0, offset $8000 should read first ROM byte (LoROM)
        assert_eq!(cart.read(0x008000), 0x42);
        // Bank 0x80, offset $8000 should also read first ROM byte (mirror)
        assert_eq!(cart.read(0x808000), 0x42);
    }

    #[test]
    fn test_read_rom_hirom() {
        // Create a ROM large enough for HiROM with valid header
        let mut data = vec![0; 0x10000];

        // Set up HiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x01; // Mapper type
        data[header_offset + 0x17] = 0x09; // ROM size
        data[header_offset + 0x1C] = 0x00; // Checksum complement low
        data[header_offset + 0x1D] = 0x00; // Checksum complement high
        data[header_offset + 0x1E] = 0xFF; // Checksum low
        data[header_offset + 0x1F] = 0xFF; // Checksum high
        data[header_offset + 0x3C] = 0x00; // Reset vector low
        data[header_offset + 0x3D] = 0x80; // Reset vector high

        // Put test data at ROM start
        data[0] = 0x42;

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_hirom());

        // In HiROM, bank $C0 maps directly to ROM start
        assert_eq!(cart.read(0xC00000), 0x42);
    }

    #[test]
    fn test_write_read_ram_lorom() {
        let data = vec![0; 0x8000];
        let mut cart = Cartridge::load(&data).unwrap();
        assert!(!cart.is_hirom()); // Should be LoROM

        // Write to SRAM (bank $70, offset $0000)
        cart.write(0x700000, 0x55);

        // Read back
        assert_eq!(cart.ram[0], 0x55);
        assert_eq!(cart.read(0x700000), 0x55);
    }

    #[test]
    fn test_write_read_ram_hirom() {
        // Create HiROM ROM with valid header
        let mut data = vec![0; 0x10000];

        // Set up HiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x01; // Mapper type
        data[header_offset + 0x17] = 0x09; // ROM size
        data[header_offset + 0x1C] = 0x00; // Checksum complement low
        data[header_offset + 0x1D] = 0x00; // Checksum complement high
        data[header_offset + 0x1E] = 0xFF; // Checksum low
        data[header_offset + 0x1F] = 0xFF; // Checksum high
        data[header_offset + 0x3C] = 0x00; // Reset vector low
        data[header_offset + 0x3D] = 0x80; // Reset vector high

        let mut cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_hirom());

        // Write to SRAM (bank $20, offset $6000-$7FFF)
        cart.write(0x206000, 0xAA);

        // Read back
        assert_eq!(cart.ram[0], 0xAA);
        assert_eq!(cart.read(0x206000), 0xAA);
    }

    #[test]
    fn test_mapping_mode_detection() {
        // LoROM: header at $7FC0 should score higher
        let mut lorom_data = vec![0; 0x8000];
        lorom_data[0x7FC0 + 0x15] = 0x01; // Valid mapper
        lorom_data[0x7FC0 + 0x17] = 0x09; // Valid size
        lorom_data[0x7FC0 + 0x3C] = 0x00; // Reset vector
        lorom_data[0x7FC0 + 0x3D] = 0x80;

        let lorom_cart = Cartridge::load(&lorom_data).unwrap();
        assert!(!lorom_cart.is_hirom());

        // HiROM: header at $FFC0 should score higher
        let mut hirom_data = vec![0; 0x10000];
        hirom_data[0xFFC0 + 0x15] = 0x01; // Valid mapper
        hirom_data[0xFFC0 + 0x17] = 0x09; // Valid size
        hirom_data[0xFFC0 + 0x1C] = 0x00; // Checksum complement
        hirom_data[0xFFC0 + 0x1D] = 0x00;
        hirom_data[0xFFC0 + 0x1E] = 0xFF; // Checksum
        hirom_data[0xFFC0 + 0x1F] = 0xFF;
        hirom_data[0xFFC0 + 0x3C] = 0x00; // Reset vector
        hirom_data[0xFFC0 + 0x3D] = 0x80;

        let hirom_cart = Cartridge::load(&hirom_data).unwrap();
        assert!(hirom_cart.is_hirom());
    }

    #[test]
    fn test_exhirom_detection_0x25() {
        // Create an ExHiROM ROM with map mode byte $25
        let mut data = vec![0; 0x10000];

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM map mode
        data[header_offset + 0x17] = 0x0A; // ROM size (8MB)
        data[header_offset + 0x1C] = 0x00; // Checksum complement
        data[header_offset + 0x1D] = 0x00;
        data[header_offset + 0x1E] = 0xFF; // Checksum
        data[header_offset + 0x1F] = 0xFF;
        data[header_offset + 0x3C] = 0x00; // Reset vector
        data[header_offset + 0x3D] = 0x80;

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_exhirom());
        assert!(!cart.is_hirom());
    }

    #[test]
    fn test_exhirom_detection_0x35() {
        // Create an ExHiROM ROM with map mode byte $35 (FastROM)
        let mut data = vec![0; 0x10000];

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x35; // ExHiROM + FastROM map mode
        data[header_offset + 0x17] = 0x0A; // ROM size (8MB)
        data[header_offset + 0x1C] = 0x00; // Checksum complement
        data[header_offset + 0x1D] = 0x00;
        data[header_offset + 0x1E] = 0xFF; // Checksum
        data[header_offset + 0x1F] = 0xFF;
        data[header_offset + 0x3C] = 0x00; // Reset vector
        data[header_offset + 0x3D] = 0x80;

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_exhirom());
    }

    #[test]
    fn test_exhirom_read_banks_00_3f() {
        // Create an ExHiROM ROM
        let mut data = vec![0; 0x800000]; // 8MB ROM for proper testing

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM
        data[header_offset + 0x17] = 0x0C; // ROM size (8MB)
        data[header_offset + 0x1C] = 0x00; // Checksum complement
        data[header_offset + 0x1D] = 0x00;
        data[header_offset + 0x1E] = 0xFF; // Checksum
        data[header_offset + 0x1F] = 0xFF;
        data[header_offset + 0x3C] = 0x00; // Reset vector
        data[header_offset + 0x3D] = 0x80;

        // Put test data in bank $40 area (which will be mirrored by $00:$8000)
        // Bank $00:$8000 mirrors Bank $40:$0000
        data[0x400000] = 0xAB; // Bank $40:$0000 = SNES $00:$8000
        data[0x408000] = 0xCD; // Bank $40:$8000 = SNES $00:$10000 (wraps) or $01:$8000

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_exhirom());

        // Bank $00:$8000 should map to ROM offset $400000 (Bank $40:$0000)
        assert_eq!(cart.read(0x008000), 0xAB);
    }

    #[test]
    fn test_exhirom_read_banks_40_7d() {
        // Create an ExHiROM ROM large enough to test banks $40-$7D
        let mut data = vec![0; 0x800000]; // 8MB ROM

        // For ExHiROM, the header is at $40FFC0 for large ROMs
        // But detection looks at $FFC0 first, so put a valid ExHiROM header there
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM
        data[header_offset + 0x17] = 0x0C; // ROM size (8MB)
        data[header_offset + 0x1C] = 0x00; // Checksum complement
        data[header_offset + 0x1D] = 0x00;
        data[header_offset + 0x1E] = 0xFF; // Checksum
        data[header_offset + 0x1F] = 0xFF;
        data[header_offset + 0x3C] = 0x00; // Reset vector
        data[header_offset + 0x3D] = 0x80;

        // Put test data in bank $40 area
        data[0x400000] = 0xCD; // Bank $40:$0000
        data[0x401234] = 0xEF; // Bank $40:$1234

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_exhirom());

        // Banks $40-$7D map directly to ROM
        assert_eq!(cart.read(0x400000), 0xCD);
        assert_eq!(cart.read(0x401234), 0xEF);
    }

    #[test]
    fn test_exhirom_read_banks_c0_ff() {
        // Create an ExHiROM ROM
        let mut data = vec![0; 0x10000];

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM
        data[header_offset + 0x17] = 0x0A; // ROM size

        // Put test data
        data[0xC000] = 0x12;
        data[0xC001] = 0x34;

        let cart = Cartridge::load(&data).unwrap();

        // Banks $C0-$FF map directly to ROM
        assert_eq!(cart.read(0xC0C000), 0x12);
        assert_eq!(cart.read(0xC0C001), 0x34);
    }

    #[test]
    fn test_exhirom_sram_write_read() {
        // Create an ExHiROM ROM
        let mut data = vec![0; 0x10000];

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM
        data[header_offset + 0x17] = 0x0A; // ROM size

        let mut cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_exhirom());

        // Write to SRAM (bank $20, offset $6000-$7FFF, same as HiROM)
        cart.write(0x206000, 0x55);
        cart.write(0x207FFF, 0xAA);

        // Read back
        assert_eq!(cart.ram[0], 0x55);
        assert_eq!(cart.ram[0x1FFF], 0xAA);
        assert_eq!(cart.read(0x206000), 0x55);
        assert_eq!(cart.read(0x207FFF), 0xAA);

        // Test mirror in $A0-$BF range
        assert_eq!(cart.read(0xA06000), 0x55);
        cart.write(0xA16001, 0x77);
        assert_eq!(cart.read(0x206001), 0x77);
    }

    #[test]
    fn test_exhirom_mirror_banks_80_bf() {
        // Create an ExHiROM ROM
        let mut data = vec![0; 0x800000]; // 8MB ROM

        // Set up ExHiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x25; // ExHiROM
        data[header_offset + 0x17] = 0x0C; // ROM size (8MB)
        data[header_offset + 0x1C] = 0x00; // Checksum complement
        data[header_offset + 0x1D] = 0x00;
        data[header_offset + 0x1E] = 0xFF; // Checksum
        data[header_offset + 0x1F] = 0xFF;
        data[header_offset + 0x3C] = 0x00; // Reset vector
        data[header_offset + 0x3D] = 0x80;

        data[0x400000] = 0x99; // Bank $40:$0000

        let cart = Cartridge::load(&data).unwrap();

        // Banks $80-$BF at $8000-$FFFF should mirror $00-$3F behavior
        // which maps to bank $40-$7F ROM area
        // So $80:$8000 should read same as $00:$8000 which maps to $400000
        assert_eq!(cart.read(0x808000), 0x99);
        assert_eq!(cart.read(0x008000), 0x99);
    }
}
