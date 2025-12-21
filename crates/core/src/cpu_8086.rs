//! Intel 8086 CPU core implementation
//!
//! This module provides a reusable, generic 8086 CPU implementation that can be used
//! by any system (IBM PC, PC XT, etc.) by implementing the `Memory8086` trait.

/// Memory interface trait for the 8086 CPU
///
/// Systems using the 8086 must implement this trait to provide memory access.
pub trait Memory8086 {
    /// Read a byte from memory at the given address
    fn read(&self, addr: u32) -> u8;

    /// Write a byte to memory at the given address
    fn write(&mut self, addr: u32, val: u8);
}

/// Intel 8086 CPU state and execution engine
///
/// This is a generic, reusable 8086 CPU implementation that works with any
/// system through the `Memory8086` trait.
#[derive(Debug)]
pub struct Cpu8086<M: Memory8086> {
    // General purpose registers (16-bit)
    /// AX register (accumulator) - can be accessed as AH:AL
    pub ax: u16,
    /// BX register (base) - can be accessed as BH:BL
    pub bx: u16,
    /// CX register (count) - can be accessed as CH:CL
    pub cx: u16,
    /// DX register (data) - can be accessed as DH:DL
    pub dx: u16,

    // Index and pointer registers
    /// SI register (source index)
    pub si: u16,
    /// DI register (destination index)
    pub di: u16,
    /// BP register (base pointer)
    pub bp: u16,
    /// SP register (stack pointer)
    pub sp: u16,

    // Segment registers
    /// CS register (code segment)
    pub cs: u16,
    /// DS register (data segment)
    pub ds: u16,
    /// ES register (extra segment)
    pub es: u16,
    /// SS register (stack segment)
    pub ss: u16,

    // Control registers
    /// IP register (instruction pointer)
    pub ip: u16,
    /// FLAGS register (status flags)
    pub flags: u16,

    /// Total cycles executed
    pub cycles: u64,

    /// Memory interface
    pub memory: M,

    /// Halt flag
    halted: bool,
}

// Flag bit positions in FLAGS register
const FLAG_CF: u16 = 0x0001; // Carry Flag
const FLAG_PF: u16 = 0x0004; // Parity Flag
#[allow(dead_code)]
const FLAG_AF: u16 = 0x0010; // Auxiliary Carry Flag
const FLAG_ZF: u16 = 0x0040; // Zero Flag
const FLAG_SF: u16 = 0x0080; // Sign Flag
#[allow(dead_code)]
const FLAG_TF: u16 = 0x0100; // Trap Flag
const FLAG_IF: u16 = 0x0200; // Interrupt Enable Flag
const FLAG_DF: u16 = 0x0400; // Direction Flag
const FLAG_OF: u16 = 0x0800; // Overflow Flag

