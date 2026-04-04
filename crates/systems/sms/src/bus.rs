//! Sega Master System memory bus implementation
//!
//! Supports 9 mapper types:
//! 1. **None** – No banking, ROM ≤ 48 KB, direct mapping
//! 2. **Sega** – Standard SMS mapper ($FFFD/$FFFE/$FFFF)
//! 3. **Codemasters** – ROM-area writes at $0000/$4000/$8000
//! 4. **Korean** – Single register at $A000 (banks slot 2)
//! 5. **Korean8k** – Four 8 KB banks via $4000/$6000/$8000/$A000
//! 6. **MSX** – Registers at $0000/$0001/$0002/$0003
//! 7. **Nemesis** – Starts with last ROM bank in slot 0
//! 8. **FourPak** – 4PAK All Action multicart ($3FFE/$7FFE/$BFFE)
//! 9. **Janggun** – Janggun-ui Adeul extended Korean mapper

use crate::psg::SmsPsg;
use crate::vdp::Vdp;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::logging::{log, LogCategory, LogLevel};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Mapper type enum
// ---------------------------------------------------------------------------

/// All recognised SMS/GG mapper types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperType {
    /// No banking – ROM ≤ 48 KB.
    None,
    /// Standard Sega mapper ($FFFD / $FFFE / $FFFF in RAM).
    Sega,
    /// Codemasters mapper (writes to $0000 / $4000 / $8000 in ROM area).
    Codemasters,
    /// Korean mapper – single $A000 register banks slot 2.
    Korean,
    /// Korean 8 K mapper – four 8 KB registers at $4000 / $6000 / $8000 / $A000.
    Korean8k,
    /// MSX-style mapper – registers at $0000–$0003.
    Msx,
    /// Nemesis / "The Castle" – slot 0 starts at last bank.
    Nemesis,
    /// 4PAK All Action multicart ($3FFE / $7FFE / $BFFE).
    FourPak,
    /// Janggun-ui Adeul (Korean extended mapper with 8 KB pages + XOR).
    Janggun,
}

impl MapperType {
    /// Human-readable name used for logging and debug UI.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Sega => "Sega",
            Self::Codemasters => "Codemasters",
            Self::Korean => "Korean",
            Self::Korean8k => "Korean 8K",
            Self::Msx => "MSX",
            Self::Nemesis => "Nemesis",
            Self::FourPak => "4PAK All Action",
            Self::Janggun => "Janggun",
        }
    }

    /// String key used for JSON serialisation.
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sega => "sega",
            Self::Codemasters => "codemasters",
            Self::Korean => "korean",
            Self::Korean8k => "korean8k",
            Self::Msx => "msx",
            Self::Nemesis => "nemesis",
            Self::FourPak => "fourpak",
            Self::Janggun => "janggun",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "sega" => Self::Sega,
            "codemasters" => Self::Codemasters,
            "korean" => Self::Korean,
            "korean8k" => Self::Korean8k,
            "msx" => Self::Msx,
            "nemesis" => Self::Nemesis,
            "fourpak" => Self::FourPak,
            "janggun" => Self::Janggun,
            _ => Self::Sega, // safe default
        }
    }
}

// ---------------------------------------------------------------------------
// Mapper auto-detection
// ---------------------------------------------------------------------------

/// Detect the mapper type from a ROM image.
///
/// Detection order:
/// 1. CRC32 database for known special-mapper games
/// 2. Codemasters checksum heuristic
/// 3. Size-based: ≤ 48 KB → None, > 48 KB → Sega
fn detect_mapper(rom: &[u8]) -> MapperType {
    // --- CRC32 database for uncommon mappers ----------------------------------
    let crc = crc32(rom);

    if let Some(mt) = crc_lookup(crc) {
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("SMS mapper: detected {} via CRC32 {:08X}", mt.name(), crc)
        });
        return mt;
    }

    // --- Codemasters checksum heuristic --------------------------------------
    if is_codemasters(rom) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SMS mapper: detected Codemasters via checksum".to_string()
        });
        return MapperType::Codemasters;
    }

    // --- Fallback by size ----------------------------------------------------
    if rom.len() <= 0xC000 {
        MapperType::None
    } else {
        MapperType::Sega
    }
}

/// Simple CRC32 (used only for mapper detection, no crypto needed).
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Lookup a CRC32 in the built-in database of known special-mapper games.
fn crc_lookup(crc: u32) -> Option<MapperType> {
    // Korean mapper ($A000 register)
    const KOREAN: &[u32] = &[
        0x17AB6883, // Sangokushi 3 (KR)
        0x77EFE84A, // Dodgeball King (KR)
        0xA05258F5, // Jang Pung II (KR)
        0x929222C4, // Jang Pung 3 (KR)
    ];

    // Korean 8K mapper ($4000/$6000/$8000/$A000)
    const KOREAN_8K: &[u32] = &[
        0x89B79E77, // Zemina - Wonsiin (KR)
        0x18FB98A3, // Zemina - Street Master (KR)
        0x97D03541, // Zemina - Cyborg Z (KR)
        0xA67F2A5C, // Zemina - F-1 Spirit (KR)
        0x0A77FA5E, // Zemina - Knightmare (KR)
        0xDAB66797, // Zemina - Nemesis (KR)
        0x83F0EEDE, // Zemina - Nemesis 2 (KR)
        0xF89AF3CC, // Zemina - Super Boy II (KR)
    ];

    // MSX mapper ($0000-$0003)
    const MSX: &[u32] = &[
        0x0A440F96, // Zemina - MSX adapter games
        0x2BCDB8FA, // Zemina - Wonsiin II (KR)
    ];

    // Nemesis / The Castle
    const NEMESIS: &[u32] = &[
        0x2E366CCF, // The Castle (KR)
    ];

    // 4PAK All Action
    const FOURPAK: &[u32] = &[
        0xD8084A30, // 4 PAK All Action (AU)
    ];

    // Janggun-ui Adeul
    const JANGGUN: &[u32] = &[
        0x192949D5, // Janggun-ui Adeul (KR)
    ];

    if KOREAN.contains(&crc) {
        return Some(MapperType::Korean);
    }
    if KOREAN_8K.contains(&crc) {
        return Some(MapperType::Korean8k);
    }
    if MSX.contains(&crc) {
        return Some(MapperType::Msx);
    }
    if NEMESIS.contains(&crc) {
        return Some(MapperType::Nemesis);
    }
    if FOURPAK.contains(&crc) {
        return Some(MapperType::FourPak);
    }
    if JANGGUN.contains(&crc) {
        return Some(MapperType::Janggun);
    }
    None
}

