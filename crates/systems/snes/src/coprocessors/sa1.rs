//! SA-1 (Super Accelerator 1) Coprocessor Implementation
//!
//! The SA-1 is a custom 65C816-based coprocessor used in SNES cartridges to enhance
//! performance. It features:
//! - 10.74 MHz CPU (3x faster than main SNES CPU)
//! - 2KB internal I-RAM
//! - Variable-size BW-RAM (Battery-backed Work RAM)
//! - Hardware arithmetic (multiply, divide, cumulative sum)
//! - DMA capabilities
//! - Memory mapping control
//! - Timer functionality
//! - Variable-length bit processing
//!
//! ## Memory Map
//!
//! - **I-RAM**: $00-1F:3000-37FF, $80-9F:3000-37FF (2KB internal RAM)
//! - **BW-RAM**: $00-3F:6000-7FFF, $40-4F:0000-FFFF (configurable)
//! - **Registers**: $2200-$23FF
//!
//! ## References
//!
//! - SA-1 Registers: https://wiki.superfamicom.org/sa-1-registers
//! - Fullsnes SA-1 Documentation: https://problemkaputt.de/fullsnes.htm#snescartsaprocessor

use crate::coprocessors::{ChipType, EnhancementChip};
use emu_core::logging::{log, LogCategory, LogLevel};
use serde::{Deserialize, Serialize};

/// SA-1 Coprocessor state
#[derive(Serialize, Deserialize)]
pub struct Sa1 {
    // === Write Registers ($2200-$225B) ===
    /// $2200 - SA-1 CPU Control (CCNT)
    ccnt: u8,
    /// $2201 - Super Nintendo CPU INT Enable (SIE)
    sie: u8,
    /// $2202 - Super Nintendo CPU INT Clear (SIC)
    sic: u8,
    /// $2203-$2204 - SA-1 CPU Reset Vector (CRVL/CRVH)
    crv: u16,
    /// $2205-$2206 - SA-1 CPU NMI Vector (CNVL/CNVH)
    cnv: u16,
    /// $2207-$2208 - SA-1 CPU IRQ Vector (CIVL/CIVH)
    civ: u16,
    /// $2209 - Super Nintendo CPU Control (SCNT)
    scnt: u8,
    /// $220A - SA-1 CPU INT Enable (CIE)
    cie: u8,
    /// $220B - SA-1 CPU INT Clear (CIC)
    cic: u8,
    /// $220C-$220D - Super Nintendo CPU NMI Vector (SNVL/SNVH)
    snv: u16,
    /// $220E-$220F - Super Nintendo CPU IRQ Vector (SIVL/SIVH)
    siv: u16,
    /// $2210 - H/V Timer Control (TMC)
    tmc: u8,
    /// $2212-$2213 - H-Count (HCNTL/HCNTH)
    hcnt: u16,
    /// $2214-$2215 - V-Count (VCNTL/VCNTH)
    vcnt: u16,
    /// $2220-$2223 - Super MMC Bank registers (CXB, DXB, EXB, FXB)
    mmc_banks: [u8; 4],
    /// $2224 - Super Nintendo CPU BW-RAM Address Mapping (BMAPS)
    bmaps: u8,
    /// $2225 - SA-1 CPU BW-RAM Address Mapping (BMAP)
    bmap: u8,
    /// $2226 - Super Nintendo CPU BW-RAM Write Enable (SBWE)
    sbwe: u8,
    /// $2227 - SA-1 CPU BW-RAM Write Enable (CBWE)
    cbwe: u8,
    /// $2228 - BW-RAM Write-Protected Area (BWPA)
    bwpa: u8,
    /// $2229 - SA-1 I-RAM Write Protection (S-CPU controlled) (SIWP)
    siwp: u8,
    /// $222A - SA-1 I-RAM Write Protection (SA-1 controlled) (CIWP)
    ciwp: u8,
    /// $2230 - DMA Control (DCNT)
    dcnt: u8,
    /// $2231 - Character Conversion DMA Parameters (CDMA)
    cdma: u8,
    /// $2232-$2234 - DMA Source Device Start Address (SDAL/SDAH/SDAB)
    sda: u32,
    /// $2235-$2237 - DMA Destination Start Address (DDAL/DDAH/DDAB)
    dda: u32,
    /// $2238-$2239 - DMA Terminal Counter (DTCL/DTCH)
    dtc: u16,
    /// $223F - BW-RAM Bitmap Format (BBF)
    bbf: u8,
    /// $2240-$224F - Bitmap Register Files (BRF0-BRFF)
    brf: [u8; 16],
    /// $2250 - Arithmetic Control (MCNT)
    mcnt: u8,
    /// $2251-$2252 - Arithmetic Parameters: Multiplicand/Dividend (MAL/MAH)
    ma: u16,
    /// $2253-$2254 - Arithmetic Parameters: Multiplier/Divisor (MBL/MBH)
    mb: u16,
    /// $2258 - Variable-Length Bit Processing (VBD)
    vbd: u8,
    /// $2259-$225B - Variable-Length Bit Game Pack ROM Start Address (VDAL/VDAH/VDAB)
    vda: u32,