impl<M: Memory8086> Cpu8086<M> {
    /// Create a new 8086 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self {
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            bp: 0,
            sp: 0,
            cs: 0xFFFF,
            ds: 0,
            es: 0,
            ss: 0,
            ip: 0,
            flags: 0x0002, // Reserved bit 1 is always set
            cycles: 0,
            memory,
            halted: false,
        }
    }

    /// Reset the CPU to initial state (preserves memory)
    pub fn reset(&mut self) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.bp = 0;
        self.sp = 0;
        self.cs = 0xFFFF;
        self.ds = 0;
        self.es = 0;
        self.ss = 0;
        self.ip = 0;
        self.flags = 0x0002;
        self.cycles = 0;
        self.halted = false;
    }

    /// Calculate physical address from segment:offset
    #[inline]
    fn physical_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + (offset as u32)
    }

    /// Read a byte from memory using segment:offset
    #[inline]
    fn read(&self, segment: u16, offset: u16) -> u8 {
        let addr = Self::physical_address(segment, offset);
        self.memory.read(addr)
    }

    /// Write a byte to memory using segment:offset
    #[inline]
    fn write(&mut self, segment: u16, offset: u16, val: u8) {
        let addr = Self::physical_address(segment, offset);
        self.memory.write(addr, val);
    }

    /// Read a byte from code segment at IP
    #[inline]
    fn fetch_u8(&mut self) -> u8 {
        let val = self.read(self.cs, self.ip);
        self.ip = self.ip.wrapping_add(1);
        val
    }

    /// Read a word (16-bit) from code segment at IP
    #[inline]
    fn fetch_u16(&mut self) -> u16 {
        // 8086 is little-endian: fetch low byte first, then high byte
        let low_byte = self.fetch_u8() as u16;
        let high_byte = self.fetch_u8() as u16;
        (high_byte << 8) | low_byte
    }

    /// Read a word from memory at segment:offset
    #[inline]
    fn read_u16(&self, segment: u16, offset: u16) -> u16 {
        // 8086 is little-endian: read low byte at offset, then high byte at offset + 1
        let low_byte = self.read(segment, offset) as u16;
        let high_byte = self.read(segment, offset.wrapping_add(1)) as u16;
        (high_byte << 8) | low_byte
    }

    /// Write a word to memory at segment:offset
    #[inline]
    fn write_u16(&mut self, segment: u16, offset: u16, val: u16) {
        let lo = (val & 0xFF) as u8;
        let hi = ((val >> 8) & 0xFF) as u8;
        self.write(segment, offset, lo);
        self.write(segment, offset.wrapping_add(1), hi);
    }

    /// Push a word onto the stack
    #[inline]
    fn push(&mut self, val: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.write_u16(self.ss, self.sp, val);
    }

    /// Pop a word from the stack
    #[inline]
    fn pop(&mut self) -> u16 {
        let val = self.read_u16(self.ss, self.sp);
        self.sp = self.sp.wrapping_add(2);
        val
    }

    /// Get 8-bit high register
    #[inline]
    #[allow(dead_code)]
    fn get_reg8_high(&self, reg: u8) -> u8 {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit high register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => (self.ax >> 8) as u8, // AH
            1 => (self.cx >> 8) as u8, // CH
            2 => (self.dx >> 8) as u8, // DH
            3 => (self.bx >> 8) as u8, // BH
            _ => unreachable!(),
        }
    }

    /// Get 8-bit low register
    #[inline]
    #[allow(dead_code)]
    fn get_reg8_low(&self, reg: u8) -> u8 {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit low register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => (self.ax & 0xFF) as u8, // AL
            1 => (self.cx & 0xFF) as u8, // CL
            2 => (self.dx & 0xFF) as u8, // DL
            3 => (self.bx & 0xFF) as u8, // BL
            _ => unreachable!(),
        }
    }

    /// Set 8-bit high register
    #[inline]
    fn set_reg8_high(&mut self, reg: u8, val: u8) {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit high register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => self.ax = (self.ax & 0x00FF) | ((val as u16) << 8), // AH
            1 => self.cx = (self.cx & 0x00FF) | ((val as u16) << 8), // CH
            2 => self.dx = (self.dx & 0x00FF) | ((val as u16) << 8), // DH
            3 => self.bx = (self.bx & 0x00FF) | ((val as u16) << 8), // BH
            _ => unreachable!(),
        }
    }

    /// Set 8-bit low register
    #[inline]
    fn set_reg8_low(&mut self, reg: u8, val: u8) {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit low register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => self.ax = (self.ax & 0xFF00) | (val as u16), // AL
            1 => self.cx = (self.cx & 0xFF00) | (val as u16), // CL
            2 => self.dx = (self.dx & 0xFF00) | (val as u16), // DL
            3 => self.bx = (self.bx & 0xFF00) | (val as u16), // BL
            _ => unreachable!(),
        }
    }

    /// Get 16-bit register
    #[inline]
    fn get_reg16(&self, reg: u8) -> u16 {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    /// Set 16-bit register
    #[inline]
    fn set_reg16(&mut self, reg: u8, val: u16) {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => self.ax = val,
            1 => self.cx = val,
            2 => self.dx = val,
            3 => self.bx = val,
            4 => self.sp = val,
            5 => self.bp = val,
            6 => self.si = val,
            7 => self.di = val,
            _ => unreachable!(),
        }
    }

    /// Get segment register
    #[inline]
    #[allow(dead_code)]
    fn get_seg(&self, seg: u8) -> u16 {
        match seg {
            0 => self.es,
            1 => self.cs,
            2 => self.ss,
            3 => self.ds,
            _ => panic!("Invalid segment register index: {} (must be 0-3)", seg),
        }
    }

    /// Set segment register
    #[inline]
    #[allow(dead_code)]
    fn set_seg(&mut self, seg: u8, val: u16) {
        match seg {
            0 => self.es = val,
            1 => self.cs = val,
            2 => self.ss = val,
            3 => self.ds = val,
            _ => panic!("Invalid segment register index: {} (must be 0-3)", seg),
        }
    }

    /// Set flag
    #[inline]
    fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get flag
    #[inline]
    fn get_flag(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    /// Calculate parity (true if even number of 1 bits in low byte)
    #[inline]
    fn calc_parity(val: u8) -> bool {
        val.count_ones() % 2 == 0
    }

    /// Update flags after 8-bit arithmetic/logic operation
    fn update_flags_8(&mut self, result: u8) {
        self.set_flag(FLAG_ZF, result == 0);
        self.set_flag(FLAG_SF, (result & 0x80) != 0);
        self.set_flag(FLAG_PF, Self::calc_parity(result));
    }

    /// Update flags after 16-bit arithmetic/logic operation
    fn update_flags_16(&mut self, result: u16) {
        self.set_flag(FLAG_ZF, result == 0);
        self.set_flag(FLAG_SF, (result & 0x8000) != 0);
        self.set_flag(FLAG_PF, Self::calc_parity((result & 0xFF) as u8));
    }

    /// Execute one instruction and return cycles used
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 1;
        }

        let opcode = self.fetch_u8();

        match opcode {
            // NOP
            0x90 => {
                self.cycles += 3;
                3
            }

            // HLT
            0xF4 => {
                self.halted = true;
                self.cycles += 2;
                2
            }

            // MOV reg8, imm8 (B0-B7)
            0xB0..=0xB7 => {
                let reg = opcode & 0x07;
                let val = self.fetch_u8();
                if reg < 4 {
                    self.set_reg8_low(reg, val);
                } else {
                    self.set_reg8_high(reg - 4, val);
                }
                self.cycles += 4;
                4
            }

            // MOV reg16, imm16 (B8-BF)
            0xB8..=0xBF => {
                let reg = opcode & 0x07;
                let val = self.fetch_u16();
                self.set_reg16(reg, val);
                self.cycles += 4;
                4
            }

            // ADD AL, imm8
            0x04 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_add(val);
                let carry = (al as u16 + val as u16) > 0xFF;
                let overflow = ((al ^ result) & (val ^ result) & 0x80) != 0;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // ADD AX, imm16
            0x05 => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_add(val);
                let carry = (self.ax as u32 + val as u32) > 0xFFFF;
                let overflow = ((self.ax ^ result) & (val ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // SUB AL, imm8
            0x2C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // SUB AX, imm16
            0x2D => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_sub(val);
                let borrow = (self.ax as u32) < (val as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // CMP AL, imm8
            0x3C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // CMP AX, imm16
            0x3D => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_sub(val);
                let borrow = (self.ax as u32) < (val as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // AND AL, imm8
            0x24 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al & val;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // AND AX, imm16
            0x25 => {
                let val = self.fetch_u16();
                let result = self.ax & val;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // OR AL, imm8
            0x0C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al | val;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // OR AX, imm16
            0x0D => {
                let val = self.fetch_u16();
                let result = self.ax | val;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // XOR AL, imm8
            0x34 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al ^ val;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // XOR AX, imm16
            0x35 => {
                let val = self.fetch_u16();
                let result = self.ax ^ val;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // INC reg16 (40-47)
            // Note: INC does not affect the Carry Flag (CF), only OF/SF/ZF/AF/PF
            0x40..=0x47 => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                let result = val.wrapping_add(1);
                let overflow = val == 0x7FFF;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 2;
                2
            }

            // DEC reg16 (48-4F)
            // Note: DEC does not affect the Carry Flag (CF), only OF/SF/ZF/AF/PF
            0x48..=0x4F => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                let result = val.wrapping_sub(1);
                let overflow = val == 0x8000;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 2;
                2
            }

            // PUSH reg16 (50-57)
            0x50..=0x57 => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                self.push(val);
                self.cycles += 11;
                11
            }

            // POP reg16 (58-5F)
            0x58..=0x5F => {
                let reg = opcode & 0x07;
                let val = self.pop();
                self.set_reg16(reg, val);
                self.cycles += 8;
                8
            }

            // JMP short (EB)
            0xEB => {
                let offset = self.fetch_u8() as i8;
                // Add signed offset to IP (wrapping_add_signed would be clearer but requires i16 cast)
                self.ip = self.ip.wrapping_add(offset as i16 as u16);
                self.cycles += 15;
                15
            }

            // JZ/JE (74) - Jump if Zero
            0x74 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_ZF) {
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNZ/JNE (75) - Jump if Not Zero
            0x75 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_ZF) {
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JC/JB (72) - Jump if Carry
            0x72 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_CF) {
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNC/JAE (73) - Jump if Not Carry
            0x73 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_CF) {
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // CLC - Clear Carry Flag
            0xF8 => {
                self.set_flag(FLAG_CF, false);
                self.cycles += 2;
                2
            }

            // STC - Set Carry Flag
            0xF9 => {
                self.set_flag(FLAG_CF, true);
                self.cycles += 2;
                2
            }

            // CLI - Clear Interrupt Flag
            0xFA => {
                self.set_flag(FLAG_IF, false);
                self.cycles += 2;
                2
            }

            // STI - Set Interrupt Flag
            0xFB => {
                self.set_flag(FLAG_IF, true);
                self.cycles += 2;
                2
            }

            // CLD - Clear Direction Flag
            0xFC => {
                self.set_flag(FLAG_DF, false);
                self.cycles += 2;
                2
            }

            // STD - Set Direction Flag
            0xFD => {
                self.set_flag(FLAG_DF, true);
                self.cycles += 2;
                2
            }

            _ => {
                // Unknown/unimplemented opcode
                eprintln!(
                    "Unknown 8086 opcode: 0x{:02X} at CS:IP={:04X}:{:04X}",
                    opcode,
                    self.cs,
                    self.ip.wrapping_sub(1)
                );
                self.cycles += 1;
                1
            }
        }
    }
}

/// Simple array-based memory for testing
pub struct ArrayMemory {
    data: Vec<u8>,
}

impl ArrayMemory {
    pub fn new() -> Self {
        Self {
            data: vec![0; 0x100000], // 1MB of memory
        }
    }

    /// Load a program at a specific physical address
    pub fn load_program(&mut self, addr: u32, program: &[u8]) {
        let start = addr as usize;
        let end = start + program.len();
        if end <= self.data.len() {
            self.data[start..end].copy_from_slice(program);
        }
    }
}

impl Default for ArrayMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory8086 for ArrayMemory {
    fn read(&self, addr: u32) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            0xFF
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_initialization() {
        let mem = ArrayMemory::new();
        let cpu = Cpu8086::new(mem);

        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.bx, 0);
        assert_eq!(cpu.cx, 0);
        assert_eq!(cpu.dx, 0);
        assert_eq!(cpu.cs, 0xFFFF);
        assert_eq!(cpu.ds, 0);
        assert_eq!(cpu.flags & 0x0002, 0x0002); // Reserved bit
    }

    #[test]
    fn test_reset() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ax = 0x1234;
        cpu.bx = 0x5678;
        cpu.flags = 0xFFFF;

        cpu.reset();

        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.bx, 0);
        assert_eq!(cpu.flags & 0x0002, 0x0002);
    }

    #[test]
    fn test_physical_address() {
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0234);
        assert_eq!(addr, 0x10234);

        let addr = Cpu8086::<ArrayMemory>::physical_address(0xFFFF, 0xFFFF);
        assert_eq!(addr, 0x10FFEF);
    }

    #[test]
    fn test_mov_immediate_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AL, 0x42
        cpu.memory.load_program(0xFFFF0, &[0xB0, 0x42]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ax & 0xFF, 0x42);
        assert_eq!((cpu.ax >> 8) & 0xFF, 0);
    }

    #[test]
    fn test_mov_immediate_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AX, 0x1234
        cpu.memory.load_program(0xFFFF0, &[0xB8, 0x34, 0x12]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ax, 0x1234);
    }

    #[test]
    fn test_add_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD AL, 0x10
        cpu.memory.load_program(0xFFFF0, &[0x04, 0x10]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x15);
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_add_with_carry() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD AL, 0xFF (0xFF + 0xFF = 0x1FE, should set carry)
        cpu.memory.load_program(0xFFFF0, &[0x04, 0xFF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xFE);
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sub_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SUB AL, 0x05
        cpu.memory.load_program(0xFFFF0, &[0x2C, 0x05]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x0B);
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sub_with_borrow() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SUB AL, 0x10 (0x05 - 0x10, should set carry/borrow)
        cpu.memory.load_program(0xFFFF0, &[0x2C, 0x10]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xF5);
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_cmp_sets_flags() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CMP AL, 0x42 (should set zero flag when equal)
        cpu.memory.load_program(0xFFFF0, &[0x3C, 0x42]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0042;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x42); // CMP doesn't modify register
        assert!(cpu.get_flag(FLAG_ZF));
    }

    #[test]
    fn test_and_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // AND AL, 0x0F
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x0F]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x0F);
        assert!(!cpu.get_flag(FLAG_CF));
        assert!(!cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_or_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // OR AL, 0xF0
        cpu.memory.load_program(0xFFFF0, &[0x0C, 0xF0]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x000F;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xFF);
    }

    #[test]
    fn test_xor_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // XOR AL, 0xFF
        cpu.memory.load_program(0xFFFF0, &[0x34, 0xFF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00AA;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x55);
    }

    #[test]
    fn test_inc_dec_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // INC AX
        cpu.memory.load_program(0xFFFF0, &[0x40]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010;

        cpu.step();
        assert_eq!(cpu.ax, 0x0011);

        // DEC BX
        cpu.memory.load_program(0xFFFF0, &[0x4B]);
        cpu.ip = 0x0000;
        cpu.bx = 0x0010;

        cpu.step();
        assert_eq!(cpu.bx, 0x000F);
    }

    #[test]
    fn test_push_pop() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);
        cpu.ss = 0x1000;
        cpu.sp = 0x0100;

        // PUSH AX
        cpu.memory.load_program(0xFFFF0, &[0x50]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x1234;

        cpu.step();
        assert_eq!(cpu.sp, 0x00FE);

        // POP BX
        cpu.memory.load_program(0xFFFF0, &[0x5B]);
        cpu.ip = 0x0000;

        cpu.step();
        assert_eq!(cpu.bx, 0x1234);
        assert_eq!(cpu.sp, 0x0100);
    }

    #[test]
    fn test_jump_short() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JMP short +5
        cpu.memory.load_program(0xFFFF0, &[0xEB, 0x05]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ip, 0x0007); // 2 (instruction size) + 5 (offset)
    }

    #[test]
    fn test_conditional_jump_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JZ +3 (should jump when ZF is set)
        cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_ZF, true);

        let cycles = cpu.step();
        assert_eq!(cycles, 16);
        assert_eq!(cpu.ip, 0x0005); // 2 + 3
    }

    #[test]
    fn test_conditional_jump_not_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JZ +3 (should not jump when ZF is clear)
        cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_ZF, false);

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ip, 0x0002); // Just past instruction
    }

    #[test]
    fn test_flag_instructions() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CLC
        cpu.memory.load_program(0xFFFF0, &[0xF8]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_CF, true);

        cpu.step();
        assert!(!cpu.get_flag(FLAG_CF));

        // STC
        cpu.memory.load_program(0xFFFF0, &[0xF9]);
        cpu.ip = 0x0000;

        cpu.step();
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_nop() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.memory.load_program(0xFFFF0, &[0x90]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        let old_ip = cpu.ip;

        let cycles = cpu.step();
        assert_eq!(cycles, 3);
        assert_eq!(cpu.ip, old_ip + 1);
    }

    #[test]
    fn test_halt() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.memory.load_program(0xFFFF0, &[0xF4]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert!(cpu.halted);

        // Further steps should do nothing
        let cycles = cpu.step();
        assert_eq!(cycles, 1);
    }

    #[test]
    fn test_parity_flag() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // AND AL, 0x03 (result = 0x03, has 2 ones = even parity)
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert!(cpu.get_flag(FLAG_PF));

        // AND AL, 0x01 (result = 0x01, has 1 one = odd parity)
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x01]);
        cpu.ip = 0x0000;
        cpu.ax = 0x00FF;

        cpu.step();
        assert!(!cpu.get_flag(FLAG_PF));
    }
}
