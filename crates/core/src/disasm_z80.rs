//! Z80 CPU disassembler
//!
//! Provides comprehensive instruction disassembly for the Zilog Z80 CPU,
//! including all documented instructions, DD/FD (IX/IY) prefixes, the CB
//! and ED prefix tables, the DDCB/FDCB compound prefix, and common
//! undocumented instructions (IXH/IXL/IYH/IYL register access).

use crate::debug::DisassembledInstruction;

// ── helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn make(address: u32, bytes: &[u8], mnemonic: String) -> DisassembledInstruction {
    DisassembledInstruction::new(address, bytes.to_vec(), mnemonic)
}

/// Standard Z80 8-bit register name (0=B … 7=A).
#[inline]
fn reg(r: u8) -> &'static str {
    match r & 7 {
        0 => "B",
        1 => "C",
        2 => "D",
        3 => "E",
        4 => "H",
        5 => "L",
        6 => "(HL)",
        _ => "A",
    }
}

/// Format a signed displacement as "(XY+$xx)" or "(XY-$xx)".
fn fmt_disp(xy: &str, d: i8) -> String {
    if d >= 0 {
        format!("({}+${:02X})", xy, d as u8)
    } else {
        format!("({}-${:02X})", xy, d.unsigned_abs())
    }
}

/// Compute the absolute target of a relative jump (JR/DJNZ).
#[inline]
fn jr_target(address: u32, offset: i8) -> u16 {
    ((address as i32) + 2 + (offset as i32)) as u16
}

/// Build a standard ALU mnemonic (ADD/ADC/SUB/SBC/AND/XOR/OR/CP).
fn alu_op(op: u8, operand: &str) -> String {
    match op & 7 {
        0 => format!("ADD A,{}", operand),
        1 => format!("ADC A,{}", operand),
        2 => format!("SUB {}", operand),
        3 => format!("SBC A,{}", operand),
        4 => format!("AND {}", operand),
        5 => format!("XOR {}", operand),
        6 => format!("OR {}", operand),
        _ => format!("CP {}", operand),
    }
}

// ── public entry point ────────────────────────────────────────────────────────

