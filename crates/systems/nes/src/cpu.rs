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

    #[inline]
    fn fetch_u8(&mut self) -> u8 {
        let v = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    #[inline]
    fn fetch_u16(&mut self) -> u16 {
        let lo = self.fetch_u8() as u16;
        let hi = self.fetch_u8() as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn addr_zero_page_x(&mut self) -> u16 {
        let zp = self.fetch_u8();
        zp.wrapping_add(self.x) as u16
    }

    #[inline]
    fn addr_absolute_x(&mut self) -> u16 {
        let base = self.fetch_u16();
        base.wrapping_add(self.x as u16)
    }

    #[inline]
    fn addr_zero_page_y(&mut self) -> u16 {
        let zp = self.fetch_u8();
        zp.wrapping_add(self.y) as u16
    }

    #[inline]
    fn addr_absolute_y(&mut self) -> u16 {
        let base = self.fetch_u16();
        base.wrapping_add(self.y as u16)
    }

    /// (Indirect,X) addressing: take zero-page operand, add X, then read 16-bit address from that page.
    #[inline]
    fn addr_indirect_x(&mut self) -> u16 {
        let zp = self.fetch_u8().wrapping_add(self.x);
        let lo = self.read(zp as u16) as u16;
        let hi = self.read(zp.wrapping_add(1) as u16) as u16;
        (hi << 8) | lo
    }

    /// (Indirect),Y addressing: take zero-page operand, read 16-bit base from page, add Y.
    #[inline]
    fn addr_indirect_y(&mut self) -> u16 {
        let zp = self.fetch_u8();
        let lo = self.read(zp as u16) as u16;
        let hi = self.read(zp.wrapping_add(1) as u16) as u16;
        let base = (hi << 8) | lo;
        base.wrapping_add(self.y as u16)
    }

    #[inline]
    fn push_u8(&mut self, v: u8) {
        let addr = 0x0100u16.wrapping_add(self.sp as u16);
        self.write(addr, v);
        self.sp = self.sp.wrapping_sub(1);
    }

    #[inline]
    fn pop_u8(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100u16.wrapping_add(self.sp as u16);
        self.read(addr)
    }

    #[inline]
    fn push_u16(&mut self, v: u16) {
        let hi = ((v >> 8) & 0xFF) as u8;
        let lo = (v & 0xFF) as u8;
        self.push_u8(hi);
        self.push_u8(lo);
    }

    #[inline]
    fn pop_u16(&mut self) -> u16 {
        let lo = self.pop_u8() as u16;
        let hi = self.pop_u8() as u16;
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
        let op = self.fetch_u8();
        match op {
            0xEA => {
                // NOP
                self.cycles += 2;
                2
            }
            0xA9 => {
                // LDA immediate
                let val = self.fetch_u8();
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x69 => {
                // ADC immediate
                let val = self.fetch_u8();
                let carry_in = if (self.status & 0x01) != 0 { 1u16 } else { 0u16 };
                let sum = self.a as u16 + val as u16 + carry_in;
                let result = sum as u8;
                if sum > 0xFF {
                    self.status |= 0x01; // set carry
                } else {
                    self.status &= !0x01;
                }
                // overflow: (~(A ^ M) & (A ^ R)) & 0x80
                if (((!(self.a ^ val)) & (self.a ^ result)) & 0x80) != 0 {
                    self.status |= 0x40;
                } else {
                    self.status &= !0x40;
                }
                self.a = result;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0xA5 => {
                // LDA zero page
                let zp = self.fetch_u8() as u16;
                let val = self.read(zp);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 3;
                3
            }
            0xB5 => {
                // LDA zero page,X
                let addr = self.addr_zero_page_x();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0x65 => {
                // ADC zero page
                let zp = self.fetch_u8() as u16;
                let val = self.read(zp as u16);
                let carry_in = if (self.status & 0x01) != 0 { 1u16 } else { 0u16 };
                let sum = self.a as u16 + val as u16 + carry_in;
                let result = sum as u8;
                if sum > 0xFF {
                    self.status |= 0x01;
                } else {
                    self.status &= !0x01;
                }
                if (((!(self.a ^ val)) & (self.a ^ result)) & 0x80) != 0 {
                    self.status |= 0x40;
                } else {
                    self.status &= !0x40;
                }
                self.a = result;
                self.set_zero_and_negative(self.a);
                self.cycles += 3;
                3
            }
            0xAD => {
                // LDA absolute
                let addr = self.fetch_u16();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0xBD => {
                // LDA absolute,X
                let addr = self.addr_absolute_x();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0xB9 => {
                // LDA absolute,Y
                let addr = self.addr_absolute_y();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0xA1 => {
                // LDA (indirect,X)
                let addr = self.addr_indirect_x();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 6;
                6
            }
            0xB1 => {
                // LDA (indirect),Y
                let addr = self.addr_indirect_y();
                let val = self.read(addr);
                self.a = val;
                self.set_zero_and_negative(self.a);
                self.cycles += 5;
                5
            }
            0x6D => {
                // ADC absolute
                let addr = self.fetch_u16();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 { 1u16 } else { 0u16 };
                let sum = self.a as u16 + val as u16 + carry_in;
                let result = sum as u8;
                if sum > 0xFF {
                    self.status |= 0x01;
                } else {
                    self.status &= !0x01;
                }
                if (((!(self.a ^ val)) & (self.a ^ result)) & 0x80) != 0 {
                    self.status |= 0x40;
                } else {
                    self.status &= !0x40;
                }
                self.a = result;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0x85 => {
                // STA zero page
                let zp = self.fetch_u8() as u16;
                self.write(zp, self.a);
                self.cycles += 3;
                3
            }
            0x8D => {
                // STA absolute
                let addr = self.fetch_u16();
                self.write(addr, self.a);
                self.cycles += 4;
                4
            }
            0x95 => {
                // STA zero page,X
                let addr = self.addr_zero_page_x();
                self.write(addr, self.a);
                self.cycles += 4;
                4
            }
            0x9D => {
                // STA absolute,X
                let addr = self.addr_absolute_x();
                self.write(addr, self.a);
                self.cycles += 5;
                5
            }
            0x99 => {
                // STA absolute,Y
                let addr = self.addr_absolute_y();
                self.write(addr, self.a);
                self.cycles += 5;
                5
            }
            0x81 => {
                // STA (indirect,X)
                let addr = self.addr_indirect_x();
                self.write(addr, self.a);
                self.cycles += 6;
                6
            }
            0x91 => {
                // STA (indirect),Y
                let addr = self.addr_indirect_y();
                self.write(addr, self.a);
                self.cycles += 6;
                6
            }
            0xAA => {
                // TAX
                self.x = self.a;
                self.set_zero_and_negative(self.x);
                self.cycles += 2;
                2
            }
            0x8A => {
                // TXA
                self.a = self.x;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0xE8 => {
                // INX
                self.x = self.x.wrapping_add(1);
                self.set_zero_and_negative(self.x);
                self.cycles += 2;
                2
            }
            0xCA => {
                // DEX
                self.x = self.x.wrapping_sub(1);
                self.set_zero_and_negative(self.x);
                self.cycles += 2;
                2
            }
            0x4C => {
                // JMP absolute
                let addr = self.fetch_u16();
                self.pc = addr;
                self.cycles += 3;
                3
            }
            0xF0 => {
                // BEQ relative
                let offset = self.fetch_u8() as i8;
                if (self.status & 0x02) != 0 {
                    // branch taken
                    let rel = offset as i16 as i32;
                    self.pc = ((self.pc as i32).wrapping_add(rel)) as u16;
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            0xD0 => {
                // BNE relative
                let offset = self.fetch_u8() as i8;
                if (self.status & 0x02) == 0 {
                    let rel = offset as i16 as i32;
                    self.pc = ((self.pc as i32).wrapping_add(rel)) as u16;
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            0x48 => {
                // PHA
                self.push_u8(self.a);
                self.cycles += 3;
                3
            }
            0x68 => {
                // PLA
                let v = self.pop_u8();
                self.a = v;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0x20 => {
                // JSR absolute
                let addr = self.fetch_u16();
                let ret = self.pc.wrapping_sub(1);
                self.push_u16(ret);
                self.pc = addr;
                self.cycles += 6;
                6
            }
            0x60 => {
                // RTS
                let ret = self.pop_u16();
                self.pc = ret.wrapping_add(1);
                self.cycles += 6;
                6
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

        #[test]
        fn adc_immediate_and_carry_overflow() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            cpu.a = 0x50;
            cpu.status &= !0x01; // clear carry
            cpu.load_program(0x8000, &[0x69, 0x10]); // ADC #$10
            assert_eq!(cpu.step(), 2);
            assert_eq!(cpu.a, 0x60);

            // test carry
            let mut cpu2 = NesCpu::new();
            cpu2.reset();
            cpu2.a = 0xFF;
            cpu2.status |= 0x01; // carry in
            cpu2.load_program(0x8000, &[0x69, 0x01]);
            assert_eq!(cpu2.step(), 2);
            assert_eq!(cpu2.a, 0x01);
            assert_eq!(cpu2.status & 0x01, 0x01);
        }

        #[test]
        fn beq_branches_when_zero() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // LDA #0; BEQ +2; LDA #1; LDA #2
            cpu.load_program(0x8000, &[0xA9, 0x00, 0xF0, 0x02, 0xA9, 0x01, 0xA9, 0x02]);
            assert_eq!(cpu.step(), 2); // LDA #0 -> sets Z
            assert_eq!(cpu.step(), 3); // BEQ taken
            assert_eq!(cpu.step(), 2); // LDA #2
            assert_eq!(cpu.a, 2);
        }

        #[test]
        fn pha_pla_roundtrip() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            cpu.a = 0x7F;
            cpu.load_program(0x8000, &[0x48, 0xA9, 0x00, 0x68]); // PHA; LDA #0; PLA
            assert_eq!(cpu.step(), 3); // PHA
            assert_eq!(cpu.step(), 2); // LDA #0
            assert_eq!(cpu.step(), 4); // PLA
            assert_eq!(cpu.a, 0x7F);
        }

        #[test]
        fn jsr_rts_returns() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // JSR to 0x8010; at 0x8010 put RTS
            // program at 0x8000: JSR $8010 ; LDA #1
            cpu.load_program(0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x01]);
            // place RTS at 0x8010
            cpu.write(0x8010, 0x60);
            assert_eq!(cpu.step(), 6); // JSR
            // Now at subroutine, execute RTS
            assert_eq!(cpu.step(), 6); // RTS
            // After RTS, next instruction should be LDA #1
            assert_eq!(cpu.step(), 2);
            assert_eq!(cpu.a, 1);
        }
        #[test]
        fn lda_zero_page_and_sta_zero_page() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // LDA #$42 ; STA $10
            cpu.load_program(0x8000, &[0xA9, 0x42, 0x85, 0x10]);
            assert_eq!(cpu.step(), 2); // A = 0x42
            assert_eq!(cpu.a, 0x42);
            assert_eq!(cpu.step(), 3); // STA stores A into $0010
            assert_eq!(cpu.read(0x0010), 0x42);
        }

        #[test]
        fn lda_indirect_x_and_indirect_y() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // set up pointer table in zero page: at $20 store pointer to $2000
            cpu.write(0x0020, 0x00);
            cpu.write(0x0021, 0x20);
            // place value at $2000
            cpu.write(0x2000, 0xAB);
            // place operand for (indirect,X) at $10 such that (10 + X) -> 20
            // set X = 0x06, operand = 0x0A -> 0x0A + 0x06 = 0x10 -> pointer at 0x10
            cpu.write(0x0010, 0x00);
            cpu.write(0x0011, 0x20);
            // test (indirect,X): set X then LDA (zp,X)
            cpu.x = 6;
            cpu.load_program(0x8000, &[0xA1, 0x0A]);
            assert_eq!(cpu.step(), 6);
            assert_eq!(cpu.a, 0xAB);

            // test (indirect),Y: pointer at $20 points to 0x2000, Y = 0
            cpu.load_program(0x8000, &[0xB1, 0x20]);
            cpu.y = 0;
            assert_eq!(cpu.step(), 5);
            assert_eq!(cpu.a, 0xAB);
        }

        #[test]
        fn lda_absolute_reads_memory() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // Place value at 0x1234, then LDA $1234
            cpu.write(0x1234, 0x99);
            cpu.load_program(0x8000, &[0xAD, 0x34, 0x12]);
            assert_eq!(cpu.step(), 4);
            assert_eq!(cpu.a, 0x99);
        }
}
