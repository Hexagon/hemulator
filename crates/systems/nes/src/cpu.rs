//! NES CPU wrapper around the reusable 6502 core

use crate::bus::Bus;
use emu_core::cpu_6502::{Cpu6502, Memory6502};

/// NES-specific memory implementation that uses NES bus or fallback array
#[derive(Debug)]
pub enum NesMemory {
    /// Simple array-based memory for testing
    Array([u8; 0x10000]),
    /// NES bus with PPU, APU, mappers, etc.
    Bus(Box<crate::bus::NesBus>),
}

impl NesMemory {
    pub fn new_array() -> Self {
        Self::Array([0; 0x10000])
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
        // Preserve CPU state
        let a = self.cpu.a;
        let x = self.cpu.x;
        let y = self.cpu.y;
        let sp = self.cpu.sp;
        let status = self.cpu.status;
        let pc = self.cpu.pc;
        let cycles = self.cpu.cycles;

        // Create new CPU with bus
        let mem = NesMemory::new_bus(bus);
        self.cpu = Cpu6502::new(mem);

        // Restore CPU state
        self.cpu.a = a;
        self.cpu.x = x;
        self.cpu.y = y;
        self.cpu.sp = sp;
        self.cpu.status = status;
        self.cpu.pc = pc;
        self.cpu.cycles = cycles;
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
    pub fn x(&self) -> u8 {
        self.cpu.x
    }
    pub fn y(&self) -> u8 {
        self.cpu.y
    }
    pub fn sp(&self) -> u8 {
        self.cpu.sp
    }
    pub fn status(&self) -> u8 {
        self.cpu.status
    }
    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }
    pub fn cycles(&self) -> u64 {
        self.cpu.cycles
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
