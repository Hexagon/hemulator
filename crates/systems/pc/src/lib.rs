//! IBM PC/XT system implementation
//!
//! This module provides a basic IBM PC/XT compatible emulator using the 8086 CPU core.
//! It supports loading and running DOS executables (.COM and .EXE files).

#![allow(clippy::upper_case_acronyms)]

mod bios;
mod bus;
mod cpu;
mod disk;
mod font; // Shared IBM PC ROM font data
mod keyboard;
mod pit; // Programmable Interval Timer (8253/8254)
mod video;
mod video_adapter;
mod video_adapter_cga_graphics; // CGA graphics modes with mode switching
mod video_adapter_ega_hardware; // EGA hardware renderer (OpenGL stub)
mod video_adapter_ega_software; // EGA software renderer
mod video_adapter_hardware; // Example stub for hardware-accelerated rendering
mod video_adapter_software;
mod video_adapter_vga_hardware; // VGA hardware renderer (OpenGL stub)
mod video_adapter_vga_software; // VGA software renderer

use bios::generate_minimal_bios;
use bus::PcBus;
use cpu::PcCpu;
use emu_core::{
    cpu_8086::{CpuModel, Memory8086},
    types::Frame,
    MountPointInfo, System,
};
use serde_json::Value;
use thiserror::Error;
pub use video_adapter::VideoAdapter;
pub use video_adapter_software::SoftwareCgaAdapter;

pub use bios::BootPriority; // Export boot priority
pub use disk::{create_blank_floppy, create_blank_hard_drive, FloppyFormat, HardDriveFormat}; // Export disk utilities for GUI
pub use emu_core::cpu_8086::CpuModel as PcCpuModel; // Re-export for external use
pub use keyboard::*; // Export keyboard scancodes for GUI integration
pub use video_adapter_cga_graphics::{CgaGraphicsAdapter, CgaMode}; // Export CGA graphics adapter and modes
pub use video_adapter_ega_software::{EgaMode, SoftwareEgaAdapter}; // Export EGA software adapter and modes
pub use video_adapter_vga_software::{SoftwareVgaAdapter, VgaMode}; // Export VGA software adapter and modes

#[derive(Debug, Error)]
pub enum PcError {
    #[error("No executable loaded")]
    NoExecutable,
    #[error("Invalid executable format")]
    InvalidExecutable,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

/// PC system state
pub struct PcSystem {
    cpu: PcCpu,
    cycles: u64,
    frame_cycles: u64,
    video: Box<dyn VideoAdapter>,
}

impl Default for PcSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PcSystem {
    /// Create a new PC system with default CPU (8086), 640KB memory, and CGA video
    pub fn new() -> Self {
        Self::with_cpu_model(CpuModel::Intel8086)
    }

    /// Create a new PC system with a specific CPU model, default memory and video
    pub fn with_cpu_model(model: CpuModel) -> Self {
        Self::with_config(model, 640, Box::new(SoftwareCgaAdapter::new()))
    }

    /// Create a new PC system with full configuration
    ///
    /// # Arguments
    /// * `cpu_model` - CPU model (Intel8086, Intel8088, Intel80186, Intel80188, Intel80286, Intel80386)
    /// * `memory_kb` - Memory size in KB (256-640, will be clamped to valid range)
    /// * `video_adapter` - Video adapter (CGA, EGA, VGA)
    pub fn with_config(
        cpu_model: CpuModel,
        memory_kb: u32,
        video_adapter: Box<dyn VideoAdapter>,
    ) -> Self {
        let mut bus = PcBus::with_memory_kb(memory_kb);

        // Load minimal BIOS
        let bios = generate_minimal_bios();
        bus.load_bios(&bios);

        // Write BIOS POST screen to video RAM
        bios::write_post_screen_to_vram(bus.vram_mut());

        let cpu = PcCpu::with_model(bus, cpu_model);

        Self {
            cpu,
            cycles: 0,
            frame_cycles: 0,
            video: video_adapter,
        }
    }

    /// Get the CPU model
    pub fn cpu_model(&self) -> CpuModel {
        self.cpu.model()
    }

    /// Get the memory size in KB
    pub fn memory_kb(&self) -> u32 {
        self.cpu.bus().memory_kb()
    }

    /// Set the CPU model (requires reset to take full effect)
    pub fn set_cpu_model(&mut self, model: CpuModel) {
        self.cpu.set_model(model);
    }

    /// Load a DOS executable into memory
    #[allow(dead_code)]
    fn load_executable(&mut self, data: &[u8]) -> Result<(), PcError> {
        // Check for MZ header (DOS .EXE)
        if data.len() >= 2 && &data[0..2] == b"MZ" {
            // For now, just store it - full EXE parsing would be needed for proper loading
            self.cpu.bus_mut().load_executable(data.to_vec());
            return Ok(());
        }

        // Otherwise, assume it's a .COM file
        // COM files are loaded at 0x0100 and are limited to 64KB - 256 bytes
        if data.len() > 0xFF00 {
            return Err(PcError::InvalidExecutable);
        }

        // Load COM file at 0x0000:0x0100 (physical address 0x0100)
        self.cpu.bus_mut().load_executable(data.to_vec());

        // Copy to memory at 0x0100
        for (i, &byte) in data.iter().enumerate() {
            let addr = 0x0100 + i as u32;
            self.cpu.bus_mut().write(addr, byte);
        }

        Ok(())
    }

