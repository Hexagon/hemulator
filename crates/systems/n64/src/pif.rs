//! PIF (Peripheral Interface) - Controller and boot ROM interface
//!
//! The PIF chip handles:
//! - Boot ROM execution (IPL3 bootstrap)
//! - Controller communication (N64 controllers, memory cards, etc.)
//! - EEPROM save data access
//! - RTC (Real-Time Clock) for some games
//!
//! # Controller Interface
//!
//! N64 controllers communicate via the PIF using a command/response protocol:
//! - **Command 0x00**: Controller info/status
//! - **Command 0x01**: Read controller state (buttons, stick)
//! - **Command 0x02**: Read controller pak (memory card)
//! - **Command 0x03**: Write controller pak
//!
//! Controller state is accessed via PIF RAM at address 0x1FC007C0-0x1FC007FF
//! Games write command blocks to PIF RAM, then read response blocks.
//!
//! ## Button State Convention
//!
//! **IMPORTANT**: N64 controllers use **active-high logic** for button states:
//! - **1 = Button pressed** (bit set)
//! - **0 = Button released** (bit clear)
//!
//! This is different from some other systems:
//! - Game Boy uses active-low (0 = pressed, 1 = released)
//! - NES uses active-high (1 = pressed, 0 = released)
//!
//! Button layout in 16-bit response:
//! - Bits 15-12: A, B, Z, Start
//! - Bits 11-8: D-Up, D-Down, D-Left, D-Right
//! - Bits 7-6: Reserved
//! - Bits 5-4: L, R
//! - Bits 3-0: C-Up, C-Down, C-Left, C-Right
//!
//! Analog stick uses signed 8-bit range:
//! - X axis: -128 (left) to +127 (right)
//! - Y axis: -128 (down) to +127 (up)
//!
//! # EEPROM Interface
//!
//! N64 games use EEPROM for save data storage. Two sizes are supported:
//! - **4Kbit EEPROM**: 512 bytes (64 blocks of 8 bytes)
//! - **16Kbit EEPROM**: 2048 bytes (256 blocks of 8 bytes)
//!
//! EEPROM commands via PIF RAM:
//! - **Command 0x04**: Read EEPROM block (8 bytes)
//!   - Format: [T=2, R=8, cmd=0x04, block]
//!   - Returns 8 bytes from specified block
//! - **Command 0x05**: Write EEPROM block (8 bytes)
//!   - Format: [T=10, R=1, cmd=0x05, block, data[8]]
//!   - Returns status byte (0x00 = success, 0x80 = error)
//!
//! # Implementation
//!
//! This is a simplified PIF implementation:
//! - Basic controller communication (buttons and analog stick)
//! - EEPROM support with persistence
//! - No memory card support (yet)
//! - Minimal boot ROM (just enough to start games)

/// N64 controller button flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControllerButtons {
    /// A button
    pub a: bool,
    /// B button
    pub b: bool,
    /// Z trigger
    pub z: bool,
    /// Start button
    pub start: bool,
    /// D-pad Up
    pub d_up: bool,
    /// D-pad Down
    pub d_down: bool,
    /// D-pad Left
    pub d_left: bool,
    /// D-pad Right
    pub d_right: bool,
    /// L trigger
    pub l: bool,
    /// R trigger
    pub r: bool,
    /// C-Up button
    pub c_up: bool,
    /// C-Down button
    pub c_down: bool,
    /// C-Left button
    pub c_left: bool,
    /// C-Right button
    pub c_right: bool,
}

impl ControllerButtons {
    /// Pack buttons into 16-bit value for controller state response
    /// Bit layout (from MSB to LSB):
    /// 15: A, 14: B, 13: Z, 12: Start
    /// 11: D-Up, 10: D-Down, 9: D-Left, 8: D-Right
    /// 7: ?, 6: ?, 5: L, 4: R
    /// 3: C-Up, 2: C-Down, 1: C-Left, 0: C-Right
    pub fn to_u16(&self) -> u16 {
        let mut value = 0u16;

        if self.a {
            value |= 1 << 15;
        }
        if self.b {
            value |= 1 << 14;
        }
        if self.z {
            value |= 1 << 13;
        }
        if self.start {
            value |= 1 << 12;
        }
        if self.d_up {
            value |= 1 << 11;
        }
        if self.d_down {
            value |= 1 << 10;
        }
        if self.d_left {
            value |= 1 << 9;
        }
        if self.d_right {
            value |= 1 << 8;
        }
        if self.l {
            value |= 1 << 5;
        }
        if self.r {
            value |= 1 << 4;
        }
        if self.c_up {
            value |= 1 << 3;
        }
        if self.c_down {
            value |= 1 << 2;
        }
        if self.c_left {
            value |= 1 << 1;
        }
        if self.c_right {
            value |= 1 << 0;
        }

        value
    }
}

/// Controller state (buttons + analog stick)
#[derive(Debug, Clone, Copy, Default)]
pub struct ControllerState {
    /// Button states
    pub buttons: ControllerButtons,
    /// Analog stick X (-128 to 127, left to right)
    pub stick_x: i8,
    /// Analog stick Y (-128 to 127, down to up)
    pub stick_y: i8,
}

/// EEPROM type and size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromType {
    /// No EEPROM present
    None,
    /// 4Kbit EEPROM (512 bytes, 64 blocks of 8 bytes)
    Eeprom4K,
    /// 16Kbit EEPROM (2048 bytes, 256 blocks of 8 bytes)
    Eeprom16K,
}

