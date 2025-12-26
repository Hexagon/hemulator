// Minimal boot trace to see where infinite loop happens
use std::fs;

fn main() {
    // Set trace environment variable
    std::env::set_var("EMU_TRACE_PC", "1");
    
    // Load x86BOOT.img
    let disk_data = fs::read("test_roms/pc/x86BOOT.img")
        .expect("Failed to read x86BOOT.img");
    
    println!("Loaded {} bytes from x86BOOT.img", disk_data.len());
    
    // Create PC system - need to use emu_pc
    use emu_pc::PcSystem;
    use emu_core::System;
    
    let mut sys = PcSystem::new();
    sys.mount("FloppyA", &disk_data).expect("Failed to mount");
    
    // Load boot sector
    sys.boot_delay_frames = 0;
    sys.boot_started = false;
    sys.ensure_boot_sector_loaded();
    
    println!("\n=== Executing first 50 instructions ===\n");
    
    for i in 0..50 {
        let info = sys.debug_info();
        let cycles = sys.cpu.step();
        
        if i < 10 || i > 45 {
            println!("Instr {}: CS:IP={:04X}:{:04X} cycles={}", 
                    i, info.cs, info.ip, cycles);
        } else if i == 10 {
            println!("... (instructions 10-45 hidden) ...");
        }
    }
    
    let final_info = sys.debug_info();
    println!("\n=== After 50 instructions ===");
    println!("CS:IP = {:04X}:{:04X}", final_info.cs, final_info.ip);
}
