//! ARM7TDMI disassembler
//!
//! Provides disassembly for both ARM (32-bit) and Thumb (16-bit) instruction sets.

/// Condition code suffix strings
const COND_NAMES: [&str; 16] = [
    "EQ", "NE", "CS", "CC", "MI", "PL", "VS", "VC", "HI", "LS", "GE", "LT", "GT", "LE", "", "NV",
];

/// Data processing opcode names
const DP_NAMES: [&str; 16] = [
    "AND", "EOR", "SUB", "RSB", "ADD", "ADC", "SBC", "RSC", "TST", "TEQ", "CMP", "CMN", "ORR",
    "MOV", "BIC", "MVN",
];

/// Shift type names
const SHIFT_NAMES: [&str; 4] = ["LSL", "LSR", "ASR", "ROR"];

/// Register names
const REG_NAMES: [&str; 16] = [
    "R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10", "R11", "R12", "SP", "LR",
    "PC",
];

/// Disassemble a single ARM (32-bit) instruction.
/// Returns a human-readable string representation.
pub fn disassemble_arm(instr: u32, pc: u32) -> String {
    let cond = (instr >> 28) & 0xF;
    let cond_str = COND_NAMES[cond as usize];

    let bits_27_20 = (instr >> 20) & 0xFF;
    let bits_7_4 = (instr >> 4) & 0xF;

    // Branch and Exchange (BX)
    if (instr & 0x0FFFFFF0) == 0x012FFF10 {
        let rm = instr & 0xF;
        return format!("BX{} {}", cond_str, REG_NAMES[rm as usize]);
    }

    // Branch (B/BL)
    if (instr >> 25) & 0x7 == 0b101 {
        let link = instr & (1 << 24) != 0;
        let offset = ((instr & 0x00FFFFFF) as i32) << 8 >> 6;
        let target = (pc as i32).wrapping_add(8).wrapping_add(offset) as u32;
        return format!(
            "B{}{} 0x{:08X}",
            if link { "L" } else { "" },
            cond_str,
            target
        );
    }

    // Software Interrupt (SWI)
    if (instr >> 24) & 0xF == 0xF {
        let comment = instr & 0x00FFFFFF;
        return format!("SWI{} 0x{:X}", cond_str, comment);
    }

    // Multiply (MUL/MLA)
    if bits_7_4 == 0b1001 && (bits_27_20 & 0xFC) == 0x00 {
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd = (instr >> 16) & 0xF;
        let rn = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;
        let s = if set_flags { "S" } else { "" };
        if accumulate {
            return format!(
                "MLA{}{} {}, {}, {}, {}",
                cond_str,
                s,
                REG_NAMES[rd as usize],
                REG_NAMES[rm as usize],
                REG_NAMES[rs as usize],
                REG_NAMES[rn as usize]
            );
        } else {
            return format!(
                "MUL{}{} {}, {}, {}",
                cond_str, s, REG_NAMES[rd as usize], REG_NAMES[rm as usize], REG_NAMES[rs as usize]
            );
        }
    }

    // Multiply Long (UMULL/UMLAL/SMULL/SMLAL)
    if bits_7_4 == 0b1001 && (bits_27_20 & 0xF8) == 0x08 {
        let signed = instr & (1 << 22) != 0;
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd_hi = (instr >> 16) & 0xF;
        let rd_lo = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;
        let name = match (signed, accumulate) {
            (false, false) => "UMULL",
            (false, true) => "UMLAL",
            (true, false) => "SMULL",
            (true, true) => "SMLAL",
        };
        let s = if set_flags { "S" } else { "" };
        return format!(
            "{}{}{} {}, {}, {}, {}",
            name,
            cond_str,
            s,
            REG_NAMES[rd_lo as usize],
            REG_NAMES[rd_hi as usize],
            REG_NAMES[rm as usize],
            REG_NAMES[rs as usize]
        );
    }

    // Single Data Swap (SWP/SWPB)
    if (instr & 0x0FB00FF0) == 0x01000090 {
        let byte = instr & (1 << 22) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let rm = instr & 0xF;
        return format!(
            "SWP{}{} {}, {}, [{}]",
            cond_str,
            if byte { "B" } else { "" },
            REG_NAMES[rd as usize],
            REG_NAMES[rm as usize],
            REG_NAMES[rn as usize]
        );
    }

    // MRS
    if (instr & 0x0FBF0FFF) == 0x010F0000 {
        let use_spsr = instr & (1 << 22) != 0;
        let rd = (instr >> 12) & 0xF;
        return format!(
            "MRS{} {}, {}",
            cond_str,
            REG_NAMES[rd as usize],
            if use_spsr { "SPSR" } else { "CPSR" }
        );
    }

    // MSR (register)
    if (instr & 0x0FBFFFF0) == 0x0129F000 || (instr & 0x0DBFF000) == 0x0128F000 {
        let use_spsr = instr & (1 << 22) != 0;
        let field_mask = (instr >> 16) & 0xF;
        let mut fields = String::new();
        if field_mask & 1 != 0 {
            fields.push('c');
        }
        if field_mask & 2 != 0 {
            fields.push('x');
        }
        if field_mask & 4 != 0 {
            fields.push('s');
        }
        if field_mask & 8 != 0 {
            fields.push('f');
        }
        let psr = if use_spsr { "SPSR" } else { "CPSR" };

        if instr & (1 << 25) != 0 {
            let imm = instr & 0xFF;
            let rotate = (instr >> 8) & 0xF;
            let val = imm.rotate_right(rotate * 2);
            return format!("MSR{} {}_{}, #0x{:X}", cond_str, psr, fields, val);
        } else {
            let rm = instr & 0xF;
            return format!(
                "MSR{} {}_{}, {}",
                cond_str, psr, fields, REG_NAMES[rm as usize]
            );
        }
    }

    // Halfword/Signed data transfer
    if (bits_7_4 & 0b1001) == 0b1001 && (bits_7_4 & 0b0110) != 0 && (instr >> 26) & 0x3 == 0 {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let imm_offset = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let op = (instr >> 5) & 0x3;

        let name = match (is_load, op) {
            (false, 0b01) => "STRH",
            (true, 0b01) => "LDRH",
            (true, 0b10) => "LDRSB",
            (true, 0b11) => "LDRSH",
            _ => "???H",
        };

        let offset_str = if imm_offset {
            let off = ((instr >> 4) & 0xF0) | (instr & 0xF);
            if off == 0 {
                String::new()
            } else {
                format!(", #{}{}", if up { "" } else { "-" }, off)
            }
        } else {
            let rm = instr & 0xF;
            format!(", {}{}", if up { "" } else { "-" }, REG_NAMES[rm as usize])
        };

        let wb = if write_back && pre { "!" } else { "" };
        if pre {
            return format!(
                "{}{} {}, [{}{}]{}",
                name, cond_str, REG_NAMES[rd as usize], REG_NAMES[rn as usize], offset_str, wb
            );
        } else {
            return format!(
                "{}{} {}, [{}]{}",
                name, cond_str, REG_NAMES[rd as usize], REG_NAMES[rn as usize], offset_str
            );
        }
    }

    // Data processing
    if (instr >> 26) & 0x3 == 0b00 {
        let opcode = (instr >> 21) & 0xF;
        let set_flags = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let name = DP_NAMES[opcode as usize];
        let s = if set_flags { "S" } else { "" };

        let op2_str = format_shifter_operand(instr);

        // TST, TEQ, CMP, CMN don't write Rd
        if (0x8..=0xB).contains(&opcode) {
            return format!(
                "{}{} {}, {}",
                name, cond_str, REG_NAMES[rn as usize], op2_str
            );
        }
        // MOV, MVN don't use Rn
        if opcode == 0xD || opcode == 0xF {
            return format!(
                "{}{}{} {}, {}",
                name, cond_str, s, REG_NAMES[rd as usize], op2_str
            );
        }
        return format!(
            "{}{}{} {}, {}, {}",
            name, cond_str, s, REG_NAMES[rd as usize], REG_NAMES[rn as usize], op2_str
        );
    }

    // Single data transfer (LDR/STR)
    if (instr >> 26) & 0x3 == 0b01 {
        let reg_offset = instr & (1 << 25) != 0;
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let byte = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;

        let name = format!(
            "{}{}{}",
            if is_load { "LDR" } else { "STR" },
            cond_str,
            if byte { "B" } else { "" }
        );

        let offset_str = if !reg_offset {
            let off = instr & 0xFFF;
            if off == 0 {
                String::new()
            } else {
                format!(", #{}{}", if up { "" } else { "-" }, off)
            }
        } else {
            let rm = instr & 0xF;
            let shift_type = (instr >> 5) & 3;
            let shift_amount = (instr >> 7) & 0x1F;
            let sign = if up { "" } else { "-" };
            if shift_amount == 0 && shift_type == 0 {
                format!(", {}{}", sign, REG_NAMES[rm as usize])
            } else {
                format!(
                    ", {}{}, {} #{}",
                    sign, REG_NAMES[rm as usize], SHIFT_NAMES[shift_type as usize], shift_amount
                )
            }
        };

        let wb = if write_back && pre { "!" } else { "" };
        if pre {
            return format!(
                "{} {}, [{}{}]{}",
                name, REG_NAMES[rd as usize], REG_NAMES[rn as usize], offset_str, wb
            );
        } else {
            return format!(
                "{} {}, [{}]{}",
                name, REG_NAMES[rd as usize], REG_NAMES[rn as usize], offset_str
            );
        }
    }

    // Block data transfer (LDM/STM)
    if (instr >> 25) & 0x7 == 0b100 {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let psr = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let reg_list = instr & 0xFFFF;

        let suffix = match (up, pre) {
            (true, false) => "IA",
            (true, true) => "IB",
            (false, false) => "DA",
            (false, true) => "DB",
        };

        let name = format!(
            "{}{}{}",
            if is_load { "LDM" } else { "STM" },
            cond_str,
            suffix
        );

        let wb = if write_back { "!" } else { "" };
        let regs = format_reg_list(reg_list);
        let s = if psr { "^" } else { "" };
        return format!(
            "{} {}{}, {{{}}}{}",
            name, REG_NAMES[rn as usize], wb, regs, s
        );
    }

    format!("DCD 0x{:08X}", instr)
}