    // === Read Registers ($2300-$230E) ===
    /// $2300 - Super Nintendo CPU Flag Read (SFR)
    sfr: u8,
    /// $2301 - SA-1 CPU Flag Read (CFR)
    cfr: u8,
    /// $2302-$2303 - H-Count Read (HCRL/HCRH)
    hcr: u16,
    /// $2304-$2305 - V-Count Read (VCRL/VCRH)
    vcr: u16,
    /// $2306-$230A - Arithmetic Result (MR1-MR5)
    /// 40-bit result for multiplication/division/cumulative sum
    mr: u64,
    /// $230B - Arithmetic Overflow Flag (OF)
    of: u8,
    /// $230C-$230D - Variable-Length Data Read Port (VDPL/VDPH)
    vdp: u16,

    // === Internal State ===
    /// 2KB Internal I-RAM (serialized as Vec for compatibility)
    iram: Vec<u8>,
    /// BW-RAM (Battery-backed Work RAM) - typical sizes: 256KB or 1MB
    bw_ram: Vec<u8>,
    /// Timer counter
    timer: u32,
}

impl Default for Sa1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sa1 {
    /// Create a new SA-1 instance with default BW-RAM size (256KB)
    pub fn new() -> Self {
        Self::with_bw_ram_size(0x40000) // 256KB
    }

    /// Create a new SA-1 instance with specified BW-RAM size
    pub fn with_bw_ram_size(bw_ram_size: usize) -> Self {
        Self {
            // Write registers initialized to power-on defaults
            ccnt: 0x20,
            sie: 0x00,
            sic: 0x00,
            crv: 0x0000,
            cnv: 0x0000,
            civ: 0x0000,
            scnt: 0x00,
            cie: 0x00,
            cic: 0x00,
            snv: 0x0000,
            siv: 0x0000,
            tmc: 0x00,
            hcnt: 0x0000,
            vcnt: 0x0000,
            mmc_banks: [0x00, 0x01, 0x02, 0x03], // Default bank values
            bmaps: 0x00,
            bmap: 0x00,
            sbwe: 0x00,
            cbwe: 0x00,
            bwpa: 0xFF,
            siwp: 0x00,
            ciwp: 0x00,
            dcnt: 0x00,
            cdma: 0x00,
            sda: 0x000000,
            dda: 0x000000,
            dtc: 0x0000,
            bbf: 0x00,
            brf: [0; 16],
            mcnt: 0x00,
            ma: 0x0000,
            mb: 0x0000,
            vbd: 0x00,
            vda: 0x000000,

            // Read registers
            sfr: 0x00,
            cfr: 0x00,
            hcr: 0x0000,
            vcr: 0x0000,
            mr: 0,
            of: 0x00,
            vdp: 0x0000,

            // Internal state
            iram: vec![0; 0x800],
            bw_ram: vec![0; bw_ram_size],
            timer: 0,
        }
    }