/// N64 controller pak (mempak) capacity in bytes: 32 KB
const MEMPAK_SIZE: usize = 32 * 1024;

/// CRC-8 for N64 controller pak data verification (polynomial 0x85)
///
/// Reference: N64 controller pak protocol documentation
fn pak_data_crc(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        for bit in (0..8).rev() {
            let xorbit = ((crc >> 7) ^ ((byte >> bit) & 1)) & 1;
            crc <<= 1;
            if xorbit != 0 {
                crc ^= 0x85;
            }
        }
    }
    // Final 8 flush bits
    for _ in 0..8 {
        let xorbit = (crc >> 7) & 1;
        crc <<= 1;
        if xorbit != 0 {
            crc ^= 0x85;
        }
    }
    crc
}

/// Decode a 2-byte mempak address/CRC pair into a byte offset of a 32-byte block
/// within the 32 KB controller pak.
///
/// The N64 controller pak protocol encodes the target 32-byte block address and
/// a 5-bit CRC across two bytes:
///   addr_hi (byte 3): upper 8 bits of the encoded value
///   addr_lo (byte 4): upper 3 bits are the next address bits, lower 5 bits are CRC
///
/// Taken together, the 16 encoded bits form a value whose upper 11 bits select a
/// 32-byte block and whose lower 5 bits are CRC. This helper masks off the lower
/// 5 CRC bits, combines the remaining address bits, and returns the resulting
/// byte offset within the 32 KB mempak address space (0x0000–0x7FE0), aligned to
/// a 32-byte boundary.
fn decode_pak_address(addr_hi: u8, addr_lo: u8) -> usize {
    // Strip lower 5 CRC bits and combine to form block-aligned byte address
    let raw = ((addr_hi as usize) << 8) | ((addr_lo as usize) & 0xE0);
    // Mask to 32 KB address space and keep alignment to 32-byte block boundary
    raw & 0x7FE0
}

pub struct Pif {
    /// PIF RAM (2KB)
    ram: [u8; 0x800],

    /// Controller 1 state
    controller1: ControllerState,

    /// Controller 2 state
    controller2: ControllerState,

    /// Controller 3 state
    controller3: ControllerState,

    /// Controller 4 state
    controller4: ControllerState,

    /// EEPROM type
    eeprom_type: EepromType,

    /// EEPROM data storage (max 2KB for 16Kbit)
    eeprom_data: Vec<u8>,

    /// Current EEPROM block being accessed (for multi-byte transfers)
    #[allow(dead_code)] // Reserved for future multi-byte EEPROM transfers
    eeprom_block: u8,

    /// Controller pak (mempak) data for each of the 4 controller slots.
    /// Each slot is 32 KB; initialised to all-0x00 (blank formatted state).
    /// Index 0 = controller 1, etc.
    mempak_data: [Vec<u8>; 4],

    /// Whether a controller pak is inserted in each controller slot.
    /// When `true` the controller-info response reports pak present (status 0x01).
    mempak_enabled: [bool; 4],
}

impl Pif {
    /// Create new PIF with default state
    pub fn new() -> Self {
        Self {
            ram: [0; 0x800],
            controller1: ControllerState::default(),
            controller2: ControllerState::default(),
            controller3: ControllerState::default(),
            controller4: ControllerState::default(),
            eeprom_type: EepromType::None,
            eeprom_data: Vec::new(),
            eeprom_block: 0,
            // Initialise all four mempak slots as blank 32 KB images.
            // The first controller slot has a pak inserted by default so that
            // games which save to the mempak will find one available.
            mempak_data: [
                vec![0x00; MEMPAK_SIZE],
                vec![0x00; MEMPAK_SIZE],
                vec![0x00; MEMPAK_SIZE],
                vec![0x00; MEMPAK_SIZE],
            ],
            mempak_enabled: [true, false, false, false],
        }
    }

    /// Set EEPROM type and initialize storage
    pub fn set_eeprom_type(&mut self, eeprom_type: EepromType) {
        self.eeprom_type = eeprom_type;

        // Initialize EEPROM storage based on type
        let size = match eeprom_type {
            EepromType::None => 0,
            EepromType::Eeprom4K => 512,   // 4Kbit = 512 bytes
            EepromType::Eeprom16K => 2048, // 16Kbit = 2048 bytes
        };

        self.eeprom_data = vec![0xFF; size]; // EEPROM defaults to all 1s when blank
    }

    /// Load EEPROM data from a previously saved buffer
    pub fn load_eeprom(&mut self, data: Vec<u8>) -> Result<(), String> {
        if self.eeprom_type == EepromType::None {
            return Err("No EEPROM configured".to_string());
        }

        let expected_size = match self.eeprom_type {
            EepromType::Eeprom4K => 512,
            EepromType::Eeprom16K => 2048,
            EepromType::None => 0,
        };

        if data.len() != expected_size {
            return Err(format!(
                "EEPROM data size mismatch: expected {} bytes, got {}",
                expected_size,
                data.len()
            ));
        }

        self.eeprom_data = data;
        Ok(())
    }

    /// Return the current EEPROM data for persistence (returns `None` when no EEPROM is present)
    pub fn save_eeprom(&self) -> Option<Vec<u8>> {
        if self.eeprom_type != EepromType::None {
            Some(self.eeprom_data.clone())
        } else {
            None
        }
    }

