//! Game Boy Boot ROM implementation
//!
//! This module provides a built-in boot ROM that initializes hardware registers
//! to match the state after the official Nintendo boot ROM completes.
//!
//! The boot ROM is responsible for:
//! - Initializing CPU registers to post-boot values
//! - Setting up hardware registers (PPU, APU, etc.)
//! - Clearing VRAM and OAM
//! - Setting the Nintendo logo in VRAM (optional)
//!
//! This implementation provides a minimal "instant boot" that skips the
//! logo animation and directly initializes hardware to the expected state.
//!
//! # References
//! - Pan Docs: Power-Up Sequence
//! - DMG Boot ROM disassembly
//! - Hardware register initial values from various test ROMs

/// Built-in boot ROM data
///
/// This is a minimal boot ROM that initializes hardware and immediately
/// disables itself by writing to 0xFF50.
///
/// For now, we use a simplified approach: the boot ROM is executed as a
/// sequence of register writes rather than actual Z80 code execution.
pub struct BootRom {
    /// Whether the boot ROM is enabled
    enabled: bool,
    /// Boot ROM data (256 bytes for DMG, 2304 bytes for CGB)
    data: Vec<u8>,
    /// Boot ROM type (DMG or CGB)
    boot_type: BootRomType,
}

/// Boot ROM type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootRomType {
    /// Original Game Boy (DMG) boot ROM (256 bytes)
    Dmg,
    /// Game Boy Color (CGB) boot ROM (2304 bytes)
    Cgb,
}

impl BootRom {
    /// Create a new built-in boot ROM
    pub fn new_builtin(boot_type: BootRomType) -> Self {
        let data = match boot_type {
            BootRomType::Dmg => Self::create_dmg_boot_rom(),
            BootRomType::Cgb => Self::create_cgb_boot_rom(),
        };

        Self {
            enabled: true,
            data,
            boot_type,
        }
    }

    /// Load an external boot ROM from data
    pub fn from_data(data: Vec<u8>) -> Result<Self, String> {
        let boot_type = match data.len() {
            256 => BootRomType::Dmg,
            2304 => BootRomType::Cgb,
            _ => {
                return Err(format!(
                    "Invalid boot ROM size: {} bytes (expected 256 for DMG or 2304 for CGB)",
                    data.len()
                ))
            }
        };

        Ok(Self {
            enabled: true,
            data,
            boot_type,
        })
    }

    /// Check if boot ROM is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Disable the boot ROM (called when 0xFF50 is written)
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Read a byte from the boot ROM
    pub fn read(&self, addr: u16) -> u8 {
        if !self.enabled {
            return 0xFF; // Boot ROM disabled, return open bus value
        }

        match self.boot_type {
            BootRomType::Dmg => {
                if addr < 0x0100 {
                    self.data[addr as usize]
                } else {
                    0xFF
                }
            }
            BootRomType::Cgb => {
                if addr < 0x0100 {
                    self.data[addr as usize]
                } else if addr >= 0x0200 && addr < 0x0900 {
                    self.data[(addr - 0x0100) as usize]
                } else {
                    0xFF
                }
            }
        }
    }

    /// Get the boot ROM type
    pub fn boot_type(&self) -> BootRomType {
        self.boot_type
    }

    /// Create a minimal DMG boot ROM
    ///
    /// This is a simplified boot ROM that immediately jumps to 0x0100
    /// and disables itself. The actual hardware initialization is done
    /// by applying the post-boot register values in the system initialization.
    fn create_dmg_boot_rom() -> Vec<u8> {
        let mut rom = vec![0x00; 256];

        // Minimal boot code that immediately exits
        // This code would be at address 0x0000-0x00FF
        //
        // The actual boot sequence does:
        // 1. Clear VRAM and OAM
        // 2. Load and display Nintendo logo
        // 3. Scroll logo down
        // 4. Play boot sound
        // 5. Verify cartridge header
        // 6. Jump to 0x0100
        //
        // For our purposes, we skip all that and just jump to 0x0100
        // The register initialization is handled separately in apply_post_boot_state()

        rom[0x00] = 0x31; // LD SP, $FFFE
        rom[0x01] = 0xFE;
        rom[0x02] = 0xFF;

        rom[0x03] = 0x3E; // LD A, $01
        rom[0x04] = 0x01;

        rom[0x05] = 0xE0; // LDH ($FF50), A ; Disable boot ROM
        rom[0x06] = 0x50;

        rom[0x07] = 0xC3; // JP $0100
        rom[0x08] = 0x00;
        rom[0x09] = 0x01;

        rom
    }

