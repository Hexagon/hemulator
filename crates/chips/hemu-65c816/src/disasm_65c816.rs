//! 65C816 CPU disassembler
//!
//! Provides disassembly for the WDC 65C816 CPU used in SNES.
//! The 65C816 is a 16-bit extension of the 6502 with additional addressing modes
//! and instructions.

use hemu_types::DisassembledInstruction;

/// Disassemble a single 65C816 instruction from memory (legacy version assuming 8-bit mode)
///
/// Note: This implementation assumes 8-bit mode for M and X flags.
/// For accurate disassembly, use `disassemble_65c816_with_flags` instead.
pub fn disassemble_65c816(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    // Default to 8-bit mode (m=1, x=1)
    disassemble_65c816_with_flags(memory, address, true, true)
}

/// Disassemble a single 65C816 instruction from memory with CPU flags
///
/// # Arguments
/// * `memory` - Memory slice starting at the instruction
/// * `address` - Address of the instruction for display
/// * `m_flag` - Memory/Accumulator size flag (true = 8-bit, false = 16-bit)
/// * `x_flag` - Index register size flag (true = 8-bit, false = 16-bit)
///
/// # Returns
/// A tuple of (DisassembledInstruction, new_m_flag, new_x_flag) to track mode changes
pub fn disassemble_65c816_with_flags(
    memory: &[u8],
    address: u32,
    m_flag: bool,
    x_flag: bool,
) -> Option<DisassembledInstruction> {
    disassemble_65c816_tracking_flags(memory, address, m_flag, x_flag).map(|(instr, _, _)| instr)
}

/// Disassemble a single 65C816 instruction and return updated flags
///
/// This version returns the new M and X flag values after the instruction,
/// which is useful for disassembling a range of instructions that may contain
/// REP/SEP instructions that change the processor mode.
pub fn disassemble_65c816_tracking_flags(
    memory: &[u8],
    address: u32,
    m_flag: bool,
    x_flag: bool,
) -> Option<(DisassembledInstruction, bool, bool)> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // Check for REP/SEP instructions that change flags
    let (new_m_flag, new_x_flag) = match opcode {
        0xC2 => {
            // REP - Reset Processor status bits
            let operand = memory.get(1).copied().unwrap_or(0);
            let new_m = if operand & 0x20 != 0 { false } else { m_flag }; // Clear M if bit 5 set
            let new_x = if operand & 0x10 != 0 { false } else { x_flag }; // Clear X if bit 4 set
            (new_m, new_x)
        }
        0xE2 => {
            // SEP - Set Processor status bits
            let operand = memory.get(1).copied().unwrap_or(0);
            let new_m = if operand & 0x20 != 0 { true } else { m_flag }; // Set M if bit 5 set
            let new_x = if operand & 0x10 != 0 { true } else { x_flag }; // Set X if bit 4 set
            (new_m, new_x)
        }
        0xFB => {
            // XCE - Exchange Carry and Emulation flags
            // After XCE, if entering emulation mode, M and X become 1
            // We can't know the carry flag, so assume no change for now
            (m_flag, x_flag)
        }
        _ => (m_flag, x_flag),
    };

    // Decode instruction based on opcode and flags
    let (mnemonic, len) = decode_instruction(opcode, memory, m_flag, x_flag);

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    let instr = DisassembledInstruction::new(address, bytes, mnemonic);

    Some((instr, new_m_flag, new_x_flag))
}

