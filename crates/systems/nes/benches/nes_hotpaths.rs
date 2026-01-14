use criterion::{black_box, criterion_group, criterion_main, Criterion};
use emu_core::debug::Debugger;
use emu_core::System;
use emu_nes::NesSystem;

/// Create a simple test ROM that exercises common NES operations
fn create_test_rom() -> Vec<u8> {
    let mut rom = vec![0; 16 + 0x8000]; // iNES header + 32KB PRG ROM

    // iNES header for NROM (mapper 0) with 32KB PRG, 8KB CHR
    rom[0..16].copy_from_slice(&[
        0x4E, 0x45, 0x53, 0x1A, // "NES" + MS-DOS EOF
        0x02, // 2 * 16KB PRG ROM banks = 32KB
        0x01, // 1 * 8KB CHR ROM bank = 8KB
        0x00, // Mapper 0 (NROM), horizontal mirroring
        0x00, // Mapper 0 (upper nibble), no special features
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    // Write reset vector to point to 0x8000
    rom[0x10 + 0x7FFC] = 0x00;
    rom[0x10 + 0x7FFD] = 0x80;

    // Write a loop of common instructions at 0x8000
    let mut offset = 0x10; // Start after iNES header

    // LDA #$42 ; A9 42 - immediate load (very common)
    rom[offset] = 0xA9;
    rom[offset + 1] = 0x42;
    offset += 2;

    // STA $2000 ; 8D 00 20 - absolute store to PPU (common)
    rom[offset] = 0x8D;
    rom[offset + 1] = 0x00;
    rom[offset + 2] = 0x20;
    offset += 3;

    // LDX #$10 ; A2 10 - load X register
    rom[offset] = 0xA2;
    rom[offset + 1] = 0x10;
    offset += 2;

    // LDY #$20 ; A0 20 - load Y register
    rom[offset] = 0xA0;
    rom[offset + 1] = 0x20;
    offset += 2;

    // INX ; E8 - increment X (very common in loops)
    rom[offset] = 0xE8;
    offset += 1;

    // INY ; C8 - increment Y
    rom[offset] = 0xC8;
    offset += 1;

    // DEX ; CA - decrement X (common in loops)
    rom[offset] = 0xCA;
    offset += 1;

    // DEY ; 88 - decrement Y
    rom[offset] = 0x88;
    offset += 1;

    // ADC #$01 ; 69 01 - add with carry (arithmetic)
    rom[offset] = 0x69;
    rom[offset + 1] = 0x01;
    offset += 2;

    // SBC #$01 ; E9 01 - subtract with carry
    rom[offset] = 0xE9;
    rom[offset + 1] = 0x01;
    offset += 2;

    // LDA $00 ; A5 00 - zero page load (very fast, common)
    rom[offset] = 0xA5;
    rom[offset + 1] = 0x00;
    offset += 2;

    // STA $00 ; 85 00 - zero page store
    rom[offset] = 0x85;
    rom[offset + 1] = 0x00;
    offset += 2;

    // JMP $8000 ; 4C 00 80 - loop back to start
    rom[offset] = 0x4C;
    rom[offset + 1] = 0x00;
    rom[offset + 2] = 0x80;

    // Pad with CHR ROM (8KB of zeros is fine)
    rom.extend_from_slice(&vec![0; 0x2000]);

    rom
}

fn bench_cpu_instructions(c: &mut Criterion) {
    let mut group = c.benchmark_group("nes_cpu_instructions");

    // For CPU instruction benchmarking, we measure frame execution
    // since that's the actual hot path - a frame executes ~30k CPU cycles
    group.bench_function("frame_cpu_execution", |b| {
        b.iter(|| {
            let mut nes = NesSystem::default();
            let rom = create_test_rom();
            nes.load_rom(&rom).unwrap();

            // Execute one frame (this exercises the CPU step loop heavily)
            let frame = nes.step_frame().unwrap();
            black_box(frame);
        });
    });

    group.finish();
}

fn bench_memory_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("nes_memory_access");

    group.bench_function("memory_read_ram", |b| {
        let mut nes = NesSystem::default();
        let rom = create_test_rom();
        nes.load_rom(&rom).unwrap();

        b.iter(|| {
            // Benchmark reading from different memory regions
            // This tests the bus address decoding performance
            for addr in (0x0000..0x0800).step_by(64) {
                let val = nes.read_memory(addr, 1).unwrap();
                black_box(val);
            }
        });
    });

    group.bench_function("memory_read_prg_rom", |b| {
        let mut nes = NesSystem::default();
        let rom = create_test_rom();
        nes.load_rom(&rom).unwrap();

        b.iter(|| {
            // Benchmark reading from PRG ROM (via mapper)
            for addr in (0x8000..0xC000).step_by(64) {
                let val = nes.read_memory(addr, 1).unwrap();
                black_box(val);
            }
        });
    });

    group.finish();
}

fn bench_frame_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("nes_frame_execution");

    // Reduce sample size for frame benchmarks as they're slower
    group.sample_size(10);

    group.bench_function("single_frame", |b| {
        b.iter(|| {
            let mut nes = NesSystem::default();
            let rom = create_test_rom();
            nes.load_rom(&rom).unwrap();

            // Execute one complete frame
            let frame = nes.step_frame().unwrap();
            black_box(frame);
        });
    });

    group.bench_function("ten_frames", |b| {
        b.iter(|| {
            let mut nes = NesSystem::default();
            let rom = create_test_rom();
            nes.load_rom(&rom).unwrap();

            // Execute 10 frames
            for _ in 0..10 {
                let frame = nes.step_frame().unwrap();
                black_box(frame);
            }
        });
    });

    group.finish();
}

fn bench_ppu_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("nes_ppu_operations");

    group.bench_function("frame_with_ppu", |b| {
        let mut nes = NesSystem::default();
        let rom = create_test_rom();
        nes.load_rom(&rom).unwrap();

        b.iter(|| {
            // Execute a frame which includes PPU rendering
            let frame = nes.step_frame().unwrap();
            black_box(frame);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cpu_instructions,
    bench_memory_access,
    bench_frame_execution,
    bench_ppu_operations
);
criterion_main!(benches);