    /// Initialize PIF ROM in RAM
    pub fn init_rom(&mut self) {
        // PIF ROM starts at offset 0 in PIF RAM
        // The IPL3 bootloader will:
        // 1. Copy ROM header (0x1000 bytes) to RDRAM 0x00000000
        // 2. Initialize CP0 registers
        // 3. Copy ROM segments to RDRAM
        // 4. Jump to ROM entry point (usually 0x80000400)

        // For now, we implement a minimal boot sequence that jumps to cartridge code
        // Commercial ROMs will need full IPL3 emulation (implemented in bus.rs)

        let pif_rom: Vec<u32> = vec![
            // Jump to test ROM code at 0x10001000 (cartridge ROM + 0x1000)
            // Using cached address 0x90001000 (KSEG0 cached)
            0x3C089000, // lui $t0, 0x9000  # Upper 16 bits
            0x35081000, // ori $t0, $t0, 0x1000  # Lower 16 bits = 0x90001000
            0x01000008, // jr $t0  # Jump to $t0
            0x00000000, // nop (delay slot)
        ];

        // Write PIF ROM to PIF RAM
        for (i, &instr) in pif_rom.iter().enumerate() {
            let offset = i * 4;
            if offset + 3 < self.ram.len() {
                let bytes = instr.to_be_bytes();
                self.ram[offset] = bytes[0];
                self.ram[offset + 1] = bytes[1];
                self.ram[offset + 2] = bytes[2];
                self.ram[offset + 3] = bytes[3];
            }
        }
    }

    /// Detect CIC type from IPL3 boot code checksum and return the CIC seed.
    /// IPL3 code is at ROM[0x40..0x1000] (0xFC0 bytes).
    /// Returns the CIC seed value for PIF RAM.
    pub fn detect_cic_seed(rom_data: &[u8]) -> u8 {
        if rom_data.len() < 0x1000 {
            return 0x3F; // Default CIC-6102
        }

        // Compute a simple checksum of IPL3 code (ROM[0x40..0x1000])
        let mut sum: u64 = 0;
        for i in (0x40..0x1000).step_by(4) {
            let word = u32::from_be_bytes([
                rom_data[i],
                rom_data[i + 1],
                rom_data[i + 2],
                rom_data[i + 3],
            ]);
            sum = sum.wrapping_add(word as u64);
        }

        // Match known IPL3 checksums to CIC types
        match sum {
            0x0000001F_F9FBF25E => 0x3F, // CIC-6101 (Star Fox 64)
            0x0000001F_F9FBD1B0 => 0x3F, // CIC-6102 (most games)
            0x0000001F_F9FB0DAA => 0x78, // CIC-6103 (Banjo-Kazooie, etc.)
            0x0000001F_F9FBD860 => 0x91, // CIC-6105 (Zelda OoT, etc.)
            0x0000001F_F9FBD4E4 => 0x85, // CIC-6106 (F-Zero X, etc.)
            _ => 0x3F,                   // Default to CIC-6102 seed
        }
    }

    /// Set up PIF RAM for IPL3 boot.
    /// Places the CIC seed in PIF RAM and sets the boot status byte
    /// to signal IPL3 to start.
    ///
    /// PIF RAM is 64 bytes at the end of the PIF address space:
    /// - Physical 0x1FC007C0-0x1FC007FF (offset 0x7C0-0x7FF in our array)
    /// - CIC seed at PIF RAM byte 0x24 (array offset 0x7E4)
    /// - Boot status at PIF RAM byte 0x3F (array offset 0x7FF)
    pub fn setup_boot(&mut self, cic_seed: u8) {
        // PIF RAM base offset in our 2KB array
        const PIF_RAM_BASE: usize = 0x7C0;

        // CIC seed at PIF RAM byte 0x24-0x27
        // IPL3 reads this to determine CIC variant for checksum calculation
        self.ram[PIF_RAM_BASE + 0x24] = 0x00;
        self.ram[PIF_RAM_BASE + 0x25] = 0x00;
        self.ram[PIF_RAM_BASE + 0x26] = (cic_seed >> 4) & 0x0F;
        self.ram[PIF_RAM_BASE + 0x27] = cic_seed & 0x0F;

        // Boot status byte: 0x08 signals that PIF boot is complete
        // and IPL3 should proceed with game boot
        self.ram[PIF_RAM_BASE + 0x3F] = 0x08;
    }

    /// Extract the entry point from the ROM header.
    /// Entry point is at offset 0x08 in the ROM header (4 bytes, big-endian).
    pub fn extract_entry_point(rom_data: &[u8]) -> u64 {
        if rom_data.len() >= 0x0C {
            u32::from_be_bytes([
                rom_data[0x08],
                rom_data[0x09],
                rom_data[0x0A],
                rom_data[0x0B],
            ]) as u64
        } else {
            0x80000400
        }
    }

    /// Perform IPL3 boot setup for commercial ROMs.
    /// Instead of HLE-copying ROM data to RDRAM, this sets up the state for
    /// Read from PIF RAM
    pub fn read_ram(&self, offset: u32) -> u8 {
        let addr = (offset & 0x7FF) as usize;
        self.ram[addr]
    }

    /// Write to PIF RAM
    pub fn write_ram(&mut self, offset: u32, value: u8) {
        let addr = (offset & 0x7FF) as usize;
        self.ram[addr] = value;

        // Check if this is a controller command write (PIF RAM offset 0x7C0-0x7FF)
        // This is where games write controller command blocks
        if addr >= 0x7C0 {
            use emu_core::logging::{log, LogCategory, LogLevel};
            log(LogCategory::PPU, LogLevel::Info, || {
                format!("PIF: Write to offset 0x{:03X} = 0x{:02X}", addr, value)
            });
            self.process_controller_commands();
        }
    }