fn decode_instruction(opcode: u8, memory: &[u8], m_flag: bool, x_flag: bool) -> (String, usize) {
    // Helper to get operand bytes
    let get_u8 = |offset: usize| -> u8 { memory.get(offset).copied().unwrap_or(0) };
    let get_u16 = |offset: usize| -> u16 {
        let lo = get_u8(offset) as u16;
        let hi = get_u8(offset + 1) as u16;
        (hi << 8) | lo
    };
    let get_u24 = |offset: usize| -> u32 {
        let lo = get_u8(offset) as u32;
        let mid = get_u8(offset + 1) as u32;
        let hi = get_u8(offset + 2) as u32;
        (hi << 16) | (mid << 8) | lo
    };

    // Helper for M-flag dependent immediate operands (LDA, ORA, AND, EOR, ADC, SBC, CMP, BIT)
    let imm_m = |mnemonic: &str| -> (String, usize) {
        if m_flag {
            (format!("{} #${:02X}", mnemonic, get_u8(1)), 2)
        } else {
            (format!("{} #${:04X}", mnemonic, get_u16(1)), 3)
        }
    };

    // Helper for X-flag dependent immediate operands (LDX, LDY, CPX, CPY)
    let imm_x = |mnemonic: &str| -> (String, usize) {
        if x_flag {
            (format!("{} #${:02X}", mnemonic, get_u8(1)), 2)
        } else {
            (format!("{} #${:04X}", mnemonic, get_u16(1)), 3)
        }
    };

    match opcode {
        // BRK, COP, WDM
        0x00 => ("BRK".to_string(), 2),
        0x02 => (format!("COP #${:02X}", get_u8(1)), 2),
        0x42 => (format!("WDM #${:02X}", get_u8(1)), 2),

        // Stack operations
        0x08 => ("PHP".to_string(), 1),
        0x0B => ("PHD".to_string(), 1),
        0x28 => ("PLP".to_string(), 1),
        0x2B => ("PLD".to_string(), 1),
        0x48 => ("PHA".to_string(), 1),
        0x4B => ("PHK".to_string(), 1),
        0x5B => ("TCD".to_string(), 1),
        0x68 => ("PLA".to_string(), 1),
        0x7B => ("TDC".to_string(), 1),
        0x8B => ("PHB".to_string(), 1),
        0xAB => ("PLB".to_string(), 1),
        0xDA => ("PHX".to_string(), 1),
        0xFA => ("PLX".to_string(), 1),
        0x5A => ("PHY".to_string(), 1),
        0x7A => ("PLY".to_string(), 1),

        // Control flow
        0x18 => ("CLC".to_string(), 1),
        0x38 => ("SEC".to_string(), 1),
        0x58 => ("CLI".to_string(), 1),
        0x78 => ("SEI".to_string(), 1),
        0xD8 => ("CLD".to_string(), 1),
        0xF8 => ("SED".to_string(), 1),
        0xB8 => ("CLV".to_string(), 1),
        0xC2 => (format!("REP #${:02X}", get_u8(1)), 2),
        0xE2 => (format!("SEP #${:02X}", get_u8(1)), 2),

        // Transfer operations
        0x8A => ("TXA".to_string(), 1),
        0x98 => ("TYA".to_string(), 1),
        0x9A => ("TXS".to_string(), 1),
        0x9B => ("TXY".to_string(), 1),
        0xA8 => ("TAY".to_string(), 1),
        0xAA => ("TAX".to_string(), 1),
        0xBA => ("TSX".to_string(), 1),
        0xBB => ("TYX".to_string(), 1),

        // Increment/Decrement
        0x1A => ("INC A".to_string(), 1),
        0x3A => ("DEC A".to_string(), 1),
        0xC8 => ("INY".to_string(), 1),
        0xCA => ("DEX".to_string(), 1),
        0xE8 => ("INX".to_string(), 1),
        0x88 => ("DEY".to_string(), 1),

        // Special
        0x40 => ("RTI".to_string(), 1),
        0x60 => ("RTS".to_string(), 1),
        0x6B => ("RTL".to_string(), 1),
        0xCB => ("WAI".to_string(), 1),
        0xDB => ("STP".to_string(), 1),
        0xEA => ("NOP".to_string(), 1),
        0xEB => ("XBA".to_string(), 1),
        0xFB => ("XCE".to_string(), 1),

        // ORA instructions
        0x09 => imm_m("ORA"),
        0x05 => (format!("ORA ${:02X}", get_u8(1)), 2),
        0x15 => (format!("ORA ${:02X},X", get_u8(1)), 2),
        0x0D => (format!("ORA ${:04X}", get_u16(1)), 3),
        0x1D => (format!("ORA ${:04X},X", get_u16(1)), 3),
        0x19 => (format!("ORA ${:04X},Y", get_u16(1)), 3),
        0x0F => (format!("ORA ${:06X}", get_u24(1)), 4),
        0x1F => (format!("ORA ${:06X},X", get_u24(1)), 4),
        0x01 => (format!("ORA (${:02X},X)", get_u8(1)), 2),
        0x11 => (format!("ORA (${:02X}),Y", get_u8(1)), 2),
        0x07 => (format!("ORA [${:02X}]", get_u8(1)), 2),
        0x17 => (format!("ORA [${:02X}],Y", get_u8(1)), 2),
        0x03 => (format!("ORA ${:02X},S", get_u8(1)), 2),
        0x13 => (format!("ORA (${:02X},S),Y", get_u8(1)), 2),

        // AND instructions
        0x29 => imm_m("AND"),
        0x25 => (format!("AND ${:02X}", get_u8(1)), 2),
        0x35 => (format!("AND ${:02X},X", get_u8(1)), 2),
        0x2D => (format!("AND ${:04X}", get_u16(1)), 3),
        0x3D => (format!("AND ${:04X},X", get_u16(1)), 3),
        0x39 => (format!("AND ${:04X},Y", get_u16(1)), 3),
        0x2F => (format!("AND ${:06X}", get_u24(1)), 4),
        0x3F => (format!("AND ${:06X},X", get_u24(1)), 4),
        0x21 => (format!("AND (${:02X},X)", get_u8(1)), 2),
        0x31 => (format!("AND (${:02X}),Y", get_u8(1)), 2),
        0x27 => (format!("AND [${:02X}]", get_u8(1)), 2),
        0x37 => (format!("AND [${:02X}],Y", get_u8(1)), 2),
        0x23 => (format!("AND ${:02X},S", get_u8(1)), 2),
        0x33 => (format!("AND (${:02X},S),Y", get_u8(1)), 2),

        // EOR instructions
        0x49 => imm_m("EOR"),
        0x45 => (format!("EOR ${:02X}", get_u8(1)), 2),
        0x55 => (format!("EOR ${:02X},X", get_u8(1)), 2),
        0x4D => (format!("EOR ${:04X}", get_u16(1)), 3),
        0x5D => (format!("EOR ${:04X},X", get_u16(1)), 3),
        0x59 => (format!("EOR ${:04X},Y", get_u16(1)), 3),
        0x4F => (format!("EOR ${:06X}", get_u24(1)), 4),
        0x5F => (format!("EOR ${:06X},X", get_u24(1)), 4),
        0x41 => (format!("EOR (${:02X},X)", get_u8(1)), 2),
        0x51 => (format!("EOR (${:02X}),Y", get_u8(1)), 2),
        0x47 => (format!("EOR [${:02X}]", get_u8(1)), 2),
        0x57 => (format!("EOR [${:02X}],Y", get_u8(1)), 2),
        0x43 => (format!("EOR ${:02X},S", get_u8(1)), 2),
        0x53 => (format!("EOR (${:02X},S),Y", get_u8(1)), 2),

        // ADC instructions
        0x69 => imm_m("ADC"),
        0x65 => (format!("ADC ${:02X}", get_u8(1)), 2),
        0x75 => (format!("ADC ${:02X},X", get_u8(1)), 2),
        0x6D => (format!("ADC ${:04X}", get_u16(1)), 3),
        0x7D => (format!("ADC ${:04X},X", get_u16(1)), 3),
        0x79 => (format!("ADC ${:04X},Y", get_u16(1)), 3),
        0x6F => (format!("ADC ${:06X}", get_u24(1)), 4),
        0x7F => (format!("ADC ${:06X},X", get_u24(1)), 4),
        0x61 => (format!("ADC (${:02X},X)", get_u8(1)), 2),
        0x71 => (format!("ADC (${:02X}),Y", get_u8(1)), 2),
        0x67 => (format!("ADC [${:02X}]", get_u8(1)), 2),
        0x77 => (format!("ADC [${:02X}],Y", get_u8(1)), 2),
        0x63 => (format!("ADC ${:02X},S", get_u8(1)), 2),
        0x73 => (format!("ADC (${:02X},S),Y", get_u8(1)), 2),

        // SBC instructions
        0xE9 => imm_m("SBC"),
        0xE5 => (format!("SBC ${:02X}", get_u8(1)), 2),
        0xF5 => (format!("SBC ${:02X},X", get_u8(1)), 2),
        0xED => (format!("SBC ${:04X}", get_u16(1)), 3),
        0xFD => (format!("SBC ${:04X},X", get_u16(1)), 3),
        0xF9 => (format!("SBC ${:04X},Y", get_u16(1)), 3),
        0xEF => (format!("SBC ${:06X}", get_u24(1)), 4),
        0xFF => (format!("SBC ${:06X},X", get_u24(1)), 4),
        0xE1 => (format!("SBC (${:02X},X)", get_u8(1)), 2),
        0xF1 => (format!("SBC (${:02X}),Y", get_u8(1)), 2),
        0xE7 => (format!("SBC [${:02X}]", get_u8(1)), 2),
        0xF7 => (format!("SBC [${:02X}],Y", get_u8(1)), 2),
        0xE3 => (format!("SBC ${:02X},S", get_u8(1)), 2),
        0xF3 => (format!("SBC (${:02X},S),Y", get_u8(1)), 2),

        // CMP instructions
        0xC9 => imm_m("CMP"),
        0xC5 => (format!("CMP ${:02X}", get_u8(1)), 2),
        0xD5 => (format!("CMP ${:02X},X", get_u8(1)), 2),
        0xCD => (format!("CMP ${:04X}", get_u16(1)), 3),
        0xDD => (format!("CMP ${:04X},X", get_u16(1)), 3),
        0xD9 => (format!("CMP ${:04X},Y", get_u16(1)), 3),
        0xCF => (format!("CMP ${:06X}", get_u24(1)), 4),
        0xDF => (format!("CMP ${:06X},X", get_u24(1)), 4),
        0xC1 => (format!("CMP (${:02X},X)", get_u8(1)), 2),
        0xD1 => (format!("CMP (${:02X}),Y", get_u8(1)), 2),
        0xC7 => (format!("CMP [${:02X}]", get_u8(1)), 2),
        0xD7 => (format!("CMP [${:02X}],Y", get_u8(1)), 2),
        0xC3 => (format!("CMP ${:02X},S", get_u8(1)), 2),
        0xD3 => (format!("CMP (${:02X},S),Y", get_u8(1)), 2),

        // CPX, CPY
        0xE0 => imm_x("CPX"),
        0xE4 => (format!("CPX ${:02X}", get_u8(1)), 2),
        0xEC => (format!("CPX ${:04X}", get_u16(1)), 3),
        0xC0 => imm_x("CPY"),
        0xC4 => (format!("CPY ${:02X}", get_u8(1)), 2),
        0xCC => (format!("CPY ${:04X}", get_u16(1)), 3),

        // LDA instructions
        0xA9 => imm_m("LDA"),
        0xA5 => (format!("LDA ${:02X}", get_u8(1)), 2),
        0xB5 => (format!("LDA ${:02X},X", get_u8(1)), 2),
        0xAD => (format!("LDA ${:04X}", get_u16(1)), 3),
        0xBD => (format!("LDA ${:04X},X", get_u16(1)), 3),
        0xB9 => (format!("LDA ${:04X},Y", get_u16(1)), 3),
        0xAF => (format!("LDA ${:06X}", get_u24(1)), 4),
        0xBF => (format!("LDA ${:06X},X", get_u24(1)), 4),
        0xA1 => (format!("LDA (${:02X},X)", get_u8(1)), 2),
        0xB1 => (format!("LDA (${:02X}),Y", get_u8(1)), 2),
        0xA7 => (format!("LDA [${:02X}]", get_u8(1)), 2),
        0xB7 => (format!("LDA [${:02X}],Y", get_u8(1)), 2),
        0xA3 => (format!("LDA ${:02X},S", get_u8(1)), 2),
        0xB3 => (format!("LDA (${:02X},S),Y", get_u8(1)), 2),

        // LDX, LDY
        0xA2 => imm_x("LDX"),
        0xA6 => (format!("LDX ${:02X}", get_u8(1)), 2),
        0xB6 => (format!("LDX ${:02X},Y", get_u8(1)), 2),
        0xAE => (format!("LDX ${:04X}", get_u16(1)), 3),
        0xBE => (format!("LDX ${:04X},Y", get_u16(1)), 3),
        0xA0 => imm_x("LDY"),
        0xA4 => (format!("LDY ${:02X}", get_u8(1)), 2),
        0xB4 => (format!("LDY ${:02X},X", get_u8(1)), 2),
        0xAC => (format!("LDY ${:04X}", get_u16(1)), 3),
        0xBC => (format!("LDY ${:04X},X", get_u16(1)), 3),

        // STA instructions
        0x85 => (format!("STA ${:02X}", get_u8(1)), 2),
        0x95 => (format!("STA ${:02X},X", get_u8(1)), 2),
        0x8D => (format!("STA ${:04X}", get_u16(1)), 3),
        0x9D => (format!("STA ${:04X},X", get_u16(1)), 3),
        0x99 => (format!("STA ${:04X},Y", get_u16(1)), 3),
        0x8F => (format!("STA ${:06X}", get_u24(1)), 4),
        0x9F => (format!("STA ${:06X},X", get_u24(1)), 4),
        0x81 => (format!("STA (${:02X},X)", get_u8(1)), 2),
        0x91 => (format!("STA (${:02X}),Y", get_u8(1)), 2),
        0x87 => (format!("STA [${:02X}]", get_u8(1)), 2),
        0x97 => (format!("STA [${:02X}],Y", get_u8(1)), 2),
        0x83 => (format!("STA ${:02X},S", get_u8(1)), 2),
        0x93 => (format!("STA (${:02X},S),Y", get_u8(1)), 2),

        // STX, STY, STZ
        0x86 => (format!("STX ${:02X}", get_u8(1)), 2),
        0x96 => (format!("STX ${:02X},Y", get_u8(1)), 2),
        0x8E => (format!("STX ${:04X}", get_u16(1)), 3),
        0x84 => (format!("STY ${:02X}", get_u8(1)), 2),
        0x94 => (format!("STY ${:02X},X", get_u8(1)), 2),
        0x8C => (format!("STY ${:04X}", get_u16(1)), 3),
        0x64 => (format!("STZ ${:02X}", get_u8(1)), 2),
        0x74 => (format!("STZ ${:02X},X", get_u8(1)), 2),
        0x9C => (format!("STZ ${:04X}", get_u16(1)), 3),
        0x9E => (format!("STZ ${:04X},X", get_u16(1)), 3),

        // Shifts and rotates
        0x0A => ("ASL A".to_string(), 1),
        0x06 => (format!("ASL ${:02X}", get_u8(1)), 2),
        0x16 => (format!("ASL ${:02X},X", get_u8(1)), 2),
        0x0E => (format!("ASL ${:04X}", get_u16(1)), 3),
        0x1E => (format!("ASL ${:04X},X", get_u16(1)), 3),
        0x4A => ("LSR A".to_string(), 1),
        0x46 => (format!("LSR ${:02X}", get_u8(1)), 2),
        0x56 => (format!("LSR ${:02X},X", get_u8(1)), 2),
        0x4E => (format!("LSR ${:04X}", get_u16(1)), 3),
        0x5E => (format!("LSR ${:04X},X", get_u16(1)), 3),
        0x2A => ("ROL A".to_string(), 1),
        0x26 => (format!("ROL ${:02X}", get_u8(1)), 2),
        0x36 => (format!("ROL ${:02X},X", get_u8(1)), 2),
        0x2E => (format!("ROL ${:04X}", get_u16(1)), 3),
        0x3E => (format!("ROL ${:04X},X", get_u16(1)), 3),
        0x6A => ("ROR A".to_string(), 1),
        0x66 => (format!("ROR ${:02X}", get_u8(1)), 2),
        0x76 => (format!("ROR ${:02X},X", get_u8(1)), 2),
        0x6E => (format!("ROR ${:04X}", get_u16(1)), 3),
        0x7E => (format!("ROR ${:04X},X", get_u16(1)), 3),

        // Bit operations
        0x89 => imm_m("BIT"),
        0x24 => (format!("BIT ${:02X}", get_u8(1)), 2),
        0x34 => (format!("BIT ${:02X},X", get_u8(1)), 2),
        0x2C => (format!("BIT ${:04X}", get_u16(1)), 3),
        0x3C => (format!("BIT ${:04X},X", get_u16(1)), 3),
        0x04 => (format!("TSB ${:02X}", get_u8(1)), 2),
        0x0C => (format!("TSB ${:04X}", get_u16(1)), 3),
        0x14 => (format!("TRB ${:02X}", get_u8(1)), 2),
        0x1C => (format!("TRB ${:04X}", get_u16(1)), 3),

        // Branches
        0x10 => (format!("BPL ${:02X}", get_u8(1)), 2),
        0x30 => (format!("BMI ${:02X}", get_u8(1)), 2),
        0x50 => (format!("BVC ${:02X}", get_u8(1)), 2),
        0x70 => (format!("BVS ${:02X}", get_u8(1)), 2),
        0x80 => (format!("BRA ${:02X}", get_u8(1)), 2),
        0x82 => (format!("BRL ${:04X}", get_u16(1)), 3),
        0x90 => (format!("BCC ${:02X}", get_u8(1)), 2),
        0xB0 => (format!("BCS ${:02X}", get_u8(1)), 2),
        0xD0 => (format!("BNE ${:02X}", get_u8(1)), 2),
        0xF0 => (format!("BEQ ${:02X}", get_u8(1)), 2),

        // Jumps and calls
        0x4C => (format!("JMP ${:04X}", get_u16(1)), 3),
        0x5C => (format!("JML ${:06X}", get_u24(1)), 4),
        0x6C => (format!("JMP (${:04X})", get_u16(1)), 3),
        0x7C => (format!("JMP (${:04X},X)", get_u16(1)), 3),
        0xDC => (format!("JML [${:04X}]", get_u16(1)), 3),
        0x20 => (format!("JSR ${:04X}", get_u16(1)), 3),
        0x22 => (format!("JSL ${:06X}", get_u24(1)), 4),
        0xFC => (format!("JSR (${:04X},X)", get_u16(1)), 3),

        // Block moves
        0x44 => (format!("MVP ${:02X},${:02X}", get_u8(2), get_u8(1)), 3),
        0x54 => (format!("MVN ${:02X},${:02X}", get_u8(2), get_u8(1)), 3),

        // PEI, PER, PEA
        0xD4 => (format!("PEI (${:02X})", get_u8(1)), 2),
        0x62 => (format!("PER ${:04X}", get_u16(1)), 3),
        0xF4 => (format!("PEA ${:04X}", get_u16(1)), 3),

        // INC, DEC absolute
        0xE6 => (format!("INC ${:02X}", get_u8(1)), 2),
        0xF6 => (format!("INC ${:02X},X", get_u8(1)), 2),
        0xEE => (format!("INC ${:04X}", get_u16(1)), 3),
        0xFE => (format!("INC ${:04X},X", get_u16(1)), 3),
        0xC6 => (format!("DEC ${:02X}", get_u8(1)), 2),
        0xD6 => (format!("DEC ${:02X},X", get_u8(1)), 2),
        0xCE => (format!("DEC ${:04X}", get_u16(1)), 3),
        0xDE => (format!("DEC ${:04X},X", get_u16(1)), 3),

        _ => (format!("??? ${:02X}", opcode), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0xEA];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.address, 0x8000);
        assert_eq!(instr.len(), 1);
        assert_eq!(instr.mnemonic, "NOP");
    }

    #[test]
    fn test_disassemble_lda_immediate() {
        let memory = [0xA9, 0x42];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "LDA #$42");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jmp_absolute() {
        let memory = [0x4C, 0x00, 0x80];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "JMP $8000");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_rep() {
        let memory = [0xC2, 0x30];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "REP #$30");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jsl() {
        let memory = [0x22, 0x00, 0x80, 0x00];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "JSL $008000");
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_disassemble_empty() {
        let memory = [];
        assert!(disassemble_65c816(&memory, 0x8000).is_none());
    }

    #[test]
    fn test_disassemble_lda_absolute_long() {
        let memory = [0xAF, 0x00, 0x80, 0x01];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "LDA $018000");
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_disassemble_sta_absolute_long_x() {
        let memory = [0x9F, 0x00, 0x90, 0x7E];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "STA $7E9000,X");
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_disassemble_cmp_absolute_long() {
        let memory = [0xCF, 0xFF, 0xFF, 0xFF];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "CMP $FFFFFF");
        assert_eq!(instr.len(), 4);
    }
}
