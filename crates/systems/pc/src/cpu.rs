//! PC CPU wrapper
//!
//! This module wraps the core 8086 CPU with PC-specific initialization and state.

use crate::bus::PcBus;
use emu_core::cpu_8086::Cpu8086;

/// PC CPU wrapper
pub struct PcCpu {
    cpu: Cpu8086<PcBus>,
}

impl PcCpu {
    /// Create a new PC CPU with the given bus
    pub fn new(bus: PcBus) -> Self {
        let mut cpu = Cpu8086::new(bus);

        // IBM PC/XT boots at CS:IP = 0xFFFF:0x0000 (physical address 0xFFFF0)
        // This is the BIOS entry point
        cpu.cs = 0xFFFF;
        cpu.ip = 0x0000;

        // Initialize stack pointer
        cpu.ss = 0x0000;
        cpu.sp = 0xFFFE;

        // Initialize data segments
        cpu.ds = 0x0000;
        cpu.es = 0x0000;

        Self { cpu }
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.cpu.reset();

        // Restore PC boot state
        self.cpu.cs = 0xFFFF;
        self.cpu.ip = 0x0000;
        self.cpu.ss = 0x0000;
        self.cpu.sp = 0xFFFE;
        self.cpu.ds = 0x0000;
        self.cpu.es = 0x0000;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        self.cpu.step()
    }

    /// Get a reference to the bus
    pub fn bus(&self) -> &PcBus {
        &self.cpu.memory
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> &mut PcBus {
        &mut self.cpu.memory
    }

    /// Get CPU register state for debugging/save states
    pub fn get_registers(&self) -> CpuRegisters {
        CpuRegisters {
            ax: self.cpu.ax,
            bx: self.cpu.bx,
            cx: self.cpu.cx,
            dx: self.cpu.dx,
            si: self.cpu.si,
            di: self.cpu.di,
            bp: self.cpu.bp,
            sp: self.cpu.sp,
            cs: self.cpu.cs,
            ds: self.cpu.ds,
            es: self.cpu.es,
            ss: self.cpu.ss,
            ip: self.cpu.ip,
            flags: self.cpu.flags,
        }
    }

    /// Set CPU register state (for loading save states)
    pub fn set_registers(&mut self, regs: &CpuRegisters) {
        self.cpu.ax = regs.ax;
        self.cpu.bx = regs.bx;
        self.cpu.cx = regs.cx;
        self.cpu.dx = regs.dx;
        self.cpu.si = regs.si;
        self.cpu.di = regs.di;
        self.cpu.bp = regs.bp;
        self.cpu.sp = regs.sp;
        self.cpu.cs = regs.cs;
        self.cpu.ds = regs.ds;
        self.cpu.es = regs.es;
        self.cpu.ss = regs.ss;
        self.cpu.ip = regs.ip;
        self.cpu.flags = regs.flags;
    }

    /// Get total cycles executed
    #[allow(dead_code)]
    pub fn cycles(&self) -> u64 {
        self.cpu.cycles
    }
}

/// CPU register state for save/load
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuRegisters {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub sp: u16,
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub ip: u16,
    pub flags: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_initialization() {
        let bus = PcBus::new();
        let cpu = PcCpu::new(bus);

        // Check PC boot state
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000);
        assert_eq!(cpu.cpu.ss, 0x0000);
        assert_eq!(cpu.cpu.sp, 0xFFFE);
    }

    #[test]
    fn test_cpu_reset() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Modify some registers
        cpu.cpu.ax = 0x1234;
        cpu.cpu.cs = 0x0100;

        cpu.reset();

        // Should be back to boot state
        assert_eq!(cpu.cpu.ax, 0x0000);
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000);
    }

    #[test]
    fn test_register_save_load() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        cpu.cpu.ax = 0x1234;
        cpu.cpu.bx = 0x5678;
        cpu.cpu.cs = 0xABCD;

        let regs = cpu.get_registers();
        assert_eq!(regs.ax, 0x1234);
        assert_eq!(regs.bx, 0x5678);
        assert_eq!(regs.cs, 0xABCD);

        cpu.reset();
        assert_eq!(cpu.cpu.ax, 0x0000);

        cpu.set_registers(&regs);
        assert_eq!(cpu.cpu.ax, 0x1234);
        assert_eq!(cpu.cpu.bx, 0x5678);
        assert_eq!(cpu.cpu.cs, 0xABCD);
    }
}
