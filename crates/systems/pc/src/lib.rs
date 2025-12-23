//! IBM PC/XT system implementation
//!
//! This module provides a basic IBM PC/XT compatible emulator using the 8086 CPU core.
//! It supports loading and running DOS executables (.COM and .EXE files).

#![allow(clippy::upper_case_acronyms)]

mod bios;
mod bus;
mod cpu;
mod disk;
mod keyboard;
mod video;

use bios::generate_minimal_bios;
use bus::PcBus;
use cpu::{CpuRegisters, PcCpu};
use emu_core::{cpu_8086::Memory8086, types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;
use video::CgaVideo;

pub use bios::BootPriority; // Export boot priority
pub use disk::{create_blank_floppy, create_blank_hard_drive, FloppyFormat, HardDriveFormat}; // Export disk utilities for GUI
pub use keyboard::*; // Export keyboard scancodes for GUI integration

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
    video: CgaVideo,
}

impl Default for PcSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PcSystem {
    /// Create a new PC system
    pub fn new() -> Self {
        let mut bus = PcBus::new();

        // Load minimal BIOS
        let bios = generate_minimal_bios();
        bus.load_bios(&bios);

        let cpu = PcCpu::new(bus);

        Self {
            cpu,
            cycles: 0,
            frame_cycles: 0,
            video: CgaVideo::new(),
        }
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
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // PC runs at ~4.77 MHz
        // At 60 Hz, that's ~79,500 cycles per frame
        const CYCLES_PER_FRAME: u32 = 79500;

        // Ensure boot sector is loaded before first execution
        self.ensure_boot_sector_loaded();

        // Create frame buffer for text mode 80x25 (640x400 pixels)
        let mut frame = Frame::new(640, 400);

        let mut cycles_this_frame = 0u32;

        // Execute until we've completed a frame
        while cycles_this_frame < CYCLES_PER_FRAME {
            let cycles = self.cpu.step();
            cycles_this_frame += cycles;
            self.cycles += cycles as u64;
            self.frame_cycles += cycles as u64;
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
        let regs = self.cpu.get_registers();
        serde_json::json!({
            "version": 1,
            "system": "pc",
            "registers": regs,
            "cycles": self.cycles,
        })
    }

    fn load_state(&mut self, state: &Value) -> Result<(), serde_json::Error> {
        if let Some(regs) = state.get("registers") {
            let regs: CpuRegisters = serde_json::from_value(regs.clone())?;
            self.cpu.set_registers(&regs);
        }

        if let Some(cycles) = state.get("cycles").and_then(|v| v.as_u64()) {
            self.cycles = cycles;
        }

        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
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

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                self.cpu.bus_mut().load_bios(data);
                Ok(())
            }
            "FloppyA" => {
                self.cpu.bus_mut().mount_floppy_a(data.to_vec());
                Ok(())
            }
            "FloppyB" => {
                self.cpu.bus_mut().mount_floppy_b(data.to_vec());
                Ok(())
            }
            "HardDrive" => {
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

        let state = sys.save_state();
        assert_eq!(state["system"], "pc");
        assert_eq!(state["version"], 1);

        let mut sys2 = PcSystem::new();
        assert!(sys2.load_state(&state).is_ok());
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
        assert!(sys.supports_save_states());
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
}