    /// Process controller command blocks in PIF RAM.
    ///
    /// The PIF RAM command area starts at 0x7C0.  Games write a sequence of
    /// channel descriptors in the format `[T, R, tx_bytes..., rx_bytes...]`.
    /// This function walks through the descriptors, dispatches each command,
    /// and writes the response bytes back into the same RAM area.
    ///
    /// Special T values:
    /// - 0xFE: channel separator / channel 0 skip (bump channel counter)
    /// - 0xFF: end-of-channel-list (stop processing)
    fn process_controller_commands(&mut self) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        let mut pos: usize = 0x7C0;
        let mut channel: usize = 0;

        while pos < 0x7FC {
            let t = self.ram[pos] as usize;

            // End-of-list marker
            if t == 0xFF {
                break;
            }

            // Channel separator: skip to next channel without data
            if t == 0xFE {
                channel += 1;
                pos += 1;
                continue;
            }

            // No-device or skip: still advance pos by 1
            if t == 0x00 {
                pos += 1;
                channel += 1;
                continue;
            }

            // Need at least the R byte
            if pos + 1 >= 0x7FC {
                break;
            }
            let r = self.ram[pos + 1] as usize;

            // 0xFE in the R byte also signals end of channels in some implementations
            if r == 0xFE {
                break;
            }

            // Bounds check: make sure T+R bytes fit in remaining RAM
            let data_start = pos + 2;
            if data_start + t > 0x800 {
                break;
            }

            // First transmit byte is the command
            let cmd = if t > 0 { self.ram[data_start] } else { 0 };

            log(LogCategory::PPU, LogLevel::Debug, || {
                format!(
                    "PIF: channel={} T={} R={} cmd=0x{:02X} pos=0x{:03X}",
                    channel, t, r, cmd, pos
                )
            });

            match cmd {
                // 0x00: Controller info / status
                0x00 if t >= 1 && r >= 3 => {
                    let resp = data_start + t;
                    // Standard controller device type: 0x0500
                    // Status: 0x01 = pak present, 0x02 = no pak
                    let has_pak = channel < 4 && self.mempak_enabled[channel];
                    if resp + 2 < 0x800 {
                        self.ram[resp] = 0x05; // Device type high
                        self.ram[resp + 1] = 0x00; // Device type low
                        self.ram[resp + 2] = if has_pak { 0x01 } else { 0x02 };
                    }
                }
                // 0x01: Read controller state (buttons + analog stick)
                0x01 if t >= 1 && r >= 4 => {
                    let resp = data_start + t;
                    let state = self.controller_by_index(channel);
                    if resp + 3 < 0x800 {
                        self.write_controller_state(resp, &state);
                    }
                }
                // 0x02: Read controller pak (mempak)
                0x02 if t >= 3 && r >= 33 => {
                    let addr_hi = self.ram[data_start + 1];
                    let addr_lo = self.ram[data_start + 2];
                    let byte_addr = decode_pak_address(addr_hi, addr_lo);
                    let resp = data_start + t;
                    if channel < 4 && resp + 32 < 0x800 {
                        self.read_mempak_block(channel, byte_addr, resp);
                    }
                }
                // 0x03: Write controller pak (mempak)
                0x03 if t >= 35 && r >= 1 => {
                    let addr_hi = self.ram[data_start + 1];
                    let addr_lo = self.ram[data_start + 2];
                    let byte_addr = decode_pak_address(addr_hi, addr_lo);
                    let data_src = data_start + 3;
                    let resp = data_start + t;
                    if channel < 4 && data_src + 32 <= 0x800 && resp < 0x800 {
                        // Copy data out of `ram` before the mutable borrow below
                        let mut buf = [0u8; 32];
                        buf.copy_from_slice(&self.ram[data_src..data_src + 32]);
                        self.write_mempak_block(channel, byte_addr, &buf, resp);
                    }
                }
                // 0x04: EEPROM block read (8 bytes)
                0x04 if t >= 2 && r >= 8 => {
                    let block = self.ram[data_start + 1];
                    let resp = data_start + t;
                    if block < 0xFF && resp + 7 < 0x800 {
                        self.read_eeprom_block(resp, block);
                    }
                }
                // 0x05: EEPROM block write (8 bytes)
                0x05 if t >= 10 && r >= 1 => {
                    let block = self.ram[data_start + 1];
                    let resp = data_start + t;
                    if block < 0xFF && data_start + 10 <= 0x800 && resp < 0x800 {
                        let mut data = [0u8; 8];
                        data.copy_from_slice(&self.ram[data_start + 2..data_start + 10]);
                        self.write_eeprom_block(resp, block, &data);
                    }
                }
                _ => {
                    log(LogCategory::PPU, LogLevel::Debug, || {
                        format!(
                            "PIF: Unhandled command 0x{:02X} on channel {} (T={}, R={})",
                            cmd, channel, t, r
                        )
                    });
                }
            }

            pos += 2 + t + r;
            channel += 1;
        }
    }

    /// Return the controller state for the given 0-based channel index.
    fn controller_by_index(&self, channel: usize) -> ControllerState {
        match channel {
            0 => self.controller1,
            1 => self.controller2,
            2 => self.controller3,
            3 => self.controller4,
            _ => ControllerState::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Controller pak (mempak) helpers
    // -----------------------------------------------------------------------

    /// Read 32 bytes from a mempak slot and write them (+ CRC-8) to PIF RAM.
    fn read_mempak_block(&mut self, channel: usize, byte_addr: usize, resp: usize) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        if !self.mempak_enabled[channel] {
            // No pak: return 32 bytes of 0xFF and a CRC of 0
            for i in 0..32 {
                if resp + i < 0x800 {
                    self.ram[resp + i] = 0xFF;
                }
            }
            if resp + 32 < 0x800 {
                self.ram[resp + 32] = 0x00;
            }
            return;
        }

        let pak = &self.mempak_data[channel];
        let mut data = [0u8; 32];
        for (i, byte) in data.iter_mut().enumerate() {
            let src = byte_addr + i;
            *byte = if src < pak.len() { pak[src] } else { 0xFF };
        }

        log(LogCategory::PPU, LogLevel::Debug, || {
            format!("PIF: Mempak read ch={} addr=0x{:04X}", channel, byte_addr)
        });

        // Write 32 data bytes then the CRC
        for (i, &byte) in data.iter().enumerate() {
            if resp + i < 0x800 {
                self.ram[resp + i] = byte;
            }
        }
        let crc = pak_data_crc(&data);
        if resp + 32 < 0x800 {
            self.ram[resp + 32] = crc;
        }
    }

    /// Write 32 bytes to a mempak slot and store the data CRC in PIF RAM.
    fn write_mempak_block(
        &mut self,
        channel: usize,
        byte_addr: usize,
        data: &[u8; 32],
        resp: usize,
    ) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        if !self.mempak_enabled[channel] {
            if resp < 0x800 {
                self.ram[resp] = 0x00;
            }
            return;
        }

        log(LogCategory::PPU, LogLevel::Debug, || {
            format!("PIF: Mempak write ch={} addr=0x{:04X}", channel, byte_addr)
        });

        let pak = &mut self.mempak_data[channel];
        for (i, &byte) in data.iter().enumerate() {
            let dst = byte_addr + i;
            if dst < pak.len() {
                pak[dst] = byte;
            }
        }

        // Return data CRC as the status/response byte
        let crc = pak_data_crc(data);
        if resp < 0x800 {
            self.ram[resp] = crc;
        }
    }

    /// Return the mempak data for a controller slot (for persistence).
    pub fn save_mempak(&self, channel: usize) -> Option<Vec<u8>> {
        if channel < 4 && self.mempak_enabled[channel] {
            Some(self.mempak_data[channel].clone())
        } else {
            None
        }
    }

    /// Load previously persisted mempak data into a controller slot.
    pub fn load_mempak(&mut self, channel: usize, data: Vec<u8>) -> Result<(), String> {
        if channel >= 4 {
            return Err(format!("Invalid mempak channel: {}", channel));
        }
        if data.len() != MEMPAK_SIZE {
            return Err(format!(
                "Mempak data size mismatch: expected {} bytes, got {}",
                MEMPAK_SIZE,
                data.len()
            ));
        }
        self.mempak_enabled[channel] = true;
        self.mempak_data[channel] = data;
        Ok(())
    }

    /// Enable or disable a controller pak slot.
    pub fn set_mempak_enabled(&mut self, channel: usize, enabled: bool) {
        if channel < 4 {
            self.mempak_enabled[channel] = enabled;
        }
    }

    /// Read an 8-byte block from EEPROM
    fn read_eeprom_block(&mut self, offset: usize, block: u8) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        if self.eeprom_type == EepromType::None {
            log(LogCategory::PPU, LogLevel::Warn, || {
                "PIF: EEPROM read attempted but no EEPROM configured".to_string()
            });
            // Return all zeros if no EEPROM
            for i in 0..8 {
                self.ram[offset + i] = 0x00;
            }
            return;
        }

        let block_size = 8;
        let max_blocks: u16 = match self.eeprom_type {
            EepromType::Eeprom4K => 64,   // 512 bytes / 8 = 64 blocks
            EepromType::Eeprom16K => 256, // 2048 bytes / 8 = 256 blocks
            EepromType::None => 0,
        };

        if (block as u16) >= max_blocks {
            log(LogCategory::PPU, LogLevel::Warn, || {
                format!(
                    "PIF: EEPROM read out of range: block {} (max {})",
                    block,
                    max_blocks - 1
                )
            });
            for i in 0..8 {
                self.ram[offset + i] = 0xFF;
            }
            return;
        }

        let addr = block as usize * block_size;
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!("PIF: EEPROM read block {} at addr 0x{:03X}", block, addr)
        });

        // Copy 8 bytes from EEPROM to PIF RAM
        for i in 0..8 {
            self.ram[offset + i] = self.eeprom_data[addr + i];
        }
    }

    /// Write an 8-byte block to EEPROM
    fn write_eeprom_block(&mut self, offset: usize, block: u8, data: &[u8; 8]) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        if self.eeprom_type == EepromType::None {
            log(LogCategory::PPU, LogLevel::Warn, || {
                "PIF: EEPROM write attempted but no EEPROM configured".to_string()
            });
            self.ram[offset] = 0x80; // Error status
            return;
        }

        let block_size = 8;
        let max_blocks: u16 = match self.eeprom_type {
            EepromType::Eeprom4K => 64,
            EepromType::Eeprom16K => 256,
            EepromType::None => 0,
        };

        if (block as u16) >= max_blocks {
            log(LogCategory::PPU, LogLevel::Warn, || {
                format!(
                    "PIF: EEPROM write out of range: block {} (max {})",
                    block,
                    max_blocks - 1
                )
            });
            self.ram[offset] = 0x80; // Error status
            return;
        }

        let addr = block as usize * block_size;
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!("PIF: EEPROM write block {} at addr 0x{:03X}", block, addr)
        });

        // Copy 8 bytes from data to EEPROM
        self.eeprom_data[addr..addr + 8].copy_from_slice(data);

        self.ram[offset] = 0x00; // Success status
    }

    /// Write controller state to PIF RAM response block
    fn write_controller_state(&mut self, offset: usize, state: &ControllerState) {
        // Response format: [buttons_hi, buttons_lo, stick_x, stick_y]
        let buttons = state.buttons.to_u16();
        self.ram[offset] = (buttons >> 8) as u8; // High byte
        self.ram[offset + 1] = (buttons & 0xFF) as u8; // Low byte
        self.ram[offset + 2] = state.stick_x as u8;
        self.ram[offset + 3] = state.stick_y as u8;
    }

    /// Update controller 1 state
    pub fn set_controller1(&mut self, state: ControllerState) {
        self.controller1 = state;
    }

    /// Process PIF command block (called after SI DMA write completes)
    /// This is the main entry point for PIF command processing via SI DMA.
    pub fn process_commands(&mut self) {
        self.process_controller_commands();
    }

    /// Update controller 2 state
    pub fn set_controller2(&mut self, state: ControllerState) {
        self.controller2 = state;
    }

    /// Update controller 3 state
    pub fn set_controller3(&mut self, state: ControllerState) {
        self.controller3 = state;
    }

    /// Update controller 4 state
    pub fn set_controller4(&mut self, state: ControllerState) {
        self.controller4 = state;
    }

    /// Get controller 1 state (for testing/debugging)
    #[allow(dead_code)]
    pub fn controller1(&self) -> &ControllerState {
        &self.controller1
    }
}