    /// Read from SA-1 register space
    fn read_register(&mut self, addr: u32) -> u8 {
        let offset = (addr & 0xFFFF) as u16;

        match offset {
            // Read registers ($2300-$230E)
            0x2300 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Read SFR (Super Nintendo CPU Flag) = ${:02X}",
                        self.sfr
                    )
                });
                self.sfr
            }
            0x2301 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Read CFR (SA-1 CPU Flag) = ${:02X}", self.cfr)
                });
                self.cfr
            }
            0x2302 => (self.hcr & 0xFF) as u8,        // HCRL
            0x2303 => ((self.hcr >> 8) & 0x01) as u8, // HCRH (only bit 0)
            0x2304 => (self.vcr & 0xFF) as u8,        // VCRL
            0x2305 => ((self.vcr >> 8) & 0x01) as u8, // VCRH (only bit 0)
            0x2306 => (self.mr & 0xFF) as u8,         // MR1
            0x2307 => ((self.mr >> 8) & 0xFF) as u8,  // MR2
            0x2308 => ((self.mr >> 16) & 0xFF) as u8, // MR3
            0x2309 => ((self.mr >> 24) & 0xFF) as u8, // MR4
            0x230A => ((self.mr >> 32) & 0xFF) as u8, // MR5
            0x230B => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Read OF (Overflow Flag) = ${:02X}", self.of)
                });
                self.of
            }
            0x230C => (self.vdp & 0xFF) as u8,        // VDPL
            0x230D => ((self.vdp >> 8) & 0xFF) as u8, // VDPH
            0x230E => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    "SA-1: Read VC (Version Code)".to_string()
                });
                0x23 // Version code
            }
            _ => {
                log(LogCategory::Bus, LogLevel::Warn, || {
                    format!("SA-1: Read from unimplemented register ${:04X}", offset)
                });
                0
            }
        }
    }

    /// Write to SA-1 register space
    fn write_register(&mut self, addr: u32, value: u8) {
        let offset = (addr & 0xFFFF) as u16;

        match offset {
            // === SA-1 CPU Control ===
            0x2200 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write CCNT (SA-1 CPU Control) = ${:02X}", value)
                });
                self.ccnt = value;
            }
            0x2201 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write SIE (SNES CPU INT Enable) = ${:02X}", value)
                });
                self.sie = value;
            }
            0x2202 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write SIC (SNES CPU INT Clear) = ${:02X}", value)
                });
                self.sic = value;
            }
            0x2203 => self.crv = (self.crv & 0xFF00) | (value as u16), // CRVL
            0x2204 => self.crv = (self.crv & 0x00FF) | ((value as u16) << 8), // CRVH
            0x2205 => self.cnv = (self.cnv & 0xFF00) | (value as u16), // CNVL
            0x2206 => self.cnv = (self.cnv & 0x00FF) | ((value as u16) << 8), // CNVH
            0x2207 => self.civ = (self.civ & 0xFF00) | (value as u16), // CIVL
            0x2208 => self.civ = (self.civ & 0x00FF) | ((value as u16) << 8), // CIVH

            // === SNES CPU Control ===
            0x2209 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write SCNT (SNES CPU Control) = ${:02X}", value)
                });
                self.scnt = value;
            }
            0x220A => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write CIE (SA-1 CPU INT Enable) = ${:02X}", value)
                });
                self.cie = value;
            }
            0x220B => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write CIC (SA-1 CPU INT Clear) = ${:02X}", value)
                });
                self.cic = value;
            }
            0x220C => self.snv = (self.snv & 0xFF00) | (value as u16), // SNVL
            0x220D => self.snv = (self.snv & 0x00FF) | ((value as u16) << 8), // SNVH
            0x220E => self.siv = (self.siv & 0xFF00) | (value as u16), // SIVL
            0x220F => self.siv = (self.siv & 0x00FF) | ((value as u16) << 8), // SIVH

            // === Timer Control ===
            0x2210 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write TMC (Timer Control) = ${:02X}", value)
                });
                self.tmc = value;
            }
            0x2211 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    "SA-1: Write CTR (Timer Restart)".to_string()
                });
                self.timer = 0;
            }
            0x2212 => self.hcnt = (self.hcnt & 0xFF00) | (value as u16), // HCNTL
            0x2213 => self.hcnt = (self.hcnt & 0x00FF) | ((value as u16 & 0x01) << 8), // HCNTH
            0x2214 => self.vcnt = (self.vcnt & 0xFF00) | (value as u16), // VCNTL
            0x2215 => self.vcnt = (self.vcnt & 0x00FF) | ((value as u16 & 0x01) << 8), // VCNTH

            // === Super MMC Bank Registers ===
            0x2220 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write CXB (Super MMC Bank C) = ${:02X}", value)
                });
                self.mmc_banks[0] = value;
            }
            0x2221 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write DXB (Super MMC Bank D) = ${:02X}", value)
                });
                self.mmc_banks[1] = value;
            }
            0x2222 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write EXB (Super MMC Bank E) = ${:02X}", value)
                });
                self.mmc_banks[2] = value;
            }
            0x2223 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write FXB (Super MMC Bank F) = ${:02X}", value)
                });
                self.mmc_banks[3] = value;
            }

            // === BW-RAM Mapping ===
            0x2224 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write BMAPS (SNES BW-RAM Mapping) = ${:02X}", value)
                });
                self.bmaps = value;
            }
            0x2225 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write BMAP (SA-1 BW-RAM Mapping) = ${:02X}", value)
                });
                self.bmap = value;
            }
            0x2226 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write SBWE (SNES BW-RAM Write Enable) = ${:02X}",
                        value
                    )
                });
                self.sbwe = value;
            }
            0x2227 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write CBWE (SA-1 BW-RAM Write Enable) = ${:02X}",
                        value
                    )
                });
                self.cbwe = value;
            }
            0x2228 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write BWPA (BW-RAM Write-Protected Area) = ${:02X}",
                        value
                    )
                });
                self.bwpa = value;
            }
            0x2229 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write SIWP (I-RAM Write Protection - S-CPU) = ${:02X}",
                        value
                    )
                });
                self.siwp = value;
            }
            0x222A => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write CIWP (I-RAM Write Protection - SA-1) = ${:02X}",
                        value
                    )
                });
                self.ciwp = value;
            }

            // === DMA Control ===
            0x2230 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write DCNT (DMA Control) = ${:02X}", value)
                });
                self.dcnt = value;
            }
            0x2231 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write CDMA (Character Conversion DMA) = ${:02X}",
                        value
                    )
                });
                self.cdma = value;
            }
            0x2232 => self.sda = (self.sda & 0xFFFF00) | (value as u32), // SDAL
            0x2233 => self.sda = (self.sda & 0xFF00FF) | ((value as u32) << 8), // SDAH
            0x2234 => self.sda = (self.sda & 0x00FFFF) | ((value as u32) << 16), // SDAB
            0x2235 => self.dda = (self.dda & 0xFFFF00) | (value as u32), // DDAL
            0x2236 => {
                self.dda = (self.dda & 0xFF00FF) | ((value as u32) << 8); // DDAH
                log(LogCategory::Bus, LogLevel::Trace, || {
                    "SA-1: Write DDAH - Initialize I-RAM DMA".to_string()
                });
            }
            0x2237 => {
                self.dda = (self.dda & 0x00FFFF) | ((value as u32) << 16); // DDAB
                log(LogCategory::Bus, LogLevel::Trace, || {
                    "SA-1: Write DDAB - Initialize BW-RAM DMA".to_string()
                });
            }
            0x2238 => self.dtc = (self.dtc & 0xFF00) | (value as u16), // DTCL
            0x2239 => self.dtc = (self.dtc & 0x00FF) | ((value as u16) << 8), // DTCH

            // === Bitmap Registers ===
            0x223F => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write BBF (BW-RAM Bitmap Format) = ${:02X}", value)
                });
                self.bbf = value;
            }
            0x2240..=0x224F => {
                let idx = (offset - 0x2240) as usize;
                self.brf[idx] = value;
            }

            // === Arithmetic Control ===
            0x2250 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SA-1: Write MCNT (Arithmetic Control) = ${:02X}", value)
                });
                self.mcnt = value;
            }
            0x2251 => self.ma = (self.ma & 0xFF00) | (value as u16), // MAL
            0x2252 => {
                self.ma = (self.ma & 0x00FF) | ((value as u16) << 8); // MAH
                self.execute_arithmetic(); // Trigger arithmetic operation
            }
            0x2253 => self.mb = (self.mb & 0xFF00) | (value as u16), // MBL
            0x2254 => {
                self.mb = (self.mb & 0x00FF) | ((value as u16) << 8); // MBH
                self.execute_arithmetic(); // Trigger arithmetic operation
            }

            // === Variable-Length Bit Processing ===
            0x2258 => {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write VBD (Variable-Length Bit Processing) = ${:02X}",
                        value
                    )
                });
                self.vbd = value;
            }
            0x2259 => self.vda = (self.vda & 0xFFFF00) | (value as u32), // VDAL
            0x225A => self.vda = (self.vda & 0xFF00FF) | ((value as u32) << 8), // VDAH
            0x225B => {
                self.vda = (self.vda & 0x00FFFF) | ((value as u32) << 16); // VDAB
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SA-1: Write VDAB - Start VBD execution at ${:06X}",
                        self.vda
                    )
                });
            }

            _ => {
                log(LogCategory::Bus, LogLevel::Warn, || {
                    format!(
                        "SA-1: Write to unimplemented register ${:04X} = ${:02X}",
                        offset, value
                    )
                });
            }
        }
    }

    /// Execute arithmetic operation based on MCNT register
    fn execute_arithmetic(&mut self) {
        let operation = self.mcnt & 0x03;

        match operation {
            0x00 => {
                // Multiplication (signed 16-bit x signed 16-bit = signed 32-bit)
                let a = self.ma as i16;
                let b = self.mb as i16;
                let result = (a as i32).wrapping_mul(b as i32);
                self.mr = result as u32 as u64;
                self.of = 0; // Multiplication doesn't set overflow
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "SA-1: Multiplication {} * {} = {} (${:08X})",
                        a, b, result, result
                    )
                });
            }
            0x01 => {
                // Division (signed 16-bit / unsigned 16-bit)
                let dividend = self.ma as i16;
                let divisor = self.mb;

                if divisor == 0 {
                    // Division by zero
                    self.mr = 0;
                    self.of = 0x80;
                    log(LogCategory::Bus, LogLevel::Warn, || {
                        "SA-1: Division by zero".to_string()
                    });
                } else {
                    let quotient = dividend / (divisor as i16);
                    let remainder = (dividend % (divisor as i16)) as u16;
                    // Quotient in MR1-MR2 (signed), remainder in MR3-MR4 (unsigned)
                    self.mr = ((quotient as u16) as u64) | (((remainder as u64) & 0xFFFF) << 16);
                    self.of = 0;
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!(
                            "SA-1: Division {} / {} = {} remainder {}",
                            dividend, divisor, quotient, remainder
                        )
                    });
                }
            }
            0x02 => {
                // Cumulative sum (signed 40-bit accumulation)
                let a = self.ma as i16;
                let b = self.mb as i16;
                let product = (a as i64).wrapping_mul(b as i64);
                let current = self.mr as i64;
                let sum = current.wrapping_add(product);

                // Check for overflow (40-bit)
                let max_40bit = (1i64 << 39) - 1;
                let min_40bit = -(1i64 << 39);

                if sum > max_40bit || sum < min_40bit {
                    self.of = 0x80;
                } else {
                    self.of = 0;
                }

                self.mr = (sum as u64) & 0xFF_FFFF_FFFF;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "SA-1: Cumulative sum: {} + ({} * {}) = {}",
                        current, a, b, sum
                    )
                });
            }
            _ => {
                log(LogCategory::Bus, LogLevel::Warn, || {
                    format!("SA-1: Unknown arithmetic operation {}", operation)
                });
            }
        }
    }

    /// Read from I-RAM
    fn read_iram(&self, offset: u16) -> u8 {
        if (offset as usize) < self.iram.len() {
            self.iram[offset as usize]
        } else {
            0
        }
    }

    /// Write to I-RAM (with protection check)
    fn write_iram(&mut self, offset: u16, value: u8, from_sa1: bool) {
        if (offset as usize) >= self.iram.len() {
            return;
        }

        // Check write protection
        let block = (offset / 0x100) as usize;
        let protection = if from_sa1 { self.ciwp } else { self.siwp };

        if (protection & (1 << block)) != 0 {
            log(LogCategory::Bus, LogLevel::Trace, || {
                format!("SA-1: I-RAM write blocked by protection at ${:04X}", offset)
            });
            return;
        }

        self.iram[offset as usize] = value;
    }

    /// Read from BW-RAM
    fn read_bw_ram(&self, offset: usize) -> u8 {
        if offset < self.bw_ram.len() {
            self.bw_ram[offset]
        } else {
            0
        }
    }

    /// Write to BW-RAM (with protection check)
    fn write_bw_ram(&mut self, offset: usize, value: u8, from_sa1: bool) {
        if offset >= self.bw_ram.len() {
            return;
        }

        // Check write enable
        let write_enabled = if from_sa1 {
            (self.cbwe & 0x80) != 0
        } else {
            (self.sbwe & 0x80) != 0
        };

        if !write_enabled {
            log(LogCategory::Bus, LogLevel::Trace, || {
                format!(
                    "SA-1: BW-RAM write blocked (not enabled) at ${:06X}",
                    offset
                )
            });
            return;
        }

        // Check write protection area
        // BWPA bits 0-3 define protected area size: 1024 * 2^(AAAA+1)
        let area_code = (self.bwpa & 0x0F) as usize;
        let protected_size = 1024 * (1 << (area_code + 1));
        if offset < protected_size {
            log(LogCategory::Bus, LogLevel::Trace, || {
                format!(
                    "SA-1: BW-RAM write blocked by protection at ${:06X}",
                    offset
                )
            });
            return;
        }

        self.bw_ram[offset] = value;
    }
}

