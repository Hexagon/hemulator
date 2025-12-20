//! IBM PC/XT system implementation
//!
//! This module provides a basic IBM PC/XT compatible emulator using the 8086 CPU core.
//! It supports loading and running DOS executables (.COM and .EXE files).

#![allow(clippy::upper_case_acronyms)]

mod bios;
mod bus;
mod cpu;

use bios::generate_minimal_bios;
use bus::PcBus;
use cpu::{CpuRegisters, PcCpu};
use emu_core::{cpu_8086::Memory8086, types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

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
        }
    }

    /// Load a DOS executable into memory
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

        // Create a simple frame buffer (text mode 80x25, scaled to 640x400 for display)
        let mut frame = Frame::new(640, 400);

        // Fill with black background
        frame.pixels.fill(0xFF000000);

        let mut cycles_this_frame = 0u32;

        // Execute until we've completed a frame
        while cycles_this_frame < CYCLES_PER_FRAME {
            let cycles = self.cpu.step();
            cycles_this_frame += cycles;
            self.cycles += cycles as u64;
            self.frame_cycles += cycles as u64;
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
        vec![MountPointInfo {
            id: "Executable".to_string(),
            name: "DOS Executable".to_string(),
            extensions: vec!["com".to_string(), "exe".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Executable" {
            return Err(PcError::InvalidMountPoint(mount_point_id.to_string()));
        }

        self.load_executable(data)?;
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Executable" {
            return Err(PcError::InvalidMountPoint(mount_point_id.to_string()));
        }

        // Reset the system when unmounting
        self.reset();
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Executable" && self.cpu.bus().executable().is_some()
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

        assert!(sys.load_executable(&program).is_ok());
        assert!(sys.is_mounted("Executable"));
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

        assert_eq!(mps.len(), 1);
        assert_eq!(mps[0].id, "Executable");
        assert!(mps[0].required);
        assert!(mps[0].extensions.contains(&"com".to_string()));
        assert!(mps[0].extensions.contains(&"exe".to_string()));
    }

    #[test]
    fn test_invalid_mount_point() {
        let mut sys = PcSystem::new();
        let result = sys.mount("InvalidMount", &[]);
        assert!(result.is_err());
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
}