    /// Create a minimal CGB boot ROM
    fn create_cgb_boot_rom() -> Vec<u8> {
        // Similar to DMG but larger (2304 bytes)
        // The CGB boot ROM has two parts:
        // - 0x0000-0x00FF: Initial boot code
        // - 0x0200-0x08FF: Extended CGB initialization
        let mut rom = vec![0x00; 2304];

        // Minimal boot code (same as DMG for first 256 bytes)
        rom[0x00] = 0x31; // LD SP, $FFFE
        rom[0x01] = 0xFE;
        rom[0x02] = 0xFF;

        rom[0x03] = 0x3E; // LD A, $01
        rom[0x04] = 0x01;

        rom[0x05] = 0xE0; // LDH ($FF50), A ; Disable boot ROM
        rom[0x06] = 0x50;

        rom[0x07] = 0xC3; // JP $0100
        rom[0x08] = 0x00;
        rom[0x09] = 0x01;

        rom
    }
}

/// Post-boot hardware register state
///
/// These are the values of hardware registers after the boot ROM completes.
/// This allows us to skip the boot ROM animation while still having correct
/// hardware initialization.
///
/// Reference: Pan Docs - Power-Up Sequence
pub struct PostBootState {
    /// CPU registers after boot
    pub cpu: CpuPostBootState,
    /// Hardware I/O registers after boot
    pub io: IoPostBootState,
}

/// CPU register state after boot
pub struct CpuPostBootState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

/// Hardware I/O register state after boot
pub struct IoPostBootState {
    // Timer registers
    pub tima: u8,  // 0xFF05
    pub tma: u8,   // 0xFF06
    pub tac: u8,   // 0xFF07

    // APU registers
    pub nr10: u8,  // 0xFF10
    pub nr11: u8,  // 0xFF11
    pub nr12: u8,  // 0xFF12
    pub nr14: u8,  // 0xFF14
    pub nr21: u8,  // 0xFF16
    pub nr22: u8,  // 0xFF17
    pub nr24: u8,  // 0xFF19
    pub nr30: u8,  // 0xFF1A
    pub nr31: u8,  // 0xFF1B
    pub nr32: u8,  // 0xFF1C
    pub nr34: u8,  // 0xFF1E
    pub nr41: u8,  // 0xFF20
    pub nr42: u8,  // 0xFF21
    pub nr43: u8,  // 0xFF22
    pub nr44: u8,  // 0xFF23
    pub nr50: u8,  // 0xFF24
    pub nr51: u8,  // 0xFF25
    pub nr52: u8,  // 0xFF26

    // PPU registers
    pub lcdc: u8,  // 0xFF40
    pub stat: u8,  // 0xFF41
    pub scy: u8,   // 0xFF42
    pub scx: u8,   // 0xFF43
    pub lyc: u8,   // 0xFF45
    pub bgp: u8,   // 0xFF47
    pub obp0: u8,  // 0xFF48
    pub obp1: u8,  // 0xFF49
    pub wy: u8,    // 0xFF4A
    pub wx: u8,    // 0xFF4B

    // Interrupt registers
    pub ie: u8,    // 0xFFFF
}

impl PostBootState {
    /// Get the DMG post-boot state
    pub fn dmg() -> Self {
        Self {
            cpu: CpuPostBootState {
                a: 0x01,
                f: 0xB0, // Z=1, N=0, H=1, C=1
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                sp: 0xFFFE,
                pc: 0x0100,
            },
            io: IoPostBootState {
                tima: 0x00,
                tma: 0x00,
                tac: 0x00,

                nr10: 0x80,
                nr11: 0xBF,
                nr12: 0xF3,
                nr14: 0xBF,
                nr21: 0x3F,
                nr22: 0x00,
                nr24: 0xBF,
                nr30: 0x7F,
                nr31: 0xFF,
                nr32: 0x9F,
                nr34: 0xBF,
                nr41: 0xFF,
                nr42: 0x00,
                nr43: 0x00,
                nr44: 0xBF,
                nr50: 0x77,
                nr51: 0xF3,
                nr52: 0xF1,

                lcdc: 0x91,
                stat: 0x00,
                scy: 0x00,
                scx: 0x00,
                lyc: 0x00,
                bgp: 0xFC,
                obp0: 0xFF,
                obp1: 0xFF,
                wy: 0x00,
                wx: 0x00,

                ie: 0x00,
            },
        }
    }

