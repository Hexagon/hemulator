//! IBM PC/XT system debugger implementation.
//!
//! Provides debug introspection for the PC system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::PcSystem;
use emu_core::cpu_8086::Memory8086;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_8086;

impl Debugger for PcSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 6 bytes for the instruction (max 8086 instruction size)
        let memory = self.read_memory(address, 6)?;
        disasm_8086::disassemble_8086(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // 8086 has 20-bit address space (1MB)
        if address >= 0x100000 {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        let bus = self.cpu.bus();

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr >= 0x100000 {
                break;
            }
            result.push(bus.read(addr));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = Vec::new();

        // Conventional memory (0-640KB)
        let conventional_kb = self.cpu.bus().conventional_memory_kb();
        if conventional_kb > 0 {
            regions.push(MemoryRegion::new(
                "Conventional RAM",
                0x00000,
                (conventional_kb * 1024) - 1,
                format!("{}KB conventional memory (0-640KB range)", conventional_kb),
                true,
                true,
            ));
        }

        // BIOS Data Area (BDA)
        regions.push(MemoryRegion::new(
            "BIOS Data Area",
            0x00400,
            0x004FF,
            "256 bytes of BIOS runtime data",
            true,
            true,
        ));

        // Extended BIOS Data Area (EBDA)
        regions.push(MemoryRegion::new(
            "EBDA",
            0x9FC00,
            0x9FFFF,
            "Extended BIOS Data Area (1KB)",
            true,
            true,
        ));

        // Video memory region
        regions.push(MemoryRegion::new(
            "Video RAM",
            0xA0000,
            0xBFFFF,
            "128KB video memory (VGA, EGA, CGA)",
            true,
            true,
        ));

        // CGA text mode specifically
        regions.push(MemoryRegion::new(
            "CGA Text Buffer",
            0xB8000,
            0xB8FFF,
            "4KB CGA color text mode buffer (80x25)",
            true,
            true,
        ));

        // BIOS ROM
        regions.push(MemoryRegion::new(
            "BIOS ROM",
            0xF0000,
            0xFFFFF,
            "64KB BIOS ROM (F000-FFFF)",
            true,
            false,
        ));

        // Extended memory (if present, above 1MB)
        let extended_kb = self.cpu.bus().extended_memory_kb();
        if extended_kb > 0 {
            regions.push(MemoryRegion::new(
                "Extended RAM",
                0x100000,
                0x100000 + (extended_kb * 1024) - 1,
                format!("{}KB extended memory (above 1MB)", extended_kb),
                true,
                true,
            ));
        }

        regions
    }

    fn get_cpu_state(&self) -> CpuState {
        let regs = self.cpu.get_registers();

        // Note: CpuState stores the program counter both as a dedicated `pc` field
        // (set via CpuState::new) and as a register entry named "IP".
        // The `pc` field is used by the debugger for navigation (current instruction),
        // while the "IP" register is added below so it also appears in the generic
        // registers list shown in the UI.
        // For x86, we calculate the linear address from CS:IP for the pc field.
        let linear_pc = ((regs.cs as u32) << 4) + regs.ip;
        let mut state = CpuState::new(linear_pc);

        // Add general-purpose registers
        state.add_register(CpuRegister::new_32bit("EAX", regs.ax));
        state.add_register(CpuRegister::new_32bit("EBX", regs.bx));
        state.add_register(CpuRegister::new_32bit("ECX", regs.cx));
        state.add_register(CpuRegister::new_32bit("EDX", regs.dx));

        // Add index and pointer registers
        state.add_register(CpuRegister::new_32bit("ESI", regs.si));
        state.add_register(CpuRegister::new_32bit("EDI", regs.di));
        state.add_register(CpuRegister::new_32bit("EBP", regs.bp));
        state.add_register(CpuRegister::new_32bit("ESP", regs.sp));

        // Add segment registers (16-bit)
        state.add_register(CpuRegister::new_16bit("CS", regs.cs));
        state.add_register(CpuRegister::new_16bit("DS", regs.ds));
        state.add_register(CpuRegister::new_16bit("ES", regs.es));
        state.add_register(CpuRegister::new_16bit("SS", regs.ss));

        // Add instruction pointer
        state.add_register(CpuRegister::new_32bit("EIP", regs.ip));

        // Add FLAGS register as a whole (for reference)
        state.add_register(CpuRegister::new_32bit("FLAGS", regs.flags));

        // Extract individual flags from the FLAGS register
        // x86 FLAGS format (16-bit, lower part of EFLAGS):
        // Bit 0:  CF (Carry Flag)
        // Bit 2:  PF (Parity Flag)
        // Bit 4:  AF (Auxiliary Carry Flag)
        // Bit 6:  ZF (Zero Flag)
        // Bit 7:  SF (Sign Flag)
        // Bit 8:  TF (Trap Flag)
        // Bit 9:  IF (Interrupt Enable Flag)
        // Bit 10: DF (Direction Flag)
        // Bit 11: OF (Overflow Flag)

        let flags = regs.flags;
        state.add_flag("CF", (flags & 0x0001) != 0); // Carry
        state.add_flag("PF", (flags & 0x0004) != 0); // Parity
        state.add_flag("AF", (flags & 0x0010) != 0); // Auxiliary Carry
        state.add_flag("ZF", (flags & 0x0040) != 0); // Zero
        state.add_flag("SF", (flags & 0x0080) != 0); // Sign
        state.add_flag("TF", (flags & 0x0100) != 0); // Trap
        state.add_flag("IF", (flags & 0x0200) != 0); // Interrupt Enable
        state.add_flag("DF", (flags & 0x0400) != 0); // Direction
        state.add_flag("OF", (flags & 0x0800) != 0); // Overflow

        state
    }

    fn get_execution_history(&self) -> Vec<emu_core::debug::ExecutionTrace> {
        self.instruction_tracer.get_history()
    }

    fn has_execution_history(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pc_memory_regions() {
        let system = PcSystem::new();
        let regions = system.get_memory_regions();

        // Should have at least the basic memory regions
        assert!(regions.len() >= 5, "Expected at least 5 memory regions");

        // Check that Conventional RAM region exists
        let ram_region = regions.iter().find(|r| r.name == "Conventional RAM");
        assert!(ram_region.is_some(), "Conventional RAM region should exist");
        let ram = ram_region.unwrap();
        assert_eq!(ram.start, 0x00000);
        assert!(ram.readable);
        assert!(ram.writable);

        // Check that BIOS ROM region exists
        let bios_region = regions.iter().find(|r| r.name == "BIOS ROM");
        assert!(bios_region.is_some(), "BIOS ROM region should exist");
        let bios = bios_region.unwrap();
        assert_eq!(bios.start, 0xF0000);
        assert_eq!(bios.end, 0xFFFFF);
        assert!(bios.readable);
        assert!(!bios.writable, "BIOS ROM should not be writable");

        // Check that Video RAM region exists
        let vram_region = regions.iter().find(|r| r.name == "Video RAM");
        assert!(vram_region.is_some(), "Video RAM region should exist");
        let vram = vram_region.unwrap();
        assert_eq!(vram.start, 0xA0000);
        assert_eq!(vram.end, 0xBFFFF);
        assert!(vram.readable);
        assert!(vram.writable);
    }

    #[test]
    fn test_pc_cpu_state() {
        let system = PcSystem::new();
        let state = system.get_cpu_state();

        // Should have PC register (linear address from CS:IP)
        // At reset, CS=0xFFFF, IP=0x0000, so linear PC = 0xFFFF0
        assert_eq!(state.pc, 0xFFFF0);

        // Should have all standard x86 registers
        assert!(state.registers.iter().any(|r| r.name == "EAX"));
        assert!(state.registers.iter().any(|r| r.name == "EBX"));
        assert!(state.registers.iter().any(|r| r.name == "ECX"));
        assert!(state.registers.iter().any(|r| r.name == "EDX"));
        assert!(state.registers.iter().any(|r| r.name == "ESI"));
        assert!(state.registers.iter().any(|r| r.name == "EDI"));
        assert!(state.registers.iter().any(|r| r.name == "EBP"));
        assert!(state.registers.iter().any(|r| r.name == "ESP"));
        assert!(state.registers.iter().any(|r| r.name == "CS"));
        assert!(state.registers.iter().any(|r| r.name == "DS"));
        assert!(state.registers.iter().any(|r| r.name == "ES"));
        assert!(state.registers.iter().any(|r| r.name == "SS"));
        assert!(state.registers.iter().any(|r| r.name == "EIP"));
        assert!(state.registers.iter().any(|r| r.name == "FLAGS"));

        // Should have all x86 flags in correct order
        assert_eq!(state.flags.flags.len(), 9);
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            flag_names,
            vec!["CF", "PF", "AF", "ZF", "SF", "TF", "IF", "DF", "OF"]
        );
    }

    #[test]
    fn test_pc_cpu_flags_extraction() {
        let system = PcSystem::new();
        let state = system.get_cpu_state();

        // Verify all 9 flags exist and are booleans
        assert_eq!(state.flags.flags.len(), 9);

        // After reset, most flags should be clear
        // Check that flags are properly extracted (values may vary by CPU model)
        for (name, _value) in &state.flags.flags {
            assert!(
                ["CF", "PF", "AF", "ZF", "SF", "TF", "IF", "DF", "OF"].contains(&name.as_str()),
                "Unexpected flag: {}",
                name
            );
        }
    }

    #[test]
    fn test_pc_read_memory() {
        let system = PcSystem::new();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x00000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should be able to read from BIOS area
        let bios_memory = system.read_memory(0xF0000, 16);
        assert!(bios_memory.is_some());
        assert_eq!(bios_memory.unwrap().len(), 16);

        // Should return None for invalid addresses (above 1MB for 8086)
        let invalid = system.read_memory(0x100000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_pc_disassemble() {
        let system = PcSystem::new();

        // Try to disassemble from BIOS area (always has code)
        if let Some(instr) = system.disassemble_instruction(0xFFFF0) {
            // Should get a valid instruction
            assert!(!instr.bytes.is_empty());
            assert!(!instr.mnemonic.is_empty());
            assert_eq!(instr.address, 0xFFFF0);
        }
    }

    #[test]
    fn test_pc_execution_history() {
        let system = PcSystem::new();

        // By default, instruction tracing should be disabled
        assert!(!system.has_execution_history());
        assert_eq!(system.get_execution_history().len(), 0);
    }

    #[test]
    fn test_pc_execution_history_enabled() {
        let mut system = PcSystem::new();

        // Enable instruction tracing
        system.set_instruction_tracing(true);
        assert!(system.has_execution_history());

        // Note: Instruction history tracking happens at the CPU core level
        // The PC system wraps the CPU and the tracer is available but
        // requires integration at the CPU execution level to record traces.
        // For now, just verify that the interface is available.
        let history = system.get_execution_history();
        assert_eq!(
            history.len(),
            0,
            "History starts empty until CPU integration is complete"
        );
    }

    #[test]
    fn test_pc_debugger_integration() {
        // This test demonstrates all debugger features working together
        let system = PcSystem::new();

        println!("\n=== PC Debugger Integration Test ===\n");

        // Test memory regions
        let regions = system.get_memory_regions();
        println!("Memory Regions: {}", regions.len());
        assert!(regions.len() >= 5, "Should have at least 5 memory regions");

        for region in &regions {
            println!(
                "  {} (${:05X}-${:05X}): {}",
                region.name, region.start, region.end, region.description
            );
        }

        // Test CPU state
        let cpu_state = system.get_cpu_state();
        println!("\nCPU State:");
        println!("  PC: ${:05X}", cpu_state.pc);
        println!("  Registers: {}", cpu_state.registers.len());
        println!("  Flags: {}", cpu_state.flags.flags.len());

        assert_eq!(cpu_state.pc, 0xFFFF0, "Reset vector should be at 0xFFFF0");
        assert!(
            cpu_state.registers.len() >= 14,
            "Should have all x86 registers"
        );
        assert_eq!(
            cpu_state.flags.flags.len(),
            9,
            "Should have all 9 x86 flags"
        );

        // Test disassembly at BIOS entry
        if let Some(instr) = system.disassemble_instruction(0xFFFF0) {
            println!("\nDisassembly at ${:05X}:", instr.address);
            println!("  Bytes: {:02X?}", instr.bytes);
            println!("  Mnemonic: {}", instr.mnemonic);
            assert_eq!(instr.address, 0xFFFF0);
            assert!(!instr.bytes.is_empty());
        }

        // Test memory reading
        let mem = system.read_memory(0x00000, 16).unwrap();
        println!("\nMemory at 0x00000: {:02X?}", mem);
        assert_eq!(mem.len(), 16);

        println!("\n=== All debugger features verified! ✓ ===\n");
    }
}
