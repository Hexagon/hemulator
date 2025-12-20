//! NES CPU wrapper around the reusable 6502 core

use crate::bus::Bus;
use emu_core::cpu_6502::{Cpu6502, Memory6502};

/// NES-specific memory implementation that uses NES bus or fallback array
#[derive(Debug)]
pub enum NesMemory {
    /// Simple array-based memory for testing
    Array(Box<[u8; 0x10000]>),
    /// NES bus with PPU, APU, mappers, etc.
    Bus(Box<crate::bus::NesBus>),
}

impl NesMemory {
    pub fn new_array() -> Self {
        Self::Array(Box::new([0; 0x10000]))
    }

    pub fn new_bus(bus: crate::bus::NesBus) -> Self {
        Self::Bus(Box::new(bus))
    }

    pub fn bus(&self) -> Option<&crate::bus::NesBus> {
        match self {
            Self::Bus(b) => Some(b),
            _ => None,
        }
    }

    pub fn bus_mut(&mut self) -> Option<&mut crate::bus::NesBus> {
        match self {
            Self::Bus(b) => Some(b),
            _ => None,
        }
    }
}

impl Memory6502 for NesMemory {
    fn read(&self, addr: u16) -> u8 {
        match self {
            Self::Array(mem) => mem[addr as usize],
            Self::Bus(bus) => bus.read(addr),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match self {
            Self::Array(mem) => mem[addr as usize] = val,
            Self::Bus(bus) => bus.write(addr, val),
        }
    }
}

/// NES CPU - wrapper around the reusable 6502 core
#[derive(Debug)]
pub struct NesCpu {
    cpu: Cpu6502<NesMemory>,
}

impl NesCpu {
    pub fn new() -> Self {
        let mem = NesMemory::new_array();
        Self {
            cpu: Cpu6502::new(mem),
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    /// Set the NES bus (PPU, APU, mappers, etc.)
    pub fn set_bus(&mut self, bus: crate::bus::NesBus) {
        // Replace memory while preserving CPU state
        let mem = NesMemory::new_bus(bus);
        // Take ownership of cpu, swap memory, put it back
        let old_cpu = std::mem::replace(&mut self.cpu, Cpu6502::new(NesMemory::new_array()));
        self.cpu = old_cpu.with_memory(mem);
    }

    /// Get reference to bus if available
    pub fn bus(&self) -> Option<&crate::bus::NesBus> {
        self.cpu.memory.bus()
    }

    /// Get mutable reference to bus if available
    pub fn bus_mut(&mut self) -> Option<&mut crate::bus::NesBus> {
        self.cpu.memory.bus_mut()
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        self.cpu.step()
    }

    /// Trigger NMI
    pub fn trigger_nmi(&mut self) {
        self.cpu.trigger_nmi();
    }

    /// Trigger IRQ
    pub fn trigger_irq(&mut self) {
        self.cpu.trigger_irq();
    }

    // Public accessors for CPU state (used by NES system)
    pub fn a(&self) -> u8 {
        self.cpu.a
    }
    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    // Mutable accessors (used by NES system for initialization)
    pub fn set_pc(&mut self, pc: u16) {
        self.cpu.pc = pc;
    }
}

impl Default for NesCpu {
    fn default() -> Self {
        Self::new()
    }
}
