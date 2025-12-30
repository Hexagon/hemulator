//! PC memory bus implementation
//!
//! This module implements the memory bus for an IBM PC/XT-compatible system.
//! Memory layout:
//! - 0x00000-0x9FFFF: Conventional memory (640KB)
//! - 0xA0000-0xBFFFF: Video memory (128KB)
//! - 0xC0000-0xFFFFF: ROM area (256KB)
//! - 0xF0000-0xFFFFF: BIOS ROM (64KB)

use crate::bios::BootPriority;
use crate::disk::DiskController;
use crate::dpmi::DpmiDriver;
use crate::keyboard::Keyboard;
use crate::mouse::Mouse;
use crate::pit::Pit;
use crate::xms::XmsDriver;
use emu_core::cpu_8086::Memory8086;
use std::cell::Cell;

/// Video adapter type for equipment configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoAdapterType {
    /// No video adapter
    None,
    /// Monochrome Display Adapter (MDA)
    Mda,
    /// Color Graphics Adapter (CGA)
    Cga,
    /// Enhanced Graphics Adapter (EGA)
    Ega,
    /// Video Graphics Array (VGA)
    Vga,
}

/// PC memory bus
pub struct PcBus {
    /// Main RAM (640KB)
    ram: Vec<u8>,
    /// Extended RAM (above 1MB) - for systems with >640KB total memory
    extended_ram: Vec<u8>,
    /// Video RAM (128KB)
    vram: Vec<u8>,
    /// ROM area (256KB) - includes BIOS
    rom: Vec<u8>,
    /// Loaded executable data (deprecated, kept for backward compatibility)
    executable: Option<Vec<u8>>,
    /// Keyboard controller
    pub keyboard: Keyboard,
    /// Floppy A disk image
    floppy_a: Option<Vec<u8>>,
    /// Floppy B disk image
    floppy_b: Option<Vec<u8>>,
    /// Hard drive image
    hard_drive: Option<Vec<u8>>,
    /// Disk controller
    disk_controller: DiskController,
    /// Boot priority order
    boot_priority: BootPriority,
    /// Flag to track if boot sector has been loaded
    boot_sector_loaded: bool,
    /// Programmable Interval Timer (8253/8254)
    pub pit: Pit,
    /// PC speaker gate (bit 0 of port 0x61)
    speaker_gate: bool,
    /// Microsoft Mouse Driver
    pub mouse: Mouse,
    /// XMS (Extended Memory Specification) driver
    pub xms: XmsDriver,
    /// DPMI (DOS Protected Mode Interface) driver
    pub dpmi: DpmiDriver,
    /// Video adapter type for equipment configuration
    video_adapter_type: VideoAdapterType,
    /// Keyboard controller command register (for A20 gate control)
    kb_controller_command: u8,
    /// Keyboard controller output port (bit 1 = A20 gate)
    kb_controller_output_port: u8,
    /// Keyboard controller input buffer full flag (Cell for interior mutability)
    kb_input_buffer_full: Cell<bool>,
    /// Keyboard controller last write was command (true) or data (false)
    kb_last_was_command: Cell<bool>,
    /// VGA status register state (Cell for interior mutability during io_read)
    /// Bit 0: Display enable (0 = display, 1 = retrace/blanking)
    /// Bit 3: Vertical retrace (0 = no retrace, 1 = vertical retrace)
    vga_status: Cell<u8>,
    /// Cycle counter for VGA status timing (Cell for interior mutability)
    vga_status_cycles: Cell<u64>,
}

impl PcBus {
    /// Create a new PC bus with default 640KB memory
    pub fn new() -> Self {
        Self::with_memory_kb(640)
    }

