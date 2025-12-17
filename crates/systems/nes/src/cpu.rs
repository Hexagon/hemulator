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

    /// Read a 16-bit pointer for JMP (indirect) with the 6502 page-wrapping bug.
    #[inline]
    fn read_indirect_u16_bug(&self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
        let hi = self.read(hi_addr) as u16;
        (hi << 8) | lo
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
            0x29 | 0x25 | 0x2D | 0x21 | 0x31 | 0x35 | 0x39 => {
                // AND variants: immediate/zero/abs/(ind,X)/(ind),Y/zero,X/abs,Y
                // For simplicity map common encodings to immediate-like behavior where fetch is used.
                // We'll handle immediate (0x29) here; other encodings should call appropriate addr helpers.
                if op == 0x29 {
                    let val = self.fetch_u8();
                    self.a &= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    // handle via reading address depending on opcode
                    let val = match op {
                        0x25 => { let zp = self.fetch_u8() as u16; self.read(zp) },
                        0x2D => { let a = self.fetch_u16(); self.read(a) }
                        0x21 => { let a = self.addr_indirect_x(); self.read(a) }
                        0x31 => { let a = self.addr_indirect_y(); self.read(a) }
                        0x35 => { let a = self.addr_zero_page_x(); self.read(a) }
                        0x39 => { let a = self.addr_absolute_y(); self.read(a) }
                        _ => 0,
                    };
                    self.a &= val;
                    self.set_zero_and_negative(self.a);
                    // cycles conservative
                    self.cycles += 4;
                    4
                }
            }
            0x09 | 0x05 | 0x0D | 0x01 | 0x11 | 0x15 | 0x19 => {
                // ORA variants
                if op == 0x09 {
                    let val = self.fetch_u8();
                    self.a |= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let val = match op {
                        0x05 => { let zp = self.fetch_u8() as u16; self.read(zp) },
                        0x0D => { let a = self.fetch_u16(); self.read(a) }
                        0x01 => { let a = self.addr_indirect_x(); self.read(a) }
                        0x11 => { let a = self.addr_indirect_y(); self.read(a) }
                        0x15 => { let a = self.addr_zero_page_x(); self.read(a) }
                        0x19 => { let a = self.addr_absolute_y(); self.read(a) }
                        _ => 0,
                    };
                    self.a |= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 4;
                    4
                }
            }
            0x49 | 0x45 | 0x4D | 0x41 | 0x51 | 0x55 | 0x59 => {
                // EOR variants
                if op == 0x49 {
                    let val = self.fetch_u8();
                    self.a ^= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let val = match op {
                        0x45 => { let zp = self.fetch_u8() as u16; self.read(zp) },
                        0x4D => { let a = self.fetch_u16(); self.read(a) }
                        0x41 => { let a = self.addr_indirect_x(); self.read(a) }
                        0x51 => { let a = self.addr_indirect_y(); self.read(a) }
                        0x55 => { let a = self.addr_zero_page_x(); self.read(a) }
                        0x59 => { let a = self.addr_absolute_y(); self.read(a) }
                        _ => 0,
                    };
                    self.a ^= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 4;
                    4
                }
            }
            0xC9 | 0xC5 | 0xCD | 0xC1 | 0xD1 => {
                // CMP variants (A - M)
                let val = match op {
                    0xC9 => { let v = self.fetch_u8(); v }
                    0xC5 => { let zp = self.fetch_u8() as u16; self.read(zp) },
                    0xCD => { let a = self.fetch_u16(); self.read(a) }
                    0xC1 => { let a = self.addr_indirect_x(); self.read(a) }
                    0xD1 => { let a = self.addr_indirect_y(); self.read(a) }
                    _ => 0,
                };
                let res = (self.a as i16).wrapping_sub(val as i16) as u8;
                // carry set if A >= M
                if (self.a as u16) >= (val as u16) {
                    self.status |= 0x01;
                } else {
                    self.status &= !0x01;
                }
                self.set_zero_and_negative(res);
                self.cycles += 2;
                2
            }
            0x24 | 0x2C => {
                // BIT zp/abs
                let val = if op == 0x24 {
                    let zp = self.fetch_u8() as u16;
                    self.read(zp)
                } else {
                    let a = self.fetch_u16(); self.read(a)
                };
                let res = self.a & val;
                if res == 0 { self.status |= 0x02 } else { self.status &= !0x02 }
                // set V to bit 6 of M, N to bit 7
                if (val & 0x40) != 0 { self.status |= 0x40 } else { self.status &= !0x40 }
                if (val & 0x80) != 0 { self.status |= 0x80 } else { self.status &= !0x80 }
                self.cycles += if op == 0x24 { 3 } else { 4 };
                if op == 0x24 { 3 } else { 4 }
            }
            0x0A => {
                // ASL accumulator
                let old = self.a;
                let carry = (old & 0x80) != 0;
                let res = old << 1;
                self.a = res;
                if carry { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x06 | 0x0E => {
                // ASL zp or abs
                let addr = if op == 0x06 { self.fetch_u8() as u16 } else { self.fetch_u16() };
                let old = self.read(addr);
                let carry = (old & 0x80) != 0;
                let res = old << 1;
                self.write(addr, res);
                if carry { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0x06 { 5 } else { 6 };
                if op == 0x06 { 5 } else { 6 }
            }
            0x4A => {
                // LSR accumulator
                let old = self.a;
                let carry = (old & 0x01) != 0;
                let res = old >> 1;
                self.a = res;
                if carry { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x2A | 0x26 | 0x2E => {
                // ROL accumulator / ROL zp / ROL abs
                if op == 0x2A {
                    let old = self.a;
                    let carry_in = if (self.status & 0x01) != 0 { 1 } else { 0 };
                    let carry_out = (old & 0x80) != 0;
                    let res = ((old << 1) | carry_in) as u8;
                    self.a = res;
                    if carry_out { self.status |= 0x01 } else { self.status &= !0x01 }
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let addr = if op == 0x26 { self.fetch_u8() as u16 } else { self.fetch_u16() };
                    let old = self.read(addr);
                    let carry_in = if (self.status & 0x01) != 0 { 1 } else { 0 };
                    let carry_out = (old & 0x80) != 0;
                    let res = ((old << 1) | carry_in) as u8;
                    self.write(addr, res);
                    if carry_out { self.status |= 0x01 } else { self.status &= !0x01 }
                    self.set_zero_and_negative(res);
                    self.cycles += if op == 0x26 { 5 } else { 6 };
                    if op == 0x26 { 5 } else { 6 }
                }
            }
            0x6A | 0x66 | 0x6E => {
                // ROR accumulator / ROR zp / ROR abs
                if op == 0x6A {
                    let old = self.a;
                    let carry_in = if (self.status & 0x01) != 0 { 0x80 } else { 0 };
                    let carry_out = (old & 0x01) != 0;
                    let res = (old >> 1) | carry_in;
                    self.a = res;
                    if carry_out { self.status |= 0x01 } else { self.status &= !0x01 }
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let addr = if op == 0x66 { self.fetch_u8() as u16 } else { self.fetch_u16() };
                    let old = self.read(addr);
                    let carry_in = if (self.status & 0x01) != 0 { 0x80 } else { 0 };
                    let carry_out = (old & 0x01) != 0;
                    let res = (old >> 1) | carry_in;
                    self.write(addr, res);
                    if carry_out { self.status |= 0x01 } else { self.status &= !0x01 }
                    self.set_zero_and_negative(res);
                    self.cycles += if op == 0x66 { 5 } else { 6 };
                    if op == 0x66 { 5 } else { 6 }
                }
            }
            0xE9 | 0xE5 | 0xED | 0xE1 | 0xF1 => {
                // SBC variants (immediate, zp, abs, (ind,X), (ind),Y)
                // Implement using ADC on one's complement: A = A - M - (1 - C)
                let m = match op {
                    0xE9 => { let v = self.fetch_u8(); v }
                    0xE5 => { let zp = self.fetch_u8() as u16; self.read(zp) }
                    0xED => { let a = self.fetch_u16(); self.read(a) }
                    0xE1 => { let a = self.addr_indirect_x(); self.read(a) }
                    0xF1 => { let a = self.addr_indirect_y(); self.read(a) }
                    _ => 0
                } as i16;
                let carry = if (self.status & 0x01) != 0 { 1 } else { 0 };
                let value = (m ^ 0xFF) as i16; // one's complement
                let sum = (self.a as i16) + value + (carry as i16);
                let result = (sum & 0xFF) as u8;
                // set carry if result didn't borrow (i.e., sum >= 0)
                if sum >= 0 { self.status |= 0x01 } else { self.status &= !0x01 }
                // overflow detection similar to ADC
                if (((!(self.a ^ (m as u8)) ) & (self.a ^ result)) & 0x80) != 0 {
                    self.status |= 0x40;
                } else {
                    self.status &= !0x40;
                }
                self.a = result;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0xE0 | 0xE4 | 0xEC => {
                // CPX immediate/zp/abs
                let val = if op == 0xE0 { let v = self.fetch_u8(); v } else if op == 0xE4 { let zp = self.fetch_u8() as u16; self.read(zp) } else { let a = self.fetch_u16(); self.read(a) };
                let res = self.x.wrapping_sub(val);
                if (self.x as u16) >= (val as u16) { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0xE0 { 2 } else if op == 0xE4 { 3 } else { 4 };
                if op == 0xE0 { 2 } else if op == 0xE4 { 3 } else { 4 }
            }
            0xC0 | 0xC4 | 0xCC => {
                // CPY immediate/zp/abs
                let val = if op == 0xC0 { let v = self.fetch_u8(); v } else if op == 0xC4 { let zp = self.fetch_u8() as u16; self.read(zp) } else { let a = self.fetch_u16(); self.read(a) };
                let res = self.y.wrapping_sub(val);
                if (self.y as u16) >= (val as u16) { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0xC0 { 2 } else if op == 0xC4 { 3 } else { 4 };
                if op == 0xC0 { 2 } else if op == 0xC4 { 3 } else { 4 }
            }
            0x90 | 0xB0 | 0x70 | 0x50 | 0x10 | 0x30 | 0xD0 | 0xF0 => {
                // Branches: BCC(0x90), BCS(0xB0), BMI(0x30), BPL(0x10), BVC(0x50), BVS(0x70), BNE(0xD0), BEQ(0xF0)
                let offset = self.fetch_u8() as i8;
                let cond = match op {
                    0x90 => (self.status & 0x01) == 0, // BCC
                    0xB0 => (self.status & 0x01) != 0, // BCS
                    0x30 => (self.status & 0x80) != 0, // BMI
                    0x10 => (self.status & 0x80) == 0, // BPL
                    0x50 => (self.status & 0x40) == 0, // BVC
                    0x70 => (self.status & 0x40) != 0, // BVS
                    0xD0 => (self.status & 0x02) == 0, // BNE
                    0xF0 => (self.status & 0x02) != 0, // BEQ
                    _ => false
                };
                if cond {
                    let rel = offset as i16 as i32;
                    self.pc = ((self.pc as i32).wrapping_add(rel)) as u16;
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            0x46 | 0x4E => {
                // LSR zp or abs
                let addr = if op == 0x46 { self.fetch_u8() as u16 } else { self.fetch_u16() };
                let old = self.read(addr);
                let carry = (old & 0x01) != 0;
                let res = old >> 1;
                self.write(addr, res);
                if carry { self.status |= 0x01 } else { self.status &= !0x01 }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0x46 { 5 } else { 6 };
                if op == 0x46 { 5 } else { 6 }
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
            0x6C => {
                // JMP indirect (with 6502 page-wrapping bug)
                let ptr = self.fetch_u16();
                let addr = self.read_indirect_u16_bug(ptr);
                self.pc = addr;
                self.cycles += 5;
                5
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
        fn and_ora_eor_and_cmp_asl_lsr() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // AND immediate
            cpu.a = 0xF0;
            cpu.load_program(0x8000, &[0x29, 0x0F]);
            cpu.step();
            assert_eq!(cpu.a, 0x00);
            assert_eq!(cpu.status & 0x02, 0x02); // zero

            // ORA immediate
            cpu.a = 0x0F;
            cpu.load_program(0x8000, &[0x09, 0xF0]);
            cpu.step();
            assert_eq!(cpu.a, 0xFF);

            // EOR immediate
            cpu.a = 0xFF;
            cpu.load_program(0x8000, &[0x49, 0x0F]);
            cpu.step();
            assert_eq!(cpu.a, 0xF0);

            // CMP immediate (A >= M)
            cpu.a = 0x10;
            cpu.load_program(0x8000, &[0xC9, 0x0F]);
            cpu.step();
            assert_eq!(cpu.status & 0x01, 0x01);

            // ASL accumulator
            cpu.a = 0x80;
            cpu.load_program(0x8000, &[0x0A]);
            cpu.step();
            assert_eq!(cpu.a, 0x00);
            assert_eq!(cpu.status & 0x01, 0x01); // carry set

            // LSR accumulator
            cpu.a = 0x01;
            cpu.load_program(0x8000, &[0x4A]);
            cpu.step();
            assert_eq!(cpu.a, 0x00);
            assert_eq!(cpu.status & 0x01, 0x01);
        }

        #[test]
        fn jmp_indirect_page_wrap_bug() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // program: JMP ($80FF) placed at 0x8100 so it doesn't overwrite the pointer bytes
            cpu.load_program(0x8100, &[0x6C, 0xFF, 0x80]);
            // place indirect pointer at 0x80FF -> low byte at 0x80FF, high byte should wrap to 0x8000
            cpu.write(0x80FF, 0x34);
            cpu.write(0x8000, 0x12); // wrapped high byte
            // ensure PC points to our program start
            cpu.pc = 0x8100;
            cpu.step();
            assert_eq!(cpu.pc, 0x1234);
        }

        #[test]
        fn rol_ror_and_sbc_cpx_cpy_branches() {
            let mut cpu = NesCpu::new();
            cpu.reset();
            // ROL accumulator
            cpu.a = 0x80;
            cpu.load_program(0x8000, &[0x2A]);
            cpu.step();
            assert_eq!(cpu.a, 0x00);
            assert_eq!(cpu.status & 0x01, 0x01);

            // ROR accumulator
            cpu.a = 0x01;
            cpu.status &= !0x01;
            cpu.load_program(0x8000, &[0x6A]);
            cpu.step();
            assert_eq!(cpu.a, 0x00);
            assert_eq!(cpu.status & 0x01, 0x01);

            // SBC immediate: 0x10 - 0x01 = 0x0F
            cpu.a = 0x10;
            cpu.status |= 0x01; // carry set
            cpu.load_program(0x8000, &[0xE9, 0x01]);
            cpu.step();
            assert_eq!(cpu.a, 0x0F);

            // CPX immediate
            cpu.x = 0x05;
            cpu.load_program(0x8000, &[0xE0, 0x05]);
            cpu.step();
            assert_eq!(cpu.status & 0x02, 0x02);

            // CPY immediate
            cpu.y = 0x03;
            cpu.load_program(0x8000, &[0xC0, 0x03]);
            cpu.step();
            assert_eq!(cpu.status & 0x02, 0x02);

            // Branch BCS taken
            cpu.status |= 0x01;
            cpu.load_program(0x8000, &[0xB0, 0x01, 0xEA, 0xEA]);
            cpu.step();
            assert_eq!(cpu.step(), 2); // land on the last NOP
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
