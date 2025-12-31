use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use emu_core::cpu_6502::{Cpu6502, Memory6502};

/// Simple memory implementation for benchmarking
struct BenchMemory {
    ram: Vec<u8>,
}

impl BenchMemory {
    fn new() -> Self {
        let mut ram = vec![0; 0x10000];

        // Set reset vector to point to 0x8000
        ram[0xFFFC] = 0x00;
        ram[0xFFFD] = 0x80;

        // Write some test code at 0x8000
        // LDA #$42 ; A9 42
        ram[0x8000] = 0xA9;
        ram[0x8001] = 0x42;
        // STA $2000 ; 8D 00 20
        ram[0x8002] = 0x8D;
        ram[0x8003] = 0x00;
        ram[0x8004] = 0x20;
        // LDX #$10 ; A2 10
        ram[0x8005] = 0xA2;
        ram[0x8006] = 0x10;
        // LDY #$20 ; A0 20
        ram[0x8007] = 0xA0;
        ram[0x8008] = 0x20;
        // INX ; E8
        ram[0x8009] = 0xE8;
        // INY ; C8
        ram[0x800A] = 0xC8;
        // DEX ; CA
        ram[0x800B] = 0xCA;
        // DEY ; 88
        ram[0x800C] = 0x88;
        // ADC #$01 ; 69 01
        ram[0x800D] = 0x69;
        ram[0x800E] = 0x01;
        // JMP $8000 ; 4C 00 80 (loop back)
        ram[0x800F] = 0x4C;
        ram[0x8010] = 0x00;
        ram[0x8011] = 0x80;

        Self { ram }
    }
}

impl Memory6502 for BenchMemory {
    fn read(&self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.ram[addr as usize] = val;
    }
}

fn bench_cpu_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_6502_step");

    group.bench_function("single_instruction", |b| {
        b.iter(|| {
            let mut cpu = Cpu6502::new(BenchMemory::new());
            cpu.reset();
            cpu.step();
            black_box(cpu.a);
        });
    });

    group.finish();
}

fn bench_cpu_multiple_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_6502_multiple_steps");

    for step_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(step_count),
            step_count,
            |b, &count| {
                b.iter(|| {
                    let mut cpu = Cpu6502::new(BenchMemory::new());
                    cpu.reset();
                    for _ in 0..count {
                        cpu.step();
                    }
                    black_box(cpu.cycles);
                });
            },
        );
    }

    group.finish();
}

fn bench_cpu_addressing_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_6502_addressing");

    // Benchmark different addressing modes
    group.bench_function("immediate_mode", |b| {
        b.iter(|| {
            let mut cpu = Cpu6502::new(BenchMemory::new());
            cpu.reset();
            // Execute LDA #$42 multiple times
            for _ in 0..100 {
                cpu.step();
            }
            black_box(cpu.a);
        });
    });

    group.finish();
}

fn bench_cpu_reset(c: &mut Criterion) {
    c.bench_function("cpu_6502_reset", |b| {
        let mut cpu = Cpu6502::new(BenchMemory::new());
        b.iter(|| {
            cpu.reset();
            black_box(cpu.pc);
        });
    });
}

criterion_group!(
    benches,
    bench_cpu_step,
    bench_cpu_multiple_steps,
    bench_cpu_addressing_modes,
    bench_cpu_reset
);
criterion_main!(benches);
