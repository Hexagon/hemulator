---
title: "CPU & Hardware References"
nav_order: 3
---

# Technical References

This section contains detailed technical documentation for the CPU and hardware components implemented in Hemulator.

## CPU References

Hemulator implements several CPU architectures as reusable components. Each CPU has comprehensive documentation including instruction sets, addressing modes, and implementation notes.

### Available CPU Documentation

- [6502 CPU](6502.md) - MOS 6502 (NES, Atari 2600, Apple II)
- [65C816 CPU](65c816.md) - WDC 65C816 (SNES, Apple IIGS)
- [8080 CPU](8080.md) - Intel 8080 (foundation for Z80)
- [8086 CPU](8086.md) - Intel 8086/80186/80286/80386 (PC/XT)
- [LR35902 CPU](lr35902.md) - Sharp LR35902 (Game Boy)
- [MIPS R4300i CPU](mips-r4300i.md) - MIPS R4300i (Nintendo 64)
- [SPC700 CPU](spc700.md) - Sony SPC700 (SNES Audio)
- [Z80 CPU](z80.md) - Zilog Z80 (Sega Master System, Game Gear)

## Hardware Component References

- [PC Interrupts](pc-interrupts.md) - Comprehensive interrupt handling documentation for PC/DOS
- [SPC700 IPL Protocol](spc700-ipl.md) - SNES audio processor boot protocol

## Implementation Guidelines

For guidelines on implementing new CPU components or systems, see:

- [Architecture Overview](../developer/architecture.md)
- [Contributing Guide](../developer/contributing.md)

## External Resources

The implementation of these CPUs references numerous external sources:

### 6502 Resources
- [6502.org](http://www.6502.org/) - Official MOS 6502 documentation
- [NESDev Wiki](https://www.nesdev.org/) - NES-specific 6502 details
- [Obelisk 6502 Reference](http://www.obelisk.me.uk/6502/) - Comprehensive instruction reference

### Z80 Resources
- [Z80 CPU User Manual](http://www.zilog.com/docs/z80/um0080.pdf) - Official Zilog documentation
- [ClrHome Z80 Reference](http://clrhome.org/table/) - Instruction timing and flags

### 8086 Resources
- [Intel 80186/80188/80386 Programmer's Reference Manual](https://www.intel.com/)
- [x86 Instruction Set Reference](https://www.felixcloutier.com/x86/)
- [OSDev Wiki](https://wiki.osdev.org/) - PC hardware documentation

### MIPS Resources
- [MIPS R4000 Microprocessor User's Manual](https://www.cs.cmu.edu/afs/cs/academic/class/15740-f97/public/doc/mips-isa.pdf)
- [N64 Development Wiki](https://n64brew.dev/)

### Game Boy Resources
- [Pan Docs](https://gbdev.io/pandocs/) - Comprehensive Game Boy technical reference
- [Game Boy CPU Manual](http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf)

### SNES Resources
- [Super Famicom Development Wiki](https://wiki.superfamicom.org/)
- [65816 Programming Manual](http://www.westerndesigncenter.com/wdc/documentation/w65c816s.pdf)
- [Anomie's SNES Documents](https://www.romhacking.net/documents/226/)

## Contributing References

When implementing CPU features or fixing bugs, please:

1. **Add references** to the appropriate CPU documentation file
2. **Document sources** used for implementation decisions
3. **Include links** to datasheets, manuals, or technical documents
4. **Update test cases** based on official documentation

See the [Contributing Guide](../developer/contributing.md) for more information.
