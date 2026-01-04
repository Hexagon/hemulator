//! CHIP-8 system implementation
//!
//! CHIP-8 is an interpreted programming language designed for 8-bit microcomputers in the mid-1970s.
//! It has since become a popular target for emulator developers due to its simplicity.
//!
//! # Architecture
//!
//! ## CPU/Interpreter
//! - 16 8-bit general-purpose registers (V0-VF, where VF is used as a flag)
//! - 16-bit index register (I)
//! - 16-bit program counter (PC)
//! - 8-bit stack pointer (SP)
//! - 16 levels of stack (for subroutines)
//! - Two timers (delay timer and sound timer) that count down at 60Hz
//!
//! ## Memory
//! - 4KB (4096 bytes) of RAM
//! - Programs start at address 0x200
//! - 0x000-0x1FF: Reserved for interpreter (includes font data)
//! - 0x200-0xFFF: Program ROM and work RAM
//!
//! ## Display
//! - 64x32 pixel monochrome display
//! - Sprites are 8 pixels wide and 1-15 pixels tall
//! - Drawing uses XOR mode (toggle pixels)
//! - VF register set to 1 if any pixels are turned off during draw
//!
//! ## Input
//! - 16-key hexadecimal keypad (0x0-0xF)
//! - Original layout:
//!   ```text
//!   1 2 3 C
//!   4 5 6 D
//!   7 8 9 E
//!   A 0 B F
//!   ```
//! - Commonly mapped to QWERTY keyboard
//!
//! ## Audio
//! - Single tone beep
//! - Sounds when sound timer is non-zero
//!
//! ## Instruction Set
//! - 35 opcodes, all 2 bytes long
//! - Big-endian format
//! - Executed at variable speed (traditionally ~700 instructions per second)

#![allow(clippy::upper_case_acronyms)]

