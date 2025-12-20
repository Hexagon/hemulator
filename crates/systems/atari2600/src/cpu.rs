//! CPU wrapper for Atari 2600 (6507 variant of 6502)

use emu_core::cpu_6502::Cpu6502;
use serde::{Deserialize, Serialize};

use crate::bus::Atari2600Bus;

/// Atari 2600 CPU (6507 - 6502 variant with 13-bit address bus)
#[derive(Debug, Serialize, Deserialize)]
pub struct Atari2600Cpu {
    #[serde(skip)]
    cpu: Option<Cpu6502<Atari2600Bus>>,
}

impl Atari2600Cpu {
    /// Create a new CPU with the given bus
    pub fn new(bus: Atari2600Bus) -> Self {
        Self {
            cpu: Some(Cpu6502::new(bus)),
        }
    }

    /// Reset the CPU
    pub fn reset(&mut self) {
        if let Some(cpu) = &mut self.cpu {
            cpu.reset();
        }
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        if let Some(cpu) = &mut self.cpu {
            cpu.step()
        } else {
            0
        }
    }

    /// Get a reference to the bus
    pub fn bus(&self) -> Option<&Atari2600Bus> {
        self.cpu.as_ref().map(|cpu| &cpu.memory)
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> Option<&mut Atari2600Bus> {
        self.cpu.as_mut().map(|cpu| &mut cpu.memory)
    }

    /// Replace the bus
    #[allow(dead_code)]
    pub fn with_bus(mut self, bus: Atari2600Bus) -> Self {
        if let Some(cpu) = self.cpu.take() {
            self.cpu = Some(cpu.with_memory(bus));
        }
        self
    }
}