/// Disassemble a single Z80 instruction.
///
/// `memory` must start at the first byte of the instruction; `address` is the
/// program-counter value of that byte.  Returns `None` if `memory` is too short
/// to decode the instruction.
pub fn disassemble_z80(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    match opcode {
        0xCB => return disassemble_cb(memory, address),
        0xDD => return disassemble_xy(memory, address, "IX"),
        0xED => return disassemble_ed(memory, address),
        0xFD => return disassemble_xy(memory, address, "IY"),
        _ => {}
    }

    // ── 0x40–0x7F  LD r,r'  (and HALT) ──────────────────────────────────────
    if (0x40..=0x7F).contains(&opcode) {
        if opcode == 0x76 {
            return Some(make(address, &memory[..1], "HALT".into()));
        }
        let dst = (opcode >> 3) & 7;
        let src = opcode & 7;
        return Some(make(
            address,
            &memory[..1],
            format!("LD {},{}", reg(dst), reg(src)),
        ));
    }

    // ── 0x80–0xBF  ALU A,r ───────────────────────────────────────────────────
    if (0x80..=0xBF).contains(&opcode) {
        let op = (opcode >> 3) & 7;
        let r = opcode & 7;
        return Some(make(address, &memory[..1], alu_op(op, reg(r))));
    }

    // ── main opcode table ─────────────────────────────────────────────────────
    macro_rules! need {
        ($n:expr) => {
            if memory.len() < $n {
                return None;
            }
        };
    }
    macro_rules! nn {
        () => {{
            need!(3);
            u16::from_le_bytes([memory[1], memory[2]])
        }};
    }
    macro_rules! n {
        () => {{
            need!(2);
            memory[1]
        }};
    }

    let (mnemonic, len): (String, usize) = match opcode {
        0x00 => ("NOP".into(), 1),
        0x01 => (format!("LD BC,${:04X}", nn!()), 3),
        0x02 => ("LD (BC),A".into(), 1),
        0x03 => ("INC BC".into(), 1),
        0x04 => ("INC B".into(), 1),
        0x05 => ("DEC B".into(), 1),
        0x06 => (format!("LD B,${:02X}", n!()), 2),
        0x07 => ("RLCA".into(), 1),
        0x08 => ("EX AF,AF'".into(), 1),
        0x09 => ("ADD HL,BC".into(), 1),
        0x0A => ("LD A,(BC)".into(), 1),
        0x0B => ("DEC BC".into(), 1),
        0x0C => ("INC C".into(), 1),
        0x0D => ("DEC C".into(), 1),
        0x0E => (format!("LD C,${:02X}", n!()), 2),
        0x0F => ("RRCA".into(), 1),

        0x10 => {
            let t = jr_target(address, n!() as i8);
            (format!("DJNZ ${:04X}", t), 2)
        }
        0x11 => (format!("LD DE,${:04X}", nn!()), 3),
        0x12 => ("LD (DE),A".into(), 1),
        0x13 => ("INC DE".into(), 1),
        0x14 => ("INC D".into(), 1),
        0x15 => ("DEC D".into(), 1),
        0x16 => (format!("LD D,${:02X}", n!()), 2),
        0x17 => ("RLA".into(), 1),
        0x18 => {
            let t = jr_target(address, n!() as i8);
            (format!("JR ${:04X}", t), 2)
        }
        0x19 => ("ADD HL,DE".into(), 1),
        0x1A => ("LD A,(DE)".into(), 1),
        0x1B => ("DEC DE".into(), 1),
        0x1C => ("INC E".into(), 1),
        0x1D => ("DEC E".into(), 1),
        0x1E => (format!("LD E,${:02X}", n!()), 2),
        0x1F => ("RRA".into(), 1),

        0x20 => {
            let t = jr_target(address, n!() as i8);
            (format!("JR NZ,${:04X}", t), 2)
        }
        0x21 => (format!("LD HL,${:04X}", nn!()), 3),
        0x22 => (format!("LD (${:04X}),HL", nn!()), 3),
        0x23 => ("INC HL".into(), 1),
        0x24 => ("INC H".into(), 1),
        0x25 => ("DEC H".into(), 1),
        0x26 => (format!("LD H,${:02X}", n!()), 2),
        0x27 => ("DAA".into(), 1),
        0x28 => {
            let t = jr_target(address, n!() as i8);
            (format!("JR Z,${:04X}", t), 2)
        }
        0x29 => ("ADD HL,HL".into(), 1),
        0x2A => (format!("LD HL,(${:04X})", nn!()), 3),
        0x2B => ("DEC HL".into(), 1),
        0x2C => ("INC L".into(), 1),
        0x2D => ("DEC L".into(), 1),
        0x2E => (format!("LD L,${:02X}", n!()), 2),
        0x2F => ("CPL".into(), 1),

        0x30 => {
            let t = jr_target(address, n!() as i8);
            (format!("JR NC,${:04X}", t), 2)
        }
        0x31 => (format!("LD SP,${:04X}", nn!()), 3),
        0x32 => (format!("LD (${:04X}),A", nn!()), 3),
        0x33 => ("INC SP".into(), 1),
        0x34 => ("INC (HL)".into(), 1),
        0x35 => ("DEC (HL)".into(), 1),
        0x36 => (format!("LD (HL),${:02X}", n!()), 2),
        0x37 => ("SCF".into(), 1),
        0x38 => {
            let t = jr_target(address, n!() as i8);
            (format!("JR C,${:04X}", t), 2)
        }
        0x39 => ("ADD HL,SP".into(), 1),
        0x3A => (format!("LD A,(${:04X})", nn!()), 3),
        0x3B => ("DEC SP".into(), 1),
        0x3C => ("INC A".into(), 1),
        0x3D => ("DEC A".into(), 1),
        0x3E => (format!("LD A,${:02X}", n!()), 2),
        0x3F => ("CCF".into(), 1),

        // 0x40–0xBF handled above
        0xC0 => ("RET NZ".into(), 1),
        0xC1 => ("POP BC".into(), 1),
        0xC2 => (format!("JP NZ,${:04X}", nn!()), 3),
        0xC3 => (format!("JP ${:04X}", nn!()), 3),
        0xC4 => (format!("CALL NZ,${:04X}", nn!()), 3),
        0xC5 => ("PUSH BC".into(), 1),
        0xC6 => (format!("ADD A,${:02X}", n!()), 2),
        0xC7 => ("RST $00".into(), 1),
        0xC8 => ("RET Z".into(), 1),
        0xC9 => ("RET".into(), 1),
        0xCA => (format!("JP Z,${:04X}", nn!()), 3),
        // 0xCB handled above
        0xCC => (format!("CALL Z,${:04X}", nn!()), 3),
        0xCD => (format!("CALL ${:04X}", nn!()), 3),
        0xCE => (format!("ADC A,${:02X}", n!()), 2),
        0xCF => ("RST $08".into(), 1),

        0xD0 => ("RET NC".into(), 1),
        0xD1 => ("POP DE".into(), 1),
        0xD2 => (format!("JP NC,${:04X}", nn!()), 3),
        0xD3 => (format!("OUT (${:02X}),A", n!()), 2),
        0xD4 => (format!("CALL NC,${:04X}", nn!()), 3),
        0xD5 => ("PUSH DE".into(), 1),
        0xD6 => (format!("SUB ${:02X}", n!()), 2),
        0xD7 => ("RST $10".into(), 1),
        0xD8 => ("RET C".into(), 1),
        0xD9 => ("EXX".into(), 1),
        0xDA => (format!("JP C,${:04X}", nn!()), 3),
        0xDB => (format!("IN A,(${:02X})", n!()), 2),
        0xDC => (format!("CALL C,${:04X}", nn!()), 3),
        // 0xDD handled above
        0xDE => (format!("SBC A,${:02X}", n!()), 2),
        0xDF => ("RST $18".into(), 1),

        0xE0 => ("RET PO".into(), 1),
        0xE1 => ("POP HL".into(), 1),
        0xE2 => (format!("JP PO,${:04X}", nn!()), 3),
        0xE3 => ("EX (SP),HL".into(), 1),
        0xE4 => (format!("CALL PO,${:04X}", nn!()), 3),
        0xE5 => ("PUSH HL".into(), 1),
        0xE6 => (format!("AND ${:02X}", n!()), 2),
        0xE7 => ("RST $20".into(), 1),
        0xE8 => ("RET PE".into(), 1),
        0xE9 => ("JP (HL)".into(), 1),
        0xEA => (format!("JP PE,${:04X}", nn!()), 3),
        0xEB => ("EX DE,HL".into(), 1),
        0xEC => (format!("CALL PE,${:04X}", nn!()), 3),
        // 0xED handled above
        0xEE => (format!("XOR ${:02X}", n!()), 2),
        0xEF => ("RST $28".into(), 1),

        0xF0 => ("RET P".into(), 1),
        0xF1 => ("POP AF".into(), 1),
        0xF2 => (format!("JP P,${:04X}", nn!()), 3),
        0xF3 => ("DI".into(), 1),
        0xF4 => (format!("CALL P,${:04X}", nn!()), 3),
        0xF5 => ("PUSH AF".into(), 1),
        0xF6 => (format!("OR ${:02X}", n!()), 2),
        0xF7 => ("RST $30".into(), 1),
        0xF8 => ("RET M".into(), 1),
        0xF9 => ("LD SP,HL".into(), 1),
        0xFA => (format!("JP M,${:04X}", nn!()), 3),
        0xFB => ("EI".into(), 1),
        0xFC => (format!("CALL M,${:04X}", nn!()), 3),
        // 0xFD handled above
        0xFE => (format!("CP ${:02X}", n!()), 2),
        0xFF => ("RST $38".into(), 1),

        _ => (format!("DB ${:02X}", opcode), 1),
    };

    Some(make(address, &memory[..len], mnemonic))
}

