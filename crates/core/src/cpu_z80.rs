//! Zilog Z80 CPU core implementation
//!
//! The Z80 extends the 8080 with additional registers and instructions.
//! This module provides a reusable Z80 implementation.

/// Memory interface trait for the Z80 CPU
pub trait MemoryZ80 {
    /// Read a byte from memory
    fn read(&self, addr: u16) -> u8;

    /// Write a byte to memory
    fn write(&mut self, addr: u16, val: u8);

    /// Read from I/O port
    fn io_read(&mut self, port: u8) -> u8 {
        let _ = port;
        0xFF
    }

    /// Write to I/O port
    fn io_write(&mut self, port: u8, val: u8) {
        let _ = (port, val);
    }
}

/// Zilog Z80 CPU state
#[derive(Debug)]
pub struct CpuZ80<M: MemoryZ80> {
    /// Main registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    /// Shadow registers (Z80 specific)
    pub a_prime: u8,
    pub f_prime: u8,
    pub b_prime: u8,
    pub c_prime: u8,
    pub d_prime: u8,
    pub e_prime: u8,
    pub h_prime: u8,
    pub l_prime: u8,

    /// Index registers (Z80 specific)
    pub ix: u16,
    pub iy: u16,

    /// Special registers
    pub i: u8, // Interrupt vector
    pub r: u8, // Memory refresh

    /// Stack pointer
    pub sp: u16,
    /// Program counter
    pub pc: u16,

    /// Interrupt flags
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8, // Interrupt mode (0, 1, or 2)

    /// State
    pub halted: bool,
    pub cycles: u64,

    /// Memory interface
    pub memory: M,
}

impl<M: MemoryZ80> CpuZ80<M> {
    /// Create a new Z80 CPU
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
            a_prime: 0,
            f_prime: 0,
            b_prime: 0,
            c_prime: 0,
            d_prime: 0,
            e_prime: 0,
            h_prime: 0,
            l_prime: 0,
            ix: 0,
            iy: 0,
            i: 0,
            r: 0,
            sp: 0,
            pc: 0,
            iff1: false,
            iff2: false,
            im: 0,
            halted: false,
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
        self.pc = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
        self.cycles = 0;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 4;
        }

        let opcode = self.read_pc();
        self.execute(opcode)
    }

    fn read_pc(&mut self) -> u8 {
        let val = self.memory.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        self.r = self.r.wrapping_add(1);
        val
    }

    fn execute(&mut self, opcode: u8) -> u32 {
        match opcode {
            0x00 => 4, // NOP
            0x76 => {
                self.halted = true;
                4
            } // HALT
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
                4
            } // DI
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                4
            } // EI
            _ => 4,    // Placeholder - stub implementation
        }
    }
}

impl<M: MemoryZ80> crate::Cpu for CpuZ80<M> {
    fn reset(&mut self) {
        self.reset();
    }

    fn step(&mut self) -> u32 {
        self.step()
    }
}