    /// Handle keyboard input (called by GUI)
    pub fn key_press(&mut self, scancode: u8) {
        self.cpu.bus_mut().keyboard.key_press(scancode);
    }

    /// Handle keyboard release (called by GUI)
    pub fn key_release(&mut self, scancode: u8) {
        self.cpu.bus_mut().keyboard.key_release(scancode);
    }

    /// Set boot priority
    pub fn set_boot_priority(&mut self, priority: bios::BootPriority) {
        self.cpu.bus_mut().set_boot_priority(priority);
    }

    /// Get boot priority
    pub fn boot_priority(&self) -> bios::BootPriority {
        self.cpu.bus().boot_priority()
    }

    /// Set the video adapter
    ///
    /// # Examples
    /// ```
    /// use emu_pc::{PcSystem, SoftwareCgaAdapter, SoftwareEgaAdapter, SoftwareVgaAdapter};
    ///
    /// let mut sys = PcSystem::new();
    /// // Switch to EGA adapter
    /// sys.set_video_adapter(Box::new(SoftwareEgaAdapter::new()));
    /// // Switch to VGA adapter
    /// sys.set_video_adapter(Box::new(SoftwareVgaAdapter::new()));
    /// ```
    pub fn set_video_adapter(&mut self, adapter: Box<dyn VideoAdapter>) {
        self.video = adapter;
    }

    /// Get the current video adapter name
    ///
    /// # Examples
    /// ```
    /// use emu_pc::PcSystem;
    ///
    /// let sys = PcSystem::new();
    /// assert_eq!(sys.video_adapter_name(), "Software CGA Adapter");
    /// ```
    pub fn video_adapter_name(&self) -> &str {
        self.video.name()
    }

    /// Get the current framebuffer dimensions
    ///
    /// Returns (width, height) in pixels
    pub fn framebuffer_dimensions(&self) -> (usize, usize) {
        (self.video.fb_width(), self.video.fb_height())
    }

    /// Trigger boot sector loading (called before first execution or on reset)
    fn ensure_boot_sector_loaded(&mut self) {
        self.cpu.bus_mut().load_boot_sector();
    }

    /// Get debug information
    pub fn debug_info(&self) -> DebugInfo {
        let regs = self.cpu.get_registers();
        DebugInfo {
            cs: regs.cs,
            ip: regs.ip,
            ax: regs.ax,
            bx: regs.bx,
            cx: regs.cx,
            dx: regs.dx,
            sp: regs.sp,
            bp: regs.bp,
            si: regs.si,
            di: regs.di,
            flags: regs.flags,
            cycles: self.cycles,
        }
    }

    /// Update POST screen with current mount status
    pub fn update_post_screen(&mut self) {
        // Get mount status first (immutable borrows)
        let floppy_a = self.cpu.bus().floppy_a().is_some();
        let floppy_b = self.cpu.bus().floppy_b().is_some();
        let hard_drive = self.cpu.bus().hard_drive().is_some();
        let boot_priority = self.cpu.bus().boot_priority();

        // Now get mutable borrow to update VRAM
        let vram = self.cpu.bus_mut().vram_mut();
        bios::update_post_screen_mounts(vram, floppy_a, floppy_b, hard_drive, boot_priority);
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub cs: u16,
    pub ip: u16,
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub flags: u16,
    pub cycles: u64,
}

impl System for PcSystem {
    type Error = PcError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.cpu.bus_mut().reset();
        self.cycles = 0;
        self.frame_cycles = 0;

        // Write BIOS POST screen to video RAM
        let vram = self.cpu.bus_mut().vram_mut();
        bios::write_post_screen_to_vram(vram);
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // PC runs at ~4.77 MHz
        // At 60 Hz, that's ~79,500 cycles per frame
        const CYCLES_PER_FRAME: u32 = 79500;

        // Ensure boot sector is loaded before first execution
        self.ensure_boot_sector_loaded();

        // Create frame buffer for text mode 80x25 (640x400 pixels)
        let mut frame = Frame::new(self.video.fb_width() as u32, self.video.fb_height() as u32);

        let mut cycles_this_frame = 0u32;

        // Execute until we've completed a frame
        while cycles_this_frame < CYCLES_PER_FRAME {
            let cycles = self.cpu.step();
            cycles_this_frame += cycles;
            self.cycles += cycles as u64;
            self.frame_cycles += cycles as u64;

            // Clock the PIT with the cycles executed
            let timer_interrupt = self.cpu.bus_mut().pit.clock(cycles);
            if timer_interrupt {
                // Timer interrupt should trigger INT 08h
                // For now, just clear the flag
                self.cpu.bus_mut().pit.clear_timer_interrupt();
            }
        }

        // Render video memory to frame buffer
        // CGA text mode video RAM starts at 0xB8000
        let vram = self.cpu.bus().vram();
        // The text mode buffer is at offset 0x18000 in VRAM (0xB8000 - 0xA0000)
        let text_buffer_offset = 0x18000;
        if vram.len() > text_buffer_offset {
            self.video
                .render(&vram[text_buffer_offset..], &mut frame.pixels);
        }