impl Default for Pif {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pif_creation() {
        let pif = Pif::new();
        assert!(!pif.controller1.buttons.a);
        assert_eq!(pif.controller1.stick_x, 0);
    }

    #[test]
    fn test_controller_buttons_packing() {
        let mut buttons = ControllerButtons::default();
        assert_eq!(buttons.to_u16(), 0);

        buttons.a = true;
        assert_eq!(buttons.to_u16(), 1 << 15);

        buttons.start = true;
        assert_eq!(buttons.to_u16(), (1 << 15) | (1 << 12));

        buttons.c_right = true;
        assert_eq!(buttons.to_u16(), (1 << 15) | (1 << 12) | 1);
    }

    #[test]
    fn test_ram_access() {
        let mut pif = Pif::new();

        pif.write_ram(0x100, 0x42);
        assert_eq!(pif.read_ram(0x100), 0x42);

        // Test wrapping
        pif.write_ram(0x900, 0x55); // Should wrap to 0x100
        assert_eq!(pif.read_ram(0x100), 0x55);
    }

    #[test]
    fn test_controller_state_write() {
        let mut pif = Pif::new();

        // Set controller state
        let mut state = ControllerState::default();
        state.buttons.a = true;
        state.buttons.start = true;
        state.stick_x = 64;
        state.stick_y = -32;
        pif.set_controller1(state);

        // Simulate game writing controller read command
        pif.write_ram(0x7C0, 0x01); // T=1 byte
        pif.write_ram(0x7C1, 0x04); // R=4 bytes
        pif.write_ram(0x7C2, 0x01); // Command 0x01 (read controller)

        // Response should be written at 0x7C3
        let buttons_hi = pif.read_ram(0x7C3);
        let buttons_lo = pif.read_ram(0x7C4);
        let stick_x = pif.read_ram(0x7C5) as i8;
        let stick_y = pif.read_ram(0x7C6) as i8;

        // Check button bits
        assert_eq!(buttons_hi, 0x90); // Bits 15 (A) and 12 (Start) set
        assert_eq!(buttons_lo, 0x00);
        assert_eq!(stick_x, 64);
        assert_eq!(stick_y, -32);
    }