impl EnhancementChip for Sa1 {
    fn read(&mut self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // Register space: $2200-$23FF (check first, takes precedence)
        if (0x2200..=0x23FF).contains(&offset) {
            return self.read_register(addr);
        }

        match bank {
            // I-RAM: Banks $00-$1F, $80-$9F at $3000-$37FF
            0x00..=0x1F | 0x80..=0x9F => {
                if (0x3000..=0x37FF).contains(&offset) {
                    return self.read_iram(offset - 0x3000);
                }
            }
            // BW-RAM: Banks $40-$4F (full 64KB each) or Banks $60-$6F
            0x40..=0x4F => {
                let bw_offset = (((bank - 0x40) as usize) << 16) | (offset as usize);
                return self.read_bw_ram(bw_offset);
            }
            0x60..=0x6F => {
                let bw_offset = (((bank - 0x60) as usize) << 16) | (offset as usize);
                return self.read_bw_ram(bw_offset);
            }
            _ => {}
        }

        log(LogCategory::Bus, LogLevel::Warn, || {
            format!("SA-1: Unhandled read from ${:06X}", addr)
        });
        0
    }

    fn write(&mut self, addr: u32, value: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // Register space: $2200-$23FF (check first, takes precedence)
        if (0x2200..=0x23FF).contains(&offset) {
            self.write_register(addr, value);
            return;
        }

        match bank {
            // I-RAM: Banks $00-$1F, $80-$9F at $3000-$37FF
            0x00..=0x1F | 0x80..=0x9F => {
                if (0x3000..=0x37FF).contains(&offset) {
                    self.write_iram(offset - 0x3000, value, false);
                    return;
                }
            }
            // BW-RAM: Banks $40-$4F (full 64KB each) or Banks $60-$6F
            0x40..=0x4F => {
                let bw_offset = (((bank - 0x40) as usize) << 16) | (offset as usize);
                self.write_bw_ram(bw_offset, value, false);
                return;
            }
            0x60..=0x6F => {
                let bw_offset = (((bank - 0x60) as usize) << 16) | (offset as usize);
                self.write_bw_ram(bw_offset, value, false);
                return;
            }
            _ => {}
        }

        log(LogCategory::Bus, LogLevel::Warn, || {
            format!("SA-1: Unhandled write to ${:06X} = ${:02X}", addr, value)
        });
    }