    /// Create a new PC bus with a specific memory size in KB
    ///
    /// The memory_kb parameter specifies total system memory:
    /// - If memory_kb <= 640: All memory is conventional (640KB max)
    /// - If memory_kb > 640: 640KB conventional + rest as extended memory
    ///
    /// Valid range: 256KB minimum, no maximum (extended memory can be very large)
    pub fn with_memory_kb(kb: u32) -> Self {
        // Conventional memory is clamped to 256-640KB range
        let conventional_kb = kb.clamp(256, 640);
        let ram_size = (conventional_kb as usize) * 1024;

        // Extended memory is any memory beyond 640KB
        // Real PCs have extended memory starting at 1MB, but we calculate it from the total
        let extended_kb = kb.saturating_sub(640);
        let extended_ram_size = (extended_kb as usize) * 1024;

        let mut pit = Pit::new();
        pit.reset(); // Initialize with default system timer

        // Initialize XMS with calculated extended memory
        let mut xms = XmsDriver::new(extended_kb);
        xms.install();
        xms.init_umbs(); // Initialize Upper Memory Blocks

        // Initialize DPMI driver
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let mut bus = Self {
            ram: vec![0; ram_size],
            extended_ram: vec![0xFF; extended_ram_size], // Initialize with 0xFF to distinguish from low RAM
            vram: vec![0; 0x20000],                      // 128KB
            rom: vec![0; 0x40000],                       // 256KB
            executable: None,
            keyboard: Keyboard::new(),
            floppy_a: None,
            floppy_b: None,
            hard_drive: None,
            disk_controller: DiskController::new(),
            boot_priority: BootPriority::default(),
            boot_sector_loaded: false,
            pit,
            speaker_gate: false,
            mouse: Mouse::new(),
            xms,
            dpmi,
            video_adapter_type: VideoAdapterType::Cga, // Default to CGA
            kb_controller_command: 0,
            kb_controller_output_port: 0x02, // A20 enabled by default (bit 1 set)
            kb_input_buffer_full: Cell::new(false), // Input buffer starts empty
            kb_last_was_command: Cell::new(false), // No command yet
            vga_status: Cell::new(0x00),     // Start with display active (not in retrace)
            vga_status_cycles: Cell::new(0),
        };

        // Initialize Interrupt Vector Table (IVT) in low RAM
        // The IVT is at 0x0000:0x0000 and contains 256 interrupt vectors
        // Each vector is 4 bytes: offset (2 bytes) + segment (2 bytes)
        // For now, point all vectors to a simple IRET handler in BIOS
        // This prevents crashes when interrupts are triggered

        // Set INT 0 (divide error) to F000:0050 (BIOS IRET handler)
        // Note: x86 is little-endian, so low byte first
        bus.ram[0x0000] = 0x50; // offset low byte
        bus.ram[0x0001] = 0x00; // offset high byte
        bus.ram[0x0002] = 0x00; // segment low byte
        bus.ram[0x0003] = 0xF0; // segment high byte (F000)

        bus
    }

    /// Get the total system memory in KB (conventional + extended)
    pub fn memory_kb(&self) -> u32 {
        let conventional_kb = (self.ram.len() / 1024) as u32;
        let extended_kb = self.xms.total_extended_memory_kb();
        conventional_kb + extended_kb
    }

    /// Get the size of conventional memory in KB (max 640KB)
    pub fn conventional_memory_kb(&self) -> u32 {
        (self.ram.len() / 1024) as u32
    }

    /// Get the size of extended memory in KB (above 1MB)
    #[allow(dead_code)] // Public API for external use
    pub fn extended_memory_kb(&self) -> u32 {
        self.xms.total_extended_memory_kb()
    }

    /// Set the video adapter type for equipment configuration
    pub fn set_video_adapter_type(&mut self, adapter_type: VideoAdapterType) {
        self.video_adapter_type = adapter_type;
    }

    /// Get the video adapter type
    pub fn video_adapter_type(&self) -> VideoAdapterType {
        self.video_adapter_type
    }

    /// Update VGA status register based on elapsed cycles
    ///
    /// This simulates the vertical retrace timing. At 60 Hz, a frame is ~16.67ms.
    /// Assuming 4.77 MHz (original PC), that's about 79,583 cycles per frame.
    /// We'll simulate vertical retrace for about 5% of the frame time.
    pub fn update_vga_status(&self, cycles: u64) {
        let current_cycles = self.vga_status_cycles.get() + cycles;
        self.vga_status_cycles.set(current_cycles);

        // Cycles per frame at various speeds (at 60 Hz):
        // 4.77 MHz (PC/XT): 79,583 cycles/frame
        // We'll use a generic approach: retrace happens for ~5% of frame
        // Frame period: ~80,000 cycles (approximate for 4.77 MHz)
        const CYCLES_PER_FRAME: u64 = 80000;
        const RETRACE_CYCLES: u64 = CYCLES_PER_FRAME / 20; // 5% of frame

        let frame_position = current_cycles % CYCLES_PER_FRAME;

        if frame_position < RETRACE_CYCLES {
            // In vertical retrace (bit 3 set, bit 0 set for blanking)
            self.vga_status.set(0x09); // Bits 0 and 3 set
        } else {
            // Not in retrace (bits clear for active display)
            self.vga_status.set(0x00);
        }
    }