/// Disassemble a single Thumb (16-bit) instruction.
pub fn disassemble_thumb(instr: u16, pc: u32) -> String {
    let instr = instr as u32;
    let bits_15_8 = (instr >> 8) & 0xFF;

    // Format 1: Move shifted register
    if (instr >> 13) == 0b000 && (instr >> 11) & 0x3 != 0x3 {
        let op = (instr >> 11) & 0x3;
        let offset = (instr >> 6) & 0x1F;
        let rs = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = SHIFT_NAMES[op as usize];
        return format!(
            "{} {}, {}, #{}",
            name, REG_NAMES[rd as usize], REG_NAMES[rs as usize], offset
        );
    }

    // Format 2: Add/Subtract
    if (instr >> 11) & 0x1F == 0b00011 {
        let is_imm = instr & (1 << 10) != 0;
        let is_sub = instr & (1 << 9) != 0;
        let rn_imm = (instr >> 6) & 0x7;
        let rs = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = if is_sub { "SUB" } else { "ADD" };
        if is_imm {
            return format!(
                "{} {}, {}, #{}",
                name, REG_NAMES[rd as usize], REG_NAMES[rs as usize], rn_imm
            );
        } else {
            return format!(
                "{} {}, {}, {}",
                name, REG_NAMES[rd as usize], REG_NAMES[rs as usize], REG_NAMES[rn_imm as usize]
            );
        }
    }

    // Format 3: Move/Compare/Add/Subtract immediate
    if (instr >> 13) == 0b001 {
        let op = (instr >> 11) & 0x3;
        let rd = (instr >> 8) & 0x7;
        let imm = instr & 0xFF;
        let name = match op {
            0 => "MOV",
            1 => "CMP",
            2 => "ADD",
            3 => "SUB",
            _ => unreachable!(),
        };
        return format!("{} {}, #0x{:X}", name, REG_NAMES[rd as usize], imm);
    }

    // Format 4: ALU operations
    if bits_15_8 >> 2 == 0b010000 {
        let op = (instr >> 6) & 0xF;
        let rs = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = match op {
            0x0 => "AND",
            0x1 => "EOR",
            0x2 => "LSL",
            0x3 => "LSR",
            0x4 => "ASR",
            0x5 => "ADC",
            0x6 => "SBC",
            0x7 => "ROR",
            0x8 => "TST",
            0x9 => "NEG",
            0xA => "CMP",
            0xB => "CMN",
            0xC => "ORR",
            0xD => "MUL",
            0xE => "BIC",
            0xF => "MVN",
            _ => unreachable!(),
        };
        return format!(
            "{} {}, {}",
            name, REG_NAMES[rd as usize], REG_NAMES[rs as usize]
        );
    }

    // Format 5: Hi register operations / BX
    if bits_15_8 >> 2 == 0b010001 {
        let op = (instr >> 8) & 0x3;
        let h1 = (instr >> 7) & 1;
        let h2 = (instr >> 6) & 1;
        let rs = ((h2 << 3) | ((instr >> 3) & 0x7)) & 0xF;
        let rd = ((h1 << 3) | (instr & 0x7)) & 0xF;
        match op {
            0 => return format!("ADD {}, {}", REG_NAMES[rd as usize], REG_NAMES[rs as usize]),
            1 => return format!("CMP {}, {}", REG_NAMES[rd as usize], REG_NAMES[rs as usize]),
            2 => return format!("MOV {}, {}", REG_NAMES[rd as usize], REG_NAMES[rs as usize]),
            3 => return format!("BX {}", REG_NAMES[rs as usize]),
            _ => unreachable!(),
        }
    }

    // Format 6: PC-relative load
    if bits_15_8 >> 3 == 0b01001 {
        let rd = (instr >> 8) & 0x7;
        let imm = (instr & 0xFF) << 2;
        let addr = (pc.wrapping_add(4) & !3).wrapping_add(imm);
        return format!(
            "LDR {}, [PC, #0x{:X}] ; =0x{:08X}",
            REG_NAMES[rd as usize], imm, addr
        );
    }

    // Format 7: Load/Store with register offset
    if (instr >> 12) & 0xF == 0b0101 && (instr >> 9) & 1 == 0 {
        let op = (instr >> 10) & 0x3;
        let ro = (instr >> 6) & 0x7;
        let rb = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = match op {
            0 => "STR",
            1 => "STRB",
            2 => "LDR",
            3 => "LDRB",
            _ => unreachable!(),
        };
        return format!(
            "{} {}, [{}, {}]",
            name, REG_NAMES[rd as usize], REG_NAMES[rb as usize], REG_NAMES[ro as usize]
        );
    }

    // Format 8: Load/Store sign-extended byte/halfword
    if (instr >> 12) & 0xF == 0b0101 && (instr >> 9) & 1 == 1 {
        let op = (instr >> 10) & 0x3;
        let ro = (instr >> 6) & 0x7;
        let rb = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = match op {
            0 => "STRH",
            1 => "LDSB",
            2 => "LDRH",
            3 => "LDSH",
            _ => unreachable!(),
        };
        return format!(
            "{} {}, [{}, {}]",
            name, REG_NAMES[rd as usize], REG_NAMES[rb as usize], REG_NAMES[ro as usize]
        );
    }

    // Format 9: Load/Store with immediate offset
    if (instr >> 13) == 0b011 {
        let byte = instr & (1 << 12) != 0;
        let is_load = instr & (1 << 11) != 0;
        let offset = (instr >> 6) & 0x1F;
        let rb = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = match (is_load, byte) {
            (false, false) => "STR",
            (false, true) => "STRB",
            (true, false) => "LDR",
            (true, true) => "LDRB",
        };
        let actual_offset = if byte { offset } else { offset << 2 };
        return format!(
            "{} {}, [{}, #0x{:X}]",
            name, REG_NAMES[rd as usize], REG_NAMES[rb as usize], actual_offset
        );
    }

    // Format 10: Load/Store halfword
    if (instr >> 12) & 0xF == 0b1000 {
        let is_load = instr & (1 << 11) != 0;
        let offset = ((instr >> 6) & 0x1F) << 1;
        let rb = (instr >> 3) & 0x7;
        let rd = instr & 0x7;
        let name = if is_load { "LDRH" } else { "STRH" };
        return format!(
            "{} {}, [{}, #0x{:X}]",
            name, REG_NAMES[rd as usize], REG_NAMES[rb as usize], offset
        );
    }

    // Format 11: SP-relative Load/Store
    if (instr >> 12) & 0xF == 0b1001 {
        let is_load = instr & (1 << 11) != 0;
        let rd = (instr >> 8) & 0x7;
        let imm = (instr & 0xFF) << 2;
        let name = if is_load { "LDR" } else { "STR" };
        return format!("{} {}, [SP, #0x{:X}]", name, REG_NAMES[rd as usize], imm);
    }

    // Format 12: Load address
    if (instr >> 12) & 0xF == 0b1010 {
        let use_sp = instr & (1 << 11) != 0;
        let rd = (instr >> 8) & 0x7;
        let imm = (instr & 0xFF) << 2;
        return format!(
            "ADD {}, {}, #0x{:X}",
            REG_NAMES[rd as usize],
            if use_sp { "SP" } else { "PC" },
            imm
        );
    }

    // Format 13: Add offset to SP
    if (instr >> 8) & 0xFF == 0b10110000 {
        let negative = instr & (1 << 7) != 0;
        let imm = (instr & 0x7F) << 2;
        return format!("ADD SP, #{}{}", if negative { "-" } else { "" }, imm);
    }

    // Format 14: Push/Pop
    if (instr >> 12) & 0xF == 0b1011 && (instr >> 9) & 0x3 == 0b10 {
        let is_pop = instr & (1 << 11) != 0;
        let pc_lr = instr & (1 << 8) != 0;
        let reg_list = instr & 0xFF;
        let mut regs = format_reg_list(reg_list);
        if pc_lr {
            if !regs.is_empty() {
                regs.push_str(", ");
            }
            regs.push_str(if is_pop { "PC" } else { "LR" });
        }
        let name = if is_pop { "POP" } else { "PUSH" };
        return format!("{} {{{}}}", name, regs);
    }

    // Format 15: Multiple Load/Store
    if (instr >> 12) & 0xF == 0b1100 {
        let is_load = instr & (1 << 11) != 0;
        let rb = (instr >> 8) & 0x7;
        let reg_list = instr & 0xFF;
        let name = if is_load { "LDMIA" } else { "STMIA" };
        let regs = format_reg_list(reg_list);
        return format!("{} {}!, {{{}}}", name, REG_NAMES[rb as usize], regs);
    }

    // Format 16: Conditional branch
    if (instr >> 12) & 0xF == 0b1101 && (instr >> 8) & 0xF != 0xF {
        let cond = (instr >> 8) & 0xF;
        if cond == 0xE {
            // Undefined
            return format!("DCD 0x{:04X}", instr);
        }
        let offset = ((instr & 0xFF) as i8 as i32) << 1;
        let target = (pc as i32).wrapping_add(4).wrapping_add(offset) as u32;
        return format!("B{} 0x{:08X}", COND_NAMES[cond as usize], target);
    }

    // Format 17: SWI
    if (instr >> 8) & 0xFF == 0b11011111 {
        let comment = instr & 0xFF;
        return format!("SWI 0x{:X}", comment);
    }

    // Format 18: Unconditional branch
    if (instr >> 11) & 0x1F == 0b11100 {
        let offset = ((instr & 0x7FF) << 21) as i32 >> 20;
        let target = (pc as i32).wrapping_add(4).wrapping_add(offset) as u32;
        return format!("B 0x{:08X}", target);
    }

    // Format 19: Long branch with link
    if (instr >> 12) & 0xF == 0b1111 {
        let h = (instr >> 11) & 1;
        if h == 0 {
            let offset = instr & 0x7FF;
            return format!("BL (setup) offset=0x{:X}", offset);
        } else {
            let offset = instr & 0x7FF;
            return format!("BL (branch) offset=0x{:X}", offset);
        }
    }

    format!("DCD 0x{:04X}", instr)
}

