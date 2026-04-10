//! CPU wrapper for Atari 5200 (6502C variant)

use emu_core::cpu_6502::Cpu6502;
use serde::{Deserialize, Serialize};

use crate::bus::Atari5200Bus;

/// Atari 5200 CPU (6502C - CMOS variant of 6502)
#[derive(Debug, Serialize, Deserialize)]
pub struct Atari5200Cpu {
    #[serde(skip)]
    pub(crate) cpu: Option<Cpu6502<Atari5200Bus>>,
}

impl Atari5200Cpu {
    /// Create a new CPU with the given bus
    pub fn new(bus: Atari5200Bus) -> Self {
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
    pub fn bus(&self) -> Option<&Atari5200Bus> {
        self.cpu.as_ref().map(|cpu| &cpu.memory)
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> Option<&mut Atari5200Bus> {
        self.cpu.as_mut().map(|cpu| &mut cpu.memory)
    }

    /// Replace the bus (used for state loading)
    #[allow(dead_code)]
    pub fn with_bus(mut self, bus: Atari5200Bus) -> Self {
        if let Some(cpu) = self.cpu.take() {
            self.cpu = Some(cpu.with_memory(bus));
        }
        self
    }
}