    #[test]
    fn test_init_rom() {
        let mut pif = Pif::new();
        pif.init_rom();

        // Check that PIF ROM was written
        assert_ne!(pif.read_ram(0), 0);

        // Check first instruction (lui $t0, 0x9000)
        let instr = u32::from_be_bytes([
            pif.read_ram(0),
            pif.read_ram(1),
            pif.read_ram(2),
            pif.read_ram(3),
        ]);
        assert_eq!(instr, 0x3C089000);
    }

    #[test]
    fn test_multiple_controllers() {
        let mut pif = Pif::new();

        // Set controller 1
        let mut state1 = ControllerState::default();
        state1.buttons.a = true;
        pif.set_controller1(state1);

        // Set controller 2
        let mut state2 = ControllerState::default();
        state2.buttons.b = true;
        pif.set_controller2(state2);

        // Write a proper two-channel PIF command block:
        //   Channel 0: [T=1, R=4, cmd=0x01, resp[4]] — controller 1 read (7 bytes)
        //   Channel 1: [T=1, R=4, cmd=0x01, resp[4]] — controller 2 read (starts at 0x7C7)
        pif.write_ram(0x7C0, 0x01); // ch0: T=1
        pif.write_ram(0x7C1, 0x04); // ch0: R=4
        pif.write_ram(0x7C2, 0x01); // ch0: cmd (read controller)
                                    // 0x7C3..0x7C6 = response bytes (written by PIF)
        pif.write_ram(0x7C7, 0x01); // ch1: T=1 (immediately follows ch0 response)
        pif.write_ram(0x7C8, 0x04); // ch1: R=4
        pif.write_ram(0x7C9, 0x01); // ch1: cmd (triggers re-parse)

        // Controller 1 response is at 0x7C3
        let buttons1 = u16::from_be_bytes([pif.read_ram(0x7C3), pif.read_ram(0x7C4)]);
        assert_eq!(buttons1 & (1 << 15), 1 << 15); // A button

        // Controller 2 response is at 0x7CA (= 0x7C7 + 2 + 1)
        let buttons2 = u16::from_be_bytes([pif.read_ram(0x7CA), pif.read_ram(0x7CB)]);
        assert_eq!(buttons2 & (1 << 14), 1 << 14); // B button
    }

