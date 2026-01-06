# SPC700 CPU Reference

## Overview
The SPC700 is an 8-bit CPU used in the SNES Audio Processing Unit (APU). It has 256 documented opcodes with no illegal instructions.

## Architecture
- **Clock Speed**: 1.024 MHz  
- **Registers**: A (accumulator), X, Y (index), SP (stack pointer), PC (program counter), PSW (flags)
- **Address Space**: 64KB
- **Stack**: Page 1 ($0100-$01FF)

## Processor Status Word (PSW) Flags
- **N** (Bit 7): Negative flag
- **V** (Bit 6): Overflow flag  
- **P** (Bit 5): Direct page flag ($0000 or $0100)
- **B** (Bit 4): Break flag
- **H** (Bit 3): Half-carry flag (BCD)
- **I** (Bit 2): Interrupt enable flag
- **Z** (Bit 1): Zero flag
- **C** (Bit 0): Carry flag

## Addressing Modes
- **Immediate**: `#nn`
- **Direct Page**: `dp` (page 0 or 1 based on P flag)
- **Direct Page + X**: `dp+X`
- **Direct Page + Y**: `dp+Y`
- **Absolute**: `!abs` ($xxxx)
- **Absolute + X**: `!abs+X`
- **Absolute + Y**: `!abs+Y`
- **Indirect X**: `(X)`
- **Indirect X with increment**: `(X)+`
- **Indirect DP**: `(dp)`
- **Indirect DP + X**: `(dp+X)`
- **Indirect DP + Y**: `(dp)+Y`

## Opcode Categories

### Data Movement (MOV)
80+ opcodes for moving data between registers, memory, and immediate values.

### Arithmetic
- **ADC**: Add with carry
- **SBC**: Subtract with carry
- **INC/DEC**: Increment/Decrement (8-bit)
- **INCW/DECW**: Increment/Decrement (16-bit)
- **MUL**: 8x8 multiply (YA = Y * A)
- **DIV**: 16÷8 divide (A = YA / X, Y = YA % X)
- **DAA/DAS**: Decimal adjust for add/subtract

### Logic
- **AND**: Bitwise AND
- **OR**: Bitwise OR
- **EOR**: Bitwise XOR

### Shifts
- **ASL**: Arithmetic shift left
- **LSR**: Logical shift right
- **ROL**: Rotate left through carry
- **ROR**: Rotate right through carry

### Bit Operations
- **SET1/CLR1**: Set/clear direct page bit (8 variants each)
- **BBS/BBC**: Branch if bit set/clear (8 variants each)
- **AND1/OR1/EOR1/NOT1**: Bit operations with carry flag
- **MOV1**: Move bit to/from carry
- **TSET1/TCLR1**: Test and set/clear bits

### Compares
- **CMP**: Compare A, X, or Y with memory
- **CMPW**: Compare 16-bit YA with memory

### Branches
- **BEQ/BNE**: Branch if equal/not equal (Z flag)
- **BCS/BCC**: Branch if carry set/clear
- **BVS/BVC**: Branch if overflow set/clear
- **BMI/BPL**: Branch if minus/plus (N flag)
- **BRA**: Branch always

### Jumps and Calls
- **JMP**: Jump to address
- **CALL**: Call subroutine
- **TCALL**: Table call (16 variants, $01-$F1)
- **PCALL**: Page call (within page $FF)
- **RET/RETI**: Return from subroutine/interrupt
- **BRK**: Software interrupt

### Stack Operations
- **PUSH/POP**: Push/pop A, X, Y, or PSW

### Special
- **CBNE**: Compare and branch if not equal
- **DBNZ**: Decrement and branch if not zero
- **XCN**: Exchange nibbles in A
- **CLRP/SETP**: Clear/set direct page flag
- **CLRC/SETC**: Clear/set carry flag
- **CLRV**: Clear overflow flag
- **NOTC**: Complement carry flag
- **EI/DI**: Enable/disable interrupts
- **NOP**: No operation
- **SLEEP/STOP**: Halt CPU

## 16-bit Operations
- **MOVW**: Move 16-bit word
- **INCW/DECW**: Increment/decrement 16-bit
- **ADDW/SUBW**: Add/subtract 16-bit
- **CMPW**: Compare 16-bit

## Cycle Counts
- Most instructions: 2-8 cycles
- Branches add 2 cycles if taken
- Write operations add 1 cycle
- See specific opcode documentation for exact counts

## References
- [SPC700 Opcode Matrix - SnesLab](https://sneslab.net/wiki/SPC700_Opcode_Matrix)
- [Anomie's SPC700 Documentation](https://github.com/gilligan/snesdev/blob/master/docs/spc700.txt)
- [SPC700 Reference - Super Famicom Wiki](https://wiki.superfamicom.org/spc700-reference)
- [SPC700 Instruction Set - SNESdev Wiki](https://snes.nesdev.org/wiki/SPC-700_instruction_set)
- [Fullsnes - Nocash SNES Specs](https://www.problemkaputt.de/fullsnes.htm)
- [Booting the SPC700](https://snes.nesdev.org/wiki/Booting_the_SPC700)