/// Format a shifter operand (operand 2) for ARM data processing instructions
fn format_shifter_operand(instr: u32) -> String {
    if instr & (1 << 25) != 0 {
        // Immediate
        let imm = instr & 0xFF;
        let rotate = (instr >> 8) & 0xF;
        if rotate == 0 {
            format!("#0x{:X}", imm)
        } else {
            let val = imm.rotate_right(rotate * 2);
            format!("#0x{:X}", val)
        }
    } else {
        let rm = instr & 0xF;
        let shift_type = (instr >> 5) & 3;
        let shift_by_reg = instr & (1 << 4) != 0;

        if shift_by_reg {
            let rs = (instr >> 8) & 0xF;
            format!(
                "{}, {} {}",
                REG_NAMES[rm as usize], SHIFT_NAMES[shift_type as usize], REG_NAMES[rs as usize]
            )
        } else {
            let amount = (instr >> 7) & 0x1F;
            if amount == 0 && shift_type == 0 {
                REG_NAMES[rm as usize].to_string()
            } else if amount == 0 && shift_type == 0b11 {
                format!("{}, RRX", REG_NAMES[rm as usize])
            } else {
                let actual_amount = if amount == 0 { 32 } else { amount };
                format!(
                    "{}, {} #{}",
                    REG_NAMES[rm as usize], SHIFT_NAMES[shift_type as usize], actual_amount
                )
            }
        }
    }
}

