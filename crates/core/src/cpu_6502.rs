//! MOS 6502 CPU core implementation
//!
//! This module provides a reusable, generic 6502 CPU implementation that can be used
//! by any system (NES, Atari 2600, Apple II, etc.) by implementing the `Memory6502` trait.

/// Memory interface trait for the 6502 CPU
///
/// Systems using the 6502 must implement this trait to provide memory access.
pub trait Memory6502 {
    /// Read a byte from memory at the given address
    fn read(&self, addr: u16) -> u8;

    /// Write a byte to memory at the given address
    fn write(&mut self, addr: u16, val: u8);
}

use std::sync::OnceLock;

fn log_unknown_ops() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("EMU_LOG_UNKNOWN_OPS").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        )
    })
}

fn log_brk() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("EMU_LOG_BRK").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        )
    })
}
/// MOS 6502 CPU state and execution engine
///
/// This is a generic, reusable 6502 CPU implementation that works with any
/// system through the `Memory6502` trait.
#[derive(Debug)]
pub struct Cpu6502<M: Memory6502> {
    /// Accumulator register
    pub a: u8,
    /// X index register
    pub x: u8,
    /// Y index register
    pub y: u8,
    /// Stack pointer (points to 0x0100 + sp)
    pub sp: u8,
    /// Status register (NV-BDIZC)
    pub status: u8,
    /// Program counter
    pub pc: u16,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
    /// NMI in progress flag
    in_nmi: bool,
}