// ── CB prefix ─────────────────────────────────────────────────────────────────

fn disassemble_cb(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    let op = memory[1];
    let bit = (op >> 3) & 7;
    let r = op & 7;
    let rn = reg(r);

    let mnemonic = match op >> 6 {
        0 => match bit {
            0 => format!("RLC {}", rn),
            1 => format!("RRC {}", rn),
            2 => format!("RL {}", rn),
            3 => format!("RR {}", rn),
            4 => format!("SLA {}", rn),
            5 => format!("SRA {}", rn),
            6 => format!("SLL {}", rn), // undocumented
            _ => format!("SRL {}", rn),
        },
        1 => format!("BIT {},{}", bit, rn),
        2 => format!("RES {},{}", bit, rn),
        _ => format!("SET {},{}", bit, rn),
    };

    Some(make(address, &memory[..2], mnemonic))
}

// ── DD/FD prefix  (IX / IY) ──────────────────────────────────────────────────

fn disassemble_xy(memory: &[u8], address: u32, xy: &str) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    let op = memory[1];

    if op == 0xCB {
        return disassemble_xycb(memory, address, xy);
    }

    let xyh: &str = if xy == "IX" { "IXH" } else { "IYH" };
    let xyl: &str = if xy == "IX" { "IXL" } else { "IYL" };

    // Displacement helpers (memory[2] is the displacement byte for (XY+d))
    macro_rules! disp {
        () => {{
            if memory.len() < 3 {
                return None;
            }
            memory[2] as i8
        }};
    }
    macro_rules! ds {
        () => {{
            fmt_disp(xy, disp!())
        }};
    }

    // LD r,(XY+d) : op = 0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E | 0x76
    // LD (XY+d),r : op = 0x70–0x77
    // ALU (XY+d)  : op = 0x86 | 0x8E | … | 0xBE

    let (mnemonic, len): (String, usize) = match op {
        // ADD XY,rr
        0x09 => (format!("ADD {},BC", xy), 2),
        0x19 => (format!("ADD {},DE", xy), 2),
        0x29 => (format!("ADD {},{}", xy, xy), 2),
        0x39 => (format!("ADD {},SP", xy), 2),

        // INC/DEC 16-bit
        0x23 => (format!("INC {}", xy), 2),
        0x2B => (format!("DEC {}", xy), 2),

        // LD XY,nn
        0x21 => {
            if memory.len() < 4 {
                return None;
            }
            let nn = u16::from_le_bytes([memory[2], memory[3]]);
            (format!("LD {},${:04X}", xy, nn), 4)
        }
        // LD (nn),XY
        0x22 => {
            if memory.len() < 4 {
                return None;
            }
            let nn = u16::from_le_bytes([memory[2], memory[3]]);
            (format!("LD (${:04X}),{}", nn, xy), 4)
        }
        // LD XY,(nn)
        0x2A => {
            if memory.len() < 4 {
                return None;
            }
            let nn = u16::from_le_bytes([memory[2], memory[3]]);
            (format!("LD {},(${:04X})", xy, nn), 4)
        }

        // INC/DEC XYH/XYL (undocumented)
        0x24 => (format!("INC {}", xyh), 2),
        0x25 => (format!("DEC {}", xyh), 2),
        0x2C => (format!("INC {}", xyl), 2),
        0x2D => (format!("DEC {}", xyl), 2),

        // LD XYH,n / LD XYL,n (undocumented)
        0x26 => {
            if memory.len() < 3 {
                return None;
            }
            (format!("LD {},${:02X}", xyh, memory[2]), 3)
        }
        0x2E => {
            if memory.len() < 3 {
                return None;
            }
            (format!("LD {},${:02X}", xyl, memory[2]), 3)
        }

        // INC/DEC (XY+d)
        0x34 => (format!("INC {}", ds!()), 3),
        0x35 => (format!("DEC {}", ds!()), 3),

        // LD (XY+d),n
        0x36 => {
            if memory.len() < 4 {
                return None;
            }
            (
                format!("LD {},${:02X}", fmt_disp(xy, memory[2] as i8), memory[3]),
                4,
            )
        }

        // LD r,(XY+d)
        0x46 => (format!("LD B,{}", ds!()), 3),
        0x4E => (format!("LD C,{}", ds!()), 3),
        0x56 => (format!("LD D,{}", ds!()), 3),
        0x5E => (format!("LD E,{}", ds!()), 3),
        0x66 => (format!("LD H,{}", ds!()), 3),
        0x6E => (format!("LD L,{}", ds!()), 3),
        0x7E => (format!("LD A,{}", ds!()), 3),

        // LD (XY+d),r
        0x70 => (format!("LD {},B", ds!()), 3),
        0x71 => (format!("LD {},C", ds!()), 3),
        0x72 => (format!("LD {},D", ds!()), 3),
        0x73 => (format!("LD {},E", ds!()), 3),
        0x74 => (format!("LD {},H", ds!()), 3),
        0x75 => (format!("LD {},L", ds!()), 3),
        0x77 => (format!("LD {},A", ds!()), 3),

        // LD r,XYH / LD r,XYL (undocumented)
        0x44 => (format!("LD B,{}", xyh), 2),
        0x45 => (format!("LD B,{}", xyl), 2),
        0x4C => (format!("LD C,{}", xyh), 2),
        0x4D => (format!("LD C,{}", xyl), 2),
        0x54 => (format!("LD D,{}", xyh), 2),
        0x55 => (format!("LD D,{}", xyl), 2),
        0x5C => (format!("LD E,{}", xyh), 2),
        0x5D => (format!("LD E,{}", xyl), 2),
        0x7C => (format!("LD A,{}", xyh), 2),
        0x7D => (format!("LD A,{}", xyl), 2),

        // LD XYH,r / LD XYL,r (undocumented)
        0x60 => (format!("LD {},B", xyh), 2),
        0x61 => (format!("LD {},C", xyh), 2),
        0x62 => (format!("LD {},D", xyh), 2),
        0x63 => (format!("LD {},E", xyh), 2),
        0x64 => (format!("LD {},{}", xyh, xyh), 2),
        0x65 => (format!("LD {},{}", xyh, xyl), 2),
        0x67 => (format!("LD {},A", xyh), 2),
        0x68 => (format!("LD {},B", xyl), 2),
        0x69 => (format!("LD {},C", xyl), 2),
        0x6A => (format!("LD {},D", xyl), 2),
        0x6B => (format!("LD {},E", xyl), 2),
        0x6C => (format!("LD {},{}", xyl, xyh), 2),
        0x6D => (format!("LD {},{}", xyl, xyl), 2),
        0x6F => (format!("LD {},A", xyl), 2),

        // ALU A,(XY+d)
        0x86 => (format!("ADD A,{}", ds!()), 3),
        0x8E => (format!("ADC A,{}", ds!()), 3),
        0x96 => (format!("SUB {}", ds!()), 3),
        0x9E => (format!("SBC A,{}", ds!()), 3),
        0xA6 => (format!("AND {}", ds!()), 3),
        0xAE => (format!("XOR {}", ds!()), 3),
        0xB6 => (format!("OR {}", ds!()), 3),
        0xBE => (format!("CP {}", ds!()), 3),

        // ALU A,XYH / ALU A,XYL (undocumented)
        0x84 => (format!("ADD A,{}", xyh), 2),
        0x85 => (format!("ADD A,{}", xyl), 2),
        0x8C => (format!("ADC A,{}", xyh), 2),
        0x8D => (format!("ADC A,{}", xyl), 2),
        0x94 => (format!("SUB {}", xyh), 2),
        0x95 => (format!("SUB {}", xyl), 2),
        0x9C => (format!("SBC A,{}", xyh), 2),
        0x9D => (format!("SBC A,{}", xyl), 2),
        0xA4 => (format!("AND {}", xyh), 2),
        0xA5 => (format!("AND {}", xyl), 2),
        0xAC => (format!("XOR {}", xyh), 2),
        0xAD => (format!("XOR {}", xyl), 2),
        0xB4 => (format!("OR {}", xyh), 2),
        0xB5 => (format!("OR {}", xyl), 2),
        0xBC => (format!("CP {}", xyh), 2),
        0xBD => (format!("CP {}", xyl), 2),

        // Stack / jump / misc
        0xE1 => (format!("POP {}", xy), 2),
        0xE3 => (format!("EX (SP),{}", xy), 2),
        0xE5 => (format!("PUSH {}", xy), 2),
        0xE9 => (format!("JP ({})", xy), 2),
        0xF9 => (format!("LD SP,{}", xy), 2),

        _ => {
            // Treat as if the prefix were absent (prefix is effectively a NOP
            // for opcodes that don't use H/L/(HL)).
            let no_prefix = disassemble_z80(&memory[1..], address + 1)?;
            let mut bytes = vec![memory[0]];
            bytes.extend_from_slice(&no_prefix.bytes);
            return Some(make(address, &bytes, no_prefix.mnemonic));
        }
    };

    Some(make(address, &memory[..len], mnemonic))
}