    /// Get the number of floppy drives installed
    pub fn floppy_count(&self) -> u8 {
        let mut count = 0;
        if self.floppy_a.is_some() {
            count += 1;
        }
        if self.floppy_b.is_some() {
            count += 1;
        }
        count
    }

    /// Check if a hard drive is installed
    #[allow(dead_code)] // Part of public API, may be used in the future
    pub fn has_hard_drive(&self) -> bool {
        self.hard_drive.is_some()
    }

    /// Reset the bus to initial state
    pub fn reset(&mut self) {
        // Clear RAM but preserve ROM and executable
        self.ram.fill(0);
        self.vram.fill(0);
        self.keyboard.clear();
        self.disk_controller.reset();
        self.pit.reset();
        self.speaker_gate = false;
        self.mouse = Mouse::new(); // Reset mouse state
                                   // XMS driver state is preserved across resets (like hardware)
        self.boot_sector_loaded = false;
        // Reset VGA status
        self.vga_status.set(0x00);
        self.vga_status_cycles.set(0);
    }

    /// Set boot priority
    pub fn set_boot_priority(&mut self, priority: BootPriority) {
        self.boot_priority = priority;
    }

    /// Get boot priority
    pub fn boot_priority(&self) -> BootPriority {
        self.boot_priority
    }

    /// Load boot sector from the appropriate disk based on boot priority
    ///
    /// This method attempts to load the boot sector (sector 0, 512 bytes) from
    /// the configured boot disk to memory address 0x7C00. It verifies the boot
    /// signature (0xAA55) at the end of the sector.
    ///
    /// Returns: true if boot sector was loaded successfully, false otherwise
    pub fn load_boot_sector(&mut self) -> bool {
        // Prevent loading boot sector multiple times
        if self.boot_sector_loaded {
            return true;
        }

        // Determine which disk(s) to try based on boot priority
        let boot_devices: Vec<(u8, Option<&[u8]>)> = match self.boot_priority {
            BootPriority::FloppyFirst => vec![
                (0x00, self.floppy_a.as_deref()),
                (0x80, self.hard_drive.as_deref()),
            ],
            BootPriority::HardDriveFirst => vec![
                (0x80, self.hard_drive.as_deref()),
                (0x00, self.floppy_a.as_deref()),
            ],
            BootPriority::FloppyOnly => vec![(0x00, self.floppy_a.as_deref())],
            BootPriority::HardDriveOnly => vec![(0x80, self.hard_drive.as_deref())],
        };

        // Try each device in order
        for (drive, disk_image) in boot_devices {
            if let Some(image) = disk_image {
                // Check if disk image is large enough for boot sector
                if image.len() < 512 {
                    continue;
                }

                // Read boot sector (first 512 bytes)
                let boot_sector = &image[0..512];

                // Check for boot signature 0xAA55 at offset 510-511
                if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
                    println!("Boot sector on drive 0x{:02X} has invalid signature", drive);
                    continue;
                }

                // Load boot sector to 0x0000:0x7C00 (physical address 0x7C00)
                self.ram[0x7C00..0x7C00 + 512].copy_from_slice(boot_sector);

                // Debug: Check boot sector signature and first few bytes
                eprintln!(
                    "Boot sector loaded: signature={:02X}{:02X}, OEM={}",
                    self.ram[0x7C00 + 510],
                    self.ram[0x7C00 + 511],
                    String::from_utf8_lossy(&self.ram[0x7C00 + 3..0x7C00 + 11])
                );

                self.boot_sector_loaded = true;
                println!("Loaded boot sector from drive 0x{:02X}", drive);
                return true;
            }
        }