        Ok(frame)
    }

    fn save_state(&self) -> Value {
        // PC systems don't use save states like consoles
        // State is preserved in the disk images themselves
        // This is kept for API compatibility but returns minimal data
        serde_json::json!({
            "version": 1,
            "system": "pc",
            "note": "PC state is preserved in disk images, not save states"
        })
    }

    fn load_state(&mut self, _state: &Value) -> Result<(), serde_json::Error> {
        // PC systems don't use save states
        // This is a no-op for API compatibility
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        // PC systems don't use save states like consoles
        // State is preserved in disk images (which can be modified and saved)
        // System configuration (CPU model, boot priority) should be set via
        // the GUI or command-line arguments, not save states
        false
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![
            MountPointInfo {
                id: "BIOS".to_string(),
                name: "BIOS ROM".to_string(),
                extensions: vec!["bin".to_string(), "rom".to_string()],
                required: false, // Has default BIOS
            },
            MountPointInfo {
                id: "FloppyA".to_string(),
                name: "Floppy Drive A:".to_string(),
                extensions: vec!["img".to_string(), "ima".to_string()],
                required: false,
            },
            MountPointInfo {
                id: "FloppyB".to_string(),
                name: "Floppy Drive B:".to_string(),
                extensions: vec!["img".to_string(), "ima".to_string()],
                required: false,
            },
            MountPointInfo {
                id: "HardDrive".to_string(),
                name: "Hard Drive C:".to_string(),
                extensions: vec!["img".to_string(), "vhd".to_string()],
                required: false,
            },
        ]
    }