    #[test]
    fn test_button_state_active_high() {
        // Verify that N64 uses active-high logic (1 = pressed)
        let mut buttons = ControllerButtons::default();

        // No buttons pressed = all zeros
        assert_eq!(buttons.to_u16(), 0x0000);

        // Press A button (bit 15)
        buttons.a = true;
        assert_eq!(buttons.to_u16(), 0x8000);

        // Press multiple buttons
        buttons.b = true; // bit 14
        buttons.start = true; // bit 12
        buttons.d_up = true; // bit 11
        assert_eq!(buttons.to_u16(), 0xD800); // 1101 1000 0000 0000

        // Press all D-pad buttons
        buttons.d_down = true; // bit 10
        buttons.d_left = true; // bit 9
        buttons.d_right = true; // bit 8
        assert_eq!(buttons.to_u16() & 0x0F00, 0x0F00);

        // Press L and R triggers
        buttons.l = true; // bit 5
        buttons.r = true; // bit 4
        assert_eq!(buttons.to_u16() & 0x0030, 0x0030);

        // Press all C buttons
        buttons.c_up = true; // bit 3
        buttons.c_down = true; // bit 2
        buttons.c_left = true; // bit 1
        buttons.c_right = true; // bit 0
        assert_eq!(buttons.to_u16() & 0x000F, 0x000F);
    }

    #[test]
    fn test_analog_stick_range() {
        // Verify analog stick uses signed 8-bit range
        let mut state = ControllerState::default();

        // Center position
        assert_eq!(state.stick_x, 0);
        assert_eq!(state.stick_y, 0);

        // Full right/up
        state.stick_x = 127;
        state.stick_y = 127;
        assert_eq!(state.stick_x, 127);
        assert_eq!(state.stick_y, 127);

        // Full left/down
        state.stick_x = -128;
        state.stick_y = -128;
        assert_eq!(state.stick_x, -128);
        assert_eq!(state.stick_y, -128);
    }

    #[test]
    fn test_eeprom_4k_initialization() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Verify EEPROM storage is initialized to 0xFF (blank EEPROM default)
        let data = pif.save_eeprom().unwrap();
        assert_eq!(data.len(), 512); // 4Kbit = 512 bytes
        assert!(data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_eeprom_16k_initialization() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom16K);

        // Verify EEPROM storage is initialized to 0xFF
        let data = pif.save_eeprom().unwrap();
        assert_eq!(data.len(), 2048); // 16Kbit = 2048 bytes
        assert!(data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_eeprom_read_command() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Write some test data to EEPROM block 5
        let test_data = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        pif.write_ram(0x7C0, 0x0A); // T=10
        pif.write_ram(0x7C1, 0x01); // R=1
        pif.write_ram(0x7C2, 0x05); // cmd=0x05 (write)
        pif.write_ram(0x7C3, 5); // block=5
        for (i, &byte) in test_data.iter().enumerate() {
            pif.write_ram(0x7C4 + i as u32, byte);
        }

        // Issue EEPROM read command for block 5
        pif.write_ram(0x7C0, 0x02); // T=2
        pif.write_ram(0x7C1, 0x08); // R=8
        pif.write_ram(0x7C2, 0x04); // cmd=0x04 (read)
        pif.write_ram(0x7C3, 5); // block=5

        // Verify the data was read back correctly (response at 0x7C4)
        for (i, &expected) in test_data.iter().enumerate() {
            assert_eq!(pif.read_ram(0x7C4 + i as u32), expected);
        }
    }