/// Heuristic: Codemasters ROMs have a 16-bit checksum at $7FE6 that, when
/// added to the sum of all other bytes (mod 0x10000), equals 0x10000.
/// Additionally they do NOT carry a "TMR SEGA" header.
fn is_codemasters(rom: &[u8]) -> bool {
    if rom.len() < 0x8000 {
        return false;
    }

    // Must NOT have a standard Sega header
    let sega_header_offsets: &[usize] = &[0x7FF0, 0x3FF0, 0x1FF0];
    for &off in sega_header_offsets {
        if rom.len() > off + 8 && &rom[off..off + 8] == b"TMR SEGA" {
            return false;
        }
    }

    // Codemasters checksum at $7FE6-$7FE7 (little-endian)
    let stored = u16::from_le_bytes([rom[0x7FE6], rom[0x7FE7]]);
    if stored == 0 {
        return false;
    }

    let mut sum: u16 = 0;
    for (i, &b) in rom.iter().enumerate() {
        if i != 0x7FE6 && i != 0x7FE7 {
            sum = sum.wrapping_add(b as u16);
        }
    }

    // The stored checksum should be 0x10000 − sum (mod 0x10000)
    // i.e. sum + stored ≡ 0 (mod 0x10000)
    sum.wrapping_add(stored) == 0
}

// ---------------------------------------------------------------------------
// SMS Memory bus
// ---------------------------------------------------------------------------

/// SMS Memory bus
///
/// Memory Map:
/// - 0x0000-0xBFFF: ROM (up to 48KB direct mapped, or banked)
/// - 0xC000-0xDFFF: RAM (8KB)
/// - 0xE000-0xFFFF: RAM mirror
///
/// BIOS Support:
/// - When bit 3 of memory control (port 0x3E) is 0, BIOS is mapped at 0x0000-0x03FF (1KB)
/// - When bit 3 is set, cartridge ROM is mapped starting at 0x0000
///
/// I/O Ports:
/// - 0x7E/0x7F: PSG
/// - 0xBE: VDP data port
/// - 0xBF: VDP control/status port
/// - 0xDC/0xDD: Controller ports
/// - 0x3E: Memory control (banking, BIOS enable)
pub struct SmsMemory {
    // ROM data
    rom: Vec<u8>,

    // BIOS ROM (1KB or 8KB, optional)
    bios: Vec<u8>,

    // RAM (8KB)
    ram: [u8; 0x2000],

    // Shared VDP reference
    vdp: Rc<RefCell<Vdp>>,

    // Shared PSG reference
    psg: Rc<RefCell<SmsPsg>>,

    // --------------- Mapper --------------------------------------------------
    /// Active mapper type.
    mapper_type: MapperType,

    /// Number of 16 KB ROM banks.
    num_banks: usize,

    /// Number of 8 KB ROM banks (used by Korean8k / MSX / Janggun).
    num_8k_banks: usize,

    // -- Sega mapper state --
    /// Sega-style 16 KB bank registers: [slot0, slot1, slot2].
    /// Also used by Korean, Nemesis, FourPak as their base registers.
    sega_banks: [usize; 3],

    /// Sega mapper: cartridge RAM (up to 32 KB, battery-backed).
    cart_ram: Vec<u8>,

    /// Sega mapper: cartridge RAM enabled (bit 3 of $FFFC).
    cart_ram_enabled: bool,

    /// Sega mapper: cart RAM bank select (bit 2 of $FFFC → 0 or 1).
    cart_ram_bank: usize,

    // -- Codemasters mapper state --
    codemasters_banks: [usize; 3],

    // -- Korean 8K mapper state --
    /// Four 8 KB bank indices for slots at $4000-$5FFF … $A000-$BFFF.
    korean_8k_banks: [usize; 4],

    // -- MSX mapper state --
    /// Four 8 KB bank indices mapped via $0000-$0003.
    msx_banks: [usize; 4],

    // -- Nemesis mapper state --
    /// Once the first write to $0000 occurs, switch from start-up mode.
    nemesis_activated: bool,

    // -- Janggun mapper state --
    /// Six 8 KB sub-bank indices (slots 0,1,2 each have two 8 KB halves).
    janggun_banks: [usize; 6],

    /// Janggun control byte (bit 6 = enable XOR for slot 2 low, bit 7 = slot 2 high).
    janggun_control: u8,

    // --------------- Controller / memory control ----------------------------
    /// Controller state
    controller_1: u8,
    controller_2: u8,

    /// Memory control register (port $3E)
    memory_control: u8,

    /// True if this bus is for a Game Gear system
    is_game_gear: bool,

    /// Game Gear Start button state (active low: bit 7 = 0 when pressed)
    gg_start_button: u8,
}

