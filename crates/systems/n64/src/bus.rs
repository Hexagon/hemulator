//! N64 memory bus implementation

use crate::ai::AudioInterface;
use crate::cartridge::Cartridge;
use crate::mi::MipsInterface;
use crate::pif::Pif;
use crate::rdp::Rdp;
use crate::rsp::Rsp;
use crate::tlb::Tlb;
use crate::vi::VideoInterface;
use crate::N64Error;
use emu_core::cpu_mips_r4300i::MemoryMips;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::types::Frame;

/// Macronix MX29L1100 FlashRAM operating mode.
///
/// The FlashRAM uses a command-register protocol: the game writes a command
/// byte (in the most-significant byte of a 32-bit word) to address 0x08000000,
/// which transitions the chip between these modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashRamMode {
    /// Normal read-array mode (default after reset/power-on)
    Read,
    /// Status-register mode: reads return the 32-bit status word
    Status,
    /// Erase mode: waiting for the 0x78 confirm write to erase the sector
    Erase,
    /// Page-write mode: subsequent byte/word writes go into flash data
    Write,
}

/// FlashRAM sector size in bytes (Macronix MX29L1100 uses 128-byte pages)
const FLASH_SECTOR_SIZE: usize = 128;

/// N64 memory bus
pub struct N64Bus {
    /// 4MB RDRAM
    rdram: Vec<u8>,
    /// PIF (Peripheral Interface - controllers and boot ROM)
    pif: Pif,
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// Cartridge save storage (32 KB for SRAM, 128 KB for FlashRAM).
    /// Named `cart_save` because it holds both SRAM and the FlashRAM data;
    /// the two are physically different chips — access is dispatched through
    /// `save_is_flashram` so the correct protocol is applied.
    cart_save: Option<Vec<u8>>,
    /// True when `cart_save` holds FlashRAM data; false for plain SRAM.
    save_is_flashram: bool,
    /// Current FlashRAM operating mode (only valid when `save_is_flashram`).
    flashram_mode: FlashRamMode,
    /// Sector start offset for a pending erase operation (set by the 0x4B command).
    flashram_erase_offset: usize,
    /// Current write position within the flash data buffer (advances per write in Write mode).
    flashram_write_offset: usize,
    /// RDP (Reality Display Processor)
    rdp: Rdp,
    /// RSP (Reality Signal Processor)
    rsp: Rsp,
    /// VI (Video Interface)
    vi: VideoInterface,
    /// MI (MIPS Interface - interrupt controller)
    mi: MipsInterface,
    /// AI (Audio Interface)
    ai: AudioInterface,
    /// TLB (Translation Lookaside Buffer)
    tlb: Tlb,
    /// Entry point from ROM header (set during cartridge load)
    entry_point: Option<u64>,
    /// PI DMA addresses
    pi_dram_addr: u32,
    pi_cart_addr: u32,
    /// SI DMA address
    si_dram_addr: u32,
    /// CIC seed (detected from ROM header, used for IPL3 register init)
    cic_seed: u8,
}

impl N64Bus {
    /// Create a new N64 bus with OpenGL renderer
    /// Requires a GL context for hardware-accelerated rendering
    pub fn new(gl: glow::Context) -> Result<Self, String> {
        let rdp = Rdp::new(gl)?;

        let mut bus = Self {
            rdram: vec![0; 4 * 1024 * 1024], // 4MB
            pif: Pif::new(),
            cartridge: None,
            cart_save: None,
            save_is_flashram: false,
            flashram_mode: FlashRamMode::Read,
            flashram_erase_offset: 0,
            flashram_write_offset: 0,
            rdp,
            rsp: Rsp::new(),
            vi: VideoInterface::new(),
            mi: MipsInterface::new(),
            ai: AudioInterface::new(),
            tlb: Tlb::new(),
            entry_point: None,
            pi_dram_addr: 0,
            pi_cart_addr: 0,
            si_dram_addr: 0,
            cic_seed: 0,
        };

        // Initialize PIF ROM
        bus.pif.init_rom();

        Ok(bus)
    }

    /// Create a new N64 bus using the software (CPU-based) renderer.
    ///
    /// Does **not** require an OpenGL context, so this is suitable for unit
    /// tests that run in CI without GPU support.
    pub fn new_for_test() -> Self {
        use crate::rdp_renderer_software::SoftwareRdpRenderer;
        let rdp = Rdp::with_renderer(Box::new(SoftwareRdpRenderer::new(320, 240)));

        let mut bus = Self {
            rdram: vec![0; 4 * 1024 * 1024],
            pif: Pif::new(),
            cartridge: None,
            cart_save: None,
            save_is_flashram: false,
            flashram_mode: FlashRamMode::Read,
            flashram_erase_offset: 0,
            flashram_write_offset: 0,
            rdp,
            rsp: Rsp::new(),
            vi: VideoInterface::new(),
            mi: MipsInterface::new(),
            ai: AudioInterface::new(),
            tlb: Tlb::new(),
            entry_point: None,
            pi_dram_addr: 0,
            pi_cart_addr: 0,
            si_dram_addr: 0,
            cic_seed: 0,
        };
        bus.pif.init_rom();
        bus
    }

    /// Update controller state (for input handling)
    pub fn set_controller1(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller1(state);
    }

    pub fn set_controller2(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller2(state);
    }

    pub fn set_controller3(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller3(state);
    }