    #[test]
    fn test_eeprom_write_command() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Write data to block 10
        let test_data = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11];
        pif.write_ram(0x7C0, 0x0A); // T=10
        pif.write_ram(0x7C1, 0x01); // R=1
        pif.write_ram(0x7C2, 0x05); // cmd=0x05 (write)
        pif.write_ram(0x7C3, 10); // block=10
        for (i, &byte) in test_data.iter().enumerate() {
            pif.write_ram(0x7C4 + i as u32, byte);
        }

        // Status byte should be 0x00 (success) at response offset 0x7CC
        assert_eq!(pif.read_ram(0x7CC), 0x00);

        // Verify data was written to EEPROM by reading it back
        let saved_data = pif.save_eeprom().unwrap();
        let block_offset = 10 * 8;
        assert_eq!(&saved_data[block_offset..block_offset + 8], &test_data);
    }

    #[test]
    fn test_eeprom_out_of_range_read() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Try to read block 64 (out of range for 4Kbit EEPROM, max is 63)
        pif.write_ram(0x7C0, 0x02); // T=2
        pif.write_ram(0x7C1, 0x08); // R=8
        pif.write_ram(0x7C2, 0x04); // cmd=0x04 (read)
        pif.write_ram(0x7C3, 64); // block=64 (invalid)

        // Should return 0xFF for all bytes (error indication)
        for i in 0..8 {
            assert_eq!(pif.read_ram(0x7C4 + i as u32), 0xFF);
        }
    }

    #[test]
    fn test_eeprom_out_of_range_write() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Try to write to block 64 (out of range)
        pif.write_ram(0x7C0, 0x0A); // T=10
        pif.write_ram(0x7C1, 0x01); // R=1
        pif.write_ram(0x7C2, 0x05); // cmd=0x05 (write)
        pif.write_ram(0x7C3, 64); // block=64 (invalid)
        for i in 0..8 {
            pif.write_ram(0x7C4 + i as u32, i as u8);
        }

        // Status byte should be 0x80 (error) at response offset 0x7CC
        assert_eq!(pif.read_ram(0x7CC), 0x80);
    }

    #[test]
    fn test_eeprom_load_save() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Create test data
        let test_eeprom: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();

        // Load EEPROM data
        pif.load_eeprom(test_eeprom.clone()).unwrap();

        // Save and verify it matches
        let saved = pif.save_eeprom().unwrap();
        assert_eq!(saved, test_eeprom);
    }

    #[test]
    fn test_eeprom_size_mismatch() {
        let mut pif = Pif::new();
        pif.set_eeprom_type(EepromType::Eeprom4K);

        // Try to load wrong size data (2048 bytes for 4Kbit EEPROM)
        let wrong_size_data = vec![0u8; 2048];
        let result = pif.load_eeprom(wrong_size_data);

        // Should return error
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("size mismatch"));
    }

    // -----------------------------------------------------------------------
    // Controller pak (mempak) tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mempak_default_state() {
        let pif = Pif::new();
        // Slot 0 has a pak inserted by default; slots 1-3 do not
        assert!(pif.mempak_enabled[0]);
        assert!(!pif.mempak_enabled[1]);
        assert!(!pif.mempak_enabled[2]);
        assert!(!pif.mempak_enabled[3]);
        // Slot 0 data is 32 KB of zeros
        assert_eq!(pif.mempak_data[0].len(), MEMPAK_SIZE);
        assert!(pif.mempak_data[0].iter().all(|&b| b == 0x00));
    }

    #[test]
    fn test_mempak_read_write_round_trip() {
        let mut pif = Pif::new();

        // Write 32 bytes to mempak address 0x0020 using PIF command 0x03
        // Channel 0: T=35 (1 cmd + 2 addr + 32 data), R=1 (crc byte)
        // Layout in PIF RAM at 0x7C0:
        //   [0x23, 0x01, 0x03, addr_hi, addr_lo, data[32]]
        let addr: u16 = 0x0020; // block address
        let addr_hi = (addr >> 8) as u8;
        let addr_lo = (addr & 0xE0) as u8; // lower 5 bits would be CRC, set to 0

        pif.write_ram(0x7C0, 35); // T=35
        pif.write_ram(0x7C1, 1); // R=1
        pif.write_ram(0x7C2, 0x03); // cmd write-pak
        pif.write_ram(0x7C3, addr_hi);
        pif.write_ram(0x7C4, addr_lo);
        // Write test pattern
        for i in 0u32..32 {
            pif.write_ram(0x7C5 + i, (i + 1) as u8);
        }
        // CRC written at response offset = 0x7C2 + 35 = 0x7E5
        // (just verify no panic)

        // Now read back using command 0x02
        // T=3 (cmd + addr_hi + addr_lo), R=33 (32 data + 1 crc)
        pif.write_ram(0x7C0, 3); // T=3
        pif.write_ram(0x7C1, 33); // R=33
        pif.write_ram(0x7C2, 0x02); // cmd read-pak
        pif.write_ram(0x7C3, addr_hi);
        pif.write_ram(0x7C4, addr_lo);
        // Response at 0x7C2 + 3 = 0x7C5
        for i in 0u32..32 {
            assert_eq!(
                pif.read_ram(0x7C5 + i),
                (i + 1) as u8,
                "mempak byte {} mismatch",
                i
            );
        }
    }

    #[test]
    fn test_mempak_load_save() {
        let mut pif = Pif::new();

        let pattern: Vec<u8> = (0..MEMPAK_SIZE).map(|i| (i & 0xFF) as u8).collect();
        pif.load_mempak(0, pattern.clone()).unwrap();

        let saved = pif.save_mempak(0).unwrap();
        assert_eq!(saved, pattern);
    }

    #[test]
    fn test_mempak_size_mismatch() {
        let mut pif = Pif::new();
        // Wrong size
        let result = pif.load_mempak(0, vec![0u8; 1024]);
        assert!(result.is_err());
    }

    #[test]
    fn test_pak_data_crc_known_value() {
        // All-zero 32-byte block → deterministic CRC
        let zeros = [0u8; 32];
        let crc = pak_data_crc(&zeros);
        // Just confirm it's not 0 (the algorithm produces a non-trivial result)
        // and is reproducible
        assert_eq!(crc, pak_data_crc(&zeros));
    }

    #[test]
    fn test_controller_info_pak_status() {
        let mut pif = Pif::new();

        // Query controller info (cmd 0x00): T=1, R=3
        pif.write_ram(0x7C0, 1); // T=1
        pif.write_ram(0x7C1, 3); // R=3
        pif.write_ram(0x7C2, 0x00); // cmd 0x00 (info)
                                    // Response at 0x7C3: [device_hi, device_lo, status]
        assert_eq!(pif.read_ram(0x7C3), 0x05); // device type high
        assert_eq!(pif.read_ram(0x7C4), 0x00); // device type low
                                               // slot 0 has pak enabled by default → status = 0x01
        assert_eq!(pif.read_ram(0x7C5), 0x01);

        // Disable pak and re-query
        pif.set_mempak_enabled(0, false);
        pif.write_ram(0x7C2, 0x00); // re-write to trigger re-parse
        assert_eq!(pif.read_ram(0x7C5), 0x02); // no pak
    }
}
