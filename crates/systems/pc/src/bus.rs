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
use crate::keyboard::Keyboard;
use emu_core::cpu_8086::Memory8086;

/// PC memory bus
pub struct PcBus {
    /// Main RAM (640KB)
    ram: Vec<u8>,
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
}

impl PcBus {
    /// Create a new PC bus
    pub fn new() -> Self {
        Self {
            ram: vec![0; 0xA0000],  // 640KB
            vram: vec![0; 0x20000], // 128KB
            rom: vec![0; 0x40000],  // 256KB
            executable: None,
            keyboard: Keyboard::new(),
            floppy_a: None,
            floppy_b: None,
            hard_drive: None,
            disk_controller: DiskController::new(),
            boot_priority: BootPriority::default(),
            boot_sector_loaded: false,
        }
    }

    /// Reset the bus to initial state
    pub fn reset(&mut self) {
        // Clear RAM but preserve ROM and executable
        self.ram.fill(0);
        self.vram.fill(0);
        self.keyboard.clear();
        self.disk_controller.reset();
        self.boot_sector_loaded = false;
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
            BootPriority::FloppyOnly => vec![
                (0x00, self.floppy_a.as_deref()),
            ],
            BootPriority::HardDriveOnly => vec![
                (0x80, self.hard_drive.as_deref()),
            ],
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
}

impl Default for PcBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory8086 for PcBus {
    fn read(&self, addr: u32) -> u8 {
        match addr {
            // Conventional memory (640KB)
            0x00000..=0x9FFFF => {
                let offset = addr as usize;
                if offset < self.ram.len() {
                    self.ram[offset]
                } else {
                    0xFF
                }
            }
            // Video memory (128KB)
            0xA0000..=0xBFFFF => {
                let offset = (addr - 0xA0000) as usize;
                if offset < self.vram.len() {
                    self.vram[offset]
                } else {
                    0xFF
                }
            }
            // ROM area (256KB) - includes BIOS
            0xC0000..=0xFFFFF => {
                let offset = (addr - 0xC0000) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            // Wrap around for addresses beyond 1MB (8086 behavior)
            _ => {
                let wrapped = addr & 0xFFFFF;
                self.read(wrapped)
            }
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        match addr {
            // Conventional memory (640KB) - writable
            0x00000..=0x9FFFF => {
                let offset = addr as usize;
                if offset < self.ram.len() {
                    self.ram[offset] = val;
                }
            }
            // Video memory (128KB) - writable
            0xA0000..=0xBFFFF => {
                let offset = (addr - 0xA0000) as usize;
                if offset < self.vram.len() {
                    self.vram[offset] = val;
                }
            }
            // ROM area - read-only, writes are ignored
            0xC0000..=0xFFFFF => {
                // ROM writes are ignored
            }
            // Wrap around for addresses beyond 1MB
            _ => {
                let wrapped = addr & 0xFFFFF;
                self.write(wrapped, val);
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
}