use emu_core::{types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Chip8Error {
    #[error("No program loaded")]
    NoProgram,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
    #[error("Program too large (max {max} bytes, got {size} bytes)")]
    ProgramTooLarge { size: usize, max: usize },
}

/// CHIP-8 font data (0-F hexadecimal sprites)
/// Each character is 5 bytes tall, 8 pixels wide (4 pixels used)
const FONT_DATA: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

const MEMORY_SIZE: usize = 4096;
const PROGRAM_START: usize = 0x200;
const DISPLAY_WIDTH: usize = 64;
const DISPLAY_HEIGHT: usize = 32;
const STACK_SIZE: usize = 16;

/// CHIP-8 system state
pub struct Chip8System {
    // Registers
    v: [u8; 16], // V0-VF general purpose registers
    i: u16,      // Index register
    pc: u16,     // Program counter
    sp: u8,      // Stack pointer

    // Memory
    memory: [u8; MEMORY_SIZE],
    stack: [u16; STACK_SIZE],

    // Display
    display: [bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
    display_updated: bool, // Flag to know when to redraw

    // Timers
    delay_timer: u8,
    sound_timer: u8,

    // Input (16 keys)
    keys: [bool; 16],

    // Execution
    cycles_this_frame: u32,
    program_loaded: bool,

    // For waiting for keypress (FX0A instruction)
    waiting_for_key: Option<usize>, // Some(register_index) when waiting
}

impl Default for Chip8System {
    fn default() -> Self {
        Self::new()
    }
}

impl Chip8System {
    /// Create a new CHIP-8 system
    pub fn new() -> Self {
        let mut system = Self {
            v: [0; 16],
            i: 0,
            pc: PROGRAM_START as u16,
            sp: 0,
            memory: [0; MEMORY_SIZE],
            stack: [0; STACK_SIZE],
            display: [false; DISPLAY_WIDTH * DISPLAY_HEIGHT],
            display_updated: false,
            delay_timer: 0,
            sound_timer: 0,
            keys: [false; 16],
            cycles_this_frame: 0,
            program_loaded: false,
            waiting_for_key: None,
        };

        // Load font data into memory (at 0x000-0x04F)
        system.memory[0..FONT_DATA.len()].copy_from_slice(&FONT_DATA);

        system
    }

    /// Execute one instruction
    fn execute_instruction(&mut self) {
        // Check if waiting for key press
        if let Some(register) = self.waiting_for_key {
            // Check if any key is pressed
            for (i, &pressed) in self.keys.iter().enumerate() {
                if pressed {
                    self.v[register] = i as u8;
                    self.waiting_for_key = None;
                    break;
                }
            }
            return; // Don't execute instructions while waiting
        }

        // Fetch opcode (2 bytes, big-endian)
        let opcode = u16::from_be_bytes([
            self.memory[self.pc as usize],
            self.memory[self.pc as usize + 1],
        ]);

        // Decode and execute
        self.pc += 2; // Increment PC before execution (some instructions modify PC)

        let nnn = opcode & 0x0FFF; // 12-bit address
        let nn = (opcode & 0x00FF) as u8; // 8-bit constant
        let n = (opcode & 0x000F) as u8; // 4-bit constant
        let x = ((opcode & 0x0F00) >> 8) as usize; // 4-bit register index
        let y = ((opcode & 0x00F0) >> 4) as usize; // 4-bit register index

        match opcode & 0xF000 {
            0x0000 => match nn {
                0xE0 => {
                    // 00E0 - CLS: Clear display
                    self.display.fill(false);
                    self.display_updated = true;
                }
                0xEE => {
                    // 00EE - RET: Return from subroutine
                    self.sp -= 1;
                    self.pc = self.stack[self.sp as usize];
                }
                _ => {
                    // 0NNN - SYS addr: Call machine code routine (ignored)
                }
            },
            0x1000 => {
                // 1NNN - JP addr: Jump to address
                self.pc = nnn;
            }
            0x2000 => {
                // 2NNN - CALL addr: Call subroutine
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            0x3000 => {
                // 3XNN - SE Vx, byte: Skip next instruction if Vx == NN
                if self.v[x] == nn {
                    self.pc += 2;
                }
            }
            0x4000 => {
                // 4XNN - SNE Vx, byte: Skip next instruction if Vx != NN
                if self.v[x] != nn {
                    self.pc += 2;
                }
            }
            0x5000 => {
                // 5XY0 - SE Vx, Vy: Skip next instruction if Vx == Vy
                if self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            0x6000 => {
                // 6XNN - LD Vx, byte: Set Vx = NN
                self.v[x] = nn;
            }
            0x7000 => {
                // 7XNN - ADD Vx, byte: Set Vx = Vx + NN
                self.v[x] = self.v[x].wrapping_add(nn);
            }
            0x8000 => match n {
                0x0 => {
                    // 8XY0 - LD Vx, Vy: Set Vx = Vy
                    self.v[x] = self.v[y];
                }
                0x1 => {
                    // 8XY1 - OR Vx, Vy: Set Vx = Vx OR Vy
                    self.v[x] |= self.v[y];
                }
                0x2 => {
                    // 8XY2 - AND Vx, Vy: Set Vx = Vx AND Vy
                    self.v[x] &= self.v[y];
                }
                0x3 => {
                    // 8XY3 - XOR Vx, Vy: Set Vx = Vx XOR Vy
                    self.v[x] ^= self.v[y];
                }
                0x4 => {
                    // 8XY4 - ADD Vx, Vy: Set Vx = Vx + Vy, set VF = carry
                    let (result, overflow) = self.v[x].overflowing_add(self.v[y]);
                    self.v[x] = result;
                    self.v[0xF] = overflow as u8;
                }
                0x5 => {
                    // 8XY5 - SUB Vx, Vy: Set Vx = Vx - Vy, set VF = NOT borrow
                    let (result, borrow) = self.v[x].overflowing_sub(self.v[y]);
                    self.v[x] = result;
                    self.v[0xF] = !borrow as u8;
                }
                0x6 => {
                    // 8XY6 - SHR Vx: Set Vx = Vx SHR 1 (shift right)
                    self.v[0xF] = self.v[x] & 0x1;
                    self.v[x] >>= 1;
                }
                0x7 => {
                    // 8XY7 - SUBN Vx, Vy: Set Vx = Vy - Vx, set VF = NOT borrow
                    let (result, borrow) = self.v[y].overflowing_sub(self.v[x]);
                    self.v[x] = result;
                    self.v[0xF] = !borrow as u8;
                }
                0xE => {
                    // 8XYE - SHL Vx: Set Vx = Vx SHL 1 (shift left)
                    self.v[0xF] = (self.v[x] & 0x80) >> 7;
                    self.v[x] <<= 1;
                }
                _ => {}
            },
            0x9000 => {
                // 9XY0 - SNE Vx, Vy: Skip next instruction if Vx != Vy
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            0xA000 => {
                // ANNN - LD I, addr: Set I = NNN
                self.i = nnn;
            }
            0xB000 => {
                // BNNN - JP V0, addr: Jump to address NNN + V0
                self.pc = nnn + self.v[0] as u16;
            }
            0xC000 => {
                // CXNN - RND Vx, byte: Set Vx = random byte AND NN
                // Use simple LCG for deterministic behavior (using cycle count as seed)
                let random = ((self
                    .cycles_this_frame
                    .wrapping_mul(1103515245)
                    .wrapping_add(12345))
                    >> 16) as u8;
                self.v[x] = random & nn;
            }
            0xD000 => {
                // DXYN - DRW Vx, Vy, nibble: Display n-byte sprite at (Vx, Vy)
                self.draw_sprite(x, y, n as usize);
            }
            0xE000 => match nn {
                0x9E => {
                    // EX9E - SKP Vx: Skip next instruction if key Vx is pressed
                    if self.v[x] < 16 && self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                0xA1 => {
                    // EXA1 - SKNP Vx: Skip next instruction if key Vx is not pressed
                    if self.v[x] >= 16 || !self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            0xF000 => match nn {
                0x07 => {
                    // FX07 - LD Vx, DT: Set Vx = delay timer
                    self.v[x] = self.delay_timer;
                }
                0x0A => {
                    // FX0A - LD Vx, K: Wait for key press, store key in Vx
                    self.waiting_for_key = Some(x);
                    self.pc -= 2; // Stay on this instruction until key is pressed
                }
                0x15 => {
                    // FX15 - LD DT, Vx: Set delay timer = Vx
                    self.delay_timer = self.v[x];
                }
                0x18 => {
                    // FX18 - LD ST, Vx: Set sound timer = Vx
                    self.sound_timer = self.v[x];
                }
                0x1E => {
                    // FX1E - ADD I, Vx: Set I = I + Vx
                    self.i = self.i.wrapping_add(self.v[x] as u16);
                }
                0x29 => {
                    // FX29 - LD F, Vx: Set I = location of sprite for digit Vx
                    self.i = (self.v[x] as u16 & 0x0F) * 5;
                }
                0x33 => {
                    // FX33 - LD B, Vx: Store BCD representation of Vx in memory locations I, I+1, I+2
                    let value = self.v[x];
                    self.memory[self.i as usize] = value / 100;
                    self.memory[self.i as usize + 1] = (value / 10) % 10;
                    self.memory[self.i as usize + 2] = value % 10;
                }
                0x55 => {
                    // FX55 - LD [I], Vx: Store registers V0 through Vx in memory starting at I
                    for reg in 0..=x {
                        self.memory[self.i as usize + reg] = self.v[reg];
                    }
                }
                0x65 => {
                    // FX65 - LD Vx, [I]: Read registers V0 through Vx from memory starting at I
                    for reg in 0..=x {
                        self.v[reg] = self.memory[self.i as usize + reg];
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Draw sprite at (Vx, Vy) with height n
    fn draw_sprite(&mut self, x_reg: usize, y_reg: usize, height: usize) {
        let x_pos = self.v[x_reg] as usize % DISPLAY_WIDTH;
        let y_pos = self.v[y_reg] as usize % DISPLAY_HEIGHT;

        self.v[0xF] = 0; // Reset collision flag

        for row in 0..height {
            let y = (y_pos + row) % DISPLAY_HEIGHT;
            let sprite_byte = self.memory[self.i as usize + row];

            for col in 0..8 {
                let x = (x_pos + col) % DISPLAY_WIDTH;
                let pixel = (sprite_byte & (0x80 >> col)) != 0;

                if pixel {
                    let index = y * DISPLAY_WIDTH + x;
                    if self.display[index] {
                        self.v[0xF] = 1; // Collision detected
                    }
                    self.display[index] ^= true; // XOR pixel
                }
            }
        }

        self.display_updated = true;
    }

    /// Set key state (0-15)
    pub fn set_key(&mut self, key: u8, pressed: bool) {
        if key < 16 {
            self.keys[key as usize] = pressed;
        }
    }

    /// Set controller state using standard button mapping
    /// Maps keyboard to CHIP-8's 16-key hexadecimal keypad
    /// Standard mapping (QWERTY):
    ///   1 2 3 4  ->  1 2 3 C
    ///   Q W E R  ->  4 5 6 D
    ///   A S D F  ->  7 8 9 E
    ///   Z X C V  ->  A 0 B F
    pub fn set_controller(&mut self, state: u16) {
        // Clear all keys first
        self.keys.fill(false);

        // Map 16-bit state to CHIP-8 keys
        // Bits 0-15 represent keys 0x0-0xF
        for i in 0..16 {
            self.keys[i] = (state & (1 << i)) != 0;
        }
    }

    /// Get debug information
    pub fn debug_info(&self) -> DebugInfo {
        DebugInfo {
            pc: self.pc,
            i: self.i,
            sp: self.sp,
            v0: self.v[0],
            vf: self.v[0xF],
            delay_timer: self.delay_timer,
            sound_timer: self.sound_timer,
        }
    }

    /// Check if sound should be playing
    pub fn is_sound_playing(&self) -> bool {
        self.sound_timer > 0
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub pc: u16,
    pub i: u16,
    pub sp: u8,
    pub v0: u8,
    pub vf: u8,
    pub delay_timer: u8,
    pub sound_timer: u8,
}

impl System for Chip8System {
    type Error = Chip8Error;

    fn reset(&mut self) {
        *self = Self::new();
        self.program_loaded = false;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.program_loaded {
            return Err(Chip8Error::NoProgram);
        }

        // Execute instructions for one frame
        // CHIP-8 runs at ~700 instructions/second, 60 fps = ~11-12 instructions per frame
        // We'll use 10 instructions per frame for simplicity
        const INSTRUCTIONS_PER_FRAME: u32 = 10;

        self.cycles_this_frame = 0;
        self.display_updated = false;

        for _ in 0..INSTRUCTIONS_PER_FRAME {
            self.execute_instruction();
            self.cycles_this_frame += 1;
        }

        // Update timers (they count down at 60Hz)
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }

        // Convert display to frame
        let mut frame = Frame::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32);
        for (i, &pixel) in self.display.iter().enumerate() {
            frame.pixels[i] = if pixel { 0xFFFFFFFF } else { 0xFF000000 };
        }

        Ok(frame)
    }

    fn save_state(&self) -> Value {
        // Manually create JSON for save state due to large array serialization limitations
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        serde_json::json!({
            "v": self.v,
            "i": self.i,
            "pc": self.pc,
            "sp": self.sp,
            "memory": STANDARD.encode(self.memory),
            "stack": self.stack,
            "delay_timer": self.delay_timer,
            "sound_timer": self.sound_timer,
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        // Manually deserialize from JSON
        if let Some(v_array) = v["v"].as_array() {
            for (i, val) in v_array.iter().enumerate().take(16) {
                self.v[i] = val.as_u64().unwrap_or(0) as u8;
            }
        }
        self.i = v["i"].as_u64().unwrap_or(0) as u16;
        self.pc = v["pc"].as_u64().unwrap_or(PROGRAM_START as u64) as u16;
        self.sp = v["sp"].as_u64().unwrap_or(0) as u8;

        // Decode base64 memory
        if let Some(memory_b64) = v["memory"].as_str() {
            if let Ok(memory_bytes) = STANDARD.decode(memory_b64) {
                if memory_bytes.len() == MEMORY_SIZE {
                    self.memory.copy_from_slice(&memory_bytes);
                }
            }
        }

        if let Some(stack_array) = v["stack"].as_array() {
            for (i, val) in stack_array.iter().enumerate().take(STACK_SIZE) {
                self.stack[i] = val.as_u64().unwrap_or(0) as u16;
            }
        }
        self.delay_timer = v["delay_timer"].as_u64().unwrap_or(0) as u8;
        self.sound_timer = v["sound_timer"].as_u64().unwrap_or(0) as u8;

        self.program_loaded = true; // Ensure program is marked as loaded
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Program".to_string(),
            name: "CHIP-8 Program".to_string(),
            extensions: vec!["ch8".to_string(), "c8".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Program" {
            return Err(Chip8Error::InvalidMountPoint(mount_point_id.to_string()));
        }

        let max_size = MEMORY_SIZE - PROGRAM_START;
        if data.len() > max_size {
            return Err(Chip8Error::ProgramTooLarge {
                size: data.len(),
                max: max_size,
            });
        }

        // Clear previous program
        self.memory[PROGRAM_START..].fill(0);

        // Load new program
        self.memory[PROGRAM_START..PROGRAM_START + data.len()].copy_from_slice(data);

        // Reset system state
        self.pc = PROGRAM_START as u16;
        self.sp = 0;
        self.v.fill(0);
        self.i = 0;
        self.stack.fill(0);
        self.display.fill(false);
        self.delay_timer = 0;
        self.sound_timer = 0;
        self.keys.fill(false);
        self.waiting_for_key = None;
        self.program_loaded = true;

        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Program" {
            return Err(Chip8Error::InvalidMountPoint(mount_point_id.to_string()));
        }

        self.memory[PROGRAM_START..].fill(0);
        self.program_loaded = false;
        self.reset();

        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Program" && self.program_loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chip8_initialization() {
        let system = Chip8System::new();
        assert_eq!(system.pc, PROGRAM_START as u16);
        assert_eq!(system.sp, 0);
        assert!(!system.program_loaded);

        // Check font data is loaded
        assert_eq!(system.memory[0], 0xF0); // First byte of '0' sprite
    }

    #[test]
    fn test_load_program() {
        let mut system = Chip8System::new();
        let program = vec![0x00, 0xE0]; // CLS instruction

        system.mount("Program", &program).unwrap();
        assert!(system.program_loaded);
        assert_eq!(system.memory[PROGRAM_START], 0x00);
        assert_eq!(system.memory[PROGRAM_START + 1], 0xE0);
    }

    #[test]
    fn test_program_too_large() {
        let mut system = Chip8System::new();
        let large_program = vec![0; MEMORY_SIZE]; // Too large

        let result = system.mount("Program", &large_program);
        assert!(result.is_err());
    }

    #[test]
    fn test_cls_instruction() {
        let mut system = Chip8System::new();
        system.display.fill(true);
        system.memory[PROGRAM_START] = 0x00;
        system.memory[PROGRAM_START + 1] = 0xE0;
        system.program_loaded = true;

        system.execute_instruction();
        assert!(system.display.iter().all(|&p| !p));
    }

    #[test]
    fn test_set_register() {
        let mut system = Chip8System::new();
        system.memory[PROGRAM_START] = 0x61; // LD V1, 0x42
        system.memory[PROGRAM_START + 1] = 0x42;
        system.program_loaded = true;

        system.execute_instruction();
        assert_eq!(system.v[1], 0x42);
    }

    #[test]
    fn test_add_register() {
        let mut system = Chip8System::new();
        system.v[1] = 0x10;
        system.memory[PROGRAM_START] = 0x71; // ADD V1, 0x05
        system.memory[PROGRAM_START + 1] = 0x05;
        system.program_loaded = true;

        system.execute_instruction();
        assert_eq!(system.v[1], 0x15);
    }

    #[test]
    fn test_save_load_state() {
        let mut system = Chip8System::new();
        system.v[0] = 42;
        system.pc = 0x300;

        let state = system.save_state();

        let mut system2 = Chip8System::new();
        system2.load_state(&state).unwrap();

        assert_eq!(system2.v[0], 42);
        assert_eq!(system2.pc, 0x300);
    }

    #[test]
    fn smoke_test() {
        // Load the test ROM and verify it runs without crashing
        let test_rom_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test_roms/chip8/test.ch8"
        );
        let rom_data = std::fs::read(test_rom_path).expect("Failed to read test ROM");

        let mut system = Chip8System::new();
        system
            .mount("Program", &rom_data)
            .expect("Failed to mount test ROM");

        // Run a few frames to ensure nothing crashes
        for _ in 0..10 {
            let frame = system.step_frame().expect("Frame execution failed");
            assert_eq!(frame.width, DISPLAY_WIDTH as u32);
            assert_eq!(frame.height, DISPLAY_HEIGHT as u32);
            assert_eq!(frame.pixels.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
        }

        // The test ROM should have drawn some pixels
        // Check that at least some pixels are white (0xFFFFFFFF)
        let white_pixels = system.display.iter().filter(|&&p| p).count();
        assert!(white_pixels > 0, "Test ROM should have drawn some pixels");
    }
}