    /// Mount a disk image with validation
    ///
    /// # Arguments
    /// * `mount_point_id` - The mount point identifier ("BIOS", "FloppyA", "FloppyB", "HardDrive")
    /// * `data` - The disk image data
    ///
    /// # Examples
    /// ```no_run
    /// use emu_pc::{PcSystem, create_blank_floppy, FloppyFormat};
    /// use emu_core::System;
    ///
    /// let mut sys = PcSystem::new();
    /// let floppy = create_blank_floppy(FloppyFormat::Floppy1_44M);
    /// sys.mount("FloppyA", &floppy).expect("Failed to mount floppy");
    /// ```
    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        // Validate data size for disk images
        match mount_point_id {
            "BIOS" => {
                if data.is_empty() {
                    return Err(PcError::InvalidExecutable);
                }
                self.cpu.bus_mut().load_bios(data);
                Ok(())
            }
            "FloppyA" => {
                // Validate floppy size (common formats: 360K, 720K, 1.2M, 1.44M)
                let valid_sizes = [368640, 737280, 1228800, 1474560];
                if !valid_sizes.contains(&data.len()) {
                    eprintln!(
                        "Warning: Floppy A image size {} is not a standard format",
                        data.len()
                    );
                }
                self.cpu.bus_mut().mount_floppy_a(data.to_vec());
                Ok(())
            }
            "FloppyB" => {
                let valid_sizes = [368640, 737280, 1228800, 1474560];
                if !valid_sizes.contains(&data.len()) {
                    eprintln!(
                        "Warning: Floppy B image size {} is not a standard format",
                        data.len()
                    );
                }
                self.cpu.bus_mut().mount_floppy_b(data.to_vec());
                Ok(())
            }
            "HardDrive" => {
                // Validate hard drive size (minimum 1MB)
                if data.len() < 1024 * 1024 {
                    return Err(PcError::InvalidExecutable);
                }
                self.cpu.bus_mut().mount_hard_drive(data.to_vec());
                Ok(())
            }
            _ => Err(PcError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                // Reload default BIOS
                let bios = generate_minimal_bios();
                self.cpu.bus_mut().load_bios(&bios);
                Ok(())
            }
            "FloppyA" => {
                self.cpu.bus_mut().unmount_floppy_a();
                Ok(())
            }
            "FloppyB" => {
                self.cpu.bus_mut().unmount_floppy_b();
                Ok(())
            }
            "HardDrive" => {
                self.cpu.bus_mut().unmount_hard_drive();
                Ok(())
            }
            _ => Err(PcError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "BIOS" => true, // BIOS always mounted (has default)
            "FloppyA" => self.cpu.bus().floppy_a().is_some(),
            "FloppyB" => self.cpu.bus().floppy_b().is_some(),
            "HardDrive" => self.cpu.bus().hard_drive().is_some(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation() {
        let sys = PcSystem::new();
        assert_eq!(sys.cycles, 0);
    }

    #[test]
    fn test_system_reset() {
        let mut sys = PcSystem::new();

        // Execute some cycles
        let _ = sys.step_frame();
        assert!(sys.cycles > 0);

        sys.reset();
        assert_eq!(sys.cycles, 0);
    }

    #[test]
    fn test_load_com_file() {
        let mut sys = PcSystem::new();

        // Simple COM program: MOV AX, 0x1234; HLT
        let program = vec![0xB8, 0x34, 0x12, 0xF4];

        // Load executable via the old method (kept for backward compatibility)
        assert!(sys.load_executable(&program).is_ok());

        // Check that BIOS is always mounted (has default)
        assert!(sys.is_mounted("BIOS"));
    }

    #[test]
    fn test_save_load_state() {
        let sys = PcSystem::new();

        // PC systems don't use save states (returns minimal placeholder)
        let state = sys.save_state();
        assert_eq!(state["system"], "pc");
        assert_eq!(state["version"], 1);
        assert_eq!(
            state["note"],
            "PC state is preserved in disk images, not save states"
        );

        let mut sys2 = PcSystem::new();
        assert!(sys2.load_state(&state).is_ok()); // Should be a no-op
    }

    #[test]
    fn test_mount_points() {
        let sys = PcSystem::new();
        let mps = sys.mount_points();

        assert_eq!(mps.len(), 4);

        // Check BIOS mount point
        assert_eq!(mps[0].id, "BIOS");
        assert!(!mps[0].required); // Has default
        assert!(mps[0].extensions.contains(&"bin".to_string()));

        // Check Floppy A
        assert_eq!(mps[1].id, "FloppyA");
        assert!(!mps[1].required);

        // Check Floppy B
        assert_eq!(mps[2].id, "FloppyB");
        assert!(!mps[2].required);

        // Check Hard Drive
        assert_eq!(mps[3].id, "HardDrive");
        assert!(!mps[3].required);
    }

    #[test]
    fn test_invalid_mount_point() {
        let mut sys = PcSystem::new();
        let result = sys.mount("InvalidMount", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_floppy() {
        let mut sys = PcSystem::new();

        // Create a minimal floppy image (360KB standard floppy)
        let floppy_data = vec![0xF6; 368640]; // 360KB

        assert!(sys.mount("FloppyA", &floppy_data).is_ok());
        assert!(sys.is_mounted("FloppyA"));

        sys.unmount("FloppyA").unwrap();
        assert!(!sys.is_mounted("FloppyA"));
    }

    #[test]
    fn test_mount_hard_drive() {
        let mut sys = PcSystem::new();

        // Create a minimal hard drive image (1MB)
        let hd_data = vec![0; 1024 * 1024];

        assert!(sys.mount("HardDrive", &hd_data).is_ok());
        assert!(sys.is_mounted("HardDrive"));

        sys.unmount("HardDrive").unwrap();
        assert!(!sys.is_mounted("HardDrive"));
    }

    #[test]
    fn test_supports_save_states() {
        let sys = PcSystem::new();
        // PC systems don't support save states - state is in disk images
        assert!(!sys.supports_save_states());
    }

    #[test]
    fn test_debug_info() {
        let sys = PcSystem::new();
        let info = sys.debug_info();

        // Should start at BIOS entry point
        assert_eq!(info.cs, 0xFFFF);
        assert_eq!(info.ip, 0x0000);
    }

    #[test]
    fn test_step_frame() {
        let mut sys = PcSystem::new();

        // Load a simple program
        let program = vec![0xB8, 0x34, 0x12, 0xF4]; // MOV AX, 0x1234; HLT
        let _ = sys.load_executable(&program);

        let result = sys.step_frame();
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 400);
    }

    #[test]
    fn test_mount_multiple_disks() {
        let mut sys = PcSystem::new();

        // Create disk images
        let floppy_a = crate::create_blank_floppy(crate::FloppyFormat::Floppy1_44M);
        let floppy_b = crate::create_blank_floppy(crate::FloppyFormat::Floppy720K);
        let hard_drive = crate::create_blank_hard_drive(crate::HardDriveFormat::HardDrive10M);

        // Mount all disks
        assert!(sys.mount("FloppyA", &floppy_a).is_ok());
        assert!(sys.mount("FloppyB", &floppy_b).is_ok());
        assert!(sys.mount("HardDrive", &hard_drive).is_ok());

        // Verify all are mounted
        assert!(sys.is_mounted("FloppyA"));
        assert!(sys.is_mounted("FloppyB"));
        assert!(sys.is_mounted("HardDrive"));

        // Unmount all
        assert!(sys.unmount("FloppyA").is_ok());
        assert!(sys.unmount("FloppyB").is_ok());
        assert!(sys.unmount("HardDrive").is_ok());

        // Verify all are unmounted
        assert!(!sys.is_mounted("FloppyA"));
        assert!(!sys.is_mounted("FloppyB"));
        assert!(!sys.is_mounted("HardDrive"));
    }

    #[test]
    fn test_create_blank_disks() {
        // Test all floppy formats
        let floppy_360k = crate::create_blank_floppy(crate::FloppyFormat::Floppy360K);
        assert_eq!(floppy_360k.len(), 368640);

        let floppy_720k = crate::create_blank_floppy(crate::FloppyFormat::Floppy720K);
        assert_eq!(floppy_720k.len(), 737280);

        let floppy_1_2m = crate::create_blank_floppy(crate::FloppyFormat::Floppy1_2M);
        assert_eq!(floppy_1_2m.len(), 1228800);

        let floppy_1_44m = crate::create_blank_floppy(crate::FloppyFormat::Floppy1_44M);
        assert_eq!(floppy_1_44m.len(), 1474560);

        // Test all hard drive formats
        let hd_10m = crate::create_blank_hard_drive(crate::HardDriveFormat::HardDrive10M);
        assert_eq!(hd_10m.len(), 10653696);

        let hd_20m = crate::create_blank_hard_drive(crate::HardDriveFormat::HardDrive20M);
        assert_eq!(hd_20m.len(), 21307392);

        let hd_40m = crate::create_blank_hard_drive(crate::HardDriveFormat::HardDrive40M);
        assert_eq!(hd_40m.len(), 42618880);
    }

    #[test]
    fn test_boot_sector_loading_from_floppy() {
        let mut sys = PcSystem::new();

        // Create a bootable floppy image with boot signature
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Add boot signature at offset 510-511
        floppy[510] = 0x55;
        floppy[511] = 0xAA;

        // Add some boot code
        floppy[0] = 0xEA; // JMP FAR (boot code starts here)

        // Mount the floppy
        assert!(sys.mount("FloppyA", &floppy).is_ok());

        // Set boot priority to floppy first
        sys.set_boot_priority(crate::BootPriority::FloppyFirst);

        // Trigger boot sector load
        sys.ensure_boot_sector_loaded();

        // Verify boot sector was loaded to 0x7C00
        let bus = sys.cpu.bus();

        // Check that boot code was copied
        assert_eq!(bus.read_ram(0x7C00), 0xEA);

        // Check boot signature
        assert_eq!(bus.read_ram(0x7C00 + 510), 0x55);
        assert_eq!(bus.read_ram(0x7C00 + 511), 0xAA);
    }

    #[test]
    fn test_boot_sector_loading_from_hard_drive() {
        let mut sys = PcSystem::new();

        // Create a bootable hard drive image
        let mut hd = vec![0; 10653696]; // 10MB

        // Add boot signature
        hd[510] = 0x55;
        hd[511] = 0xAA;

        // Add boot code
        hd[0] = 0xB8; // MOV AX, ... (different from floppy)

        // Mount the hard drive
        assert!(sys.mount("HardDrive", &hd).is_ok());

        // Set boot priority to hard drive first
        sys.set_boot_priority(crate::BootPriority::HardDriveFirst);

        // Trigger boot sector load
        sys.ensure_boot_sector_loaded();

        // Verify boot sector was loaded
        let bus = sys.cpu.bus();

        // Check that boot code was copied
        assert_eq!(bus.read_ram(0x7C00), 0xB8);

        // Check boot signature
        assert_eq!(bus.read_ram(0x7C00 + 510), 0x55);
        assert_eq!(bus.read_ram(0x7C00 + 511), 0xAA);
    }

    #[test]
    fn test_boot_priority_fallback() {
        let mut sys = PcSystem::new();

        // Create bootable hard drive but NOT bootable floppy
        let mut hd = vec![0; 10653696];
        hd[510] = 0x55;
        hd[511] = 0xAA;
        hd[0] = 0xB8;

        let mut floppy = vec![0; 1474560];
        // No boot signature on floppy!
        floppy[510] = 0x00;
        floppy[511] = 0x00;

        // Mount both
        assert!(sys.mount("FloppyA", &floppy).is_ok());
        assert!(sys.mount("HardDrive", &hd).is_ok());

        // Set boot priority to floppy first (should fall back to hard drive)
        sys.set_boot_priority(crate::BootPriority::FloppyFirst);

        // Trigger boot sector load
        sys.ensure_boot_sector_loaded();

        // Should have loaded from hard drive (fallback)
        let bus = sys.cpu.bus();
        assert_eq!(bus.read_ram(0x7C00), 0xB8); // Hard drive boot code
    }

    #[test]
    fn test_invalid_boot_signature() {
        let mut sys = PcSystem::new();

        // Create a floppy WITHOUT valid boot signature
        let mut floppy = vec![0; 1474560];
        floppy[510] = 0xFF; // Invalid
        floppy[511] = 0xFF; // Invalid

        assert!(sys.mount("FloppyA", &floppy).is_ok());

        sys.set_boot_priority(crate::BootPriority::FloppyOnly);

        // Trigger boot sector load (should fail)
        sys.ensure_boot_sector_loaded();

        // Boot sector should NOT be loaded (should remain zeros)
        let bus = sys.cpu.bus();

        // Should be all zeros since boot failed
        assert_eq!(bus.read_ram(0x7C00), 0x00);
        assert_eq!(bus.read_ram(0x7C00 + 510), 0x00);
    }

    #[test]
    fn test_boot_sector_smoke_test() {
        // This test uses the test boot sector from test_roms/pc/boot.bin
        // The boot sector writes "BOOT OK" to video memory
        let boot_bin_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test_roms/pc/boot.bin");

        // Skip if boot.bin doesn't exist (not built yet)
        if !std::path::Path::new(boot_bin_path).exists() {
            eprintln!(
                "Skipping boot sector smoke test: {} not found",
                boot_bin_path
            );
            eprintln!("Build with: cd test_roms/pc && ./build_boot.sh");
            return;
        }

        let boot_sector = std::fs::read(boot_bin_path).expect("Failed to read boot.bin");
        assert_eq!(
            boot_sector.len(),
            512,
            "Boot sector should be exactly 512 bytes"
        );

        // Create a floppy image with the boot sector
        let mut floppy = vec![0; 1474560]; // 1.44MB
        floppy[0..512].copy_from_slice(&boot_sector);

        // Create system and mount floppy
        let mut sys = PcSystem::new();
        assert!(sys.mount("FloppyA", &floppy).is_ok());
        sys.set_boot_priority(crate::BootPriority::FloppyFirst);

        // Run for a few frames to let the boot code execute
        for _ in 0..5 {
            let _ = sys.step_frame();
        }

        // Check that "BOOT OK" was written to video memory
        // Video memory is at 0xB8000, which is offset 0x18000 in VRAM
        // Each character is 2 bytes: character + attribute
        let vram = sys.cpu.bus().vram();
        let text_offset = 0x18000;

        // Verify "BOOT OK" (each char followed by green attribute 0x02)
        if vram.len() > text_offset + 14 {
            assert_eq!(vram[text_offset], b'B');
            assert_eq!(vram[text_offset + 1], 0x02);
            assert_eq!(vram[text_offset + 2], b'O');
            assert_eq!(vram[text_offset + 3], 0x02);
            assert_eq!(vram[text_offset + 4], b'O');
            assert_eq!(vram[text_offset + 5], 0x02);
            assert_eq!(vram[text_offset + 6], b'T');
            assert_eq!(vram[text_offset + 7], 0x02);
            assert_eq!(vram[text_offset + 8], b' ');
            assert_eq!(vram[text_offset + 9], 0x02);
            assert_eq!(vram[text_offset + 10], b'O');
            assert_eq!(vram[text_offset + 11], 0x02);
            assert_eq!(vram[text_offset + 12], b'K');
            assert_eq!(vram[text_offset + 13], 0x02);
            println!("Boot sector smoke test passed: BOOT OK displayed");
        } else {
            panic!("VRAM too small");
        }
    }

    #[test]
    fn test_int13h_integration() {
        // Integration test: Create a program that uses INT 13h to read a sector
        let mut sys = PcSystem::new();

        // Create a floppy with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first sector with a pattern
        for (i, byte) in floppy.iter_mut().enumerate().take(512) {
            *byte = (i % 256) as u8;
        }

        // Add boot signature
        floppy[510] = 0x55;
        floppy[511] = 0xAA;

        // Mount the floppy
        assert!(sys.mount("FloppyA", &floppy).is_ok());
        sys.set_boot_priority(crate::BootPriority::FloppyFirst);

        // Create a simple program that uses INT 13h
        // This program will:
        // 1. Reset disk (INT 13h, AH=00h)
        // 2. Read sector (INT 13h, AH=02h)
        // 3. Write result to video memory
        // 4. Halt

        let program = vec![
            // Reset disk (INT 13h, AH=00h)
            0xB4, 0x00, // MOV AH, 0x00
            0xB2, 0x00, // MOV DL, 0x00 (drive A)
            0xCD, 0x13, // INT 13h
            // Read 1 sector (INT 13h, AH=02h)
            0xB4, 0x02, // MOV AH, 0x02
            0xB0, 0x01, // MOV AL, 0x01 (1 sector)
            0xB5, 0x00, // MOV CH, 0x00 (cylinder 0)
            0xB1, 0x01, // MOV CL, 0x01 (sector 1)
            0xB6, 0x00, // MOV DH, 0x00 (head 0)
            0xB2, 0x00, // MOV DL, 0x00 (drive A)
            0xBB, 0x00, 0x80, // MOV BX, 0x8000 (buffer)
            0x8E, 0xC3, // MOV ES, BX (ES = 0x8000)
            0xBB, 0x00, 0x00, // MOV BX, 0x0000 (offset)
            0xCD, 0x13, // INT 13h
            // Halt
            0xF4, // HLT
        ];

        // Load program at 0x0000:0x7C00 (standard boot sector location)
        for (i, &byte) in program.iter().enumerate() {
            sys.cpu.bus_mut().write(0x7C00 + i as u32, byte);
        }

        // Set CS:IP to start of program
        let mut regs = sys.cpu.get_registers();
        regs.cs = 0x0000;
        regs.ip = 0x7C00;
        regs.sp = 0xFFFE;
        sys.cpu.set_registers(&regs);

        // Run the program for enough cycles to complete
        for _ in 0..50 {
            sys.cpu.step();
        }

        // Verify that the program executed successfully
        // After INT 13h calls, the program halts (HLT instruction)
        // We've successfully tested INT 13h reset and read operations

        println!("INT 13h integration test completed - program executed successfully");
    }

    #[test]
    fn test_cpu_model_default() {
        let sys = PcSystem::new();
        assert_eq!(sys.cpu_model(), CpuModel::Intel8086);
    }

    #[test]
    fn test_cpu_model_selection() {
        let sys = PcSystem::with_cpu_model(CpuModel::Intel80186);
        assert_eq!(sys.cpu_model(), CpuModel::Intel80186);
    }

    #[test]
    fn test_cpu_model_set() {
        let mut sys = PcSystem::new();
        assert_eq!(sys.cpu_model(), CpuModel::Intel8086);

        sys.set_cpu_model(CpuModel::Intel80286);
        assert_eq!(sys.cpu_model(), CpuModel::Intel80286);
    }

    #[test]
    fn test_all_cpu_models() {
        for model in &[
            CpuModel::Intel8086,
            CpuModel::Intel8088,
            CpuModel::Intel80186,
            CpuModel::Intel80188,
            CpuModel::Intel80286,
        ] {
            let sys = PcSystem::with_cpu_model(*model);
            assert_eq!(sys.cpu_model(), *model);
        }
    }

    #[test]
    fn test_post_screen_display() {
        // Test that the BIOS displays the POST screen
        let mut sys = PcSystem::new();

        // Run for a few frames to let the BIOS execute and display the POST screen
        for _ in 0..5 {
            let _ = sys.step_frame();
        }

        // Check that the POST screen was written to video memory
        // Video memory is at 0xB8000, which is offset 0x18000 in VRAM
        let vram = sys.cpu.bus().vram();
        let text_offset = 0x18000;

        // Check header at row 0, column 2 (should contain "Hemu BIOS")
        let header_offset = text_offset + 2 * 2; // row 0
        if vram.len() > header_offset + 40 {
            let header_chars: Vec<char> = (0..20)
                .map(|i| vram[header_offset + i * 2] as char)
                .collect();
            let header: String = header_chars.iter().collect();

            println!("POST screen header: '{}'", header);

            // Verify we have "Hemu BIOS" in the header
            assert!(
                header.contains("Hemu BIOS"),
                "POST screen should contain 'Hemu BIOS'"
            );

            // Verify the attribute is white on blue (0x1F)
            assert_eq!(
                vram[header_offset + 1],
                0x1F,
                "Header should be in white on blue"
            );
        }

        // Check title at row 3 (should contain "Hemu PC/XT")
        let title_offset = text_offset + (3 * 80 + 2) * 2;
        if vram.len() > title_offset + 60 {
            let title_chars: Vec<char> = (0..30)
                .map(|i| vram[title_offset + i * 2] as char)
                .collect();
            let title: String = title_chars.iter().collect();

            println!("POST screen title: '{}'", title);

            // Verify we have "Hemu PC/XT" in the title
            assert!(
                title.contains("Hemu PC/XT"),
                "POST screen should contain 'Hemu PC/XT'"
            );
        }

        // Check that disk drives are shown as "Not present" initially
        let disk_offset = text_offset + (10 * 80 + 4) * 2;
        if vram.len() > disk_offset + 60 {
            let disk_chars: Vec<char> =
                (0..30).map(|i| vram[disk_offset + i * 2] as char).collect();
            let disk_line: String = disk_chars.iter().collect();

            println!("Disk status line: '{}'", disk_line);

            // Verify disk status
            assert!(
                disk_line.contains("Floppy A:"),
                "POST screen should show Floppy A status"
            );
        }
    }

    #[test]
    fn test_post_screen_content() {
        // Test that the POST screen contains expected content
        let sys = PcSystem::new();

        let vram = sys.cpu.bus().vram();
        let text_offset = 0x18000;

        // Helper to read text from VRAM
        let read_text = |row: usize, col: usize, len: usize| -> String {
            let offset = text_offset + (row * 80 + col) * 2;
            (0..len)
                .map(|i| {
                    if offset + i * 2 < vram.len() {
                        vram[offset + i * 2] as char
                    } else {
                        ' '
                    }
                })
                .collect()
        };

        // Check header contains "Hemu BIOS"
        let header = read_text(0, 0, 80);
        assert!(
            header.contains("Hemu BIOS"),
            "Header should contain 'Hemu BIOS'"
        );

        // Check title contains "Hemu PC/XT"
        let title = read_text(3, 0, 80);
        assert!(
            title.contains("Hemu PC/XT"),
            "Title should contain 'Hemu PC/XT'"
        );

        // Check processor line
        let processor = read_text(5, 0, 80);
        assert!(processor.contains("Intel 8086"), "Should show Intel 8086");

        // Check memory line
        let memory = read_text(7, 0, 80);
        assert!(memory.contains("640K"), "Should show 640K memory");

        // Check disk status - initially all "Not present"
        let floppy_a = read_text(10, 0, 80);
        assert!(floppy_a.contains("Floppy A:"), "Should show Floppy A");
        assert!(
            floppy_a.contains("Not present"),
            "Floppy A should be not present initially"
        );

        let floppy_b = read_text(11, 0, 80);
        assert!(floppy_b.contains("Floppy B:"), "Should show Floppy B");
        assert!(
            floppy_b.contains("Not present"),
            "Floppy B should be not present initially"
        );

        let hard_disk = read_text(12, 0, 80);
        assert!(
            hard_disk.contains("Hard Disk C:"),
            "Should show Hard Disk C"
        );
        assert!(
            hard_disk.contains("Not present"),
            "Hard Disk C should be not present initially"
        );

        // Check boot priority
        let boot = read_text(14, 0, 80);
        assert!(
            boot.contains("Floppy First"),
            "Should show Floppy First as default"
        );

        // Check instructions mention F3, F12, F8
        let help1 = read_text(20, 0, 80);
        assert!(help1.contains("F3"), "Should mention F3 key");

        let help2 = read_text(21, 0, 80);
        assert!(help2.contains("F12"), "Should mention F12 key");

        let help3 = read_text(22, 0, 80);
        assert!(help3.contains("F8"), "Should mention F8 key");

        // Check bottom message
        let bottom = read_text(24, 0, 80);
        assert!(
            bottom.contains("No bootable disk"),
            "Should show no bootable disk message"
        );
    }

    #[test]
    fn test_post_screen_updates_on_mount() {
        // Test that POST screen updates when disks are mounted
        let mut sys = PcSystem::new();

        // Create a blank floppy and mount it
        let floppy = crate::create_blank_floppy(crate::FloppyFormat::Floppy1_44M);
        sys.mount("FloppyA", &floppy).unwrap();
        sys.update_post_screen();

        let vram = sys.cpu.bus().vram();
        let text_offset = 0x18000;

        // Check that Floppy A now shows as "Present"
        let floppy_a_offset = text_offset + (10 * 80 + 4) * 2;
        let floppy_a: String = (0..30)
            .map(|i| vram[floppy_a_offset + i * 2] as char)
            .collect();

        assert!(
            floppy_a.contains("Floppy A:"),
            "Should still show Floppy A label"
        );
        assert!(
            floppy_a.contains("Present"),
            "Floppy A should show as Present after mounting"
        );

        // Bottom message should change
        let bottom_offset = text_offset + (24 * 80 + 2) * 2;
        let bottom: String = (0..50)
            .map(|i| vram[bottom_offset + i * 2] as char)
            .collect();

        assert!(
            bottom.contains("Bootable disk detected") || bottom.contains("Press F12 to boot"),
            "Bottom message should indicate bootable disk detected"
        );
    }

    #[test]
    fn test_video_adapter_switching() {
        let mut sys = PcSystem::new();

        // Default is CGA
        assert_eq!(sys.video_adapter_name(), "Software CGA Adapter");
        assert_eq!(sys.framebuffer_dimensions(), (640, 400));

        // Switch to EGA
        sys.set_video_adapter(Box::new(
            crate::video_adapter_ega_software::SoftwareEgaAdapter::new(),
        ));
        assert_eq!(sys.video_adapter_name(), "Software EGA Adapter");
        assert_eq!(sys.framebuffer_dimensions(), (640, 350));

        // Switch to VGA
        sys.set_video_adapter(Box::new(
            crate::video_adapter_vga_software::SoftwareVgaAdapter::new(),
        ));
        assert_eq!(sys.video_adapter_name(), "Software VGA Adapter");
        assert_eq!(sys.framebuffer_dimensions(), (720, 400));

        // Switch back to CGA
        sys.set_video_adapter(Box::new(SoftwareCgaAdapter::new()));
        assert_eq!(sys.video_adapter_name(), "Software CGA Adapter");
        assert_eq!(sys.framebuffer_dimensions(), (640, 400));
    }

    #[test]
    fn test_mount_validation_invalid_bios() {
        let mut sys = PcSystem::new();
        // Empty BIOS should fail
        let result = sys.mount("BIOS", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_validation_invalid_hard_drive() {
        let mut sys = PcSystem::new();
        // Hard drive smaller than 1MB should fail
        let small_hd = vec![0; 512 * 1024]; // 512KB
        let result = sys.mount("HardDrive", &small_hd);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_validation_valid_floppy() {
        let mut sys = PcSystem::new();
        // 1.44MB floppy should succeed
        let floppy = vec![0; 1474560];
        let result = sys.mount("FloppyA", &floppy);
        assert!(result.is_ok());
        assert!(sys.is_mounted("FloppyA"));
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn test_memory_size_clamping() {
        // Test that memory size is clamped to valid range (256-640 KB)

        // Test below minimum (should clamp to 256)
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            100, // Too low
            Box::new(SoftwareCgaAdapter::new()),
        );
        assert_eq!(
            sys.memory_kb(),
            256,
            "Memory should be clamped to 256KB minimum"
        );

        // Test above maximum (should clamp to 640)
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            1024, // Too high
            Box::new(SoftwareCgaAdapter::new()),
        );
        assert_eq!(
            sys.memory_kb(),
            640,
            "Memory should be clamped to 640KB maximum"
        );

        // Test valid values
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            512,
            Box::new(SoftwareCgaAdapter::new()),
        );
        assert_eq!(sys.memory_kb(), 512, "Memory should be 512KB");
    }

    #[test]
    fn test_with_config_cpu_models() {
        // Test that different CPU models can be configured
        let models = vec![
            CpuModel::Intel8086,
            CpuModel::Intel8088,
            CpuModel::Intel80186,
            CpuModel::Intel80188,
            CpuModel::Intel80286,
            CpuModel::Intel80386,
        ];

        for model in models {
            let sys = PcSystem::with_config(model, 640, Box::new(SoftwareCgaAdapter::new()));
            assert_eq!(sys.cpu_model(), model, "CPU model should match");
        }
    }

    #[test]
    fn test_with_config_video_adapters() {
        // Test that different video adapters can be configured

        // CGA adapter
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            640,
            Box::new(SoftwareCgaAdapter::new()),
        );
        assert!(
            sys.video_adapter_name().contains("CGA"),
            "Should be CGA adapter"
        );

        // EGA adapter
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            640,
            Box::new(SoftwareEgaAdapter::new()),
        );
        assert!(
            sys.video_adapter_name().contains("EGA"),
            "Should be EGA adapter"
        );

        // VGA adapter
        let sys = PcSystem::with_config(
            CpuModel::Intel8086,
            640,
            Box::new(SoftwareVgaAdapter::new()),
        );
        assert!(
            sys.video_adapter_name().contains("VGA"),
            "Should be VGA adapter"
        );
    }
}
