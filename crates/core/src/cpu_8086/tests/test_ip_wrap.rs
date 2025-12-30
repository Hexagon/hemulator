// Test IP wrapping behavior
use crate::cpu_8086::{ArrayMemory, Cpu8086};

#[test]
fn test_ip_with_upper_bits_set() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Manually set IP with upper bits (simulating a 32-bit operation)
    cpu.ip = 0x00010100;  // IP with upper bits set
    
    // Load a simple JMP instruction
    cpu.memory.load_program(
        0x0100,
        &[
            0xEB, 0xFE,  // JMP -2 (infinite loop to itself)
        ],
    );
    
    cpu.cs = 0x0000;
    cpu.ip = 0x00010100;  // Start with upper bits set
    
    // Execute the JMP
    cpu.step();
    
    // IP should be wrapped to 16 bits and jump to 0x0100
    assert_eq!(cpu.ip, 0x0100, "IP should be wrapped to 16 bits");
}
