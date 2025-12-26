// Test to reproduce the POP SI infinite loop issue
// This is a temporary test file to debug the boot sector issue

use emu_core::cpu_8086::{Cpu8086, ArrayMemory};

fn main() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Set up scenario similar to boot sector cleanup code
    // Address 0x7CF7 (segment 0x0000, offset 0x7CF7)
    // Instruction: POP SI (opcode 0x5E)
    
    cpu.cs = 0x0000;
    cpu.ip = 0x7CF7;
    cpu.ss = 0x0000;
    cpu.sp = 0x0100; // Initial stack pointer
    
    // Put some data on the stack for POP to read
    cpu.memory.load_program(0x00000 + 0x0100, &[0x34, 0x12]); // Stack data: 0x1234
    
    // Load the POP SI instruction at 0x7CF7
    cpu.memory.load_program(0x7CF7, &[0x5E]); // POP SI
    
    println!("Before step:");
    println!("  CS:IP = {:04X}:{:04X}", cpu.cs, cpu.ip);
    println!("  SP = {:04X}", cpu.sp);
    println!("  SI = {:04X}", cpu.si);
    
    // Execute one instruction
    let cycles = cpu.step();
    
    println!("\nAfter step:");
    println!("  CS:IP = {:04X}:{:04X}", cpu.cs, cpu.ip);
    println!("  SP = {:04X}", cpu.sp);
    println!("  SI = {:04X}", cpu.si);
    println!("  Cycles = {}", cycles);
    
    // Check if IP advanced
    if cpu.ip == 0x7CF7 {
        println!("\n❌ ERROR: IP did not advance! This is the bug.");
        std::process::exit(1);
    } else if cpu.ip == 0x7CF8 {
        println!("\n✅ SUCCESS: IP advanced correctly.");
        std::process::exit(0);
    } else {
        println!("\n⚠️  WARNING: IP advanced to unexpected value {:04X}", cpu.ip);
        std::process::exit(2);
    }
}