/// Format a register list bitmask as a string (e.g., "R0, R1, R4-R7")
fn format_reg_list(reg_list: u32) -> String {
    let mut parts = Vec::new();
    let mut i = 0;

    while i < 16 {
        if reg_list & (1 << i) != 0 {
            let start = i;
            while i < 15 && reg_list & (1 << (i + 1)) != 0 {
                i += 1;
            }
            if i == start {
                parts.push(REG_NAMES[start as usize].to_string());
            } else if i == start + 1 {
                parts.push(REG_NAMES[start as usize].to_string());
                parts.push(REG_NAMES[i as usize].to_string());
            } else {
                parts.push(format!(
                    "{}-{}",
                    REG_NAMES[start as usize], REG_NAMES[i as usize]
                ));
            }
        }
        i += 1;
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm_mov_imm() {
        // MOV R0, #42
        let dis = disassemble_arm(0xE3A0002A, 0);
        assert!(dis.contains("MOV"));
        assert!(dis.contains("R0"));
        assert!(dis.contains("2A"));
    }

    #[test]
    fn test_arm_branch() {
        // B +8
        let dis = disassemble_arm(0xEA000000, 0);
        assert!(dis.starts_with("B ") || dis.starts_with("B 0x"));
    }

    #[test]
    fn test_arm_bx() {
        let dis = disassemble_arm(0xE12FFF10, 0);
        assert!(dis.contains("BX"));
        assert!(dis.contains("R0"));
    }

    #[test]
    fn test_arm_ldr() {
        // LDR R0, [R1]
        let dis = disassemble_arm(0xE5910000, 0);
        assert!(dis.contains("LDR"));
    }

    #[test]
    fn test_arm_stm() {
        // STMIA R4!, {R0-R2}
        let dis = disassemble_arm(0xE8A40007, 0);
        assert!(dis.contains("STM"));
    }

    #[test]
    fn test_arm_swi() {
        let dis = disassemble_arm(0xEF000000, 0);
        assert!(dis.contains("SWI"));
    }

    #[test]
    fn test_thumb_mov() {
        let dis = disassemble_thumb(0x202A, 0);
        assert!(dis.contains("MOV"));
        assert!(dis.contains("R0"));
    }

    #[test]
    fn test_thumb_push() {
        let dis = disassemble_thumb(0xB503, 0);
        assert!(dis.contains("PUSH"));
    }

    #[test]
    fn test_thumb_bx() {
        // BX R0
        let dis = disassemble_thumb(0x4700, 0);
        assert!(dis.contains("BX"));
    }

    #[test]
    fn test_reg_list() {
        assert_eq!(format_reg_list(0x07), "R0-R2");
        assert_eq!(format_reg_list(0x05), "R0, R2");
        assert_eq!(format_reg_list(0x8000), "PC");
    }
}