        println!("No bootable disk found");
        false
    }

    /// Load an executable at a specific address
    #[allow(dead_code)]
    pub fn load_executable(&mut self, data: Vec<u8>) {
        self.executable = Some(data);
    }

    /// Load BIOS ROM
    pub fn load_bios(&mut self, data: &[u8]) {
        // BIOS is typically loaded at 0xF0000-0xFFFFF (last 64KB of ROM area)
        let bios_offset = 0x30000; // Offset within rom array (0x40000 - 0x10000)
        let len = data.len().min(0x10000);
        self.rom[bios_offset..bios_offset + len].copy_from_slice(&data[..len]);
    }

    /// Get a reference to the executable data
    #[allow(dead_code)]
    pub fn executable(&self) -> Option<&[u8]> {
        self.executable.as_deref()
    }

    /// Get a reference to the video RAM (for rendering)
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    /// Get a mutable reference to the video RAM (for BIOS initialization)
    pub fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }

    /// Read a byte from RAM at the given offset (for testing)
    #[cfg(test)]
    pub fn read_ram(&self, offset: usize) -> u8 {
        if offset < self.ram.len() {
            self.ram[offset]
        } else {
            0xFF
        }
    }

    /// Mount floppy A disk image
    pub fn mount_floppy_a(&mut self, data: Vec<u8>) {
        self.floppy_a = Some(data);
    }

    /// Unmount floppy A
    pub fn unmount_floppy_a(&mut self) {
        self.floppy_a = None;
    }

    /// Get reference to floppy A
    pub fn floppy_a(&self) -> Option<&[u8]> {
        self.floppy_a.as_deref()
    }

    /// Mount floppy B disk image
    pub fn mount_floppy_b(&mut self, data: Vec<u8>) {
        self.floppy_b = Some(data);
    }

    /// Unmount floppy B
    pub fn unmount_floppy_b(&mut self) {
        self.floppy_b = None;
    }

    /// Get reference to floppy B
    pub fn floppy_b(&self) -> Option<&[u8]> {
        self.floppy_b.as_deref()
    }

    /// Mount hard drive image
    pub fn mount_hard_drive(&mut self, data: Vec<u8>) {
        self.hard_drive = Some(data);
    }

    /// Unmount hard drive
    pub fn unmount_hard_drive(&mut self) {
        self.hard_drive = None;
    }

    /// Get reference to hard drive
    pub fn hard_drive(&self) -> Option<&[u8]> {
        self.hard_drive.as_deref()
    }

    /// Get mutable reference to hard drive (for write operations)
    #[allow(dead_code)]
    pub fn hard_drive_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.hard_drive.as_mut()
    }

    /// Get mutable reference to floppy A (for write operations)
    #[allow(dead_code)]
    pub fn floppy_a_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.floppy_a.as_mut()
    }

    /// Get mutable reference to floppy B (for write operations)
    #[allow(dead_code)]
    pub fn floppy_b_mut(&mut self) -> Option<&mut Vec<u8>> {
        self.floppy_b.as_mut()
    }

    /// Get reference to disk controller
    #[allow(dead_code)]
    pub fn disk_controller(&self) -> &DiskController {
        &self.disk_controller
    }

    /// Get mutable reference to disk controller
    #[allow(dead_code)]
    pub fn disk_controller_mut(&mut self) -> &mut DiskController {
        &mut self.disk_controller
    }

    /// Check if a floppy drive has a disk mounted
    pub fn has_floppy(&self, drive: u8) -> bool {
        match drive {
            0 => self.floppy_a.is_some(),
            1 => self.floppy_b.is_some(),
            _ => false,
        }
    }

    /// Perform a disk read operation
    pub fn disk_read(&mut self, request: &crate::disk::DiskRequest, buffer: &mut [u8]) -> u8 {
        let disk_image = if request.drive < 0x80 {
            // Floppy drive
            if request.drive == 0x00 {
                self.floppy_a.as_deref()
            } else if request.drive == 0x01 {
                self.floppy_b.as_deref()
            } else {
                None
            }
        } else {
            // Hard drive
            self.hard_drive.as_deref()
        };

        self.disk_controller
            .read_sectors(request, buffer, disk_image)
    }

    /// Perform a disk write operation
    pub fn disk_write(&mut self, request: &crate::disk::DiskRequest, buffer: &[u8]) -> u8 {
        let disk_mut = if request.drive < 0x80 {
            // Floppy drive
            if request.drive == 0x00 {
                self.floppy_a.as_mut()
            } else if request.drive == 0x01 {
                self.floppy_b.as_mut()
            } else {
                None
            }
        } else {
            // Hard drive
            self.hard_drive.as_mut()
        };

        self.disk_controller
            .write_sectors(request, buffer, disk_mut)
    }

    /// Perform a disk read operation using LBA
    pub fn disk_read_lba(&mut self, drive: u8, lba: u32, count: u8, buffer: &mut [u8]) -> u8 {
        let disk_image = if drive < 0x80 {
            // Floppy drive
            if drive == 0x00 {
                self.floppy_a.as_deref()
            } else if drive == 0x01 {
                self.floppy_b.as_deref()
            } else {
                None
            }
        } else {
            // Hard drive
            self.hard_drive.as_deref()
        };

        self.disk_controller
            .read_sectors_lba(lba, count, buffer, disk_image)
    }

    /// Perform a disk write operation using LBA
    pub fn disk_write_lba(&mut self, drive: u8, lba: u32, count: u8, buffer: &[u8]) -> u8 {
        let disk_mut = if drive < 0x80 {
            // Floppy drive
            if drive == 0x00 {
                self.floppy_a.as_mut()
            } else if drive == 0x01 {
                self.floppy_b.as_mut()
            } else {
                None
            }
        } else {
            // Hard drive
            self.hard_drive.as_mut()
        };

        self.disk_controller
            .write_sectors_lba(lba, count, buffer, disk_mut)
    }

    /// Read from an I/O port
    pub fn io_read(&self, port: u16) -> u8 {
        let value = match port {
            // PIT Channel 0 (system timer)
            0x40 => {
                // Reading would need mutable access to update read state
                // This is a limitation of the trait design
                0x00
            }
            // PIT Channel 1 (DRAM refresh - legacy)
            0x41 => 0x00,
            // PIT Channel 2 (PC speaker)
            0x42 => 0x00,
            // PIT Mode/Command register
            0x43 => {
                // Write-only register
                0xFF
            }
            // Port B (speaker control, etc.)
            0x61 => {
                let mut value = 0x00;
                if self.speaker_gate {
                    value |= 0x01; // Speaker gate enabled
                }
                // Bit 5: PIT channel 2 output
                if self.pit.speaker_output() {
                    value |= 0x20;
                }
                value
            }
            // Port 0x60 - Keyboard controller data port
            0x60 => {
                // When command is 0xD0 (Read Output Port), return output port value
                if self.kb_controller_command == 0xD0 {
                    use emu_core::logging::{LogCategory, LogConfig, LogLevel};
                    if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
                        eprintln!(
                            "KB controller read output port: 0x{:02X}",
                            self.kb_controller_output_port
                        );
                    }
                    self.kb_controller_output_port
                } else {
                    // Normal keyboard data - return last scancode or 0
                    self.keyboard.peek_scancode()
                }
            }
            // Port 0x64 - Keyboard controller status port
            0x64 => {
                // Bit 0: Output buffer full (0 = empty, 1 = full)
                // Bit 1: Input buffer full (0 = empty, 1 = full)
                // Bit 2: System flag (0 = POST, 1 = warm boot)
                // Bit 3: Command/Data (0 = data last written to 60h, 1 = command to 64h)
                // Bit 4: Keyboard unlocked (1 = keyboard enabled)
                // Bit 5: Transmit timeout
                // Bit 6: Receive timeout
                // Bit 7: Parity error
                // Return system flag set (warm boot) + keyboard enabled + input buffer status
                let mut status = 0x14; // Bits 2 (warm boot) and 4 (enabled) set
                if self.kb_input_buffer_full.get() {
                    status |= 0x02; // Set bit 1 if input buffer full
                }
                if self.kb_last_was_command.get() {
                    status |= 0x08; // Set bit 3 if last write was command
                }

                // Debug: Log status reads during HIMEM execution
                use emu_core::logging::{LogCategory, LogConfig, LogLevel};
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Trace) {
                    eprintln!(
                        "KB status read: 0x{:02X} (input_buffer_full={}, last_was_cmd={})",
                        status,
                        self.kb_input_buffer_full.get(),
                        self.kb_last_was_command.get()
                    );
                }

                // Simulate controller processing: clear buffer after one status read
                // This allows software to see buffer full briefly after write
                if self.kb_input_buffer_full.get() {
                    self.kb_input_buffer_full.set(false);
                }

                status
            }
            // Port 0x92 - System Control Port A (PS/2)
            // Bit 0: Alternate hot reset (0 = normal, 1 = reset)
            // Bit 1: A20 gate (0 = disabled, 1 = enabled)
            // Bits 2-3: Reserved
            // Bits 4-7: Manufacturer specific
            0x92 => {
                // Return current A20 state from XMS driver
                if self.xms.is_a20_enabled() {
                    0x02
                } else {
                    0x00
                }
            }
            // Port 0x03BA - MDA/EGA Input Status Register 1 (monochrome)
            // Port 0x03DA - CGA/VGA Input Status Register 1 (color)
            // Bit 0: Display enable (0 = display time, 1 = retrace/blanking)
            // Bit 3: Vertical retrace (0 = no retrace, 1 = vertical retrace active)
            // Reading this port resets the 3C0h index flip-flop to address mode
            0x03BA | 0x03DA => {
                // Return current VGA status
                // Note: Reading this port should also reset the attribute controller
                // flip-flop (port 0x3C0), but we don't implement that yet
                self.vga_status.get()
            }
            _ => 0xFF, // Default for unimplemented ports
        };

        // Log I/O reads for debugging
        use emu_core::logging::{LogCategory, LogConfig, LogLevel};
        if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Trace) {
            eprintln!("I/O read port 0x{:04X} = 0x{:02X}", port, value);
        }

        value
    }

    /// Write to an I/O port
    pub fn io_write(&mut self, port: u16, val: u8) {
        match port {
            // PIT Channel 0 (system timer)
            0x40 => {
                self.pit.write_channel(0, val);
            }
            // PIT Channel 1 (DRAM refresh)
            0x41 => {
                self.pit.write_channel(1, val);
            }
            // PIT Channel 2 (PC speaker)
            0x42 => {
                self.pit.write_channel(2, val);
            }
            // PIT Mode/Command register
            0x43 => {
                self.pit.write_control(val);
            }
            // Port B (speaker control, keyboard acknowledge, etc.)
            0x61 => {
                self.speaker_gate = (val & 0x01) != 0;
                // Bit 1: speaker data (directly drives speaker)
                // We'll use this in combination with PIT channel 2
            }
            // Port 0x60 - Keyboard controller data port
            0x60 => {
                // Log data writes for debugging
                use emu_core::logging::{LogCategory, LogConfig, LogLevel};
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
                    eprintln!(
                        "KB controller data write: 0x{:02X} (command was 0x{:02X})",
                        val, self.kb_controller_command
                    );
                }

                self.kb_last_was_command.set(false); // Data write to port 60h
                self.kb_input_buffer_full.set(true); // Buffer becomes full when data written

                // When command is 0xD1 (Write Output Port), update output port
                if self.kb_controller_command == 0xD1 {
                    self.kb_controller_output_port = val;
                    // Bit 1 controls A20 gate
                    let a20_enabled = (val & 0x02) != 0;
                    self.xms.set_a20_enabled(a20_enabled);
                    if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
                        eprintln!(
                            "A20 gate set to: {}",
                            if a20_enabled { "enabled" } else { "disabled" }
                        );
                    }
                    self.kb_controller_command = 0; // Clear command
                }
            }
            // Port 0x64 - Keyboard controller command port
            0x64 => {
                // Store command for next data port access
                self.kb_controller_command = val;
                self.kb_input_buffer_full.set(true); // Input buffer now full
                self.kb_last_was_command.set(true); // Command write to port 64h

                // Log keyboard controller commands for debugging HIMEM.SYS
                use emu_core::logging::{LogCategory, LogConfig, LogLevel};
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
                    eprintln!("KB controller command: 0x{:02X}", val);
                }

                // Handle immediate commands that don't need data port access
                match val {
                    0xFF => {
                        // Reset keyboard controller
                        // Preserve A20 state during reset (don't force it to a specific value)
                        // Only reset the command register
                        self.kb_controller_command = 0;
                        self.kb_input_buffer_full.set(false); // Reset clears buffer
                        if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug)
                        {
                            let a20_state = if self.xms.is_a20_enabled() {
                                "enabled"
                            } else {
                                "disabled"
                            };
                            eprintln!("KB controller reset - A20 state preserved ({})", a20_state);
                        }
                    }
                    0xD0 => {
                        // Read Output Port - next read from 0x60 returns output port
                        // Command stored, will be handled on port 0x60 read
                        // Input buffer clears immediately after accepting command
                        self.kb_input_buffer_full.set(false);
                    }
                    0xD1 => {
                        // Write Output Port - next write to 0x60 sets output port
                        // Command stored, will be handled on port 0x60 write
                        // Input buffer clears immediately (real hardware clears in microseconds)
                        self.kb_input_buffer_full.set(false);
                    }
                    _ => {
                        // Other commands stored but mostly ignored
                        self.kb_input_buffer_full.set(false);
                    }
                }
            }
            // Port 0x92 - System Control Port A (PS/2)
            // HIMEM.SYS writes to this port to enable/disable A20
            0x92 => {
                // Bit 0: Alternate hot reset
                if val & 0x01 != 0 {
                    // Reset request - ignore in emulator
                }
                // Bit 1: A20 gate control
                let a20_enabled = (val & 0x02) != 0;
                self.xms.set_a20_enabled(a20_enabled);
            }
            _ => {} // Ignore writes to unimplemented ports
        }
    }
}