    /// Get the CGB post-boot state
    pub fn cgb() -> Self {
        Self {
            cpu: CpuPostBootState {
                a: 0x11, // CGB mode indicator
                f: 0x80, // Z=1, N=0, H=0, C=0
                b: 0x00,
                c: 0x00,
                d: 0xFF,
                e: 0x56,
                h: 0x00,
                l: 0x0D,
                sp: 0xFFFE,
                pc: 0x0100,
            },
            io: IoPostBootState {
                tima: 0x00,
                tma: 0x00,
                tac: 0x00,

                nr10: 0x80,
                nr11: 0xBF,
                nr12: 0xF3,
                nr14: 0xBF,
                nr21: 0x3F,
                nr22: 0x00,
                nr24: 0xBF,
                nr30: 0x7F,
                nr31: 0xFF,
                nr32: 0x9F,
                nr34: 0xBF,
                nr41: 0xFF,
                nr42: 0x00,
                nr43: 0x00,
                nr44: 0xBF,
                nr50: 0x77,
                nr51: 0xF3,
                nr52: 0xF1,

                lcdc: 0x91,
                stat: 0x00,
                scy: 0x00,
                scx: 0x00,
                lyc: 0x00,
                bgp: 0xFC,
                obp0: 0xFF,
                obp1: 0xFF,
                wy: 0x00,
                wx: 0x00,

                ie: 0x00,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dmg_boot_rom_creation() {
        let boot_rom = BootRom::new_builtin(BootRomType::Dmg);
        assert_eq!(boot_rom.data.len(), 256);
        assert!(boot_rom.is_enabled());
        assert_eq!(boot_rom.boot_type(), BootRomType::Dmg);
    }

    #[test]
    fn test_cgb_boot_rom_creation() {
        let boot_rom = BootRom::new_builtin(BootRomType::Cgb);
        assert_eq!(boot_rom.data.len(), 2304);
        assert!(boot_rom.is_enabled());
        assert_eq!(boot_rom.boot_type(), BootRomType::Cgb);
    }

    #[test]
    fn test_boot_rom_disable() {
        let mut boot_rom = BootRom::new_builtin(BootRomType::Dmg);
        assert!(boot_rom.is_enabled());
        boot_rom.disable();
        assert!(!boot_rom.is_enabled());
    }

    #[test]
    fn test_boot_rom_read() {
        let boot_rom = BootRom::new_builtin(BootRomType::Dmg);
        // Read first byte (should be LD SP instruction)
        assert_eq!(boot_rom.read(0x0000), 0x31);
        // Read beyond boot ROM area
        assert_eq!(boot_rom.read(0x0100), 0xFF);
    }

    #[test]
    fn test_boot_rom_read_disabled() {
        let mut boot_rom = BootRom::new_builtin(BootRomType::Dmg);
        boot_rom.disable();
        // When disabled, should return open bus value
        assert_eq!(boot_rom.read(0x0000), 0xFF);
    }

    #[test]
    fn test_external_boot_rom_dmg() {
        let data = vec![0xAB; 256];
        let boot_rom = BootRom::from_data(data).unwrap();
        assert_eq!(boot_rom.boot_type(), BootRomType::Dmg);
        assert_eq!(boot_rom.read(0x0000), 0xAB);
    }

    #[test]
    fn test_external_boot_rom_cgb() {
        let data = vec![0xCD; 2304];
        let boot_rom = BootRom::from_data(data).unwrap();
        assert_eq!(boot_rom.boot_type(), BootRomType::Cgb);
        assert_eq!(boot_rom.read(0x0000), 0xCD);
    }

    #[test]
    fn test_external_boot_rom_invalid_size() {
        let data = vec![0x00; 512]; // Invalid size
        let result = BootRom::from_data(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_post_boot_state_dmg() {
        let state = PostBootState::dmg();
        assert_eq!(state.cpu.a, 0x01);
        assert_eq!(state.cpu.pc, 0x0100);
        assert_eq!(state.io.lcdc, 0x91);
        assert_eq!(state.io.bgp, 0xFC);
    }

    #[test]
    fn test_post_boot_state_cgb() {
        let state = PostBootState::cgb();
        assert_eq!(state.cpu.a, 0x11); // CGB mode indicator
        assert_eq!(state.cpu.pc, 0x0100);
        assert_eq!(state.io.lcdc, 0x91);
    }
}