    pub fn set_controller4(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller4(state);
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), N64Error> {
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("N64 Bus: Loading cartridge, size={} bytes", data.len())
        });

        // Load the cartridge
        let cart = Cartridge::load(data)?;

        // Auto-detect and configure save type
        let save_type = cart.save_type();
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("N64 Bus: Detected save type: {:?}", save_type)
        });
        self.configure_save_type(save_type);

        // Get ROM data and extract entry point
        let rom_data = cart.read_range(0, cart.size());
        let entry_point = crate::pif::Pif::extract_entry_point(&rom_data);

        // Copy first 0x1000 bytes of ROM (header + IPL3) into SP DMEM
        // On real hardware, PIF copies this to DMEM before starting IPL3.
        // IPL3 boot code runs from DMEM[0x40..] and handles the ROM→RDRAM
        // copy via PI DMA, proper register init, and jump to entry point.
        let dmem_size = 0x1000.min(rom_data.len());
        for (i, &byte) in rom_data.iter().enumerate().take(dmem_size) {
            self.rsp.write_dmem(i as u32, byte);
        }

        // Detect CIC type and set up PIF RAM with the seed
        let cic_seed = crate::pif::Pif::detect_cic_seed(&rom_data);
        self.cic_seed = cic_seed;
        self.pif.setup_boot(cic_seed);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "N64 Bus: IPL3 boot setup complete, entry_point=0x{:08X}, CIC seed=0x{:02X}",
                entry_point, cic_seed
            )
        });

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "N64 Bus: IPL3 boot complete, entry point=0x{:016X}",
                entry_point
            )
        });

        // Store the entry point for CPU reset
        self.cartridge = Some(cart);
        self.entry_point = Some(entry_point);

        log(LogCategory::Bus, LogLevel::Info, || {
            "N64 Bus: Cartridge loaded successfully".to_string()
        });
        Ok(())
    }

    /// Configure the save storage for the given save type.
    ///
    /// Calling this resets both cart save storage and EEPROM to their blank (all-0xFF) state.
    pub fn configure_save_type(&mut self, save_type: crate::cartridge::SaveType) {
        use crate::cartridge::SaveType;
        use crate::pif::EepromType;

        // Reset any existing save storage and FlashRAM state machine
        self.cart_save = None;
        self.save_is_flashram = false;
        self.flashram_mode = FlashRamMode::Read;
        self.flashram_erase_offset = 0;
        self.flashram_write_offset = 0;
        self.pif.set_eeprom_type(EepromType::None);

        match save_type {
            SaveType::None => {}
            SaveType::Eeprom4K => {
                self.pif.set_eeprom_type(EepromType::Eeprom4K);
            }
            SaveType::Eeprom16K => {
                self.pif.set_eeprom_type(EepromType::Eeprom16K);
            }
            SaveType::Sram => {
                // 32 KB SRAM, initialised to all-0xFF (blank)
                self.cart_save = Some(vec![0xFF; 32768]);
                self.save_is_flashram = false;
            }
            SaveType::FlashRam => {
                // 128 KB FlashRAM (Macronix MX29L1100), initialised to all-0xFF (blank).
                // Uses the Macronix command protocol: erase/write commands are
                // dispatched through the write handlers.
                self.cart_save = Some(vec![0xFF; 131072]);
                self.save_is_flashram = true;
                self.flashram_mode = FlashRamMode::Read;
            }
        }
    }

    /// Export save data for persistence.
    ///
    /// Returns `None` when no save storage is configured for the current cartridge.
    pub fn get_save_data(&self) -> Option<Vec<u8>> {
        if let Some(ref buf) = self.cart_save {
            return Some(buf.clone());
        }
        self.pif.save_eeprom()
    }

    /// Import previously persisted save data.
    ///
    /// Returns `Err` if the data length does not match the configured save storage,
    /// if no save storage is configured for the current cartridge, or if another
    /// underlying error occurs (e.g. EEPROM type not set).
    pub fn set_save_data(&mut self, data: Vec<u8>) -> Result<(), String> {
        // SRAM / FlashRAM
        if let Some(ref mut buf) = self.cart_save {
            if data.len() != buf.len() {
                return Err(format!(
                    "Save data size mismatch: expected {} bytes, got {}",
                    buf.len(),
                    data.len()
                ));
            }
            buf.copy_from_slice(&data);
            return Ok(());
        }
        // EEPROM — or no save storage configured at all
        if self.pif.save_eeprom().is_none() {
            return Err("No save storage configured for this cartridge".to_string());
        }
        self.pif.load_eeprom(data)
    }

    /// Export controller pak (mempak) data for a given controller slot (0-based).
    ///
    /// Returns `None` when no pak is inserted in the slot.
    pub fn get_mempak_data(&self, channel: usize) -> Option<Vec<u8>> {
        self.pif.save_mempak(channel)
    }

    /// Import previously persisted mempak data for a controller slot.
    pub fn set_mempak_data(&mut self, channel: usize, data: Vec<u8>) -> Result<(), String> {
        self.pif.load_mempak(channel, data)
    }

    /// Enable or disable the controller pak in a given controller slot.
    pub fn set_mempak_enabled(&mut self, channel: usize, enabled: bool) {
        self.pif.set_mempak_enabled(channel, enabled);
    }

    /// Get the entry point from the loaded cartridge (for CPU initialization)
    pub fn get_entry_point(&self) -> Option<u64> {
        self.entry_point
    }

    pub fn get_cic_seed(&self) -> u8 {
        self.cic_seed
    }

    // -----------------------------------------------------------------------
    // FlashRAM (Macronix MX29L1100) command protocol
    // -----------------------------------------------------------------------

    /// Process a 32-bit word write that targets the FlashRAM address space.
    ///
    /// The Macronix MX29L1100 uses a command-register protocol:
    /// - Writes to offset 0x00000 with a known command byte (MSB) transition the state machine.
    /// - In Write mode, every word write is stored at the advancing write pointer.
    /// - In Erase mode, a 0x78 confirmation write erases the sector selected by the prior 0x4B command.
    ///
    /// Reference: Macronix MX29L1100 data sheet; ares N64 FlashRAM implementation.
    fn flashram_write_word(&mut self, offset: usize, val: u32) {
        let command = (val >> 24) as u8;

        match self.flashram_mode {
            FlashRamMode::Erase => {
                // In Erase mode the next write with command 0x78 triggers the sector erase
                if command == 0x78 {
                    if let Some(ref mut flash) = self.cart_save {
                        let start = self.flashram_erase_offset;
                        let end = (start + FLASH_SECTOR_SIZE).min(flash.len());
                        flash[start..end].fill(0xFF);
                        log(LogCategory::Bus, LogLevel::Info, || {
                            format!(
                                "N64 FlashRAM: Erased sector at offset 0x{:05X} ({}B)",
                                start, FLASH_SECTOR_SIZE
                            )
                        });
                    }
                    self.flashram_mode = FlashRamMode::Read;
                } else {
                    // Any other command at offset 0 in Erase mode is treated as a new command
                    self.flashram_mode = FlashRamMode::Read;
                    self.flashram_handle_command(offset, command, val);
                }
            }
            FlashRamMode::Write => {
                // In Write mode, writes at offset 0 are commands (state machine transitions);
                // writes at any other offset store data at the advancing write pointer.
                if offset == 0 {
                    self.flashram_handle_command(offset, command, val);
                } else if let Some(ref mut flash) = self.cart_save {
                    let dst = self.flashram_write_offset;
                    if dst + 3 < flash.len() {
                        let bytes = val.to_be_bytes();
                        flash[dst] = bytes[0];
                        flash[dst + 1] = bytes[1];
                        flash[dst + 2] = bytes[2];
                        flash[dst + 3] = bytes[3];
                    }
                    self.flashram_write_offset = self.flashram_write_offset.wrapping_add(4);
                }
            }
            FlashRamMode::Read | FlashRamMode::Status => {
                // In Read/Status mode, writes at offset 0 are commands
                if offset == 0 {
                    self.flashram_handle_command(offset, command, val);
                }
                // Writes to other offsets in Read/Status mode are ignored
            }
        }
    }

    /// Process a FlashRAM command byte.
    fn flashram_handle_command(&mut self, _offset: usize, command: u8, val: u32) {
        match command {
            0x4B => {
                // Set erase offset + enter Erase mode.
                // Low 16 bits of the word hold the sector index; multiplied by sector
                // size (128 bytes) to get the byte offset within flash storage.
                let sector_index = (val & 0xFFFF) as usize;
                self.flashram_erase_offset = sector_index * FLASH_SECTOR_SIZE;
                self.flashram_mode = FlashRamMode::Erase;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "N64 FlashRAM: CMD 0x4B — erase mode, sector={} (offset 0x{:05X})",
                        sector_index, self.flashram_erase_offset
                    )
                });
            }
            0x78 => {
                // Erase entire chip
                if let Some(ref mut flash) = self.cart_save {
                    flash.fill(0xFF);
                    log(LogCategory::Bus, LogLevel::Info, || {
                        "N64 FlashRAM: CMD 0x78 — chip erase".to_string()
                    });
                }
                self.flashram_mode = FlashRamMode::Read;
            }
            0xA0 | 0xE1 => {
                // Enter Write mode.
                // Low 16 bits encode the write start position as a word (4-byte) index;
                // multiply by 4 to get the byte offset within flash storage.
                let byte_offset = ((val & 0x0000_FFFF) as usize) * 4;
                self.flashram_write_offset = byte_offset;
                self.flashram_mode = FlashRamMode::Write;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "N64 FlashRAM: CMD 0x{:02X} — write mode, offset 0x{:05X}",
                        command, byte_offset
                    )
                });
            }
            0xB4 | 0xFF => {
                // Enter Read (array) mode
                self.flashram_mode = FlashRamMode::Read;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!("N64 FlashRAM: CMD 0x{:02X} — read mode", command)
                });
            }
            0xD2 => {
                // Enter Status mode
                self.flashram_mode = FlashRamMode::Status;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    "N64 FlashRAM: CMD 0xD2 — status mode".to_string()
                });
            }
            0xC2 => {
                // Page write: set write address and stay in Write mode
                let byte_offset = ((val & 0x0000_FFFF) as usize) * FLASH_SECTOR_SIZE;
                self.flashram_write_offset = byte_offset;
                self.flashram_mode = FlashRamMode::Write;
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "N64 FlashRAM: CMD 0xC2 — page write offset 0x{:05X}",
                        byte_offset
                    )
                });
            }
            _ => {
                log(LogCategory::Stubs, LogLevel::Debug, || {
                    format!(
                        "N64 FlashRAM: Unknown command 0x{:02X} (word=0x{:08X})",
                        command, val
                    )
                });
            }
        }
    }

    /// Process a 32-bit word read from the FlashRAM address space.
    fn flashram_read_word(&self, offset: usize) -> u32 {
        match self.flashram_mode {
            FlashRamMode::Status => {
                // Status word: 0x11118001 = ready, no error, page written
                // Bit meanings: [31:28]=0x1 silicon ID, [27:16]=0x111 device type,
                //               [15:0]=0x8001 ready | no error
                if offset == 0 {
                    0x1111_8001
                } else {
                    0x0000_0000
                }
            }
            FlashRamMode::Read | FlashRamMode::Erase | FlashRamMode::Write => {
                if let Some(ref flash) = self.cart_save {
                    if offset + 3 < flash.len() {
                        u32::from_be_bytes([
                            flash[offset],
                            flash[offset + 1],
                            flash[offset + 2],
                            flash[offset + 3],
                        ])
                    } else {
                        0xFFFF_FFFF
                    }
                } else {
                    0xFFFF_FFFF
                }
            }
        }
    }

    /// Process a byte read from the FlashRAM address space.
    fn flashram_read_byte(&self, offset: usize) -> u8 {
        match self.flashram_mode {
            FlashRamMode::Status => {
                // Return individual bytes of status word 0x11118001
                let status: u32 = 0x1111_8001;
                let byte_offset = offset & 3;
                status.to_be_bytes()[byte_offset]
            }
            _ => {
                if let Some(ref flash) = self.cart_save {
                    *flash.get(offset).unwrap_or(&0xFF)
                } else {
                    0xFF
                }
            }
        }
    }

    pub fn unload_cartridge(&mut self) {
        self.cartridge = None;
        self.cart_save = None;
        self.save_is_flashram = false;
        self.flashram_mode = FlashRamMode::Read;
    }

    pub fn has_cartridge(&self) -> bool {
        self.cartridge.is_some()
    }

    pub fn cartridge(&self) -> Option<&Cartridge> {
        self.cartridge.as_ref()
    }

    pub fn rdp(&self) -> &Rdp {
        &self.rdp
    }

    pub fn rdp_mut(&mut self) -> &mut Rdp {
        &mut self.rdp
    }

    pub fn rsp(&self) -> &Rsp {
        &self.rsp
    }

    /// Get current RSP status register value (for diagnostics)
    #[allow(dead_code)]
    pub fn rsp_status(&self) -> u32 {
        self.rsp.read_register(0x10) // SP_STATUS
    }

    #[allow(dead_code)] // Reserved for future use
    pub fn rsp_mut(&mut self) -> &mut Rsp {
        &mut self.rsp
    }

    #[allow(dead_code)] // Reserved for future use when VI is integrated with frame rendering
    pub fn vi(&self) -> &VideoInterface {
        &self.vi
    }

    #[allow(dead_code)] // Reserved for future use when VI is integrated with frame rendering
    pub fn vi_mut(&mut self) -> &mut VideoInterface {
        &mut self.vi
    }

    #[allow(dead_code)] // Reserved for audio output integration
    pub fn ai(&self) -> &AudioInterface {
        &self.ai
    }

    /// Get mutable reference to Audio Interface for audio output
    pub fn ai_mut(&mut self) -> &mut AudioInterface {
        &mut self.ai
    }

    #[allow(dead_code)] // Reserved for future use
    pub fn mi(&self) -> &MipsInterface {
        &self.mi
    }

    pub fn mi_mut(&mut self) -> &mut MipsInterface {
        &mut self.mi
    }

    /// Execute pending RSP task if RSP is not halted
    /// Returns true if an SP interrupt should be triggered
    pub fn process_rsp_task(&mut self) -> bool {
        // Use disjoint field borrows to avoid cloning 4MB RDRAM
        // Rust allows borrowing separate struct fields simultaneously
        let (_cycles, should_interrupt) = self.rsp.execute_task(&mut self.rdram, &mut self.rdp);

        if should_interrupt {
            // Read task type from DMEM[0xFC0] to decide whether to fire DP interrupt.
            // M_GFXTASK=1 needs DP interrupt (RDP processed display list),
            // M_AUDTASK=2 does not (no RDP involvement).
            let task_type = u32::from_be_bytes([
                self.rsp.read_dmem(0xFC0),
                self.rsp.read_dmem(0xFC1),
                self.rsp.read_dmem(0xFC2),
                self.rsp.read_dmem(0xFC3),
            ]);

            // Fire SP interrupt immediately. The N64 OS kernel exception handler
            // reads MI_INTR, finds the SP bit, and posts a message to the
            // registered event queue (e.g., gIntrMesgQueue in SM64).
            self.mi.set_interrupt(super::mi::MI_INTR_SP);

            // For graphics tasks, also fire DP interrupt (display processor done).
            // SM64's scheduler uses DP_COMPLETE to know when RDP finished rendering,
            // which triggers sending the completion message to the game thread.
            if task_type == 1 {
                self.mi.set_interrupt(super::mi::MI_INTR_DP);
            }
        }

        should_interrupt
    }

    /// Process pending RDP display list if needed
    pub fn process_rdp_display_list(&mut self) {
        if self.rdp.needs_processing() {
            self.rdp.process_display_list(&self.rdram);
            // Signal DP interrupt so games know rendering is complete
            self.mi.set_interrupt(super::mi::MI_INTR_DP);
        }
    }

    /// Sync RDP framebuffer to RDRAM once per display frame.
    /// This is deferred from individual RSP/RDP operations for performance.
    pub fn sync_rdp_framebuffer(&mut self) {
        self.rdp.write_framebuffer_to_rdram(&mut self.rdram);
    }

    /// Read the framebuffer from RDRAM using VI registers
    ///
    /// This is the primary display method for N64 emulation. The game renders
    /// to a framebuffer in RDRAM (via CPU, RSP, or RDP), and the VI reads from
    /// RDRAM at VI_ORIGIN to produce video output.
    ///
    /// Returns `Some(Frame)` if VI is enabled and has a valid framebuffer,
    /// `None` if VI is disabled or origin is 0.
    #[allow(dead_code)]
    pub fn read_vi_framebuffer(&self) -> Option<Frame> {
        // Check if VI is enabled (color depth bits != 0)
        if !self.vi.is_enabled() {
            return None;
        }

        let origin = self.vi.get_framebuffer_origin() as usize;
        let width = self.vi.get_width() as usize;
        let height = self.vi.get_display_height() as usize;
        let color_depth = self.vi.get_color_depth();

        // Validate parameters
        if origin == 0 || width == 0 || height == 0 || width > 640 || height > 480 {
            return None;
        }

        // Calculate bytes per pixel based on color depth
        let bytes_per_pixel = match color_depth {
            2 => 2, // 16-bit RGBA5551
            3 => 4, // 32-bit RGBA8888
            _ => return None,
        };

        let framebuffer_size = width * height * bytes_per_pixel;

        // Bounds check against RDRAM
        if origin + framebuffer_size > self.rdram.len() {
            return None;
        }

        // Create frame with the framebuffer dimensions
        let mut frame = Frame::new(width as u32, height as u32);

        match color_depth {
            2 => {
                // 16-bit RGBA5551 → ARGB8888
                for y in 0..height {
                    for x in 0..width {
                        let offset = origin + (y * width + x) * 2;
                        let pixel =
                            u16::from_be_bytes([self.rdram[offset], self.rdram[offset + 1]]);
                        // N64 16-bit format: RRRRR GGGGG BBBBB A
                        let r = ((pixel >> 11) & 0x1F) as u32;
                        let g = ((pixel >> 6) & 0x1F) as u32;
                        let b = ((pixel >> 1) & 0x1F) as u32;
                        // Convert 5-bit to 8-bit
                        let r8 = (r << 3) | (r >> 2);
                        let g8 = (g << 3) | (g >> 2);
                        let b8 = (b << 3) | (b >> 2);
                        let argb = 0xFF000000 | (r8 << 16) | (g8 << 8) | b8;
                        frame.pixels[y * width + x] = argb;
                    }
                }
            }
            3 => {
                // 32-bit RGBA8888 → ARGB8888
                for y in 0..height {
                    for x in 0..width {
                        let offset = origin + (y * width + x) * 4;
                        let r = self.rdram[offset] as u32;
                        let g = self.rdram[offset + 1] as u32;
                        let b = self.rdram[offset + 2] as u32;
                        let _a = self.rdram[offset + 3] as u32;
                        let argb = 0xFF000000 | (r << 16) | (g << 8) | b;
                        frame.pixels[y * width + x] = argb;
                    }
                }
            }
            _ => return None,
        }

        Some(frame)
    }

    /// Get immutable reference to RDRAM (for direct framebuffer access)
    #[allow(dead_code)] // Reserved for future use
    pub fn rdram(&self) -> &[u8] {
        &self.rdram
    }

    #[inline(always)]
    fn translate_address(&self, addr: u32) -> u32 {
        // Fast path for KSEG0 (0x80000000-0x9FFFFFFF) and KSEG1 (0xA0000000-0xBFFFFFFF)
        // These are direct-mapped segments that bypass TLB entirely.
        // Most N64 code runs in KSEG0/KSEG1, so this is the hot path.
        if (0x8000_0000..0xC000_0000).contains(&addr) {
            return addr & 0x1FFF_FFFF;
        }

        // KUSEG (0x00000000-0x7FFFFFFF) and KSSEG/KSEG3 use TLB
        let virt_addr = addr as u64;
        match self.tlb.translate(virt_addr) {
            Some((phys_addr, _is_cached)) => phys_addr,
            None => {
                // TLB miss fallback
                addr & 0x1FFFFFFF
            }
        }
    }

    /// Get mutable reference to TLB for CP0 TLB instructions
    #[allow(dead_code)] // Reserved for future CP0 TLB instruction implementation
    pub fn tlb_mut(&mut self) -> &mut Tlb {
        &mut self.tlb
    }
}