// ── DDCB / FDCB prefix ───────────────────────────────────────────────────────

fn disassemble_xycb(memory: &[u8], address: u32, xy: &str) -> Option<DisassembledInstruction> {
    // Layout: [prefix, 0xCB, disp, op]  → 4 bytes total
    if memory.len() < 4 {
        return None;
    }
    let d = memory[2] as i8;
    let op = memory[3];
    let ds = fmt_disp(xy, d);

    let bit = (op >> 3) & 7;
    let r = op & 7;
    // Standard target is (XY+d); undocumented variants also write to a register.
    let extra = if r != 6 {
        format!(",{}", reg(r))
    } else {
        String::new()
    };

    let mnemonic = match op >> 6 {
        0 => {
            let name = match bit {
                0 => "RLC",
                1 => "RRC",
                2 => "RL",
                3 => "RR",
                4 => "SLA",
                5 => "SRA",
                6 => "SLL", // undocumented
                _ => "SRL",
            };
            format!("{} {}{}", name, ds, extra)
        }
        1 => format!("BIT {},{}", bit, ds), // BIT never writes a register
        2 => format!("RES {},{}{}", bit, ds, extra),
        _ => format!("SET {},{}{}", bit, ds, extra),
    };

    Some(make(address, &memory[..4], mnemonic))
}