impl SmsMemory {
    /// Create a new SMS memory bus.
    pub fn new(rom: Vec<u8>, vdp: Rc<RefCell<Vdp>>, psg: Rc<RefCell<SmsPsg>>) -> Self {
        let mapper_type = detect_mapper(&rom);
        let num_banks = rom.len().div_ceil(0x4000).max(1);
        let num_8k_banks = rom.len().div_ceil(0x2000).max(1);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "SMS: ROM size={} KB, mapper={}, 16K banks={}, 8K banks={}",
                rom.len() / 1024,
                mapper_type.name(),
                num_banks,
                num_8k_banks
            )
        });

        let (sega_banks, codemasters_banks) = match mapper_type {
            MapperType::Codemasters => ([0, 1, 2], [0, 1, 0]),
            MapperType::Nemesis => {
                let last = num_banks.saturating_sub(1);
                ([last, 1, 2], [0, 1, 0])
            }
            _ => ([0, 1, 2], [0, 1, 0]),
        };

        Self {
            rom,
            bios: Vec::new(),
            ram: [0; 0x2000],
            vdp,
            psg,
            mapper_type,
            num_banks,
            num_8k_banks,
            sega_banks,
            cart_ram: Vec::new(),
            cart_ram_enabled: false,
            cart_ram_bank: 0,
            codemasters_banks,
            korean_8k_banks: [0, 1, 2, 3],
            msx_banks: [0, 1, 2, 3],
            nemesis_activated: false,
            janggun_banks: [0, 1, 2, 3, 4, 5],
            janggun_control: 0,
            controller_1: 0xFF,
            controller_2: 0xFF,
            memory_control: 0x08, // Bit 3 set → BIOS disabled by default
            is_game_gear: false,
            gg_start_button: 0x80, // Not pressed (active low)
        }
    }

    /// Create a new Game Gear memory bus.
    pub fn new_game_gear(rom: Vec<u8>, vdp: Rc<RefCell<Vdp>>, psg: Rc<RefCell<SmsPsg>>) -> Self {
        let mut bus = Self::new(rom, vdp, psg);
        bus.is_game_gear = true;
        bus
    }

    // -----------------------------------------------------------------------
    // Public helpers
    // -----------------------------------------------------------------------

    /// Get the active mapper type.
    pub fn mapper_type(&self) -> MapperType {
        self.mapper_type
    }

    /// Force a specific mapper type (for manual override / debugging).
    #[allow(dead_code)]
    pub fn set_mapper_type(&mut self, mt: MapperType) {
        self.mapper_type = mt;
        // Re-initialise mapper-specific state
        match mt {
            MapperType::Codemasters => {
                self.codemasters_banks = [0, 1, 0];
            }
            MapperType::Nemesis => {
                let last = self.num_banks.saturating_sub(1);
                self.sega_banks = [last, 1, 2];
                self.nemesis_activated = false;
            }
            MapperType::Korean8k => {
                self.korean_8k_banks = [0, 1, 2, 3];
            }
            MapperType::Msx => {
                self.msx_banks = [0, 1, 2, 3];
            }
            MapperType::Janggun => {
                self.janggun_banks = [0, 1, 2, 3, 4, 5];
                self.janggun_control = 0;
            }
            _ => {}
        }
    }

    /// Load BIOS ROM.
    pub fn load_bios(&mut self, bios: Vec<u8>) {
        self.bios = bios;
        if !self.bios.is_empty() {
            self.memory_control &= !0x08; // Enable BIOS
        } else {
            self.memory_control |= 0x08;
        }
    }

    /// Load cartridge ROM (preserves BIOS).
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        let mt = detect_mapper(&rom);
        self.num_banks = rom.len().div_ceil(0x4000).max(1);
        self.num_8k_banks = rom.len().div_ceil(0x2000).max(1);
        self.rom = rom;
        self.mapper_type = mt;

        // Reset all mapper state
        match mt {
            MapperType::Codemasters => {
                self.sega_banks = [0, 1, 2];
                self.codemasters_banks = [0, 1, 0];
            }
            MapperType::Nemesis => {
                let last = self.num_banks.saturating_sub(1);
                self.sega_banks = [last, 1, 2];
                self.nemesis_activated = false;
                self.codemasters_banks = [0, 1, 0];
            }
            _ => {
                self.sega_banks = [0, 1, 2];
                self.codemasters_banks = [0, 1, 0];
            }
        }
        self.korean_8k_banks = [0, 1, 2, 3];
        self.msx_banks = [0, 1, 2, 3];
        self.janggun_banks = [0, 1, 2, 3, 4, 5];
        self.janggun_control = 0;
        self.cart_ram_enabled = false;
        self.cart_ram_bank = 0;

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "SMS: Loaded ROM {} KB, mapper={}",
                self.rom.len() / 1024,
                mt.name()
            )
        });
    }

    /// Check if BIOS is currently enabled.
    pub fn is_bios_enabled(&self) -> bool {
        (self.memory_control & 0x08) == 0 && !self.bios.is_empty()
    }

    // -----------------------------------------------------------------------
    // Controller helpers
    // -----------------------------------------------------------------------

    pub fn set_controller_1(&mut self, state: u8) {
        self.controller_1 = state;
    }
    pub fn set_controller_2(&mut self, state: u8) {
        self.controller_2 = state;
    }
    pub fn get_controller_1(&self) -> u8 {
        self.controller_1
    }
    pub fn get_controller_2(&self) -> u8 {
        self.controller_2
    }
    pub fn get_memory_control(&self) -> u8 {
        self.memory_control
    }
    pub fn set_memory_control(&mut self, value: u8) {
        self.memory_control = value;
    }

    /// Set the Game Gear Start button state (bit 7: 0 = pressed, 0x80 = not pressed).
    pub fn set_gg_start_button(&mut self, pressed: bool) {
        self.gg_start_button = if pressed { 0x00 } else { 0x80 };
    }

    /// Returns true if this is a Game Gear bus.
    #[allow(dead_code)]
    pub fn is_game_gear(&self) -> bool {
        self.is_game_gear
    }

    // -----------------------------------------------------------------------
    // Save-state helpers (backward-compatible API)
    // -----------------------------------------------------------------------

    pub fn get_ram(&self) -> Vec<u8> {
        self.ram.to_vec()
    }
    pub fn set_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }

    pub fn get_rom_bank_0(&self) -> usize {
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[0],
            _ => self.sega_banks[0],
        }
    }
    pub fn get_rom_bank_1(&self) -> usize {
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[1],
            _ => self.sega_banks[1],
        }
    }
    pub fn get_rom_bank_2(&self) -> usize {
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[2],
            _ => self.sega_banks[2],
        }
    }

    pub fn set_rom_bank_0(&mut self, bank: usize) {
        let b = bank % self.num_banks.max(1);
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[0] = b,
            _ => self.sega_banks[0] = b,
        }
    }
    pub fn set_rom_bank_1(&mut self, bank: usize) {
        let b = bank % self.num_banks.max(1);
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[1] = b,
            _ => self.sega_banks[1] = b,
        }
    }
    pub fn set_rom_bank_2(&mut self, bank: usize) {
        let b = bank % self.num_banks.max(1);
        match self.mapper_type {
            MapperType::Codemasters => self.codemasters_banks[2] = b,
            _ => self.sega_banks[2] = b,
        }
    }

    /// Serialise full mapper state to JSON (for save states).
    pub fn get_mapper_state(&self) -> Value {
        serde_json::json!({
            "mapper_type": self.mapper_type.as_str(),
            "sega_banks": self.sega_banks.to_vec(),
            "cart_ram_enabled": self.cart_ram_enabled,
            "cart_ram_bank": self.cart_ram_bank,
            "cart_ram": self.cart_ram.clone(),
            "codemasters_banks": self.codemasters_banks.to_vec(),
            "korean_8k_banks": self.korean_8k_banks.to_vec(),
            "msx_banks": self.msx_banks.to_vec(),
            "nemesis_activated": self.nemesis_activated,
            "janggun_banks": self.janggun_banks.to_vec(),
            "janggun_control": self.janggun_control,
        })
    }

    /// Restore full mapper state from JSON.
    pub fn set_mapper_state(&mut self, state: &Value) {
        if let Some(mt_str) = state.get("mapper_type").and_then(|v| v.as_str()) {
            self.mapper_type = MapperType::from_str(mt_str);
        }
        if let Some(arr) = state.get("sega_banks").and_then(|v| v.as_array()) {
            for (i, val) in arr.iter().enumerate().take(3) {
                if let Some(b) = val.as_u64() {
                    self.sega_banks[i] = b as usize;
                }
            }
        }
        if let Some(v) = state.get("cart_ram_enabled").and_then(|v| v.as_bool()) {
            self.cart_ram_enabled = v;
        }
        if let Some(v) = state.get("cart_ram_bank").and_then(|v| v.as_u64()) {
            self.cart_ram_bank = v as usize;
        }
        if let Some(arr) = state.get("cart_ram").and_then(|v| v.as_array()) {
            self.cart_ram = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
        }
        if let Some(arr) = state.get("codemasters_banks").and_then(|v| v.as_array()) {
            for (i, val) in arr.iter().enumerate().take(3) {
                if let Some(b) = val.as_u64() {
                    self.codemasters_banks[i] = b as usize;
                }
            }
        }
        if let Some(arr) = state.get("korean_8k_banks").and_then(|v| v.as_array()) {
            for (i, val) in arr.iter().enumerate().take(4) {
                if let Some(b) = val.as_u64() {
                    self.korean_8k_banks[i] = b as usize;
                }
            }
        }
        if let Some(arr) = state.get("msx_banks").and_then(|v| v.as_array()) {
            for (i, val) in arr.iter().enumerate().take(4) {
                if let Some(b) = val.as_u64() {
                    self.msx_banks[i] = b as usize;
                }
            }
        }
        if let Some(v) = state.get("nemesis_activated").and_then(|v| v.as_bool()) {
            self.nemesis_activated = v;
        }
        if let Some(arr) = state.get("janggun_banks").and_then(|v| v.as_array()) {
            for (i, val) in arr.iter().enumerate().take(6) {
                if let Some(b) = val.as_u64() {
                    self.janggun_banks[i] = b as usize;
                }
            }
        }
        if let Some(v) = state.get("janggun_control").and_then(|v| v.as_u64()) {
            self.janggun_control = v as u8;
        }
    }

    // -----------------------------------------------------------------------
    // Sega mapper banking (RAM-based at $FFFC-$FFFF)
    // -----------------------------------------------------------------------

    fn update_sega_banking(&mut self) {
        // $FFFC → cart RAM control
        let fffc = self.ram[0x1FFC];
        self.cart_ram_enabled = (fffc & 0x08) != 0;
        self.cart_ram_bank = ((fffc >> 2) & 1) as usize;
        // Allocate cart RAM on first enable (up to 32 KB = 2 × 16 KB)
        if self.cart_ram_enabled && self.cart_ram.is_empty() {
            self.cart_ram.resize(0x8000, 0);
        }

        // $FFFD / $FFFE / $FFFF → page for slots 0 / 1 / 2
        let nb = self.num_banks.max(1);
        self.sega_banks[0] = (self.ram[0x1FFD] as usize) % nb;
        self.sega_banks[1] = (self.ram[0x1FFE] as usize) % nb;
        self.sega_banks[2] = (self.ram[0x1FFF] as usize) % nb;
    }

    // -----------------------------------------------------------------------
    // ROM read helper (with bounds check)
    // -----------------------------------------------------------------------

    #[inline(always)]
    fn rom_byte(&self, offset: usize) -> u8 {
        self.rom.get(offset).copied().unwrap_or(0xFF)
    }

    // -----------------------------------------------------------------------
    // Per-mapper READ helpers
    // -----------------------------------------------------------------------

    /// Read for **None** mapper (no banking).
    fn read_none(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.rom_byte(addr as usize),
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Sega** mapper.
    fn read_sega(&self, addr: u16) -> u8 {
        match addr {
            // First 1 KB is ALWAYS pinned to the start of ROM (never remapped).
            0x0000..=0x03FF => self.rom_byte(addr as usize),
            // Rest of slot 0 respects $FFFD bank register.
            0x0400..=0x3FFF => self.rom_byte(self.sega_banks[0] * 0x4000 + addr as usize),
            0x4000..=0x7FFF => {
                self.rom_byte(self.sega_banks[1] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0x8000..=0xBFFF => {
                if self.cart_ram_enabled && !self.cart_ram.is_empty() {
                    let offset = self.cart_ram_bank * 0x4000 + (addr & 0x3FFF) as usize;
                    self.cart_ram.get(offset).copied().unwrap_or(0xFF)
                } else {
                    self.rom_byte(self.sega_banks[2] * 0x4000 + (addr & 0x3FFF) as usize)
                }
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Codemasters** mapper.
    fn read_codemasters(&self, addr: u16) -> u8 {
        match addr {
            // No first-1KB pinning – entire slot 0 is remappable.
            0x0000..=0x3FFF => self.rom_byte(self.codemasters_banks[0] * 0x4000 + addr as usize),
            0x4000..=0x7FFF => {
                self.rom_byte(self.codemasters_banks[1] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0x8000..=0xBFFF => {
                self.rom_byte(self.codemasters_banks[2] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Korean** mapper (single $A000 register, banks slot 2).
    fn read_korean(&self, addr: u16) -> u8 {
        match addr {
            // Slots 0 & 1 fixed.
            0x0000..=0x3FFF => self.rom_byte(addr as usize), // always bank 0
            0x4000..=0x7FFF => self.rom_byte(0x4000 + (addr & 0x3FFF) as usize), // always bank 1
            0x8000..=0xBFFF => {
                self.rom_byte(self.sega_banks[2] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Korean 8K** mapper.
    fn read_korean_8k(&self, addr: u16) -> u8 {
        match addr {
            // $0000-$3FFF: fixed to first 16 KB of ROM.
            0x0000..=0x3FFF => self.rom_byte(addr as usize),
            // $4000-$5FFF: bank[0]
            0x4000..=0x5FFF => {
                self.rom_byte(self.korean_8k_banks[0] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            // $6000-$7FFF: bank[1]
            0x6000..=0x7FFF => {
                self.rom_byte(self.korean_8k_banks[1] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            // $8000-$9FFF: bank[2]
            0x8000..=0x9FFF => {
                self.rom_byte(self.korean_8k_banks[2] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            // $A000-$BFFF: bank[3]
            0xA000..=0xBFFF => {
                self.rom_byte(self.korean_8k_banks[3] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **MSX** mapper (registers at $0000-$0003).
    fn read_msx(&self, addr: u16) -> u8 {
        match addr {
            // $0000-$1FFF: msx_banks[0]
            0x0000..=0x1FFF => self.rom_byte(self.msx_banks[0] * 0x2000 + (addr & 0x1FFF) as usize),
            // $2000-$3FFF: msx_banks[1]
            0x2000..=0x3FFF => self.rom_byte(self.msx_banks[1] * 0x2000 + (addr & 0x1FFF) as usize),
            // $4000-$5FFF: msx_banks[2]
            0x4000..=0x5FFF => self.rom_byte(self.msx_banks[2] * 0x2000 + (addr & 0x1FFF) as usize),
            // $6000-$7FFF: msx_banks[3]
            0x6000..=0x7FFF => self.rom_byte(self.msx_banks[3] * 0x2000 + (addr & 0x1FFF) as usize),
            // $8000-$BFFF: fixed to last 16 KB of ROM
            0x8000..=0xBFFF => {
                let last_16k_start = self.rom.len().saturating_sub(0x4000);
                self.rom_byte(last_16k_start + (addr & 0x3FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Nemesis** mapper.
    fn read_nemesis(&self, addr: u16) -> u8 {
        match addr {
            // First 1 KB always pinned.
            0x0000..=0x03FF => self.rom_byte(addr as usize),
            0x0400..=0x3FFF => self.rom_byte(self.sega_banks[0] * 0x4000 + addr as usize),
            0x4000..=0x7FFF => {
                self.rom_byte(self.sega_banks[1] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0x8000..=0xBFFF => {
                self.rom_byte(self.sega_banks[2] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **4PAK All Action** mapper.
    fn read_fourpak(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom_byte(self.sega_banks[0] * 0x4000 + addr as usize),
            0x4000..=0x7FFF => {
                self.rom_byte(self.sega_banks[1] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0x8000..=0xBFFF => {
                self.rom_byte(self.sega_banks[2] * 0x4000 + (addr & 0x3FFF) as usize)
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    /// Read for **Janggun** mapper (8 KB sub-banks with optional XOR).
    fn read_janggun(&self, addr: u16) -> u8 {
        match addr {
            // Slot 0 low / high
            0x0000..=0x1FFF => {
                self.rom_byte(self.janggun_banks[0] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            0x2000..=0x3FFF => {
                self.rom_byte(self.janggun_banks[1] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            // Slot 1 low / high
            0x4000..=0x5FFF => {
                self.rom_byte(self.janggun_banks[2] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            0x6000..=0x7FFF => {
                self.rom_byte(self.janggun_banks[3] * 0x2000 + (addr & 0x1FFF) as usize)
            }
            // Slot 2 low / high (may XOR data bytes)
            0x8000..=0x9FFF => {
                let b = self.rom_byte(self.janggun_banks[4] * 0x2000 + (addr & 0x1FFF) as usize);
                if (self.janggun_control & 0x40) != 0 {
                    b ^ 0xFF
                } else {
                    b
                }
            }
            0xA000..=0xBFFF => {
                let b = self.rom_byte(self.janggun_banks[5] * 0x2000 + (addr & 0x1FFF) as usize);
                if (self.janggun_control & 0x80) != 0 {
                    b ^ 0xFF
                } else {
                    b
                }
            }
            0xC000..=0xFFFF => self.ram[(addr & 0x1FFF) as usize],
        }
    }

    // -----------------------------------------------------------------------
    // Per-mapper WRITE helpers
    // -----------------------------------------------------------------------

    /// Write for the **Sega** mapper.
    fn write_sega(&mut self, addr: u16, val: u8) {
        match addr {
            0x8000..=0xBFFF if self.cart_ram_enabled && !self.cart_ram.is_empty() => {
                let offset = self.cart_ram_bank * 0x4000 + (addr & 0x3FFF) as usize;
                if offset < self.cart_ram.len() {
                    self.cart_ram[offset] = val;
                }
            }
            0xC000..=0xFFFF => {
                let ram_addr = (addr & 0x1FFF) as usize;
                self.ram[ram_addr] = val;
                // Only the dedicated mapper-register addresses $FFFC-$FFFF trigger
                // bank updates.  Their RAM mirror at $DFFC-$DFFF shares the same
                // physical bytes but must NOT update the mapper – the real SMS
                // hardware mapper decoder only fires on the $FFFC-$FFFF range.
                // This matters when a real Sega BIOS clears work-RAM ($C000-$DFFF):
                // without this guard, writes to $DFFE would zero sega_banks[1],
                // causing the BIOS header check at $7FF0 to read the wrong ROM bank
                // and fall back to the built-in Snail Maze game.
                if addr >= 0xFFFC {
                    self.update_sega_banking();
                }
            }
            _ => {} // writes to ROM area ignored
        }
    }

    /// Write for the **Codemasters** mapper.
    fn write_codemasters(&mut self, addr: u16, val: u8) {
        let nb = self.num_banks.max(1);
        match addr {
            0x0000..=0x3FFF => {
                self.codemasters_banks[0] = (val as usize) % nb;
            }
            0x4000..=0x7FFF => {
                self.codemasters_banks[1] = (val as usize) % nb;
            }
            0x8000..=0xBFFF => {
                self.codemasters_banks[2] = (val as usize) % nb;
            }
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
        }
    }

    /// Write for the **Korean** mapper.
    fn write_korean(&mut self, addr: u16, val: u8) {
        match addr {
            0xA000 => {
                self.sega_banks[2] = (val as usize) % self.num_banks.max(1);
            }
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }

    /// Write for the **Korean 8K** mapper.
    fn write_korean_8k(&mut self, addr: u16, val: u8) {
        let nb = self.num_8k_banks.max(1);
        match addr {
            0x4000 => self.korean_8k_banks[0] = (val as usize) % nb,
            0x6000 => self.korean_8k_banks[1] = (val as usize) % nb,
            0x8000 => self.korean_8k_banks[2] = (val as usize) % nb,
            0xA000 => self.korean_8k_banks[3] = (val as usize) % nb,
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }

    /// Write for the **MSX** mapper.
    fn write_msx(&mut self, addr: u16, val: u8) {
        let nb = self.num_8k_banks.max(1);
        match addr {
            0x0000 => self.msx_banks[0] = (val as usize) % nb,
            0x0001 => self.msx_banks[1] = (val as usize) % nb,
            0x0002 => self.msx_banks[2] = (val as usize) % nb,
            0x0003 => self.msx_banks[3] = (val as usize) % nb,
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }

    /// Write for the **Nemesis** mapper.
    fn write_nemesis(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000 => {
                if !self.nemesis_activated {
                    // First write switches slot 0 from last bank to bank 0.
                    self.nemesis_activated = true;
                    self.sega_banks[0] = 0;
                }
            }
            0xC000..=0xFFFF => {
                let ram_addr = (addr & 0x1FFF) as usize;
                self.ram[ram_addr] = val;
                // Same rule as Sega mapper: only $FFFC-$FFFF fire the mapper update.
                if addr >= 0xFFFC {
                    self.update_sega_banking();
                }
            }
            _ => {}
        }
    }

    /// Write for the **4PAK All Action** mapper.
    fn write_fourpak(&mut self, addr: u16, val: u8) {
        let nb = self.num_banks.max(1);
        match addr {
            0x3FFE => self.sega_banks[0] = (val as usize) % nb,
            0x7FFE => self.sega_banks[1] = (val as usize) % nb,
            0xBFFE => self.sega_banks[2] = (val as usize) % nb,
            0xC000..=0xFFFF => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }

    /// Write for the **Janggun** mapper.
    fn write_janggun(&mut self, addr: u16, val: u8) {
        let nb = self.num_8k_banks.max(1);
        match addr {
            // $FFFE: control register (bits 6-7 = XOR for slot 2 halves)
            0xFFFE => {
                self.janggun_control = val;
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            // $FFFF: bank for slot 2 (16 KB = 2 × 8 KB pages)
            0xFFFF => {
                let base = ((val as usize) * 2) % nb;
                self.janggun_banks[4] = base;
                self.janggun_banks[5] = (base + 1) % nb;
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            // Individual 8K half-bank registers
            0x4000 => self.janggun_banks[2] = (val as usize) % nb,
            0x6000 => self.janggun_banks[3] = (val as usize) % nb,
            0x8000 => self.janggun_banks[4] = (val as usize) % nb,
            0xA000 => self.janggun_banks[5] = (val as usize) % nb,
            0xC000..=0xFFFD => {
                self.ram[(addr & 0x1FFF) as usize] = val;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryZ80 trait implementation
// ---------------------------------------------------------------------------

impl MemoryZ80 for SmsMemory {
    fn read(&self, addr: u16) -> u8 {
        // When the BIOS is enabled, it overlays the cartridge ROM for all
        // addresses within the BIOS ROM's range (0x0000..bios.len()-1). Real
        // SMS BIOS ROMs are typically 8 KB or larger, so the overlay must not
        // be artificially limited to 1 KB, or reads beyond 0x03FF would fall
        // through to the cartridge ROM and corrupt BIOS execution.
        if self.is_bios_enabled() && (addr as usize) < self.bios.len() {
            return self.bios[addr as usize];
        }

        match self.mapper_type {
            MapperType::None => self.read_none(addr),
            MapperType::Sega => self.read_sega(addr),
            MapperType::Codemasters => self.read_codemasters(addr),
            MapperType::Korean => self.read_korean(addr),
            MapperType::Korean8k => self.read_korean_8k(addr),
            MapperType::Msx => self.read_msx(addr),
            MapperType::Nemesis => self.read_nemesis(addr),
            MapperType::FourPak => self.read_fourpak(addr),
            MapperType::Janggun => self.read_janggun(addr),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match self.mapper_type {
            MapperType::None => {
                if addr >= 0xC000 {
                    self.ram[(addr & 0x1FFF) as usize] = val;
                }
            }
            MapperType::Sega => self.write_sega(addr, val),
            MapperType::Codemasters => self.write_codemasters(addr, val),
            MapperType::Korean => self.write_korean(addr, val),
            MapperType::Korean8k => self.write_korean_8k(addr, val),
            MapperType::Msx => self.write_msx(addr, val),
            MapperType::Nemesis => self.write_nemesis(addr, val),
            MapperType::FourPak => self.write_fourpak(addr, val),
            MapperType::Janggun => self.write_janggun(addr, val),
        }
    }

    fn io_read(&mut self, port: u8) -> u8 {
        let value = match port {
            // Game Gear specific: port 0x00 = Start button + region
            0x00 if self.is_game_gear => {
                // Bit 7: Start button (0 = pressed, 1 = not pressed)
                // Bit 6: Njap (0 = Japanese, 1 = overseas/export)
                // Bit 5: NNTS (0 = NTSC, 1 = PAL) – always NTSC for GG
                // Bits 4-0: unused, normally 0x1F
                0x7F | self.gg_start_button
            }
            // Game Gear specific: port 0x06 = stereo control (read returns last written)
            0x06 if self.is_game_gear => 0xFF,
            // 0x40-0x7F: V-counter (even ports) / H-counter (odd ports)
            p if (0x40..=0x7F).contains(&p) => {
                if p & 0x01 == 0 {
                    // Even port: V-counter
                    self.vdp.borrow().read_vcounter()
                } else {
                    // Odd port: H-counter
                    self.vdp.borrow().read_hcounter()
                }
            }
            // 0x80-0xBF: VDP ports (bit 0 determines data vs control)
            p if (0x80..=0xBF).contains(&p) => {
                if p & 0x01 == 0 {
                    self.vdp.borrow_mut().read_data()
                } else {
                    self.vdp.borrow_mut().read_status()
                }
            }
            // 0xC0-0xFF: Controller ports
            p if (0xC0..=0xFF).contains(&p) => {
                if p & 0x01 == 0 {
                    self.controller_1
                } else {
                    self.controller_2
                }
            }
            _ => 0xFF,
        };

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!("SMS I/O: Read port ${:02X} = ${:02X}", port, value)
        });

        value
    }

    fn io_write(&mut self, port: u8, val: u8) {
        match port {
            // 0x00-0x3F: Memory control registers
            0x3E => {
                self.memory_control = val;
            }
            0x3F => {
                // I/O port control (nationalization adapter) – not yet implemented.
            }
            // 0x40-0x7F: PSG write
            p if (0x40..=0x7F).contains(&p) => {
                self.psg.borrow_mut().write(val);
            }
            // 0x80-0xBF: VDP ports
            p if (0x80..=0xBF).contains(&p) => {
                if p & 0x01 == 0 {
                    self.vdp.borrow_mut().write_data(val);
                } else {
                    self.vdp.borrow_mut().write_control(val);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psg::SmsPsg;

    fn make_mem(rom: Vec<u8>) -> SmsMemory {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(SmsPsg::new()));
        SmsMemory::new(rom, vdp, psg)
    }

    #[test]
    fn test_ram_read_write() {
        let mut mem = make_mem(vec![0; 0x8000]);
        mem.write(0xC000, 0x42);
        assert_eq!(mem.read(0xC000), 0x42);
        // Check RAM mirror
        assert_eq!(mem.read(0xE000), 0x42);
    }

    #[test]
    fn test_rom_read() {
        let mut rom = vec![0; 0x8000];
        rom[0x100] = 0xAB;
        let mem = make_mem(rom);
        assert_eq!(mem.read(0x100), 0xAB);
    }

    #[test]
    fn test_sega_banking() {
        // 128 KB ROM (8 × 16 KB banks), tagged at offset 0 of each bank.
        let mut rom = vec![0; 0x20000];
        for i in 0..8 {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Sega);

        assert_eq!(mem.read(0x0000), 0); // pinned first 1 KB
        assert_eq!(mem.read(0x4000), 1);
        assert_eq!(mem.read(0x8000), 2);

        // Switch slot 2 to bank 5 via $FFFF
        mem.write(0xFFFF, 5);
        assert_eq!(mem.read(0x8000), 5);
    }

    #[test]
    fn test_sega_first_1kb_pinned() {
        // Ensure first 1 KB is always from physical ROM start,
        // even when bank 0 is remapped.
        let mut rom = vec![0; 0x20000]; // 128 KB
        rom[0x0038] = 0xC9; // RST $38 vector: RET
                            // Put a known value at bank 3, offset $0400 within the bank
                            // (ROM[3*0x4000 + 0x0400] = ROM[0xC400])
        rom[3 * 0x4000 + 0x0400] = 0xBB;

        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Sega);

        // Remap slot 0 to bank 3
        mem.write(0xFFFD, 3);
        // First 1 KB must still be from bank 0 (pinned)
        assert_eq!(mem.read(0x0038), 0xC9);
        // $0400+ should come from bank 3
        assert_eq!(mem.read(0x0400), 0xBB);
    }

    /// Regression test for the "real BIOS boots Snail Maze instead of cartridge" bug.
    ///
    /// The real Sega BIOS initialises/clears work-RAM ($C000-$DFFF) early in its
    /// boot sequence.  That range includes the physical bytes at $DFFC-$DFFF which
    /// are the RAM mirrors of the Sega mapper registers at $FFFC-$FFFF.  On real
    /// hardware the mapper decoder only fires on writes to $FFFC-$FFFF; the
    /// primary-RAM writes at $DFFC-$DFFF update the RAM byte but do NOT change the
    /// active bank registers.  Without this fix both address ranges would reset the
    /// banks to 0 so the header check at $7FF0 mapped to ROM offset $3FF0 (bank 0)
    /// instead of $7FF0 (bank 1) and "TMR SEGA" was never found.
    #[test]
    fn test_sega_primary_ram_write_does_not_update_mapper() {
        // 128 KB ROM (8 × 16 KB banks).  Place "TMR SEGA" at offset $7FF0 as a
        // real SMS cartridge would (header in bank 1 at the canonical location).
        let mut rom = vec![0u8; 0x20000];
        let header = b"TMR SEGA";
        rom[0x7FF0..0x7FF8].copy_from_slice(header);
        // Put a different sentinel at the location the mapper would read if
        // sega_banks[1] were incorrectly reset to 0 (bank 0, offset $3FF0).
        rom[0x3FF0] = 0xDE; // NOT 'T', so if we see this the test must fail.

        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Sega);

        // Simulate BIOS clearing primary work-RAM $C000-$DFFF to 0x00.
        // This includes $DFFE which is the primary-RAM byte backing the slot-1
        // bank register.  It must NOT reset sega_banks[1] from 1 to 0.
        for addr in 0xC000u16..=0xDFFF {
            mem.write(addr, 0x00);
        }

        // After the RAM clear, bank 1 must still be mapped to slot 1 ($4000-$7FFF).
        // Reading $7FF0 must return the first byte of "TMR SEGA", not $DE.
        assert_eq!(
            mem.read(0x7FF0),
            b'T',
            "sega_banks[1] was incorrectly reset by a write to the primary-RAM \
             mirror of the mapper register ($DFFE); header check would fail"
        );
    }

    /// Companion test: writes to the dedicated mapper-register range $FFFC-$FFFF
    /// MUST still update the banks (normal game banker behaviour).
    #[test]
    fn test_sega_mapper_register_range_still_updates_banks() {
        let mut rom = vec![0u8; 0x20000]; // 128 KB
        for i in 0..8usize {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Sega);

        // Default: slot 1 → bank 1
        assert_eq!(mem.read(0x4000), 1);

        // Write to the real mapper register at $FFFE → should remap slot 1.
        mem.write(0xFFFE, 3);
        assert_eq!(
            mem.read(0x4000),
            3,
            "write to $FFFE must update sega_banks[1]"
        );
    }

    #[test]
    fn test_codemasters_banking() {
        let mut rom = vec![0; 0x20000]; // 128 KB
        for i in 0..8 {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Codemasters);

        // Default banks: 0, 1, 0
        assert_eq!(mem.read(0x0000), 0);
        assert_eq!(mem.read(0x4000), 1);
        assert_eq!(mem.read(0x8000), 0); // slot 2 defaults to bank 0 for Codemasters

        // Remap slot 2 to bank 5 via write to $8000
        mem.write(0x8000, 5);
        assert_eq!(mem.read(0x8000), 5);

        // Remap slot 0 to bank 7 via write to $0000 (no first-1 KB pinning)
        mem.write(0x0000, 7);
        assert_eq!(mem.read(0x0000), 7);
    }

    #[test]
    fn test_korean_banking() {
        let mut rom = vec![0; 0x20000]; // 128 KB
        for i in 0..8 {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Korean);

        // Slots 0 & 1 fixed
        assert_eq!(mem.read(0x0000), 0);
        assert_eq!(mem.read(0x4000), 1);
        // Default slot 2 = bank 2
        assert_eq!(mem.read(0x8000), 2);

        // Switch slot 2 via $A000
        mem.write(0xA000, 6);
        assert_eq!(mem.read(0x8000), 6);
    }

    #[test]
    fn test_korean_8k_banking() {
        let mut rom = vec![0; 0x20000]; // 128 KB = 16 × 8 KB banks
        for i in 0..16 {
            rom[i * 0x2000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Korean8k);

        // $0000 fixed
        assert_eq!(mem.read(0x0000), 0);

        // Default 8K banks [0,1,2,3]
        assert_eq!(mem.read(0x4000), 0); // bank 0
        assert_eq!(mem.read(0x6000), 1); // bank 1
        assert_eq!(mem.read(0x8000), 2); // bank 2
        assert_eq!(mem.read(0xA000), 3); // bank 3

        // Remap $8000-$9FFF to bank 10
        mem.write(0x8000, 10);
        assert_eq!(mem.read(0x8000), 10);
    }

    #[test]
    fn test_msx_banking() {
        let mut rom = vec![0; 0x20000]; // 128 KB = 16 × 8 KB banks
        for i in 0..16 {
            rom[i * 0x2000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Msx);

        // Default banks [0,1,2,3]
        assert_eq!(mem.read(0x0000), 0);
        assert_eq!(mem.read(0x2000), 1);
        assert_eq!(mem.read(0x4000), 2);
        assert_eq!(mem.read(0x6000), 3);

        // Remap $0000-$1FFF to bank 8
        mem.write(0x0000, 8);
        assert_eq!(mem.read(0x0000), 8);
    }

    #[test]
    fn test_nemesis_startup() {
        // 64 KB ROM → 4 banks.  Nemesis starts with LAST bank in slot 0.
        let mut rom = vec![0; 0x10000];
        for i in 0..4 {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Nemesis);

        // Slot 0 should show last bank (3) at $0400+
        // First 1 KB is pinned to physical ROM start (bank 0).
        assert_eq!(mem.read(0x0000), 0); // pinned
        assert_eq!(mem.read(0x4000), 1); // slot 1 = bank 1

        // After first write to $0000, slot 0 becomes bank 0.
        mem.write(0x0000, 0);
        assert!(mem.nemesis_activated);
    }

    #[test]
    fn test_fourpak_banking() {
        let mut rom = vec![0; 0x40000]; // 256 KB
        for i in 0..16 {
            rom[i * 0x4000] = i as u8;
        }
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::FourPak);

        // Switch slot 0 via $3FFE
        mem.write(0x3FFE, 5);
        assert_eq!(mem.read(0x0000), 5);

        // Switch slot 1 via $7FFE
        mem.write(0x7FFE, 8);
        assert_eq!(mem.read(0x4000), 8);

        // Switch slot 2 via $BFFE
        mem.write(0xBFFE, 12);
        assert_eq!(mem.read(0x8000), 12);
    }

    #[test]
    fn test_janggun_banking() {
        let mut rom = vec![0; 0x40000]; // 256 KB = 32 × 8 KB banks
        for i in 0..32 {
            rom[i * 0x2000] = i as u8;
        }
        let expected_raw = rom[4 * 0x2000]; // janggun_banks[4] defaults to 4
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Janggun);

        // Remap $4000-$5FFF to bank 10
        mem.write(0x4000, 10);
        assert_eq!(mem.read(0x4000), 10);

        // Enable XOR for slot 2 low ($8000-$9FFF)
        mem.write(0xFFFE, 0x40);
        // The data from the bank should be XOR 0xFF
        assert_eq!(mem.read(0x8000), expected_raw ^ 0xFF);
    }

    #[test]
    fn test_mapper_detection_small_rom() {
        let rom = vec![0; 0x8000]; // 32 KB
        assert_eq!(detect_mapper(&rom), MapperType::None);
    }

    #[test]
    fn test_mapper_detection_large_rom() {
        let rom = vec![0; 0x40000]; // 256 KB
                                    // No Codemasters checksum, no CRC match → Sega.
        assert_eq!(detect_mapper(&rom), MapperType::Sega);
    }

    #[test]
    fn test_cart_ram() {
        let rom = vec![0; 0x20000];
        let mut mem = make_mem(rom);
        mem.set_mapper_type(MapperType::Sega);

        // Enable cart RAM via $FFFC (bit 3 = 1)
        mem.write(0xFFFC, 0x08);
        // Write to cart RAM in $8000-$BFFF region
        mem.write(0x8000, 0x42);
        assert_eq!(mem.read(0x8000), 0x42);

        // Disable cart RAM
        mem.write(0xFFFC, 0x00);
        // Now reads come from ROM again, not cart RAM
        assert_ne!(mem.read(0x8000), 0x42);
    }

    #[test]
    fn test_mapper_state_roundtrip() {
        let rom = vec![0; 0x20000];
        let mut mem = make_mem(rom.clone());
        mem.set_mapper_type(MapperType::Sega);
        mem.write(0xFFFF, 5); // bank 2 = 5

        let state = mem.get_mapper_state();

        let mut mem2 = make_mem(rom);
        mem2.set_mapper_state(&state);
        assert_eq!(mem2.mapper_type(), MapperType::Sega);
        assert_eq!(mem2.get_rom_bank_2(), 5);
    }

    #[test]
    fn test_crc32_basic() {
        // Just verify CRC32 doesn't panic and produces consistent results.
        let data = b"hello world";
        let c1 = crc32(data);
        let c2 = crc32(data);
        assert_eq!(c1, c2);
        assert_ne!(c1, 0);
    }

    #[test]
    fn test_bios_overlay_full_size() {
        // The BIOS overlay must cover the full BIOS ROM size, not just the first
        // 1 KB.  Real SMS BIOS ROMs are 8 KB; code beyond 0x03FF must come from
        // the BIOS rather than the cartridge ROM.
        let mut rom = vec![0u8; 0x8000];
        // Mark every byte of the ROM with a known sentinel so we can detect
        // accidental fall-through reads.
        rom.fill(0xCC);

        let mut mem = make_mem(rom);

        // Create an 8 KB BIOS image with distinct bytes at various offsets.
        let mut bios = vec![0xBBu8; 0x2000]; // 8 KB
        bios[0x0000] = 0x01; // first byte
        bios[0x03FF] = 0x02; // last byte of first 1 KB
        bios[0x0400] = 0x03; // first byte of second 1 KB (was broken before the fix)
        bios[0x1FFF] = 0x04; // last byte of the 8 KB BIOS

        mem.load_bios(bios);
        assert!(mem.is_bios_enabled());

        // All addresses within the 8 KB BIOS range must return BIOS data.
        assert_eq!(mem.read(0x0000), 0x01);
        assert_eq!(mem.read(0x03FF), 0x02);
        assert_eq!(mem.read(0x0400), 0x03); // this returned cartridge ROM before the fix
        assert_eq!(mem.read(0x1FFF), 0x04); // this returned cartridge ROM before the fix

        // Addresses above the BIOS range (0x2000+) must fall through to ROM/RAM.
        // The cartridge ROM was filled with 0xCC so we expect that value back.
        assert_eq!(mem.read(0x2000), 0xCC);
    }

    #[test]
    fn test_bios_overlay_disabled_after_disable() {
        // After the BIOS disables itself (port 0x3E bit 3 set to 1), the
        // cartridge ROM must be visible at 0x0000.
        let mut rom = vec![0u8; 0x8000];
        rom[0x0400] = 0xDD; // cartridge data at 0x0400

        let mut mem = make_mem(rom);

        let mut bios = vec![0xBBu8; 0x2000];
        bios[0x0400] = 0xAA; // BIOS data at 0x0400
        mem.load_bios(bios);

        // BIOS is enabled; 0x0400 should return BIOS data.
        assert_eq!(mem.read(0x0400), 0xAA);

        // Disable BIOS via port 0x3E (bit 3 = 1).
        mem.io_write(0x3E, 0x08);
        assert!(!mem.is_bios_enabled());

        // Now 0x0400 must return cartridge data.
        assert_eq!(mem.read(0x0400), 0xDD);
    }
}