impl MemoryMips for N64Bus {
    fn read_byte(&self, addr: u32) -> u8 {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM (0x00000000 - 0x003FFFFF)
            0x0000_0000..=0x003F_FFFF => self.rdram[(phys_addr & 0x003FFFFF) as usize],
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_dmem(offset)
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_imem(offset)
            }
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF, cartridge domain 2)
            0x0800_0000..=0x0FFF_FFFF => {
                let offset = (phys_addr - 0x0800_0000) as usize;
                if self.save_is_flashram {
                    self.flashram_read_byte(offset)
                } else if let Some(ref sram) = self.cart_save {
                    *sram.get(offset).unwrap_or(&0xFF)
                } else {
                    0xFF
                }
            }
            // PIF RAM (0x1FC00000 - 0x1FC007FF)
            0x1FC0_0000..=0x1FC0_07FF => {
                let offset = phys_addr & 0x7FF;
                self.pif.read_ram(offset)
            }
            // Cartridge ROM (0x10000000 - 0x1FBFFFFF)
            0x1000_0000..=0x1FBF_FFFF => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(phys_addr - 0x1000_0000)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        let phys_addr = self.translate_address(addr);
        // Fast path for RDRAM (vast majority of halfword reads)
        if phys_addr <= 0x003F_FFFE {
            let offset = (phys_addr & 0x003FFFFF) as usize;
            return u16::from_be_bytes([self.rdram[offset], self.rdram[offset + 1]]);
        }
        let b0 = self.read_byte(addr);
        let b1 = self.read_byte(addr + 1);
        u16::from_be_bytes([b0, b1])
    }

    fn read_word(&self, addr: u32) -> u32 {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                let offset = (phys_addr & 0x003FFFFF) as usize;
                if let Some(bytes) = self.rdram.get(offset..offset + 4) {
                    u32::from_be_bytes(bytes.try_into().unwrap_or([0; 4]))
                } else {
                    0
                }
            }
            // RSP registers (0x04040000 - 0x0404001F)
            0x0404_0000..=0x0404_001F => {
                let offset = phys_addr & 0x1F;
                self.rsp.read_register(offset)
            }
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.read_register(offset)
            }
            // MI registers (0x04300000 - 0x0430000F)
            0x0430_0000..=0x0430_000F => {
                let offset = phys_addr & 0x0F;
                self.mi.read_register(offset)
            }
            // VI registers (0x04400000 - 0x04400037)
            0x0440_0000..=0x0440_0037 => {
                let offset = phys_addr & 0x3F;
                self.vi.read_register(offset)
            }
            // AI registers (0x04500000 - 0x04500017)
            0x0450_0000..=0x0450_0017 => {
                let offset = phys_addr & 0x1F;
                self.ai.read_register(offset)
            }
            // PI registers (0x04600000 - 0x046FFFFF)
            // Peripheral Interface - handles DMA between ROM and RDRAM
            0x0460_0000..=0x046F_FFFF => {
                let offset = phys_addr & 0xFF;
                match offset {
                    0x00 => self.pi_dram_addr, // PI_DRAM_ADDR - DRAM address for DMA
                    0x04 => self.pi_cart_addr, // PI_CART_ADDR - Cart address for DMA
                    0x08 => 0,                 // PI_RD_LEN - Read DMA length (returns 0 when idle)
                    0x0C => 0,                 // PI_WR_LEN - Write DMA length (returns 0 when idle)
                    0x10 => 0x00,              // PI_STATUS - 0 means ready (no DMA in progress)
                    0x14 => 0xFF,              // PI_BSD_DOM1_LAT - Domain 1 latency
                    0x18 => 0xFF,              // PI_BSD_DOM1_PWD - Domain 1 pulse width
                    0x1C => 0x0F,              // PI_BSD_DOM1_PGS - Domain 1 page size
                    0x20 => 0x03,              // PI_BSD_DOM1_RLS - Domain 1 release
                    0x24 => 0xFF,              // PI_BSD_DOM2_LAT - Domain 2 latency
                    0x28 => 0xFF,              // PI_BSD_DOM2_PWD - Domain 2 pulse width
                    0x2C => 0x0F,              // PI_BSD_DOM2_PGS - Domain 2 page size
                    0x30 => 0x03,              // PI_BSD_DOM2_RLS - Domain 2 release
                    _ => 0,
                }
            }
            // RI registers (0x04700000 - 0x047FFFFF)
            // RDRAM Interface - configures RDRAM timing and parameters
            0x0470_0000..=0x047F_FFFF => {
                let offset = phys_addr & 0xFF;
                match offset {
                    0x00 => 0x0E,    // RI_MODE - Operating mode
                    0x04 => 0x40,    // RI_CONFIG - Current config
                    0x08 => 0x14,    // RI_CURRENT_LOAD
                    0x0C => 0x00,    // RI_SELECT - Bank select
                    0x10 => 0x63634, // RI_REFRESH - Refresh rate
                    0x14 => 0x00,    // RI_LATENCY
                    0x18 => 0x00,    // RI_RERROR - Read error
                    0x1C => 0x00,    // RI_WERROR - Write error
                    _ => 0,
                }
            }
            // SI registers (0x04800000 - 0x048FFFFF)
            // Serial Interface - PIF/controller communication
            0x0480_0000..=0x048F_FFFF => {
                let offset = phys_addr & 0x1F;
                match offset {
                    0x00 => 0,    // SI_DRAM_ADDR
                    0x04 => 0,    // SI_PIF_ADDR_RD64B
                    0x10 => 0,    // SI_PIF_ADDR_WR64B
                    0x18 => 0x00, // SI_STATUS - 0 means ready
                    _ => 0,
                }
            }
            // RDRAM config registers (0x03F00000 - 0x03FFFFFF)
            0x03F0_0000..=0x03FF_FFFF => {
                let offset = phys_addr & 0x3F;
                match offset {
                    0x00 => 0x0101_0101, // RDRAM_CONFIG - RDRAM present
                    0x04 => 0x0080_0000, // RDRAM_DEVICE_ID
                    0x08 => 0x0000_0000, // RDRAM_DELAY
                    0x0C => 0x0000_0000, // RDRAM_MODE
                    0x10 => 0x0000_0000, // RDRAM_REF_INTERVAL
                    0x14 => 0x0000_0000, // RDRAM_REF_ROW
                    0x18 => 0x0000_0000, // RDRAM_RAS_INTERVAL
                    0x1C => 0x0000_0000, // RDRAM_MIN_INTERVAL
                    0x20 => 0x0000_0000, // RDRAM_ADDR_SELECT
                    0x24 => 0x0000_0000, // RDRAM_DEVICE_MANUF
                    _ => 0,
                }
            }
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_dmem_word(offset)
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_imem_word(offset)
            }
            // SP PC register (0x04080000)
            0x0408_0000..=0x0408_0003 => self.rsp.pc,
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF, cartridge domain 2)
            0x0800_0000..=0x0FFF_FFFF => {
                let offset = (phys_addr - 0x0800_0000) as usize;
                if self.save_is_flashram {
                    self.flashram_read_word(offset)
                } else if let Some(ref sram) = self.cart_save {
                    u32::from_be_bytes([
                        *sram.get(offset).unwrap_or(&0xFF),
                        *sram.get(offset + 1).unwrap_or(&0xFF),
                        *sram.get(offset + 2).unwrap_or(&0xFF),
                        *sram.get(offset + 3).unwrap_or(&0xFF),
                    ])
                } else {
                    0xFFFF_FFFF
                }
            }
            // Cartridge ROM
            0x1000_0000..=0x1FBF_FFFF => {
                if let Some(ref cart) = self.cartridge {
                    let offset = phys_addr - 0x1000_0000;
                    let src = cart.read_slice(offset, 4);
                    match src.len() {
                        4 => u32::from_be_bytes([src[0], src[1], src[2], src[3]]),
                        // Partial read at ROM end: available bytes contribute,
                        // remaining bytes read as 0 (matches Cartridge::read semantics).
                        1..=3 => {
                            let mut buf = [0u8; 4];
                            buf[..src.len()].copy_from_slice(src);
                            u32::from_be_bytes(buf)
                        }
                        _ => 0,
                    }
                } else {
                    0
                }
            }
            _ => {
                let b0 = self.read_byte(addr);
                let b1 = self.read_byte(addr + 1);
                let b2 = self.read_byte(addr + 2);
                let b3 = self.read_byte(addr + 3);
                u32::from_be_bytes([b0, b1, b2, b3])
            }
        }
    }

    fn read_doubleword(&self, addr: u32) -> u64 {
        let phys_addr = self.translate_address(addr);
        // Fast path for RDRAM – avoids two translate_address calls and two
        // match dispatches for the most common case.
        if phys_addr <= 0x003F_FFF8 {
            let offset = (phys_addr & 0x003F_FFFF) as usize;
            let hi = u32::from_be_bytes(self.rdram[offset..offset + 4].try_into().unwrap_or([0; 4]))
                as u64;
            let lo = u32::from_be_bytes(
                self.rdram[offset + 4..offset + 8]
                    .try_into()
                    .unwrap_or([0; 4]),
            ) as u64;
            return (hi << 32) | lo;
        }
        let hi = self.read_word(addr) as u64;
        let lo = self.read_word(addr + 4) as u64;
        (hi << 32) | lo
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                self.rdram[(phys_addr & 0x003FFFFF) as usize] = val;
            }
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_dmem(offset, val);
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_imem(offset, val);
            }
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF)
            0x0800_0000..=0x0FFF_FFFF => {
                let offset = (phys_addr - 0x0800_0000) as usize;
                if self.save_is_flashram {
                    // Single-byte writes to FlashRAM:
                    // - At offset 0, the byte is a command byte; forward it as a word
                    //   with the byte in the MSB position (hardware command encoding).
                    // - At other offsets in Write mode, store the single byte at the
                    //   advancing write pointer (do NOT zero-pad to a word, which would
                    //   corrupt adjacent bytes).
                    // - In all other modes, single-byte writes outside offset 0 are ignored.
                    if offset == 0 {
                        let word = (val as u32) << 24;
                        self.flashram_write_word(0, word);
                    } else if self.flashram_mode == FlashRamMode::Write {
                        if let Some(ref mut flash) = self.cart_save {
                            let dst = self.flashram_write_offset;
                            if dst < flash.len() {
                                flash[dst] = val;
                            }
                            self.flashram_write_offset = self.flashram_write_offset.wrapping_add(1);
                        }
                    }
                } else if let Some(ref mut sram) = self.cart_save {
                    if offset < sram.len() {
                        sram[offset] = val;
                    }
                }
            }
            // PIF RAM
            0x1FC0_0000..=0x1FC0_07FF => {
                let offset = phys_addr & 0x7FF;
                self.pif.write_ram(offset, val);
            }
            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        let phys_addr = self.translate_address(addr);
        // Fast path for RDRAM
        if phys_addr <= 0x003F_FFFE {
            let offset = (phys_addr & 0x003FFFFF) as usize;
            let bytes = val.to_be_bytes();
            self.rdram[offset] = bytes[0];
            self.rdram[offset + 1] = bytes[1];
            return;
        }
        let bytes = val.to_be_bytes();
        self.write_byte(addr, bytes[0]);
        self.write_byte(addr + 1, bytes[1]);
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                let offset = (phys_addr & 0x003FFFFF) as usize;
                if let Some(dest) = self.rdram.get_mut(offset..offset + 4) {
                    dest.copy_from_slice(&val.to_be_bytes());
                }
            }
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_dmem_word(offset, val);
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_imem_word(offset, val);
            }
            // SP PC register (0x04080000)
            0x0408_0000..=0x0408_0003 => {
                self.rsp.pc = val & 0xFFF;
            }
            // RSP registers (0x04040000 - 0x0404001F)
            0x0404_0000..=0x0404_001F => {
                let offset = phys_addr & 0x1F;

                self.rsp.write_register(offset, val, &mut self.rdram);

                // Handle SP_STATUS write side effects
                if offset == 0x10 {
                    // Bit 3: Clear SP interrupt in MI
                    if val & 0x0008 != 0 {
                        self.mi.clear_interrupt(super::mi::MI_INTR_SP);
                    }
                    // Bit 4: Set SP interrupt in MI
                    if val & 0x0010 != 0 {
                        self.mi.set_interrupt(super::mi::MI_INTR_SP);
                    }
                    // Check if RSP was un-halted and execute pending task
                    self.process_rsp_task();
                }
            }
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.write_register(offset, val);

                // If DPC_END was written (offset 0x04), process the display list
                if offset == 0x04 {
                    self.process_rdp_display_list();
                }
            }
            // MI registers (0x04300000 - 0x0430000F)
            0x0430_0000..=0x0430_000F => {
                let offset = phys_addr & 0x0F;
                self.mi.write_register(offset, val);
            }
            // VI registers (0x04400000 - 0x04400037)
            0x0440_0000..=0x0440_0037 => {
                let offset = phys_addr & 0x3F;

                self.vi.write_register(offset, val);

                // Writing to VI_CURRENT (0x10) acknowledges VI interrupt
                if offset == 0x10 {
                    self.mi.clear_interrupt(super::mi::MI_INTR_VI);
                }
            }
            // AI registers (0x04500000 - 0x04500017)
            0x0450_0000..=0x0450_0017 => {
                let offset = phys_addr & 0x1F;
                self.ai.write_register(offset, val, &self.rdram);

                // Writing to AI_STATUS (0x0C) clears AI interrupt
                if offset == 0x0C {
                    self.mi.clear_interrupt(crate::mi::MI_INTR_AI);
                }

                // Check if AI interrupt is pending (from DMA completion)
                if self.ai.is_interrupt_pending() {
                    self.mi.set_interrupt(crate::mi::MI_INTR_AI);
                    log(LogCategory::Interrupts, LogLevel::Info, || {
                        "N64 Bus: AI interrupt triggered".to_string()
                    });
                }
            }
            // PI registers (0x04600000 - 0x046FFFFF)
            // Peripheral Interface - handles DMA between ROM and RDRAM
            0x0460_0000..=0x046F_FFFF => {
                let offset = phys_addr & 0xFF;
                match offset {
                    0x00 => {
                        // PI_DRAM_ADDR
                        self.pi_dram_addr = val & 0x00FF_FFFF;
                    }
                    0x04 => {
                        // PI_CART_ADDR
                        self.pi_cart_addr = val;
                    }
                    0x08 => {
                        // PI_RD_LEN - DMA from RDRAM to cart (read)
                        // Length is value + 1
                        let len = (val & 0x00FF_FFFF) + 1;
                        log(LogCategory::Bus, LogLevel::Info, || {
                            format!(
                                "N64 PI: DMA read RDRAM 0x{:08X} -> Cart 0x{:08X}, len=0x{:X}",
                                self.pi_dram_addr, self.pi_cart_addr, len
                            )
                        });
                        // Read DMA: RDRAM -> Cart (not commonly used by games)
                    }
                    0x0C => {
                        // PI_WR_LEN - DMA from cart to RDRAM (write)
                        // Length is value + 1, hardware rounds up to 2-byte alignment.
                        let raw_len = ((val & 0x00FF_FFFF) + 1) as usize;
                        // PI DMA transfers are 2-byte aligned (length rounded up to
                        // next even number). RDRAM destination must also be 2-byte
                        // aligned (hardware masks bit 0). Source (cart) alignment
                        // varies but cart reads are byte-addressable for the ROM bus.
                        let len = (raw_len + 1) & !1; // round up to even
                        let dram_addr = (self.pi_dram_addr & !1) as usize;
                        let cart_addr = self.pi_cart_addr;

                        log(LogCategory::Bus, LogLevel::Info, || {
                            format!(
                                "N64 PI: DMA write Cart 0x{:08X} -> RDRAM 0x{:08X}, len=0x{:X}",
                                cart_addr, dram_addr, len
                            )
                        });

                        // Perform the DMA: copy from cartridge ROM to RDRAM
                        if let Some(ref cart) = self.cartridge {
                            let cart_offset = if cart_addr >= 0x1000_0000 {
                                cart_addr - 0x1000_0000
                            } else {
                                cart_addr
                            };
                            let src = cart.read_slice(cart_offset, len);
                            let rdram_end = self.rdram.len();
                            // Bulk-copy available ROM bytes
                            let copy_len = src.len().min(rdram_end.saturating_sub(dram_addr));
                            if copy_len > 0 {
                                self.rdram[dram_addr..dram_addr + copy_len]
                                    .copy_from_slice(&src[..copy_len]);
                            }
                            // Zero-fill trailing bytes when DMA length exceeds ROM
                            // (matches original per-byte loop where cart.read() returns 0
                            // for out-of-range offsets).
                            let fill_end = (dram_addr + len).min(rdram_end);
                            let fill_start = dram_addr + copy_len;
                            if fill_start < fill_end {
                                self.rdram[fill_start..fill_end].fill(0);
                            }
                        }

                        // Trigger PI interrupt when DMA completes
                        self.mi.set_interrupt(crate::mi::MI_INTR_PI);
                    }
                    0x10 => {
                        // PI_STATUS - writing bit 1 clears PI interrupt
                        if val & 0x02 != 0 {
                            self.mi.clear_interrupt(crate::mi::MI_INTR_PI);
                        }
                    }
                    _ => {
                        // BSD domain timing registers - accept but ignore
                    }
                }
            }
            // RI registers (0x04700000 - 0x047FFFFF)
            // RDRAM Interface - accept writes silently
            0x0470_0000..=0x047F_FFFF => {
                // RI register writes are accepted but ignored for now
            }
            // SI registers (0x04800000 - 0x048FFFFF)
            // Serial Interface - PIF/controller communication
            0x0480_0000..=0x048F_FFFF => {
                let offset = phys_addr & 0x1F;
                match offset {
                    0x00 => {
                        // SI_DRAM_ADDR
                        self.si_dram_addr = val & 0x00FF_FFFF;
                    }
                    0x04 => {
                        // SI_PIF_ADDR_RD64B - DMA: PIF -> RDRAM
                        // Copies 64 bytes of PIF RAM (0x1FC007C0-0x1FC007FF) to RDRAM
                        let dram_addr = self.si_dram_addr as usize;
                        // PIF RAM is 64 bytes at offset 0x7C0 in PIF address space
                        const PIF_RAM_OFFSET: u32 = 0x7C0;
                        for i in 0..64 {
                            if dram_addr + i < self.rdram.len() {
                                self.rdram[dram_addr + i] =
                                    self.pif.read_ram(PIF_RAM_OFFSET + i as u32);
                            }
                        }
                        // Trigger SI interrupt
                        self.mi.set_interrupt(crate::mi::MI_INTR_SI);
                    }
                    0x10 => {
                        // SI_PIF_ADDR_WR64B - DMA: RDRAM -> PIF
                        // Copies 64 bytes from RDRAM to PIF RAM (0x1FC007C0-0x1FC007FF)
                        let dram_addr = self.si_dram_addr as usize;
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!("N64 SI: DMA RDRAM 0x{:08X} -> PIF", dram_addr)
                        });
                        // PIF RAM is 64 bytes at offset 0x7C0 in PIF address space
                        const PIF_RAM_OFFSET: u32 = 0x7C0;
                        for i in 0..64 {
                            if dram_addr + i < self.rdram.len() {
                                self.pif.write_ram(
                                    PIF_RAM_OFFSET + i as u32,
                                    self.rdram[dram_addr + i],
                                );
                            }
                        }
                        // Process PIF commands (controller/EEPROM)
                        self.pif.process_commands();
                        // Trigger SI interrupt
                        self.mi.set_interrupt(crate::mi::MI_INTR_SI);
                    }
                    0x18 => {
                        // SI_STATUS - writing clears SI interrupt
                        self.mi.clear_interrupt(crate::mi::MI_INTR_SI);
                    }
                    _ => {}
                }
            }
            // RDRAM config registers (0x03F00000 - 0x03FFFFFF)
            0x03F0_0000..=0x03FF_FFFF => {
                // RDRAM config writes accepted but ignored
            }
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF)
            0x0800_0000..=0x0FFF_FFFF => {
                let offset = (phys_addr - 0x0800_0000) as usize;
                if self.save_is_flashram {
                    self.flashram_write_word(offset, val);
                } else if let Some(ref mut sram) = self.cart_save {
                    if offset + 3 < sram.len() {
                        let bytes = val.to_be_bytes();
                        sram[offset] = bytes[0];
                        sram[offset + 1] = bytes[1];
                        sram[offset + 2] = bytes[2];
                        sram[offset + 3] = bytes[3];
                    }
                }
            }
            _ => {
                let bytes = val.to_be_bytes();
                self.write_byte(addr, bytes[0]);
                self.write_byte(addr + 1, bytes[1]);
                self.write_byte(addr + 2, bytes[2]);
                self.write_byte(addr + 3, bytes[3]);
            }
        }
    }

    fn write_doubleword(&mut self, addr: u32, val: u64) {
        let hi = (val >> 32) as u32;
        let lo = val as u32;
        self.write_word(addr, hi);
        self.write_word(addr + 4, lo);
    }

    // TLB operations for CP0 instructions

    fn tlb_write_indexed(&mut self, index: usize, entry: emu_core::cpu_mips_r4300i::TlbEntryData) {
        // Convert TlbEntryData to TlbEntry
        let tlb_entry = crate::tlb::TlbEntry {
            vpn2: entry.vpn2,
            asid: entry.asid,
            global: entry.global,
            page_mask: entry.page_mask,
            pfn0: entry.pfn0,
            c0: entry.c0,
            d0: entry.d0,
            v0: entry.v0,
            pfn1: entry.pfn1,
            c1: entry.c1,
            d1: entry.d1,
            v1: entry.v1,
        };

        self.tlb.write_entry(index, tlb_entry);

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Write entry at index {} - VPN2=0x{:07X}, ASID=0x{:02X}",
                index, entry.vpn2, entry.asid
            )
        });
    }

    fn tlb_write_random(&mut self, index: usize, entry: emu_core::cpu_mips_r4300i::TlbEntryData) {
        // Use the index from CP0 Random register (passed from CPU)
        // This ensures deterministic behavior for save states and debugging
        self.tlb_write_indexed(index, entry);

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Write entry at random index {} - VPN2=0x{:07X}, ASID=0x{:02X}",
                index, entry.vpn2, entry.asid
            )
        });
    }

    fn tlb_read_indexed(&self, index: usize) -> Option<emu_core::cpu_mips_r4300i::TlbEntryData> {
        self.tlb
            .read_entry(index)
            .map(|entry| emu_core::cpu_mips_r4300i::TlbEntryData {
                vpn2: entry.vpn2,
                asid: entry.asid,
                global: entry.global,
                page_mask: entry.page_mask,
                pfn0: entry.pfn0,
                c0: entry.c0,
                d0: entry.d0,
                v0: entry.v0,
                pfn1: entry.pfn1,
                c1: entry.c1,
                d1: entry.d1,
                v1: entry.v1,
            })
    }

    fn tlb_probe(&self, vpn2: u64, asid: u8) -> Option<usize> {
        // Probe TLB for matching entry
        // VPN2 is bits 39-13 of the virtual address
        // We need to check each entry manually since we can't modify self
        for (i, entry) in self.tlb.entries.iter().enumerate() {
            let mask = (entry.page_mask as u64) << 12;
            let vpn_mask = !mask;
            if (entry.vpn2 & vpn_mask) == (vpn2 & vpn_mask) && (entry.global || entry.asid == asid)
            {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "N64 TLB: Probe found match at index {} for VPN2=0x{:07X}, ASID=0x{:02X}",
                        i, vpn2, asid
                    )
                });
                return Some(i);
            }
        }

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Probe found no match for VPN2=0x{:07X}, ASID=0x{:02X}",
                vpn2, asid
            )
        });

        None
    }
}

#[cfg(test)]
mod tests {
    /// PI register readback is validated through N64Bus::read_word / write_word.
    /// However, constructing N64Bus requires a real OpenGL context (for the RDP
    /// renderer), so bus-level integration tests must run in a windowed environment.
    /// The current RDRAM / PI state is exercised by the RSP HLE and RDP unit tests
    /// in their respective modules (which mock the bus with plain byte arrays).
    #[test]
    fn test_pi_register_fields_exist() {
        // Compile-time check: ensure pi_dram_addr and pi_cart_addr fields exist
        // on N64Bus (used in read_word / write_word).  This prevents regressions
        // in the field definitions without needing a full OpenGL context.
        let _: fn(&super::N64Bus) -> u32 = |bus| bus.pi_dram_addr;
        let _: fn(&super::N64Bus) -> u32 = |bus| bus.pi_cart_addr;
    }
}