impl<M: Memory6502> Cpu6502<M> {
    /// Create a new 6502 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            status: 0x24,
            pc: 0x8000,
            cycles: 0,
            memory,
            in_nmi: false,
        }
    }

    /// Reset the CPU to initial state (preserves memory)
    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = 0x24;
        self.cycles = 0;
        self.in_nmi = false;

        // On real hardware, RESET loads the vector at $FFFC-$FFFD.
        self.pc = self.read_u16(0xFFFC);
    }

    /// Replace the memory interface while preserving CPU state
    pub fn with_memory<N: Memory6502>(self, new_memory: N) -> Cpu6502<N> {
        Cpu6502 {
            a: self.a,
            x: self.x,
            y: self.y,
            sp: self.sp,
            status: self.status,
            pc: self.pc,
            cycles: self.cycles,
            memory: new_memory,
            in_nmi: self.in_nmi,
        }
    }

    /// Check if currently executing an NMI handler
    #[allow(dead_code)]
    pub fn is_in_nmi(&self) -> bool {
        self.in_nmi
    }

    /// Read a byte from memory
    #[inline]
    fn read(&self, addr: u16) -> u8 {
        self.memory.read(addr)
    }

    /// Write a byte to memory
    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.memory.write(addr, val);
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

    /// Trigger a Non-Maskable Interrupt (NMI)
    pub fn trigger_nmi(&mut self) {
        // Avoid nested NMIs in this simplified model.
        if self.in_nmi {
            return;
        }
        self.in_nmi = true;
        // Push PC and status, then jump to NMI vector at $FFFA.
        self.push_u16(self.pc);
        let mut s = self.status;
        s &= !0x10; // clear B
        s |= 0x20; // bit 5 is always set
        self.push_u8(s);
        self.status |= 0x04; // set I
        let vector = self.read_u16(0xFFFA);
        // eprintln!("CPU: NMI triggered at PC={:04X}, jumping to {:04X}", self.pc, vector);
        self.pc = vector;
        self.cycles = self.cycles.wrapping_add(7);
    }

    /// Trigger a maskable IRQ (interrupt request)
    pub fn trigger_irq(&mut self) {
        // Respect the I flag: if set, ignore maskable IRQs.
        if (self.status & 0x04) != 0 {
            return;
        }
        // Push PC and status, then jump to IRQ/BRK vector at $FFFE.
        self.push_u16(self.pc);
        let mut s = self.status;
        s &= !0x10; // clear B
        s |= 0x20; // bit 5 always set
        self.push_u8(s);
        self.status |= 0x04; // set I
        let vector = self.read_u16(0xFFFE);
        self.pc = vector;
        self.cycles = self.cycles.wrapping_add(7);
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
            0x08 => {
                // PHP
                let mut s = self.status;
                s |= 0x10; // B flag set when pushed by PHP
                s |= 0x20; // bit 5 always set
                self.push_u8(s);
                self.cycles += 3;
                3
            }
            0x28 => {
                // PLP
                let mut s = self.pop_u8();
                s |= 0x20; // bit 5 always set
                s &= !0x10; // B flag not stored as a real latch
                self.status = s;
                self.cycles += 4;
                4
            }
            0x18 => {
                // CLC
                self.status &= !0x01;
                self.cycles += 2;
                2
            }
            0x38 => {
                // SEC
                self.status |= 0x01;
                self.cycles += 2;
                2
            }
            0x58 => {
                // CLI
                self.status &= !0x04;
                self.cycles += 2;
                2
            }
            0x78 => {
                // SEI
                self.status |= 0x04;
                self.cycles += 2;
                2
            }
            0xB8 => {
                // CLV
                self.status &= !0x40;
                self.cycles += 2;
                2
            }
            0xD8 => {
                // CLD
                self.status &= !0x08;
                self.cycles += 2;
                2
            }
            0xF8 => {
                // SED
                self.status |= 0x08;
                self.cycles += 2;
                2
            }
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
            0xA2 => {
                // LDX immediate
                let val = self.fetch_u8();
                self.x = val;
                self.set_zero_and_negative(self.x);
                self.cycles += 2;
                2
            }
            0xA6 => {
                // LDX zero page
                let addr = self.fetch_u8() as u16;
                let val = self.read(addr);
                self.x = val;
                self.set_zero_and_negative(self.x);
                self.cycles += 3;
                3
            }
            0xB6 => {
                // LDX zero page,Y
                let addr = self.addr_zero_page_y();
                let val = self.read(addr);
                self.x = val;
                self.set_zero_and_negative(self.x);
                self.cycles += 4;
                4
            }
            0xAE => {
                // LDX absolute
                let addr = self.fetch_u16();
                let val = self.read(addr);
                self.x = val;
                self.set_zero_and_negative(self.x);
                self.cycles += 4;
                4
            }
            0xBE => {
                // LDX absolute,Y
                let addr = self.addr_absolute_y();
                let val = self.read(addr);
                self.x = val;
                self.set_zero_and_negative(self.x);
                self.cycles += 4;
                4
            }
            0xA0 => {
                // LDY immediate
                let val = self.fetch_u8();
                self.y = val;
                self.set_zero_and_negative(self.y);
                self.cycles += 2;
                2
            }
            0xA4 => {
                // LDY zero page
                let addr = self.fetch_u8() as u16;
                let val = self.read(addr);
                self.y = val;
                self.set_zero_and_negative(self.y);
                self.cycles += 3;
                3
            }
            0xB4 => {
                // LDY zero page,X
                let addr = self.addr_zero_page_x();
                let val = self.read(addr);
                self.y = val;
                self.set_zero_and_negative(self.y);
                self.cycles += 4;
                4
            }
            0xAC => {
                // LDY absolute
                let addr = self.fetch_u16();
                let val = self.read(addr);
                self.y = val;
                self.set_zero_and_negative(self.y);
                self.cycles += 4;
                4
            }
            0xBC => {
                // LDY absolute,X
                let addr = self.addr_absolute_x();
                let val = self.read(addr);
                self.y = val;
                self.set_zero_and_negative(self.y);
                self.cycles += 4;
                4
            }
            0x69 => {
                // ADC immediate
                let val = self.fetch_u8();
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
                if op == 0x29 {
                    let val = self.fetch_u8();
                    self.a &= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let val = match op {
                        0x25 => {
                            let zp = self.fetch_u8() as u16;
                            self.read(zp)
                        }
                        0x2D => {
                            let a = self.fetch_u16();
                            self.read(a)
                        }
                        0x21 => {
                            let a = self.addr_indirect_x();
                            self.read(a)
                        }
                        0x31 => {
                            let a = self.addr_indirect_y();
                            self.read(a)
                        }
                        0x35 => {
                            let a = self.addr_zero_page_x();
                            self.read(a)
                        }
                        0x39 => {
                            let a = self.addr_absolute_y();
                            self.read(a)
                        }
                        _ => 0,
                    };
                    self.a &= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 4;
                    4
                }
            }
            0x3D => {
                // AND absolute,X
                let addr = self.addr_absolute_x();
                let val = self.read(addr);
                self.a &= val;
                self.set_zero_and_negative(self.a);
                self.cycles += 4;
                4
            }
            0x09 | 0x05 | 0x0D | 0x01 | 0x11 | 0x15 | 0x19 | 0x1D => {
                // ORA variants
                if op == 0x09 {
                    let val = self.fetch_u8();
                    self.a |= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let val = match op {
                        0x05 => {
                            let zp = self.fetch_u8() as u16;
                            self.read(zp)
                        }
                        0x0D => {
                            let a = self.fetch_u16();
                            self.read(a)
                        }
                        0x01 => {
                            let a = self.addr_indirect_x();
                            self.read(a)
                        }
                        0x11 => {
                            let a = self.addr_indirect_y();
                            self.read(a)
                        }
                        0x15 => {
                            let a = self.addr_zero_page_x();
                            self.read(a)
                        }
                        0x19 => {
                            let a = self.addr_absolute_y();
                            self.read(a)
                        }
                        0x1D => {
                            let a = self.addr_absolute_x();
                            self.read(a)
                        }
                        _ => 0,
                    };
                    self.a |= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 4;
                    4
                }
            }
            0x49 | 0x45 | 0x4D | 0x41 | 0x51 | 0x55 | 0x59 | 0x5D => {
                // EOR variants
                if op == 0x49 {
                    let val = self.fetch_u8();
                    self.a ^= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let val = match op {
                        0x45 => {
                            let zp = self.fetch_u8() as u16;
                            self.read(zp)
                        }
                        0x4D => {
                            let a = self.fetch_u16();
                            self.read(a)
                        }
                        0x41 => {
                            let a = self.addr_indirect_x();
                            self.read(a)
                        }
                        0x51 => {
                            let a = self.addr_indirect_y();
                            self.read(a)
                        }
                        0x55 => {
                            let a = self.addr_zero_page_x();
                            self.read(a)
                        }
                        0x59 => {
                            let a = self.addr_absolute_y();
                            self.read(a)
                        }
                        0x5D => {
                            let a = self.addr_absolute_x();
                            self.read(a)
                        }
                        _ => 0,
                    };
                    self.a ^= val;
                    self.set_zero_and_negative(self.a);
                    self.cycles += 4;
                    4
                }
            }
            0xC9 | 0xC5 | 0xCD | 0xC1 | 0xD1 | 0xD5 | 0xD9 | 0xDD => {
                // CMP variants (A - M) - all addressing modes
                let val = match op {
                    0xC9 => self.fetch_u8(),
                    0xC5 => {
                        let zp = self.fetch_u8() as u16;
                        self.read(zp)
                    }
                    0xCD => {
                        let a = self.fetch_u16();
                        self.read(a)
                    }
                    0xC1 => {
                        let a = self.addr_indirect_x();
                        self.read(a)
                    }
                    0xD1 => {
                        let a = self.addr_indirect_y();
                        self.read(a)
                    }
                    0xD5 => {
                        // CMP zp,X
                        let a = self.addr_zero_page_x();
                        self.read(a)
                    }
                    0xD9 => {
                        // CMP abs,Y
                        let a = self.addr_absolute_y();
                        self.read(a)
                    }
                    0xDD => {
                        // CMP abs,X
                        let a = self.addr_absolute_x();
                        self.read(a)
                    }
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
                    let a = self.fetch_u16();
                    self.read(a)
                };
                let res = self.a & val;
                if res == 0 {
                    self.status |= 0x02
                } else {
                    self.status &= !0x02
                }
                // set V to bit 6 of M, N to bit 7
                if (val & 0x40) != 0 {
                    self.status |= 0x40
                } else {
                    self.status &= !0x40
                }
                if (val & 0x80) != 0 {
                    self.status |= 0x80
                } else {
                    self.status &= !0x80
                }
                self.cycles += if op == 0x24 { 3 } else { 4 };
                if op == 0x24 {
                    3
                } else {
                    4
                }
            }
            0x0A => {
                // ASL accumulator
                let old = self.a;
                let carry = (old & 0x80) != 0;
                let res = old << 1;
                self.a = res;
                if carry {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x06 | 0x0E | 0x16 | 0x1E => {
                // ASL zp / abs / zp,X / abs,X
                let addr = match op {
                    0x06 => self.fetch_u8() as u16,
                    0x0E => self.fetch_u16(),
                    0x16 => self.addr_zero_page_x(),
                    0x1E => self.addr_absolute_x(),
                    _ => 0,
                };
                let old = self.read(addr);
                let carry = (old & 0x80) != 0;
                let res = old << 1;
                self.write(addr, res);
                if carry {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(res);
                let cycles = match op {
                    0x06 => 5,
                    0x0E => 6,
                    0x16 => 6,
                    0x1E => 7,
                    _ => 5,
                };
                self.cycles += cycles as u64;
                cycles
            }
            0x4A => {
                // LSR accumulator
                let old = self.a;
                let carry = (old & 0x01) != 0;
                let res = old >> 1;
                self.a = res;
                if carry {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x2A | 0x26 | 0x2E | 0x36 | 0x3E => {
                // ROL accumulator / ROL zp / ROL abs / ROL zp,X / ROL abs,X
                if op == 0x2A {
                    let old = self.a;
                    let carry_in = if (self.status & 0x01) != 0 { 1 } else { 0 };
                    let carry_out = (old & 0x80) != 0;
                    let res = (old << 1) | carry_in;
                    self.a = res;
                    if carry_out {
                        self.status |= 0x01
                    } else {
                        self.status &= !0x01
                    }
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let addr = match op {
                        0x26 => self.fetch_u8() as u16,
                        0x2E => self.fetch_u16(),
                        0x36 => self.addr_zero_page_x(),
                        0x3E => self.addr_absolute_x(),
                        _ => 0,
                    };
                    let old = self.read(addr);
                    let carry_in = if (self.status & 0x01) != 0 { 1 } else { 0 };
                    let carry_out = (old & 0x80) != 0;
                    let res = (old << 1) | carry_in;
                    self.write(addr, res);
                    if carry_out {
                        self.status |= 0x01
                    } else {
                        self.status &= !0x01
                    }
                    self.set_zero_and_negative(res);
                    let cycles = match op {
                        0x26 => 5,
                        0x2E => 6,
                        0x36 => 6,
                        0x3E => 7,
                        _ => 5,
                    };
                    self.cycles += cycles as u64;
                    cycles
                }
            }
            0x6A | 0x66 | 0x6E | 0x76 | 0x7E => {
                // ROR accumulator / ROR zp / ROR abs / ROR zp,X / ROR abs,X
                if op == 0x6A {
                    let old = self.a;
                    let carry_in = if (self.status & 0x01) != 0 { 0x80 } else { 0 };
                    let carry_out = (old & 0x01) != 0;
                    let res = (old >> 1) | carry_in;
                    self.a = res;
                    if carry_out {
                        self.status |= 0x01
                    } else {
                        self.status &= !0x01
                    }
                    self.set_zero_and_negative(self.a);
                    self.cycles += 2;
                    2
                } else {
                    let addr = match op {
                        0x66 => self.fetch_u8() as u16,
                        0x6E => self.fetch_u16(),
                        0x76 => self.addr_zero_page_x(),
                        0x7E => self.addr_absolute_x(),
                        _ => 0,
                    };
                    let old = self.read(addr);
                    let carry_in = if (self.status & 0x01) != 0 { 0x80 } else { 0 };
                    let carry_out = (old & 0x01) != 0;
                    let res = (old >> 1) | carry_in;
                    self.write(addr, res);
                    if carry_out {
                        self.status |= 0x01
                    } else {
                        self.status &= !0x01
                    }
                    self.set_zero_and_negative(res);
                    let cycles = match op {
                        0x66 => 5,
                        0x6E => 6,
                        0x76 => 6,
                        0x7E => 7,
                        _ => 5,
                    };
                    self.cycles += cycles as u64;
                    cycles
                }
            }
            0xE9 | 0xE5 | 0xED | 0xE1 | 0xF1 | 0xF5 | 0xF9 | 0xFD => {
                // SBC variants (immediate, zp, abs, (ind,X), (ind),Y, zp,X, abs,Y, abs,X)
                // Implement using ADC on one's complement: A = A - M - (1 - C)
                let m = match op {
                    0xE9 => self.fetch_u8(),
                    0xE5 => {
                        let zp = self.fetch_u8() as u16;
                        self.read(zp)
                    }
                    0xED => {
                        let a = self.fetch_u16();
                        self.read(a)
                    }
                    0xE1 => {
                        let a = self.addr_indirect_x();
                        self.read(a)
                    }
                    0xF1 => {
                        let a = self.addr_indirect_y();
                        self.read(a)
                    }
                    0xF5 => {
                        let a = self.addr_zero_page_x();
                        self.read(a)
                    }
                    0xF9 => {
                        let a = self.addr_absolute_y();
                        self.read(a)
                    }
                    0xFD => {
                        let a = self.addr_absolute_x();
                        self.read(a)
                    }
                    _ => 0,
                } as i16;
                let carry = if (self.status & 0x01) != 0 { 1 } else { 0 };
                let value = m ^ 0xFF; // one's complement
                let sum = (self.a as i16) + value + (carry as i16);
                let result = (sum & 0xFF) as u8;
                // set carry if result didn't borrow (i.e., sum >= 0)
                if sum >= 0 {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                // overflow detection similar to ADC
                if (((!(self.a ^ (m as u8))) & (self.a ^ result)) & 0x80) != 0 {
                    self.status |= 0x40;
                } else {
                    self.status &= !0x40;
                }
                self.a = result;
                self.set_zero_and_negative(self.a);
                let cycles = match op {
                    0xE9 => 2,  // immediate
                    0xE5 => 3,  // zero page
                    0xED => 4,  // absolute
                    0xE1 => 6,  // (indirect,X)
                    0xF1 => 5,  // (indirect),Y
                    0xF5 => 4,  // zero page,X
                    0xF9 => 4,  // absolute,Y
                    0xFD => 4,  // absolute,X
                    _ => 2,
                };
                self.cycles += cycles as u64;
                cycles
            }
            0xE0 | 0xE4 | 0xEC => {
                // CPX immediate/zp/abs
                let val = if op == 0xE0 {
                    self.fetch_u8()
                } else if op == 0xE4 {
                    let zp = self.fetch_u8() as u16;
                    self.read(zp)
                } else {
                    let a = self.fetch_u16();
                    self.read(a)
                };
                let res = self.x.wrapping_sub(val);
                if (self.x as u16) >= (val as u16) {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0xE0 {
                    2
                } else if op == 0xE4 {
                    3
                } else {
                    4
                };
                if op == 0xE0 {
                    2
                } else if op == 0xE4 {
                    3
                } else {
                    4
                }
            }
            0xC0 | 0xC4 | 0xCC => {
                // CPY immediate/zp/abs
                let val = if op == 0xC0 {
                    self.fetch_u8()
                } else if op == 0xC4 {
                    let zp = self.fetch_u8() as u16;
                    self.read(zp)
                } else {
                    let a = self.fetch_u16();
                    self.read(a)
                };
                let res = self.y.wrapping_sub(val);
                if (self.y as u16) >= (val as u16) {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(res);
                self.cycles += if op == 0xC0 {
                    2
                } else if op == 0xC4 {
                    3
                } else {
                    4
                };
                if op == 0xC0 {
                    2
                } else if op == 0xC4 {
                    3
                } else {
                    4
                }
            }

            0xC6 | 0xD6 | 0xCE | 0xDE => {
                // DEC zp / DEC zp,X / DEC abs / DEC abs,X
                let addr = match op {
                    0xC6 => self.fetch_u8() as u16,
                    0xD6 => self.addr_zero_page_x(),
                    0xCE => self.fetch_u16(),
                    0xDE => self.addr_absolute_x(),
                    _ => 0,
                };
                let v = self.read(addr).wrapping_sub(1);
                self.write(addr, v);
                self.set_zero_and_negative(v);
                let used = match op {
                    0xC6 => 5,
                    0xD6 => 6,
                    0xCE => 6,
                    0xDE => 7,
                    _ => 5,
                };
                self.cycles += used as u64;
                used
            }

            0xE6 | 0xF6 | 0xEE | 0xFE => {
                // INC zp / INC zp,X / INC abs / INC abs,X
                let addr = match op {
                    0xE6 => self.fetch_u8() as u16,
                    0xF6 => self.addr_zero_page_x(),
                    0xEE => self.fetch_u16(),
                    0xFE => self.addr_absolute_x(),
                    _ => 0,
                };
                let v = self.read(addr).wrapping_add(1);
                self.write(addr, v);
                self.set_zero_and_negative(v);
                let used = match op {
                    0xE6 => 5,
                    0xF6 => 6,
                    0xEE => 6,
                    0xFE => 7,
                    _ => 5,
                };
                self.cycles += used as u64;
                used
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
                    _ => false,
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
            0x46 | 0x4E | 0x56 | 0x5E => {
                // LSR zp / abs / zp,X / abs,X
                let addr = match op {
                    0x46 => self.fetch_u8() as u16,
                    0x4E => self.fetch_u16(),
                    0x56 => self.addr_zero_page_x(),
                    0x5E => self.addr_absolute_x(),
                    _ => 0,
                };
                let old = self.read(addr);
                let carry = (old & 0x01) != 0;
                let res = old >> 1;
                self.write(addr, res);
                if carry {
                    self.status |= 0x01
                } else {
                    self.status &= !0x01
                }
                self.set_zero_and_negative(res);
                let cycles = match op {
                    0x46 => 5,
                    0x4E => 6,
                    0x56 => 6,
                    0x5E => 7,
                    _ => 5,
                };
                self.cycles += cycles as u64;
                cycles
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
                let val = self.read(zp);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
            0x75 => {
                // ADC zero page,X
                let addr = self.addr_zero_page_x();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
            0x7D => {
                // ADC absolute,X
                let addr = self.addr_absolute_x();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
            0x79 => {
                // ADC absolute,Y
                let addr = self.addr_absolute_y();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
            0x61 => {
                // ADC (indirect,X)
                let addr = self.addr_indirect_x();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
                self.cycles += 6;
                6
            }
            0x71 => {
                // ADC (indirect),Y
                let addr = self.addr_indirect_y();
                let val = self.read(addr);
                let carry_in = if (self.status & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };
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
                self.cycles += 5;
                5
            }
            0x85 => {
                // STA zero page
                let zp = self.fetch_u8() as u16;
                self.write(zp, self.a);
                self.cycles += 3;
                3
            }
            0x86 => {
                // STX zero page
                let zp = self.fetch_u8() as u16;
                self.write(zp, self.x);
                self.cycles += 3;
                3
            }
            0x84 => {
                // STY zero page
                let zp = self.fetch_u8() as u16;
                self.write(zp, self.y);
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
            0x8E => {
                // STX absolute
                let addr = self.fetch_u16();
                self.write(addr, self.x);
                self.cycles += 4;
                4
            }
            0x8C => {
                // STY absolute
                let addr = self.fetch_u16();
                self.write(addr, self.y);
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
            0x96 => {
                // STX zero page,Y
                let zp = self.fetch_u8().wrapping_add(self.y) as u16;
                self.write(zp, self.x);
                self.cycles += 4;
                4
            }
            0x94 => {
                // STY zero page,X
                let addr = self.addr_zero_page_x();
                self.write(addr, self.y);
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
            0xA8 => {
                // TAY
                self.y = self.a;
                self.set_zero_and_negative(self.y);
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
            0x98 => {
                // TYA
                self.a = self.y;
                self.set_zero_and_negative(self.a);
                self.cycles += 2;
                2
            }
            0x9A => {
                // TXS
                self.sp = self.x;
                self.cycles += 2;
                2
            }
            0xBA => {
                // TSX
                self.x = self.sp;
                self.set_zero_and_negative(self.x);
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
            0xC8 => {
                // INY
                self.y = self.y.wrapping_add(1);
                self.set_zero_and_negative(self.y);
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
            0x88 => {
                // DEY
                self.y = self.y.wrapping_sub(1);
                self.set_zero_and_negative(self.y);
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
            0x40 => {
                // RTI
                let mut s = self.pop_u8();
                s |= 0x20; // bit 5 always set
                s &= !0x10; // B flag not stored by interrupts
                self.status = s;
                self.pc = self.pop_u16();
                self.in_nmi = false;
                self.cycles += 6;
                6
            }
            0x00 => {
                // BRK
                // BRK is treated as a 2-byte instruction; PC is incremented by one extra
                // before pushing.
                let pc_to_push = self.pc.wrapping_add(1);
                let brk_pc = self.pc.wrapping_sub(1);
                if log_brk() {
                    eprintln!("CPU: BRK executed at PC={:04X}, pushing {:04X}, status={:02X}", brk_pc, pc_to_push, self.status);
                }
                self.push_u16(pc_to_push);

                let mut s = self.status;
                s |= 0x10; // set B when pushed by BRK
                s |= 0x20; // bit 5 always set
                self.push_u8(s);

                self.status |= 0x04; // set I
                self.pc = self.read_u16(0xFFFE);
                if log_brk() {
                    eprintln!("CPU: BRK jumped to {:04X}", self.pc);
                }
                self.cycles += 7;
                7
            }
            _ => {
                // Unknown opcode: treat as NOP to keep forward progress
                if log_unknown_ops() {
                    let pc = self.pc.wrapping_sub(1);
                    eprintln!(
                        "UNKNOWN OPCODE: pc=0x{:04X} op=0x{:02X} a=0x{:02X} x=0x{:02X} y=0x{:02X} sp=0x{:02X} p=0x{:02X}",
                        pc, op, self.a, self.x, self.y, self.sp, self.status
                    );
                }
                self.cycles += 2;
                2
            }
        }
    }
}

/// Simple array-based memory implementation for testing
#[derive(Debug)]
pub struct ArrayMemory {
    pub data: [u8; 0x10000],
}

impl ArrayMemory {
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }

    /// Load a program into memory and set reset vector
    pub fn load_program(&mut self, offset: u16, data: &[u8]) {
        let off = offset as usize;
        self.data[off..off + data.len()].copy_from_slice(data);
        // set reset vector to offset
        let lo = (offset & 0xFF) as u8;
        let hi = ((offset >> 8) & 0xFF) as u8;
        self.data[0xFFFC] = lo;
        self.data[0xFFFD] = hi;
    }
}

impl Default for ArrayMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory6502 for ArrayMemory {
    fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lda_immediate_sets_a_and_flags() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.load_program(0x8000, &[0xA9, 0x05, 0xEA]);
        cpu.reset();
        let c1 = cpu.step();
        assert_eq!(c1, 2);
        assert_eq!(cpu.a, 5);
        assert_eq!(cpu.status & 0x02, 0); // zero flag clear
        let c2 = cpu.step();
        assert_eq!(c2, 2);
    }

    #[test]
    fn lda_zero_sets_zero_flag() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.load_program(0x8000, &[0xA9, 0x00]);
        cpu.reset();
        let _ = cpu.step();
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.status & 0x02, 0x02);
    }

    #[test]
    fn adc_immediate_and_carry_overflow() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.load_program(0x8000, &[0x69, 0x10]); // ADC #$10
        cpu.reset();
        cpu.a = 0x50;
        cpu.status &= !0x01; // clear carry
        assert_eq!(cpu.step(), 2);
        assert_eq!(cpu.a, 0x60);

        // test carry
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.load_program(0x8000, &[0x69, 0x01]);
        cpu2.reset();
        cpu2.a = 0xFF;
        cpu2.status |= 0x01; // carry in
        assert_eq!(cpu2.step(), 2);
        assert_eq!(cpu2.a, 0x01);
        assert_eq!(cpu2.status & 0x01, 0x01);
    }

    #[test]
    fn beq_branches_when_zero() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        // LDA #0; BEQ +2; LDA #1; LDA #2
        cpu.memory
            .load_program(0x8000, &[0xA9, 0x00, 0xF0, 0x02, 0xA9, 0x01, 0xA9, 0x02]);
        cpu.reset();
        assert_eq!(cpu.step(), 2); // LDA #0 -> sets Z
        assert_eq!(cpu.step(), 3); // BEQ taken
        assert_eq!(cpu.step(), 2); // LDA #2
        assert_eq!(cpu.a, 2);
    }

    #[test]
    fn pha_pla_roundtrip() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.load_program(0x8000, &[0x48, 0xA9, 0x00, 0x68]); // PHA; LDA #0; PLA
        cpu.reset();
        cpu.a = 0x7F;
        assert_eq!(cpu.step(), 3); // PHA
        assert_eq!(cpu.step(), 2); // LDA #0
        assert_eq!(cpu.step(), 4); // PLA
        assert_eq!(cpu.a, 0x7F);
    }

    #[test]
    fn jsr_rts_returns() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        // JSR to 0x8010; at 0x8010 put RTS
        // program at 0x8000: JSR $8010 ; LDA #1
        cpu.memory
            .load_program(0x8000, &[0x20, 0x10, 0x80, 0xA9, 0x01]);
        // place RTS at 0x8010
        cpu.memory.write(0x8010, 0x60);
        cpu.reset();
        assert_eq!(cpu.step(), 6); // JSR
                                   // Now at subroutine, execute RTS
        assert_eq!(cpu.step(), 6); // RTS
                                   // After RTS, next instruction should be LDA #1
        assert_eq!(cpu.step(), 2);
        assert_eq!(cpu.a, 1);
    }
    #[test]
    fn lda_zero_page_and_sta_zero_page() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        // LDA #$42 ; STA $10
        cpu.memory.load_program(0x8000, &[0xA9, 0x42, 0x85, 0x10]);
        cpu.reset();
        assert_eq!(cpu.step(), 2); // A = 0x42
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.step(), 3); // STA stores A into $0010
        assert_eq!(cpu.memory.read(0x0010), 0x42);
    }

    #[test]
    fn adc_all_addressing_modes() {
        // Test ADC zero page,X (0x75)
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.write(0x0015, 0x10);
        cpu.memory.load_program(0x8000, &[0x75, 0x10]); // ADC $10,X
        cpu.reset();
        cpu.a = 0x05;
        cpu.x = 0x05;
        cpu.status &= !0x01; // clear carry
        assert_eq!(cpu.step(), 4);
        assert_eq!(cpu.a, 0x15);

        // Test ADC absolute,X (0x7D)
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.write(0x2005, 0x20);
        cpu2.memory.load_program(0x8000, &[0x7D, 0x00, 0x20]); // ADC $2000,X
        cpu2.reset();
        cpu2.a = 0x10;
        cpu2.x = 0x05;
        cpu2.status &= !0x01;
        assert_eq!(cpu2.step(), 4);
        assert_eq!(cpu2.a, 0x30);

        // Test ADC absolute,Y (0x79)
        let mem3 = ArrayMemory::new();
        let mut cpu3 = Cpu6502::new(mem3);
        cpu3.memory.write(0x2003, 0x0F);
        cpu3.memory.load_program(0x8000, &[0x79, 0x00, 0x20]); // ADC $2000,Y
        cpu3.reset();
        cpu3.a = 0x01;
        cpu3.y = 0x03;
        cpu3.status &= !0x01;
        assert_eq!(cpu3.step(), 4);
        assert_eq!(cpu3.a, 0x10);

        // Test ADC (indirect,X) (0x61)
        let mem4 = ArrayMemory::new();
        let mut cpu4 = Cpu6502::new(mem4);
        cpu4.memory.write(0x0015, 0x00); // pointer low
        cpu4.memory.write(0x0016, 0x30); // pointer high -> $3000
        cpu4.memory.write(0x3000, 0x42);
        cpu4.memory.load_program(0x8000, &[0x61, 0x10]); // ADC ($10,X)
        cpu4.reset();
        cpu4.a = 0x08;
        cpu4.x = 0x05; // 0x10 + 0x05 = 0x15
        cpu4.status &= !0x01;
        assert_eq!(cpu4.step(), 6);
        assert_eq!(cpu4.a, 0x4A);

        // Test ADC (indirect),Y (0x71)
        let mem5 = ArrayMemory::new();
        let mut cpu5 = Cpu6502::new(mem5);
        cpu5.memory.write(0x0020, 0x00); // pointer low
        cpu5.memory.write(0x0021, 0x30); // pointer high -> $3000
        cpu5.memory.write(0x3002, 0x33);
        cpu5.memory.load_program(0x8000, &[0x71, 0x20]); // ADC ($20),Y
        cpu5.reset();
        cpu5.a = 0x0D;
        cpu5.y = 0x02; // $3000 + 0x02 = $3002
        cpu5.status &= !0x01;
        assert_eq!(cpu5.step(), 5);
        assert_eq!(cpu5.a, 0x40);
    }

    #[test]
    fn shift_and_rotate_indexed_modes() {
        // Test ASL zero page,X (0x16)
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.write(0x0015, 0x40);
        cpu.memory.load_program(0x8000, &[0x16, 0x10]); // ASL $10,X
        cpu.reset();
        cpu.x = 0x05;
        assert_eq!(cpu.step(), 6);
        assert_eq!(cpu.memory.read(0x0015), 0x80);
        assert_eq!(cpu.status & 0x01, 0); // no carry
        assert_eq!(cpu.status & 0x80, 0x80); // negative

        // Test ASL absolute,X (0x1E)
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.write(0x2005, 0x81);
        cpu2.memory.load_program(0x8000, &[0x1E, 0x00, 0x20]); // ASL $2000,X
        cpu2.reset();
        cpu2.x = 0x05;
        assert_eq!(cpu2.step(), 7);
        assert_eq!(cpu2.memory.read(0x2005), 0x02);
        assert_eq!(cpu2.status & 0x01, 0x01); // carry set

        // Test LSR zero page,X (0x56)
        let mem3 = ArrayMemory::new();
        let mut cpu3 = Cpu6502::new(mem3);
        cpu3.memory.write(0x0012, 0x82);
        cpu3.memory.load_program(0x8000, &[0x56, 0x10]); // LSR $10,X
        cpu3.reset();
        cpu3.x = 0x02;
        assert_eq!(cpu3.step(), 6);
        assert_eq!(cpu3.memory.read(0x0012), 0x41);
        assert_eq!(cpu3.status & 0x01, 0); // no carry

        // Test LSR absolute,X (0x5E)
        let mem4 = ArrayMemory::new();
        let mut cpu4 = Cpu6502::new(mem4);
        cpu4.memory.write(0x2010, 0x03);
        cpu4.memory.load_program(0x8000, &[0x5E, 0x00, 0x20]); // LSR $2000,X
        cpu4.reset();
        cpu4.x = 0x10;
        assert_eq!(cpu4.step(), 7);
        assert_eq!(cpu4.memory.read(0x2010), 0x01);
        assert_eq!(cpu4.status & 0x01, 0x01); // carry set

        // Test ROL zero page,X (0x36)
        let mem5 = ArrayMemory::new();
        let mut cpu5 = Cpu6502::new(mem5);
        cpu5.memory.write(0x0015, 0x80);
        cpu5.memory.load_program(0x8000, &[0x36, 0x10]); // ROL $10,X
        cpu5.reset();
        cpu5.x = 0x05;
        cpu5.status |= 0x01; // set carry
        assert_eq!(cpu5.step(), 6);
        assert_eq!(cpu5.memory.read(0x0015), 0x01);
        assert_eq!(cpu5.status & 0x01, 0x01); // carry still set

        // Test ROL absolute,X (0x3E)
        let mem6 = ArrayMemory::new();
        let mut cpu6 = Cpu6502::new(mem6);
        cpu6.memory.write(0x2020, 0x40);
        cpu6.memory.load_program(0x8000, &[0x3E, 0x00, 0x20]); // ROL $2000,X
        cpu6.reset();
        cpu6.x = 0x20;
        cpu6.status &= !0x01; // clear carry
        assert_eq!(cpu6.step(), 7);
        assert_eq!(cpu6.memory.read(0x2020), 0x80);

        // Test ROR zero page,X (0x76)
        let mem7 = ArrayMemory::new();
        let mut cpu7 = Cpu6502::new(mem7);
        cpu7.memory.write(0x0018, 0x01);
        cpu7.memory.load_program(0x8000, &[0x76, 0x10]); // ROR $10,X
        cpu7.reset();
        cpu7.x = 0x08;
        cpu7.status |= 0x01; // set carry
        assert_eq!(cpu7.step(), 6);
        assert_eq!(cpu7.memory.read(0x0018), 0x80);
        assert_eq!(cpu7.status & 0x01, 0x01); // carry still set

        // Test ROR absolute,X (0x7E)
        let mem8 = ArrayMemory::new();
        let mut cpu8 = Cpu6502::new(mem8);
        cpu8.memory.write(0x2030, 0x02);
        cpu8.memory.load_program(0x8000, &[0x7E, 0x00, 0x20]); // ROR $2000,X
        cpu8.reset();
        cpu8.x = 0x30;
        cpu8.status &= !0x01; // clear carry
        assert_eq!(cpu8.step(), 7);
        assert_eq!(cpu8.memory.read(0x2030), 0x01);
    }

    #[test]
    fn logical_ops_absolute_x() {
        // Test ORA absolute,X (0x1D)
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.write(0x2005, 0xF0);
        cpu.memory.load_program(0x8000, &[0x1D, 0x00, 0x20]); // ORA $2000,X
        cpu.reset();
        cpu.a = 0x0F;
        cpu.x = 0x05;
        assert_eq!(cpu.step(), 4);
        assert_eq!(cpu.a, 0xFF);

        // Test EOR absolute,X (0x5D)
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.write(0x2010, 0xAA);
        cpu2.memory.load_program(0x8000, &[0x5D, 0x00, 0x20]); // EOR $2000,X
        cpu2.reset();
        cpu2.a = 0xFF;
        cpu2.x = 0x10;
        assert_eq!(cpu2.step(), 4);
        assert_eq!(cpu2.a, 0x55);
    }

    #[test]
    fn sbc_indexed_modes() {
        // Test SBC zero page,X (0xF5)
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.write(0x0015, 0x05);
        cpu.memory.load_program(0x8000, &[0xF5, 0x10]); // SBC $10,X
        cpu.reset();
        cpu.a = 0x10;
        cpu.x = 0x05;
        cpu.status |= 0x01; // set carry (no borrow)
        assert_eq!(cpu.step(), 4);
        assert_eq!(cpu.a, 0x0B);

        // Test SBC absolute,X (0xFD)
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.write(0x2020, 0x08);
        cpu2.memory.load_program(0x8000, &[0xFD, 0x00, 0x20]); // SBC $2000,X
        cpu2.reset();
        cpu2.a = 0x20;
        cpu2.x = 0x20;
        cpu2.status |= 0x01;
        assert_eq!(cpu2.step(), 4);
        assert_eq!(cpu2.a, 0x18);

        // Test SBC absolute,Y (0xF9)
        let mem3 = ArrayMemory::new();
        let mut cpu3 = Cpu6502::new(mem3);
        cpu3.memory.write(0x2015, 0x0A);
        cpu3.memory.load_program(0x8000, &[0xF9, 0x00, 0x20]); // SBC $2000,Y
        cpu3.reset();
        cpu3.a = 0x1A;
        cpu3.y = 0x15;
        cpu3.status |= 0x01;
        assert_eq!(cpu3.step(), 4);
        assert_eq!(cpu3.a, 0x10);
    }

    #[test]
    fn sed_flag_operation() {
        // Test SED (0xF8)
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.memory.load_program(0x8000, &[0xF8]); // SED
        cpu.reset();
        cpu.status &= !0x08; // clear decimal flag
        assert_eq!(cpu.step(), 2);
        assert_eq!(cpu.status & 0x08, 0x08); // decimal flag set

        // Verify CLD still works
        let mem2 = ArrayMemory::new();
        let mut cpu2 = Cpu6502::new(mem2);
        cpu2.memory.load_program(0x8000, &[0xD8]); // CLD
        cpu2.reset();
        cpu2.status |= 0x08; // set decimal flag
        assert_eq!(cpu2.step(), 2);
        assert_eq!(cpu2.status & 0x08, 0); // decimal flag cleared
    }

    #[test]
    fn lda_indirect_x_and_indirect_y() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.reset();
        // set up pointer table in zero page: at $20 store pointer to $2000
        cpu.memory.write(0x0020, 0x00);
        cpu.memory.write(0x0021, 0x20);
        // place value at $2000
        cpu.memory.write(0x2000, 0xAB);
        // place operand for (indirect,X) at $10 such that (10 + X) -> 20
        // set X = 0x06, operand = 0x0A -> 0x0A + 0x06 = 0x10 -> pointer at 0x10
        cpu.memory.write(0x0010, 0x00);
        cpu.memory.write(0x0011, 0x20);
        // test (indirect,X): set X then LDA (zp,X)
        cpu.x = 6;
        cpu.memory.load_program(0x8000, &[0xA1, 0x0A]);
        cpu.reset();
        cpu.x = 6;
        assert_eq!(cpu.step(), 6);
        assert_eq!(cpu.a, 0xAB);

        // test (indirect),Y: pointer at $20 points to 0x2000, Y = 0
        cpu.memory.load_program(0x8000, &[0xB1, 0x20]);
        cpu.reset();
        cpu.y = 0;
        assert_eq!(cpu.step(), 5);
        assert_eq!(cpu.a, 0xAB);
    }

    #[test]
    fn and_ora_eor_and_cmp_asl_lsr() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.reset();
        // AND immediate
        cpu.a = 0xF0;
        cpu.memory.load_program(0x8000, &[0x29, 0x0F]);
        cpu.reset();
        cpu.a = 0xF0;
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status & 0x02, 0x02); // zero

        // ORA immediate
        cpu.a = 0x0F;
        cpu.memory.load_program(0x8000, &[0x09, 0xF0]);
        cpu.reset();
        cpu.a = 0x0F;
        cpu.step();
        assert_eq!(cpu.a, 0xFF);

        // EOR immediate
        cpu.a = 0xFF;
        cpu.memory.load_program(0x8000, &[0x49, 0x0F]);
        cpu.reset();
        cpu.a = 0xFF;
        cpu.step();
        assert_eq!(cpu.a, 0xF0);

        // CMP immediate (A >= M)
        cpu.a = 0x10;
        cpu.memory.load_program(0x8000, &[0xC9, 0x0F]);
        cpu.reset();
        cpu.a = 0x10;
        cpu.step();
        assert_eq!(cpu.status & 0x01, 0x01);

        // ASL accumulator
        cpu.a = 0x80;
        cpu.memory.load_program(0x8000, &[0x0A]);
        cpu.reset();
        cpu.a = 0x80;
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status & 0x01, 0x01); // carry set

        // LSR accumulator
        cpu.a = 0x01;
        cpu.memory.load_program(0x8000, &[0x4A]);
        cpu.reset();
        cpu.a = 0x01;
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status & 0x01, 0x01);
    }

    #[test]
    fn jmp_indirect_page_wrap_bug() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.reset();
        // program: JMP ($80FF) placed at 0x8100 so it doesn't overwrite the pointer bytes
        cpu.memory.load_program(0x8100, &[0x6C, 0xFF, 0x80]);
        // place indirect pointer at 0x80FF -> low byte at 0x80FF, high byte should wrap to 0x8000
        cpu.memory.write(0x80FF, 0x34);
        cpu.memory.write(0x8000, 0x12); // wrapped high byte
                                        // ensure PC points to our program start
        cpu.pc = 0x8100;
        cpu.step();
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn rol_ror_and_sbc_cpx_cpy_branches() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.reset();
        // ROL accumulator
        cpu.a = 0x80;
        cpu.memory.load_program(0x8000, &[0x2A]);
        cpu.reset();
        cpu.a = 0x80;
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status & 0x01, 0x01);

        // ROR accumulator
        cpu.a = 0x01;
        cpu.status &= !0x01;
        cpu.memory.load_program(0x8000, &[0x6A]);
        cpu.reset();
        cpu.a = 0x01;
        cpu.status &= !0x01;
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status & 0x01, 0x01);

        // SBC immediate: 0x10 - 0x01 = 0x0F
        cpu.a = 0x10;
        cpu.status |= 0x01; // carry set
        cpu.memory.load_program(0x8000, &[0xE9, 0x01]);
        cpu.reset();
        cpu.a = 0x10;
        cpu.status |= 0x01; // carry set
        cpu.step();
        assert_eq!(cpu.a, 0x0F);

        // CPX immediate
        cpu.x = 0x05;
        cpu.memory.load_program(0x8000, &[0xE0, 0x05]);
        cpu.reset();
        cpu.x = 0x05;
        cpu.step();
        assert_eq!(cpu.status & 0x02, 0x02);

        // CPY immediate
        cpu.y = 0x03;
        cpu.memory.load_program(0x8000, &[0xC0, 0x03]);
        cpu.reset();
        cpu.y = 0x03;
        cpu.step();
        assert_eq!(cpu.status & 0x02, 0x02);

        // Branch BCS taken
        cpu.status |= 0x01;
        cpu.memory.load_program(0x8000, &[0xB0, 0x01, 0xEA, 0xEA]);
        cpu.reset();
        cpu.status |= 0x01;
        cpu.step();
        assert_eq!(cpu.step(), 2); // land on the last NOP
    }

    #[test]
    fn lda_absolute_reads_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu6502::new(mem);
        cpu.reset();
        // Place value at 0x1234, then LDA $1234
        cpu.memory.write(0x1234, 0x99);
        cpu.memory.load_program(0x8000, &[0xAD, 0x34, 0x12]);
        cpu.reset();
        assert_eq!(cpu.step(), 4);
        assert_eq!(cpu.a, 0x99);
    }
}
