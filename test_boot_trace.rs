// Test program to trace boot sector execution with x86BOOT.img
// This will help us understand what's actually happening during boot

use std::fs;
use std::path::Path;

// We'll need to include the PC system modules
// For now, let's just create a basic trace

fn main() {
    println!("=== Boot Sector Trace Test ===\n");
    
    // Load x86BOOT.img
    let img_path = "test_roms/pc/x86BOOT.img";
    if !Path::new(img_path).exists() {
        eprintln!("ERROR: {} not found!", img_path);
        std::process::exit(1);
    }
    
    let disk_data = fs::read(img_path).expect("Failed to read disk image");
    println!("✅ Loaded {} ({} bytes)", img_path, disk_data.len());
    
    // Check boot sector signature
    if disk_data.len() >= 512 {
        let sig = u16::from_le_bytes([disk_data[510], disk_data[511]]);
        println!("   Boot sector signature: 0x{:04X} {}", sig, 
                 if sig == 0xAA55 { "✅" } else { "❌" });
        
        // Show first few bytes of boot sector
        println!("\n   First 32 bytes of boot sector:");
        for (i, chunk) in disk_data[0..32].chunks(16).enumerate() {
            print!("   {:04X}: ", i * 16);
            for byte in chunk {
                print!("{:02X} ", byte);
            }
            print!("  ");
            for byte in chunk {
                let ch = if *byte >= 32 && *byte < 127 { *byte as char } else { '.' };
                print!("{}", ch);
            }
            println!();
        }
    }
    
    println!("\n=== Creating PC System and Running Boot Sequence ===\n");
    println!("This test will create a minimal PC system, load the boot sector,");
    println!("and trace the first 100 instructions to see what happens.\n");
    
    // We need to actually create a PC system to test this properly
    // Since we can't easily import emu_pc here, let's create a more focused test
    println!("To run this test properly, we need to:");
    println!("1. Create PC system with x86BOOT.img mounted as floppy");
    println!("2. Enable detailed instruction tracing");
    println!("3. Run boot sequence and log first 100+ instructions");
    println!("4. Check for any anomalies (repeated instructions, wrong flags, etc.)");
    println!("\nLet's create a proper integration test instead...");
}
