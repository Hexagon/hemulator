// Minimal 6502-like CPU implementation for NES skeleton

#[derive(Debug)]
pub struct NesCpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
    pub cycles: u64,
    pub memory: [u8; 0x10000],
}

impl NesCpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            status: 0x24,
            pc: 0x8000,
            cycles: 0,
            memory: [0; 0x10000],
        }
    }

    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = 0x24;
        self.pc = 0x8000;
        self.cycles = 0;
    }

    pub fn load_program(&mut self, offset: u16, data: &[u8]) {
        let off = offset as usize;
        self.memory[off..off + data.len()].copy_from_slice(data);
        // set reset vector to offset
        let lo = (offset & 0xFF) as u8;
        let hi = ((offset >> 8) & 0xFF) as u8;
        self.memory[0xFFFC] = lo;
        self.memory[0xFFFD] = hi;
        self.pc = offset;
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.memory[addr as usize] = val;
    }

    fn read_u16(&self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn set_zero_and_negative(&mut self, v: u8) {
        if v == 0 {
            self.status |= 0x02; // Z
        } else {
            self.status &= !0x02;
        }
        if (v & 0x80) != 0 {
            self.status |= 0x80; // N
        } else {
            self.status &= !0x80;
        }
    }

    /// Execute one instruction and return cycles used.
    pub fn step(&mut self) -> u32 {
        let op = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        match op {
            0xEA => {
                // NOP
                self.cycles += 2;
                2
            }
            0xA9 => {
                // LDA immediate
                let val = self.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x00 => {
                // BRK - treat as NOP for skeleton
                self.cycles += 7;
                7
            }
            _ => {
                // Unknown opcode: treat as NOP to keep forward progress
                self.cycles += 2;
                2
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lda_immediate_sets_a_and_flags() {
        let mut cpu = NesCpu::new();
        cpu.reset();
        cpu.load_program(0x8000, &[0xA9, 0x05, 0xEA]);
        let c1 = cpu.step();
        assert_eq!(c1, 2);
        assert_eq!(cpu.a, 5);
        assert_eq!(cpu.status & 0x02, 0); // zero flag clear
        let c2 = cpu.step();
        assert_eq!(c2, 2);
    }

    #[test]
    fn lda_zero_sets_zero_flag() {
        let mut cpu = NesCpu::new();
        cpu.reset();
        cpu.load_program(0x8000, &[0xA9, 0x00]);
        let _ = cpu.step();
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.status & 0x02, 0x02);
    }
}