impl Default for PcBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory8086 for PcBus {
    fn read(&self, addr: u32) -> u8 {
        // Apply A20 gate masking if A20 is disabled
        // When A20 is disabled, bit 20 of address is forced to 0
        // This causes addresses 0x100000-0x10FFFF to wrap to 0x000000-0x00FFFF
        let effective_addr = if !self.xms.is_a20_enabled() {
            addr & !0x100000 // Mask off bit 20 when A20 disabled
        } else {
            addr
        };

        match effective_addr {
            // Conventional memory (640KB)
            0x00000..=0x9FFFF => {
                let offset = effective_addr as usize;
                if offset < self.ram.len() {
                    self.ram[offset]
                } else {
                    0xFF
                }
            }
            // Video memory (128KB)
            0xA0000..=0xBFFFF => {
                let offset = (effective_addr - 0xA0000) as usize;
                if offset < self.vram.len() {
                    self.vram[offset]
                } else {
                    0xFF
                }
            }
            // ROM area (256KB) - includes BIOS
            0xC0000..=0xFFFFF => {
                let offset = (effective_addr - 0xC0000) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            // Extended memory (starts at 1MB = 0x100000)
            0x100000..=0xFFFFFFFF => {
                let offset = (effective_addr - 0x100000) as usize;
                if offset < self.extended_ram.len() {
                    self.extended_ram[offset]
                } else {
                    // Beyond allocated extended memory, wrap to low memory
                    let wrapped = effective_addr & 0xFFFFF;
                    self.read(wrapped)
                }
            }
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        // Apply A20 gate masking if A20 is disabled
        let effective_addr = if !self.xms.is_a20_enabled() {
            addr & !0x100000 // Mask off bit 20 when A20 disabled
        } else {
            addr
        };

        match effective_addr {
            // Conventional memory (640KB) - writable
            0x00000..=0x9FFFF => {
                let offset = effective_addr as usize;
                if offset < self.ram.len() {
                    self.ram[offset] = val;
                } else {
                    // Debug: log when write is out of bounds
                    use emu_core::logging::{LogCategory, LogConfig, LogLevel};
                    if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
                        eprintln!(
                            "!!! RAM write out of bounds: addr=0x{:08X}, offset={}, ram.len()={}",
                            addr,
                            offset,
                            self.ram.len()
                        );
                    }
                }
            }
            // Video memory (128KB) - writable
            0xA0000..=0xBFFFF => {
                let offset = (effective_addr - 0xA0000) as usize;
                if offset < self.vram.len() {
                    self.vram[offset] = val;
                }
            }
            // ROM area - read-only, writes are ignored
            0xC0000..=0xFFFFF => {
                // ROM writes are ignored
            }
            // Extended memory (starts at 1MB = 0x100000)
            0x100000..=0xFFFFFFFF => {
                let offset = (effective_addr - 0x100000) as usize;
                if offset < self.extended_ram.len() {
                    self.extended_ram[offset] = val;
                } else {
                    // Beyond allocated extended memory, wrap to low memory
                    let wrapped = effective_addr & 0xFFFFF;
                    self.write(wrapped, val);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_creation() {
        let bus = PcBus::new();
        assert_eq!(bus.ram.len(), 0xA0000);
        assert_eq!(bus.vram.len(), 0x20000);
        assert_eq!(bus.rom.len(), 0x40000);
    }

    #[test]
    fn test_ram_read_write() {
        let mut bus = PcBus::new();

        // Write to RAM
        bus.write(0x1000, 0x42);
        assert_eq!(bus.read(0x1000), 0x42);

        // Write to high RAM
        bus.write(0x9FFFF, 0xAB);
        assert_eq!(bus.read(0x9FFFF), 0xAB);
    }

    #[test]
    fn test_vram_read_write() {
        let mut bus = PcBus::new();

        // Write to video RAM
        bus.write(0xA0000, 0x55);
        assert_eq!(bus.read(0xA0000), 0x55);

        bus.write(0xBFFFF, 0xAA);
        assert_eq!(bus.read(0xBFFFF), 0xAA);
    }

    #[test]
    fn test_rom_read_only() {
        let mut bus = PcBus::new();

        // Load some data into ROM
        bus.rom[0] = 0x12;
        assert_eq!(bus.read(0xC0000), 0x12);

        // Try to write to ROM (should be ignored)
        bus.write(0xC0000, 0xFF);
        assert_eq!(bus.read(0xC0000), 0x12); // Should still be 0x12
    }

    #[test]
    fn test_bios_loading() {
        let mut bus = PcBus::new();

        let bios = vec![0xEA, 0x5B, 0xE0, 0x00, 0xF0]; // Simple BIOS stub
        bus.load_bios(&bios);

        // BIOS should be at 0xF0000+
        assert_eq!(bus.read(0xF0000), 0xEA);
        assert_eq!(bus.read(0xF0001), 0x5B);
    }

    #[test]
    fn test_address_wrapping() {
        let mut bus = PcBus::new();

        // Write beyond 1MB should wrap
        bus.write(0x100000, 0x99);
        assert_eq!(bus.read(0x00000), 0x99);
    }

    #[test]
    fn test_reset() {
        let mut bus = PcBus::new();

        bus.write(0x1000, 0x42);
        bus.reset();
        assert_eq!(bus.read(0x1000), 0x00);
    }

    #[test]
    fn test_executable_loading() {
        let mut bus = PcBus::new();

        let exe = vec![0x4D, 0x5A]; // MZ header
        bus.load_executable(exe.clone());

        assert!(bus.executable().is_some());
        assert_eq!(bus.executable().unwrap(), &exe);
    }

    #[test]
    fn test_floppy_mount_unmount() {
        let mut bus = PcBus::new();

        assert!(bus.floppy_a().is_none());

        let floppy = vec![0xF6; 1440 * 1024]; // 1.44MB floppy
        bus.mount_floppy_a(floppy.clone());

        assert!(bus.floppy_a().is_some());
        assert_eq!(bus.floppy_a().unwrap().len(), 1440 * 1024);

        bus.unmount_floppy_a();
        assert!(bus.floppy_a().is_none());
    }

    #[test]
    fn test_hard_drive_mount() {
        let mut bus = PcBus::new();

        assert!(bus.hard_drive().is_none());

        let hd = vec![0; 10 * 1024 * 1024]; // 10MB hard drive
        bus.mount_hard_drive(hd.clone());

        assert!(bus.hard_drive().is_some());
        assert_eq!(bus.hard_drive().unwrap().len(), 10 * 1024 * 1024);
    }

    #[test]
    fn test_vga_status_register() {
        let bus = PcBus::new();

        // Initial state should be 0x00 (not in retrace)
        assert_eq!(bus.io_read(0x03DA), 0x00);

        // Simulate ~2000 cycles (should be in retrace - first 4000 cycles)
        bus.update_vga_status(2000);
        assert_eq!(bus.io_read(0x03DA), 0x09); // Bits 0 and 3 set (in retrace)

        // Simulate more cycles to exit retrace period
        // Frame is ~80,000 cycles, retrace is first 4,000 cycles
        bus.update_vga_status(3000); // Total: 5000 cycles (past retrace)
        assert_eq!(bus.io_read(0x03DA), 0x00); // Back to display time

        // Continue well into display time
        bus.update_vga_status(10000); // Total: 15000 cycles
        assert_eq!(bus.io_read(0x03DA), 0x00); // Still in display

        // Test MDA port (0x03BA) - should behave identically
        let bus2 = PcBus::new();
        bus2.update_vga_status(2000);
        assert_eq!(bus2.io_read(0x03BA), 0x09); // In retrace
    }

    #[test]
    fn test_vga_status_wraps_around() {
        let bus = PcBus::new();

        // Simulate a full frame and then some
        bus.update_vga_status(80000); // One full frame
        assert_eq!(bus.io_read(0x03DA), 0x09); // Should wrap to retrace again

        // Advance past retrace in the new frame
        bus.update_vga_status(5000);
        assert_eq!(bus.io_read(0x03DA), 0x00); // Back to display
    }
}