// ── ED prefix ────────────────────────────────────────────────────────────────

fn disassemble_ed(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    let op = memory[1];

    macro_rules! nn {
        () => {{
            if memory.len() < 4 {
                return None;
            }
            u16::from_le_bytes([memory[2], memory[3]])
        }};
    }

    let (mnemonic, len): (String, usize) = match op {
        // IN r,(C)
        0x40 => ("IN B,(C)".into(), 2),
        0x48 => ("IN C,(C)".into(), 2),
        0x50 => ("IN D,(C)".into(), 2),
        0x58 => ("IN E,(C)".into(), 2),
        0x60 => ("IN H,(C)".into(), 2),
        0x68 => ("IN L,(C)".into(), 2),
        0x70 => ("IN F,(C)".into(), 2), // undocumented – reads flags
        0x78 => ("IN A,(C)".into(), 2),

        // OUT (C),r
        0x41 => ("OUT (C),B".into(), 2),
        0x49 => ("OUT (C),C".into(), 2),
        0x51 => ("OUT (C),D".into(), 2),
        0x59 => ("OUT (C),E".into(), 2),
        0x61 => ("OUT (C),H".into(), 2),
        0x69 => ("OUT (C),L".into(), 2),
        0x71 => ("OUT (C),0".into(), 2), // undocumented – outputs 0
        0x79 => ("OUT (C),A".into(), 2),

        // SBC HL,rr
        0x42 => ("SBC HL,BC".into(), 2),
        0x52 => ("SBC HL,DE".into(), 2),
        0x62 => ("SBC HL,HL".into(), 2),
        0x72 => ("SBC HL,SP".into(), 2),

        // ADC HL,rr
        0x4A => ("ADC HL,BC".into(), 2),
        0x5A => ("ADC HL,DE".into(), 2),
        0x6A => ("ADC HL,HL".into(), 2),
        0x7A => ("ADC HL,SP".into(), 2),

        // LD (nn),rr
        0x43 => (format!("LD (${:04X}),BC", nn!()), 4),
        0x53 => (format!("LD (${:04X}),DE", nn!()), 4),
        0x63 => (format!("LD (${:04X}),HL", nn!()), 4),
        0x73 => (format!("LD (${:04X}),SP", nn!()), 4),

        // LD rr,(nn)
        0x4B => (format!("LD BC,(${:04X})", nn!()), 4),
        0x5B => (format!("LD DE,(${:04X})", nn!()), 4),
        0x6B => (format!("LD HL,(${:04X})", nn!()), 4),
        0x7B => (format!("LD SP,(${:04X})", nn!()), 4),

        // NEG (0x44 is canonical; 0x4C/0x54/0x5C/0x64/0x6C/0x74/0x7C are mirrors)
        0x44 | 0x4C | 0x54 | 0x5C | 0x64 | 0x6C | 0x74 | 0x7C => ("NEG".into(), 2),

        // RETN (0x45 canonical; 0x55/0x65/0x75 mirrors)
        0x45 | 0x55 | 0x65 | 0x75 => ("RETN".into(), 2),

        // RETI (0x4D canonical; 0x5D/0x6D/0x7D mirrors)
        0x4D | 0x5D | 0x6D | 0x7D => ("RETI".into(), 2),

        // IM  (0x4E/0x66/0x6E are IM 0 mirrors)
        0x46 | 0x4E | 0x66 | 0x6E => ("IM 0".into(), 2),
        0x56 | 0x76 => ("IM 1".into(), 2),
        0x5E | 0x7E => ("IM 2".into(), 2),

        // Special loads
        0x47 => ("LD I,A".into(), 2),
        0x4F => ("LD R,A".into(), 2),
        0x57 => ("LD A,I".into(), 2),
        0x5F => ("LD A,R".into(), 2),

        // Rotate decimal
        0x67 => ("RRD".into(), 2),
        0x6F => ("RLD".into(), 2),

        // Block instructions
        0xA0 => ("LDI".into(), 2),
        0xA1 => ("CPI".into(), 2),
        0xA2 => ("INI".into(), 2),
        0xA3 => ("OUTI".into(), 2),
        0xA8 => ("LDD".into(), 2),
        0xA9 => ("CPD".into(), 2),
        0xAA => ("IND".into(), 2),
        0xAB => ("OUTD".into(), 2),
        0xB0 => ("LDIR".into(), 2),
        0xB1 => ("CPIR".into(), 2),
        0xB2 => ("INIR".into(), 2),
        0xB3 => ("OTIR".into(), 2),
        0xB8 => ("LDDR".into(), 2),
        0xB9 => ("CPDR".into(), 2),
        0xBA => ("INDR".into(), 2),
        0xBB => ("OTDR".into(), 2),

        _ => (format!("ED ${:02X}", op), 2),
    };

    Some(make(address, &memory[..len], mnemonic))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0x00];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NOP");
    }

    #[test]
    fn test_disassemble_ld() {
        let memory = [0x3E, 0x42];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert!(
            instr.mnemonic.contains("LD A") && instr.mnemonic.contains("$42"),
            "unexpected mnemonic: {}",
            instr.mnemonic
        );
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jp() {
        let memory = [0xC3, 0x00, 0x10];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert!(
            instr.mnemonic.contains("JP") && instr.mnemonic.contains("$1000"),
            "unexpected mnemonic: {}",
            instr.mnemonic
        );
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_cb_bit() {
        let memory = [0xCB, 0x47];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "BIT 0,A");
    }

    #[test]
    fn test_disassemble_ed() {
        let memory = [0xED, 0x44];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NEG");
    }

    // ── additional coverage ───────────────────────────────────────────────────

    #[test]
    fn test_ld_r_r() {
        // LD B,C  (0x41)
        let m = [0x41];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD B,C");
    }

    #[test]
    fn test_ld_r_hl() {
        // LD A,(HL)  (0x7E)
        let m = [0x7E];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD A,(HL)");
    }

    #[test]
    fn test_halt() {
        let m = [0x76];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "HALT");
    }

    #[test]
    fn test_alu_add_hl() {
        // ADD A,(HL)  (0x86)
        let m = [0x86];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "ADD A,(HL)");
    }

    #[test]
    fn test_jr_nz() {
        // JR NZ, -2  (0x20 0xFE) → target = (0 + 2 + (-2)) & 0xFFFF = 0
        let m = [0x20, 0xFE];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "JR NZ,$0000");
    }

    #[test]
    fn test_djnz() {
        // DJNZ $+5 (0x10 0x03) → target = 0 + 2 + 3 = 5
        let m = [0x10, 0x03];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "DJNZ $0005");
    }

    #[test]
    fn test_dd_ld_ix_nn() {
        // LD IX,$1234  → DD 21 34 12
        let m = [0xDD, 0x21, 0x34, 0x12];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD IX,$1234");
        assert_eq!(i.len(), 4);
    }

    #[test]
    fn test_dd_ld_r_ix_d() {
        // LD B,(IX+$05)  → DD 46 05
        let m = [0xDD, 0x46, 0x05];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD B,(IX+$05)");
        assert_eq!(i.len(), 3);
    }

    #[test]
    fn test_dd_ld_ix_d_neg() {
        // LD (IX-$03),A  → DD 77 FD
        let m = [0xDD, 0x77, 0xFDu8]; // 0xFD as i8 = -3
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD (IX-$03),A");
    }

    #[test]
    fn test_dd_ld_ix_d_imm() {
        // LD (IX+$02),$FF  → DD 36 02 FF
        let m = [0xDD, 0x36, 0x02, 0xFF];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD (IX+$02),$FF");
        assert_eq!(i.len(), 4);
    }

    #[test]
    fn test_dd_add_ix() {
        // ADD IX,BC  → DD 09
        let m = [0xDD, 0x09];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "ADD IX,BC");
    }

    #[test]
    fn test_dd_push_pop_ix() {
        let push = [0xDD, 0xE5];
        let pop = [0xDD, 0xE1];
        assert_eq!(disassemble_z80(&push, 0).unwrap().mnemonic, "PUSH IX");
        assert_eq!(disassemble_z80(&pop, 0).unwrap().mnemonic, "POP IX");
    }

    #[test]
    fn test_dd_jp_ix() {
        let m = [0xDD, 0xE9];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "JP (IX)");
    }

    #[test]
    fn test_fd_ld_iy_d() {
        // LD (IY+$01),B  → FD 70 01
        let m = [0xFD, 0x70, 0x01];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "LD (IY+$01),B");
    }

    #[test]
    fn test_dd_ixh_ixl_undocumented() {
        // LD B,IXH  → DD 44
        let m = [0xDD, 0x44];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "LD B,IXH");
        // LD IXL,A  → DD 6F
        let m2 = [0xDD, 0x6F];
        assert_eq!(disassemble_z80(&m2, 0).unwrap().mnemonic, "LD IXL,A");
    }

    #[test]
    fn test_ddcb_bit() {
        // BIT 3,(IX+$00)  → DD CB 00 5E
        let m = [0xDD, 0xCB, 0x00, 0x5E];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "BIT 3,(IX+$00)");
        assert_eq!(i.len(), 4);
    }

    #[test]
    fn test_ddcb_set_undoc() {
        // SET 0,(IX+$01),B  → DD CB 01 C0
        let m = [0xDD, 0xCB, 0x01, 0xC0];
        let i = disassemble_z80(&m, 0).unwrap();
        assert_eq!(i.mnemonic, "SET 0,(IX+$01),B");
    }

    #[test]
    fn test_ed_in_out() {
        let m = [0xED, 0x40];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "IN B,(C)");
        let m = [0xED, 0x71];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "OUT (C),0");
    }

    #[test]
    fn test_ed_sbc_adc_hl() {
        let m = [0xED, 0x42];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "SBC HL,BC");
        let m = [0xED, 0x7A];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "ADC HL,SP");
    }

    #[test]
    fn test_ed_ld_nn_rr() {
        // LD ($1234),BC  → ED 43 34 12
        let m = [0xED, 0x43, 0x34, 0x12];
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "LD ($1234),BC");
    }

    #[test]
    fn test_ed_neg_mirrors() {
        for &op in &[0x4Cu8, 0x54, 0x5C, 0x64, 0x6C, 0x74, 0x7C] {
            let m = [0xED, op];
            assert_eq!(
                disassemble_z80(&m, 0).unwrap().mnemonic,
                "NEG",
                "mirror 0xED {:02X} should be NEG",
                op
            );
        }
    }

    #[test]
    fn test_ed_im_mirrors() {
        let m = [0xED, 0x4E]; // IM 0 mirror
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "IM 0");
        let m = [0xED, 0x76]; // IM 1 mirror
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "IM 1");
        let m = [0xED, 0x7E]; // IM 2 mirror
        assert_eq!(disassemble_z80(&m, 0).unwrap().mnemonic, "IM 2");
    }

    #[test]
    fn test_ed_block() {
        let cases: &[(&[u8], &str)] = &[
            (&[0xED, 0xA8], "LDD"),
            (&[0xED, 0xB8], "LDDR"),
            (&[0xED, 0xA9], "CPD"),
            (&[0xED, 0xB9], "CPDR"),
            (&[0xED, 0xB2], "INIR"),
            (&[0xED, 0xBB], "OTDR"),
        ];
        for &(mem, expected) in cases {
            assert_eq!(disassemble_z80(mem, 0).unwrap().mnemonic, expected);
        }
    }

    #[test]
    fn test_ed_rld_rrd() {
        assert_eq!(disassemble_z80(&[0xED, 0x6F], 0).unwrap().mnemonic, "RLD");
        assert_eq!(disassemble_z80(&[0xED, 0x67], 0).unwrap().mnemonic, "RRD");
    }

    #[test]
    fn test_cb_rotates() {
        // RLC B  (CB 00)
        assert_eq!(disassemble_z80(&[0xCB, 0x00], 0).unwrap().mnemonic, "RLC B");
        // SRL (HL) (CB 3E)
        assert_eq!(
            disassemble_z80(&[0xCB, 0x3E], 0).unwrap().mnemonic,
            "SRL (HL)"
        );
        // RES 7,A (CB BF)
        assert_eq!(
            disassemble_z80(&[0xCB, 0xBF], 0).unwrap().mnemonic,
            "RES 7,A"
        );
        // SET 7,A (CB FF)
        assert_eq!(
            disassemble_z80(&[0xCB, 0xFF], 0).unwrap().mnemonic,
            "SET 7,A"
        );
    }
}