    fn reset(&mut self) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SA-1: Reset".to_string()
        });

        // Reset registers to power-on defaults
        self.ccnt = 0x20;
        self.sie = 0x00;
        self.sic = 0x00;
        self.scnt = 0x00;
        self.cie = 0x00;
        self.cic = 0x00;
        self.tmc = 0x00;
        self.mmc_banks = [0x00, 0x01, 0x02, 0x03];
        self.bmaps = 0x00;
        self.bmap = 0x00;
        self.sbwe = 0x00;
        self.cbwe = 0x00;
        self.bwpa = 0xFF;
        self.siwp = 0x00;
        self.ciwp = 0x00;
        self.dcnt = 0x00;
        self.cdma = 0x00;
        self.bbf = 0x00;
        self.mcnt = 0x00;
        self.vbd = 0x00;

        // Reset internal state
        self.timer = 0;
        self.mr = 0;
        self.of = 0;
    }

    fn chip_type(&self) -> ChipType {
        ChipType::Sa1
    }

    fn save_state(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("Failed to serialize SA-1 state: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<(), String> {
        let loaded: Sa1 = serde_json::from_str(state)
            .map_err(|e| format!("Failed to deserialize SA-1 state: {}", e))?;
        *self = loaded;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sa1_creation() {
        let sa1 = Sa1::new();
        assert_eq!(sa1.chip_type(), ChipType::Sa1);
        assert_eq!(sa1.bw_ram.len(), 0x40000); // 256KB default
    }

    #[test]
    fn test_sa1_custom_bw_ram_size() {
        let sa1 = Sa1::with_bw_ram_size(0x100000); // 1MB
        assert_eq!(sa1.bw_ram.len(), 0x100000);
    }

    #[test]
    fn test_sa1_reset() {
        let mut sa1 = Sa1::new();
        sa1.ccnt = 0xFF;
        sa1.sie = 0xFF;
        sa1.reset();
        assert_eq!(sa1.ccnt, 0x20);
        assert_eq!(sa1.sie, 0x00);
    }

    #[test]
    fn test_arithmetic_multiplication() {
        let mut sa1 = Sa1::new();
        // Set mode to multiplication
        sa1.write(0x002250, 0x00);
        // Set multiplicand = 10
        sa1.write(0x002251, 10);
        sa1.write(0x002252, 0);
        // Set multiplier = 20
        sa1.write(0x002253, 20);
        sa1.write(0x002254, 0);

        // Read result
        let result_low = sa1.read(0x002306) as u32;
        let result_high = sa1.read(0x002307) as u32;
        let result = result_low | (result_high << 8);

        assert_eq!(result, 200);
    }

    #[test]
    fn test_arithmetic_division() {
        let mut sa1 = Sa1::new();
        // Set mode to division
        sa1.write(0x002250, 0x01);
        // Set dividend = 100
        sa1.write(0x002251, 100);
        sa1.write(0x002252, 0);
        // Set divisor = 7
        sa1.write(0x002253, 7);
        sa1.write(0x002254, 0);

        // Read quotient
        let quotient_low = sa1.read(0x002306) as u16;
        let quotient_high = sa1.read(0x002307) as u16;
        let quotient = quotient_low | (quotient_high << 8);

        // Read remainder
        let remainder_low = sa1.read(0x002308) as u16;
        let remainder_high = sa1.read(0x002309) as u16;
        let remainder = remainder_low | (remainder_high << 8);

        assert_eq!(quotient as i16, 14); // 100 / 7 = 14
        assert_eq!(remainder, 2); // 100 % 7 = 2
    }

    #[test]
    fn test_arithmetic_division_by_zero() {
        let mut sa1 = Sa1::new();
        // Set mode to division
        sa1.write(0x002250, 0x01);
        // Set dividend = 100
        sa1.write(0x002251, 100);
        sa1.write(0x002252, 0);
        // Set divisor = 0
        sa1.write(0x002253, 0);
        sa1.write(0x002254, 0);

        // Read overflow flag
        let overflow = sa1.read(0x00230B);
        assert_eq!(overflow, 0x80);
    }

    #[test]
    fn test_iram_read_write() {
        let mut sa1 = Sa1::new();

        // Write to I-RAM at bank $00, offset $3000
        sa1.write(0x003000, 0x42);
        // Read back
        assert_eq!(sa1.read(0x003000), 0x42);

        // Mirror in bank $80
        assert_eq!(sa1.read(0x803000), 0x42);
    }

    #[test]
    fn test_register_write() {
        let mut sa1 = Sa1::new();

        // Write to BWPA register
        sa1.write(0x002228, 0x00);
        assert_eq!(sa1.bwpa, 0x00);

        // Write to SBWE register
        sa1.write(0x002226, 0x80);
        assert_eq!(sa1.sbwe, 0x80);
    }

    #[test]
    fn test_bw_ram_read_write() {
        let mut sa1 = Sa1::new();

        // Set minimal BW-RAM protection (BWPA = 0)
        sa1.write(0x002228, 0x00);
        // Enable BW-RAM writes from SNES
        sa1.write(0x002226, 0x80);

        // Write to BW-RAM at bank $40, offset $1000 (beyond 2KB protected area)
        sa1.write(0x401000, 0xAB);
        // Read back
        assert_eq!(sa1.read(0x401000), 0xAB);
    }

    #[test]
    fn test_iram_write_protection() {
        let mut sa1 = Sa1::new();

        // Protect block 0 ($3000-$30FF) from SNES writes
        sa1.write(0x002229, 0x01);

        // Try to write to protected area
        sa1.write(0x003000, 0x42);
        // Should not write
        assert_eq!(sa1.read(0x003000), 0x00);

        // Write to unprotected area should work
        sa1.write(0x003100, 0x42);
        assert_eq!(sa1.read(0x003100), 0x42);
    }

    #[test]
    fn test_bw_ram_write_protection() {
        let mut sa1 = Sa1::new();

        // Set minimal BW-RAM protection for this test
        sa1.write(0x002228, 0x00);

        // Don't enable BW-RAM writes (default)
        // Try to write to unprotected area (beyond 2KB)
        sa1.write(0x401000, 0x42);
        // Should not write (because write not enabled)
        assert_eq!(sa1.read(0x401000), 0x00);

        // Enable writes
        sa1.write(0x002226, 0x80);
        sa1.write(0x401000, 0x42);
        assert_eq!(sa1.read(0x401000), 0x42);
    }

    #[test]
    fn test_version_register() {
        let mut sa1 = Sa1::new();
        assert_eq!(sa1.read(0x00230E), 0x23);
    }

    #[test]
    fn test_save_load_state() {
        let mut sa1 = Sa1::new();
        sa1.ccnt = 0xFF;
        sa1.iram[0] = 0x42;

        let state = sa1.save_state().unwrap();
        let mut sa1_new = Sa1::new();
        sa1_new.load_state(&state).unwrap();

        assert_eq!(sa1_new.ccnt, 0xFF);
        assert_eq!(sa1_new.iram[0], 0x42);
    }
}
