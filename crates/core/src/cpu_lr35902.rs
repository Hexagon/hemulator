//! Sharp LR35902 CPU core (Game Boy CPU)
//!
//! This is a Z80-like CPU used in Game Boy and Game Boy Color.
//! It's similar to Z80 but with some instructions removed and others modified.

/// Memory interface trait for the LR35902 CPU
pub trait MemoryLr35902 {
    /// Read a byte from memory
    fn read(&self, addr: u16) -> u8;
    
    /// Write a byte to memory
    fn write(&mut self, addr: u16, val: u8);
}

/// Sharp LR35902 CPU state
#[derive(Debug)]
pub struct CpuLr35902<M: MemoryLr35902> {
    /// Accumulator & Flags (combined as AF)
    pub a: u8,
    pub f: u8,
    /// BC register pair
    pub b: u8,
    pub c: u8,
    /// DE register pair
    pub d: u8,
    pub e: u8,
    /// HL register pair
    pub h: u8,
    pub l: u8,
    /// Stack pointer
    pub sp: u16,
    /// Program counter
    pub pc: u16,
    /// Interrupt Master Enable flag
    pub ime: bool,
    /// Halted state
    pub halted: bool,
    /// Stopped state (for STOP instruction)
    pub stopped: bool,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
}

// Flag bit positions (in F register)
const FLAG_Z: u8 = 0b10000000; // Zero
const FLAG_N: u8 = 0b01000000; // Subtract (BCD)
const FLAG_H: u8 = 0b00100000; // Half Carry (BCD)
const FLAG_C: u8 = 0b00010000; // Carry

impl<M: MemoryLr35902> CpuLr35902<M> {
    /// Create a new LR35902 CPU
    pub fn new(memory: M) -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
            ime: false,
            halted: false,
            stopped: false,
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU
    pub fn reset(&mut self) {
        self.a = 0;
        self.f = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.sp = 0;
        self.pc = 0x100; // Game Boy starts at 0x100
        self.ime = false;
        self.halted = false;
        self.stopped = false;
        self.cycles = 0;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        if self.halted || self.stopped {
            return 4;
        }

        let opcode = self.read_pc();
        self.execute(opcode)
    }

    fn read_pc(&mut self) -> u8 {
        let val = self.memory.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn execute(&mut self, opcode: u8) -> u32 {
        match opcode {
            0x00 => 4, // NOP
            0x76 => { self.halted = true; 4 } // HALT
            0x10 => { self.stopped = true; 4 } // STOP
            0xF3 => { self.ime = false; 4 } // DI
            0xFB => { self.ime = true; 4 } // EI
            _ => 4, // Placeholder - stub implementation
        }
    }
}

impl<M: MemoryLr35902> crate::Cpu for CpuLr35902<M> {
    fn reset(&mut self) {
        self.reset();
    }

    fn step(&mut self) -> u32 {
        self.step()
    }
}
