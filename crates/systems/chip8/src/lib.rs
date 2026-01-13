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

mod debugger;

use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
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
const MEMORY_SIZE_XO: usize = 65536; // XO-CHIP extended memory
const PROGRAM_START: usize = 0x200;
const DISPLAY_WIDTH_LOW: usize = 64;
const DISPLAY_HEIGHT_LOW: usize = 32;
const DISPLAY_WIDTH_HIGH: usize = 128;
const DISPLAY_HEIGHT_HIGH: usize = 64;
const DISPLAY_WIDTH_HIRES: usize = 64;
const DISPLAY_HEIGHT_HIRES: usize = 64;
const DISPLAY_WIDTH_MEGA: usize = 256;
const DISPLAY_HEIGHT_MEGA: usize = 192;
const STACK_SIZE: usize = 16;

/// Emulation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip8Mode {
    /// Original CHIP-8 (64x32, basic opcodes)
    Chip8,
    /// Super-CHIP (64x32 or 128x64, scrolling, extended opcodes)
    SuperChip,
    /// XO-CHIP (128x64, 4 colors, audio, extended memory)
    XoChip,
    /// CHIP-8 Hires (64x64, VIP 2-page mode for Cosmac VIP/Telmac 1800)
    Chip8Hires,
    /// Mega-CHIP (256x192, ultra-high resolution)
    MegaChip,
}

/// Super-CHIP large font data (10 bytes per character, 0-9)
/// Used for Super-CHIP's 16x16 font
const LARGE_FONT_DATA: [u8; 100] = [
    // 0
    0x3C, 0x7E, 0xE7, 0xC3, 0xC3, 0xC3, 0xC3, 0xE7, 0x7E, 0x3C, // 1
    0x18, 0x38, 0x58, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, // 2
    0x3E, 0x7F, 0xC3, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFF, 0xFF, // 3
    0x3C, 0x7E, 0xC3, 0x03, 0x0E, 0x0E, 0x03, 0xC3, 0x7E, 0x3C, // 4
    0x06, 0x0E, 0x1E, 0x36, 0x66, 0xC6, 0xFF, 0xFF, 0x06, 0x06, // 5
    0xFF, 0xFF, 0xC0, 0xC0, 0xFC, 0xFE, 0x03, 0xC3, 0x7E, 0x3C, // 6
    0x3E, 0x7C, 0xC0, 0xC0, 0xFC, 0xFE, 0xC3, 0xC3, 0x7E, 0x3C, // 7
    0xFF, 0xFF, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x60, 0x60, // 8
    0x3C, 0x7E, 0xC3, 0xC3, 0x7E, 0x7E, 0xC3, 0xC3, 0x7E, 0x3C, // 9
    0x3C, 0x7E, 0xC3, 0xC3, 0x7F, 0x3F, 0x03, 0x03, 0x3E, 0x7C,
];

/// CHIP-8 system state
pub struct Chip8System {
    // Registers
    v: [u8; 16], // V0-VF general purpose registers
    i: u16,      // Index register
    pc: u16,     // Program counter
    sp: u8,      // Stack pointer

    // Memory (extended for XO-CHIP)
    memory: Vec<u8>, // 4KB for CHIP-8/Super-CHIP, 64KB for XO-CHIP
    stack: [u16; STACK_SIZE],

    // Display (supports both 64x32 and 128x64)
    display_planes: [Vec<bool>; 2], // Two bit planes for XO-CHIP (4 colors)
    display_width: usize,
    display_height: usize,
    display_updated: bool,
    high_res: bool, // Super-CHIP/XO-CHIP high resolution mode

    // Timers
    delay_timer: u8,
    sound_timer: u8,

    // Input (16 keys)
    keys: [bool; 16],

    // Execution
    cycles_this_frame: u32,
    program_loaded: bool,
    mode: Chip8Mode,

    // For waiting for keypress (FX0A instruction)
    waiting_for_key: Option<usize>, // Some(register_index) when waiting for key press
    key_pressed_while_waiting: Option<u8>, // Stores which key was pressed, waiting for release

    // Super-CHIP flag registers (RPL user flags)
    flag_registers: [u8; 16],

    // XO-CHIP extensions
    selected_plane: u8,      // Which plane(s) to draw to (bitmask: 0-3)
    audio_pattern: [u8; 16], // XO-CHIP audio pattern buffer
    audio_pitch: u8,         // XO-CHIP audio playback rate

    // Debugging
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl Default for Chip8System {
    fn default() -> Self {
        Self::new()
    }
}

impl Chip8System {
    /// Create a new CHIP-8 system with default mode (Chip8)
    pub fn new() -> Self {
        Self::new_with_mode(Chip8Mode::Chip8)
    }

    /// Create a new CHIP-8 system with specified mode
    pub fn new_with_mode(mode: Chip8Mode) -> Self {
        let memory_size = match mode {
            Chip8Mode::XoChip => MEMORY_SIZE_XO,
            _ => MEMORY_SIZE,
        };

        let (display_width, display_height) = match mode {
            Chip8Mode::SuperChip | Chip8Mode::XoChip => (DISPLAY_WIDTH_HIGH, DISPLAY_HEIGHT_HIGH),
            Chip8Mode::Chip8Hires => (DISPLAY_WIDTH_HIRES, DISPLAY_HEIGHT_HIRES),
            Chip8Mode::MegaChip => (DISPLAY_WIDTH_MEGA, DISPLAY_HEIGHT_MEGA),
            Chip8Mode::Chip8 => (DISPLAY_WIDTH_LOW, DISPLAY_HEIGHT_LOW),
        };

        let display_size = display_width * display_height;

        let mut system = Self {
            v: [0; 16],
            i: 0,
            pc: PROGRAM_START as u16,
            sp: 0,
            memory: vec![0; memory_size],
            stack: [0; STACK_SIZE],
            display_planes: [vec![false; display_size], vec![false; display_size]],
            display_width,
            display_height,
            display_updated: false,
            high_res: matches!(
                mode,
                Chip8Mode::SuperChip | Chip8Mode::XoChip | Chip8Mode::MegaChip
            ),
            delay_timer: 0,
            sound_timer: 0,
            keys: [false; 16],
            cycles_this_frame: 0,
            program_loaded: false,
            mode,
            waiting_for_key: None,
            key_pressed_while_waiting: None,
            flag_registers: [0; 16],
            selected_plane: 1, // Default to plane 1 (first plane)
            audio_pattern: [0; 16],
            audio_pitch: 64, // Default pitch
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        };

        // Load font data into memory (at 0x000-0x04F)
        system.memory[0..FONT_DATA.len()].copy_from_slice(&FONT_DATA);

        // Load large font for Super-CHIP (at 0x050-0x0A0)
        if matches!(mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
            system.memory[0x50..0x50 + LARGE_FONT_DATA.len()].copy_from_slice(&LARGE_FONT_DATA);
        }

        log(LogCategory::CPU, LogLevel::Info, || {
            format!(
                "CHIP-8: Initialized - Mode: {:?}, Memory: {}KB, Display: {}x{}",
                mode,
                memory_size / 1024,
                display_width,
                display_height
            )
        });

        system
    }

    /// Set the emulation mode
    pub fn set_mode(&mut self, mode: Chip8Mode) {
        log(LogCategory::CPU, LogLevel::Info, || {
            format!("CHIP-8: Switching mode from {:?} to {:?}", self.mode, mode)
        });
        // Recreate system with new mode
        *self = Self::new_with_mode(mode);
    }

    /// Get current emulation mode
    pub fn mode(&self) -> Chip8Mode {
        self.mode
    }

    /// Execute one instruction
    fn execute_instruction(&mut self) {
        // Check if waiting for key press/release
        if let Some(register) = self.waiting_for_key {
            if let Some(pressed_key) = self.key_pressed_while_waiting {
                // We detected a key press, now wait for it to be released
                if !self.keys[pressed_key as usize] {
                    // Key has been released - store it and continue
                    log(LogCategory::CPU, LogLevel::Debug, || {
                        format!(
                            "CHIP-8: Key 0x{:X} released, stored in V{:X}, continuing execution",
                            pressed_key, register
                        )
                    });
                    self.v[register] = pressed_key;
                    self.waiting_for_key = None;
                    self.key_pressed_while_waiting = None;
                    // Advance PC past the FX0A instruction (2 bytes) and continue
                    self.pc += 2;
                } else {
                    // Still pressed, keep waiting
                    return;
                }
            } else {
                // Waiting for initial key press
                for (i, &pressed) in self.keys.iter().enumerate() {
                    if pressed {
                        log(LogCategory::CPU, LogLevel::Debug, || {
                            format!(
                                "CHIP-8: Key press detected - Key: 0x{:X}, waiting for release",
                                i
                            )
                        });
                        self.key_pressed_while_waiting = Some(i as u8);
                        break;
                    }
                }
                return; // Don't execute instructions while waiting
            }
        }

        // Fetch opcode (2 bytes, big-endian)
        let opcode = u16::from_be_bytes([
            self.memory[self.pc as usize],
            self.memory[self.pc as usize + 1],
        ]);

        log(LogCategory::CPU, LogLevel::Trace, || {
            format!(
                "CHIP-8: PC={:04X} Opcode={:04X} I={:04X} SP={:02X}",
                self.pc, opcode, self.i, self.sp
            )
        });

        // Special handling for VIP 2-page hires mode (64x64)
        // If we're at the start of the program (0x200) and opcode is 0x1260,
        // this signals a hires mode ROM that needs 64x64 resolution
        let mut modified_opcode = opcode;
        if self.pc == PROGRAM_START as u16 && opcode == 0x1260 {
            log(LogCategory::PPU, LogLevel::Info, || {
                "CHIP-8: VIP 2-page hires mode detected (64x64) - Switching to hires mode"
                    .to_string()
            });
            // Switch to hires mode
            if self.mode != Chip8Mode::Chip8Hires {
                self.mode = Chip8Mode::Chip8Hires;
                self.display_width = DISPLAY_WIDTH_HIRES;
                self.display_height = DISPLAY_HEIGHT_HIRES;
                let new_size = DISPLAY_WIDTH_HIRES * DISPLAY_HEIGHT_HIRES;
                self.display_planes[0].resize(new_size, false);
                self.display_planes[1].resize(new_size, false);
                self.display_updated = true;
            }
            // Redirect to 0x2C0 instead of 0x260
            modified_opcode = 0x12C0;
        }

        // Decode and execute
        self.pc += 2; // Increment PC before execution (some instructions modify PC)

        let nnn = modified_opcode & 0x0FFF; // 12-bit address
        let nn = (modified_opcode & 0x00FF) as u8; // 8-bit constant
        let n = (modified_opcode & 0x000F) as u8; // 4-bit constant
        let x = ((modified_opcode & 0x0F00) >> 8) as usize; // 4-bit register index
        let y = ((modified_opcode & 0x00F0) >> 4) as usize; // 4-bit register index

        match modified_opcode & 0xF000 {
            0x0000 => match modified_opcode {
                0x00E0 => {
                    // 00E0 - CLS: Clear display
                    log(LogCategory::PPU, LogLevel::Debug, || {
                        "CHIP-8: CLS - Clearing display".to_string()
                    });
                    for plane in &mut self.display_planes {
                        plane.fill(false);
                    }
                    self.display_updated = true;
                }
                0x0230 => {
                    // 0230 - CLS (Hires): Clear display for VIP 2-page hires mode (64x64)
                    if self.mode == Chip8Mode::Chip8Hires {
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            "CHIP-8: CLS (Hires 0x0230) - Clearing 64x64 display".to_string()
                        });
                        for plane in &mut self.display_planes {
                            plane.fill(false);
                        }
                        self.display_updated = true;
                    }
                }
                0x00EE => {
                    // 00EE - RET: Return from subroutine
                    self.sp -= 1;
                    self.pc = self.stack[self.sp as usize];
                    log(LogCategory::CPU, LogLevel::Debug, || {
                        format!(
                            "CHIP-8: RET - Return to PC={:04X}, SP={:02X}",
                            self.pc, self.sp
                        )
                    });
                }
                0x00FB => {
                    // 00FB - SCR (Super-CHIP): Scroll display 4 pixels right
                    // Auto-upgrade to Super-CHIP mode if in basic CHIP-8 mode
                    if self.mode == Chip8Mode::Chip8 || self.mode == Chip8Mode::Chip8Hires {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: SCR - Auto-upgrading to Super-CHIP mode for scroll support"
                                .to_string()
                        });
                        self.mode = Chip8Mode::SuperChip;
                    }
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            "CHIP-8: SCR - Scroll right 4 pixels (Super-CHIP)".to_string()
                        });
                        self.scroll_right(4);
                    }
                }
                0x00FC => {
                    // 00FC - SCL (Super-CHIP): Scroll display 4 pixels left
                    // Auto-upgrade to Super-CHIP mode if in basic CHIP-8 mode
                    if self.mode == Chip8Mode::Chip8 || self.mode == Chip8Mode::Chip8Hires {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: SCL - Auto-upgrading to Super-CHIP mode for scroll support"
                                .to_string()
                        });
                        self.mode = Chip8Mode::SuperChip;
                    }
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            "CHIP-8: SCL - Scroll left 4 pixels (Super-CHIP)".to_string()
                        });
                        self.scroll_left(4);
                    }
                }
                0x00FD => {
                    // 00FD - EXIT (Super-CHIP): Exit interpreter
                    log(LogCategory::CPU, LogLevel::Info, || {
                        "CHIP-8: EXIT - Exit interpreter opcode (treated as no-op)".to_string()
                    });
                    // We'll treat this as a no-op for now
                }
                0x00FE => {
                    // 00FE - LOW (Super-CHIP): Disable high resolution mode
                    // Auto-upgrade to Super-CHIP mode if in basic CHIP-8 mode
                    if self.mode == Chip8Mode::Chip8 || self.mode == Chip8Mode::Chip8Hires {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: LOW - Auto-upgrading to Super-CHIP mode".to_string()
                        });
                        self.mode = Chip8Mode::SuperChip;
                    }
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: LOW - Switching to low-res mode (64x32)".to_string()
                        });
                        self.set_low_res();
                    }
                }
                0x00FF => {
                    // 00FF - HIGH: Enable high resolution mode
                    // Super-CHIP/XO-CHIP: 128x64
                    // Mega-CHIP: 256x192
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: HIGH - Switching to high-res mode (128x64)".to_string()
                        });
                        self.set_high_res();
                    } else if self.mode == Chip8Mode::Chip8 || self.mode == Chip8Mode::Chip8Hires {
                        // Auto-upgrade to Super-CHIP when high-res is requested from basic CHIP-8
                        // Most games using 00FF expect Super-CHIP (128x64), not Mega-CHIP (256x192)
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: HIGH - Auto-upgrading to Super-CHIP mode (128x64)".to_string()
                        });
                        self.mode = Chip8Mode::SuperChip;
                        self.set_high_res();
                    } else if self.mode == Chip8Mode::MegaChip {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            "CHIP-8: HIGH - Already in Mega-CHIP mode (256x192)".to_string()
                        });
                    }
                }
                _ => {
                    // 00CN - SCD N (Super-CHIP): Scroll display N lines down
                    // 00DN - SCU N (XO-CHIP): Scroll display N lines up
                    let n = (opcode & 0x000F) as usize;
                    if opcode & 0x00F0 == 0x00C0 {
                        // 00CN - Scroll down
                        // Auto-upgrade to Super-CHIP mode if in basic CHIP-8 mode
                        if self.mode == Chip8Mode::Chip8 || self.mode == Chip8Mode::Chip8Hires {
                            log(LogCategory::PPU, LogLevel::Info, || {
                                "CHIP-8: SCD - Auto-upgrading to Super-CHIP mode for scroll support"
                                    .to_string()
                            });
                            self.mode = Chip8Mode::SuperChip;
                        }
                        if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                            log(LogCategory::PPU, LogLevel::Debug, || {
                                format!("CHIP-8: SCD - Scroll down {} lines (Super-CHIP)", n)
                            });
                            self.scroll_down(n);
                        }
                    } else if opcode & 0x00F0 == 0x00D0 && self.mode == Chip8Mode::XoChip {
                        // 00DN - Scroll up (XO-CHIP only)
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            format!("CHIP-8: SCU - Scroll up {} lines (XO-CHIP)", n)
                        });
                        self.scroll_up(n);
                    } else {
                        // 0NNN - SYS addr: Call machine code routine (typically ignored by modern interpreters)
                        log(LogCategory::Stubs, LogLevel::Trace, || {
                            format!(
                                "CHIP-8: SYS {:03X} - Machine code routine (typically ignored by modern interpreters)",
                                opcode & 0x0FFF
                            )
                        });
                    }
                }
            },
            0x1000 => {
                // 1NNN - JP addr: Jump to address
                log(LogCategory::CPU, LogLevel::Trace, || {
                    format!("CHIP-8: JP {:03X}", nnn)
                });
                self.pc = nnn;
            }
            0x2000 => {
                // 2NNN - CALL addr: Call subroutine
                log(LogCategory::CPU, LogLevel::Debug, || {
                    format!("CHIP-8: CALL {:03X} - SP={:02X}", nnn, self.sp)
                });
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
            0x5000 => match n {
                0x0 => {
                    // 5XY0 - SE Vx, Vy: Skip next instruction if Vx == Vy
                    if self.v[x] == self.v[y] {
                        self.pc += 2;
                    }
                }
                0x2 => {
                    // 5XY2 - SAVE Vx - Vy (XO-CHIP): Save Vx through Vy to memory at I
                    if self.mode == Chip8Mode::XoChip {
                        let start = x.min(y);
                        let end = x.max(y);
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!(
                                "CHIP-8: SAVE V{:X}-V{:X} to [{:04X}] (XO-CHIP)",
                                start, end, self.i
                            )
                        });
                        for reg in start..=end {
                            self.memory[self.i as usize + (reg - start)] = self.v[reg];
                        }
                    }
                }
                0x3 => {
                    // 5XY3 - LOAD Vx - Vy (XO-CHIP): Load Vx through Vy from memory at I
                    if self.mode == Chip8Mode::XoChip {
                        let start = x.min(y);
                        let end = x.max(y);
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!(
                                "CHIP-8: LOAD V{:X}-V{:X} from [{:04X}] (XO-CHIP)",
                                start, end, self.i
                            )
                        });
                        for reg in start..=end {
                            self.v[reg] = self.memory[self.i as usize + (reg - start)];
                        }
                    }
                }
                _ => {}
            },
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
                    log(LogCategory::CPU, LogLevel::Debug, || {
                        format!("CHIP-8: LD V{:X}, K - Waiting for key press", x)
                    });
                    self.waiting_for_key = Some(x);
                    self.pc -= 2; // Stay on this instruction until key is pressed
                }
                0x15 => {
                    // FX15 - LD DT, Vx: Set delay timer = Vx
                    log(LogCategory::APU, LogLevel::Trace, || {
                        format!("CHIP-8: LD DT, V{:X} - Set delay timer to {}", x, self.v[x])
                    });
                    self.delay_timer = self.v[x];
                }
                0x18 => {
                    // FX18 - LD ST, Vx: Set sound timer = Vx
                    log(LogCategory::APU, LogLevel::Debug, || {
                        format!("CHIP-8: LD ST, V{:X} - Set sound timer to {}", x, self.v[x])
                    });
                    self.sound_timer = self.v[x];
                }
                0x1E => {
                    // FX1E - ADD I, Vx: Set I = I + Vx
                    self.i = self.i.wrapping_add(self.v[x] as u16);
                }
                0x29 => {
                    // FX29 - LD F, Vx: Set I = location of sprite for digit Vx (small font)
                    self.i = (self.v[x] as u16 & 0x0F) * 5;
                }
                0x30 => {
                    // FX30 - LD HF, Vx (Super-CHIP): Set I = location of 16x16 sprite for digit Vx
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        self.i = 0x50 + (self.v[x] as u16 & 0x0F) * 10;
                    }
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
                0x75 => {
                    // FX75 - SAVE Vx (Super-CHIP): Save V0-Vx to flag registers
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!("CHIP-8: SAVE V0-V{:X} to flag registers (Super-CHIP)", x)
                        });
                        for reg in 0..=x.min(15) {
                            self.flag_registers[reg] = self.v[reg];
                        }
                    }
                }
                0x85 => {
                    // FX85 - LOAD Vx (Super-CHIP): Load V0-Vx from flag registers
                    if matches!(self.mode, Chip8Mode::SuperChip | Chip8Mode::XoChip) {
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!("CHIP-8: LOAD V0-V{:X} from flag registers (Super-CHIP)", x)
                        });
                        for reg in 0..=x.min(15) {
                            self.v[reg] = self.flag_registers[reg];
                        }
                    }
                }
                _ => {
                    // Check for XO-CHIP specific opcodes
                    if self.mode == Chip8Mode::XoChip {
                        match nn {
                            0x00 => {
                                // F000 NNNN - I := NNNN (XO-CHIP): Load I with 16-bit address
                                // This is a special 4-byte instruction
                                if self.pc as usize + 1 < self.memory.len() {
                                    let high_byte = self.memory[self.pc as usize] as u16;
                                    let low_byte = self.memory[self.pc as usize + 1] as u16;
                                    self.i = (high_byte << 8) | low_byte;
                                    log(LogCategory::CPU, LogLevel::Debug, || {
                                        format!(
                                            "CHIP-8: I := {:04X} (XO-CHIP extended addressing)",
                                            self.i
                                        )
                                    });
                                    self.pc += 2; // Skip the next instruction bytes
                                }
                            }
                            0x01 => {
                                // FN01 - PLANE N (XO-CHIP): Select drawing plane(s)
                                // N is in the low nibble of x (the first hex digit after F)
                                self.selected_plane = x as u8 & 0x03; // Mask to 0-3
                                log(LogCategory::PPU, LogLevel::Debug, || {
                                    format!(
                                        "CHIP-8: PLANE {} - Select drawing plane(s) (XO-CHIP)",
                                        self.selected_plane
                                    )
                                });
                            }
                            0x02 => {
                                // F002 - AUDIO (XO-CHIP): Store 16 bytes at I in audio buffer
                                log(LogCategory::APU, LogLevel::Info, || {
                                    format!("CHIP-8: AUDIO - Load 16 bytes from [{:04X}] to audio buffer (XO-CHIP)", self.i)
                                });
                                for i in 0..16 {
                                    self.audio_pattern[i] = self.memory[self.i as usize + i];
                                }
                            }
                            0x3A => {
                                // FX3A - PITCH Vx (XO-CHIP): Set audio pitch to Vx
                                self.audio_pitch = self.v[x];
                                log(LogCategory::APU, LogLevel::Info, || {
                                    format!(
                                        "CHIP-8: PITCH V{:X} - Set audio pitch to {} (XO-CHIP)",
                                        x, self.audio_pitch
                                    )
                                });
                            }
                            _ => {}
                        }
                    }
                }
            },
            _ => {}
        }
    }

    /// Draw sprite at (Vx, Vy) with height n
    fn draw_sprite(&mut self, x_reg: usize, y_reg: usize, height: usize) {
        let x_pos = self.v[x_reg] as usize;
        let y_pos = self.v[y_reg] as usize;

        self.v[0xF] = 0; // Reset collision flag

        // Determine sprite width (8 pixels for normal, 16 for Super-CHIP high-res 16x16 sprites)
        let sprite_width = if height == 0 && self.high_res {
            // DXY0 in high-res mode draws 16x16 sprite
            16
        } else {
            8
        };

        let sprite_height = if height == 0 && self.high_res {
            16
        } else {
            height
        };

        for row in 0..sprite_height {
            let y = (y_pos + row) % self.display_height;

            for col in 0..sprite_width {
                let x = (x_pos + col) % self.display_width;

                // Get sprite pixel
                let byte_offset = if sprite_width == 16 {
                    row * 2 + col / 8
                } else {
                    row
                };
                let sprite_byte = self.memory[self.i as usize + byte_offset];
                let bit_offset = 7 - (col % 8);
                let pixel = (sprite_byte & (1 << bit_offset)) != 0;

                if pixel {
                    let index = y * self.display_width + x;

                    // Draw to selected plane(s) for XO-CHIP, or plane 0 for CHIP-8/Super-CHIP
                    let planes_to_draw = if self.mode == Chip8Mode::XoChip {
                        self.selected_plane
                    } else {
                        1 // Plane 0 only
                    };

                    for plane_idx in 0..2 {
                        if (planes_to_draw & (1 << plane_idx)) != 0 {
                            if self.display_planes[plane_idx][index] {
                                self.v[0xF] = 1; // Collision detected
                            }
                            self.display_planes[plane_idx][index] ^= true; // XOR pixel
                        }
                    }
                }
            }
        }

        self.display_updated = true;
    }

    /// Scroll display down by n lines (Super-CHIP)
    fn scroll_down(&mut self, n: usize) {
        for plane in &mut self.display_planes {
            let mut new_display = vec![false; self.display_width * self.display_height];
            // Move pixels down: copy from row y to row y+n
            for y in 0..(self.display_height - n) {
                for x in 0..self.display_width {
                    let old_idx = y * self.display_width + x;
                    let new_idx = (y + n) * self.display_width + x;
                    new_display[new_idx] = plane[old_idx];
                }
            }
            // Top n rows remain blank (already initialized to false)
            *plane = new_display;
        }
        self.display_updated = true;
    }

    /// Scroll display up by n lines (XO-CHIP)
    fn scroll_up(&mut self, n: usize) {
        for plane in &mut self.display_planes {
            let mut new_display = vec![false; self.display_width * self.display_height];
            // Move pixels up: copy from row y to row y-n
            for y in n..self.display_height {
                for x in 0..self.display_width {
                    let old_idx = y * self.display_width + x;
                    let new_idx = (y - n) * self.display_width + x;
                    new_display[new_idx] = plane[old_idx];
                }
            }
            // Bottom n rows remain blank (already initialized to false)
            *plane = new_display;
        }
        self.display_updated = true;
    }

    /// Scroll display right by n pixels (Super-CHIP)
    fn scroll_right(&mut self, n: usize) {
        for plane in &mut self.display_planes {
            let mut new_display = vec![false; self.display_width * self.display_height];
            // Move pixels right: copy from column x to column x+n
            for y in 0..self.display_height {
                for x in 0..(self.display_width - n) {
                    let old_idx = y * self.display_width + x;
                    let new_idx = y * self.display_width + (x + n);
                    new_display[new_idx] = plane[old_idx];
                }
            }
            // Left n columns remain blank (already initialized to false)
            *plane = new_display;
        }
        self.display_updated = true;
    }

    /// Scroll display left by n pixels (Super-CHIP)
    fn scroll_left(&mut self, n: usize) {
        for plane in &mut self.display_planes {
            let mut new_display = vec![false; self.display_width * self.display_height];
            // Move pixels left: copy from column x to column x-n
            for y in 0..self.display_height {
                for x in n..self.display_width {
                    let old_idx = y * self.display_width + x;
                    let new_idx = y * self.display_width + (x - n);
                    new_display[new_idx] = plane[old_idx];
                }
            }
            // Right n columns remain blank (already initialized to false)
            *plane = new_display;
        }
        self.display_updated = true;
    }

    /// Enable low resolution mode (64x32) - Super-CHIP
    fn set_low_res(&mut self) {
        if self.high_res {
            self.display_width = DISPLAY_WIDTH_LOW;
            self.display_height = DISPLAY_HEIGHT_LOW;
            self.high_res = false;
            let display_size = self.display_width * self.display_height;
            self.display_planes = [vec![false; display_size], vec![false; display_size]];
            self.display_updated = true;
        }
    }

    /// Enable high resolution mode (128x64) - Super-CHIP
    fn set_high_res(&mut self) {
        if !self.high_res {
            self.display_width = DISPLAY_WIDTH_HIGH;
            self.display_height = DISPLAY_HEIGHT_HIGH;
            self.high_res = true;
            let display_size = self.display_width * self.display_height;
            self.display_planes = [vec![false; display_size], vec![false; display_size]];
            self.display_updated = true;
        }
    }

    /// Enable mega resolution mode (256x192) - Mega-CHIP
    #[allow(dead_code)] // May be used for Mega-CHIP support in the future
    fn set_mega_res(&mut self) {
        if self.mode != Chip8Mode::MegaChip {
            self.mode = Chip8Mode::MegaChip;
        }
        self.display_width = DISPLAY_WIDTH_MEGA;
        self.display_height = DISPLAY_HEIGHT_MEGA;
        self.high_res = true;
        let display_size = self.display_width * self.display_height;
        self.display_planes = [vec![false; display_size], vec![false; display_size]];
        self.display_updated = true;
    }

    /// Set key state (0-15)
    pub fn set_key(&mut self, key: u8, pressed: bool) {
        if key < 16 {
            log(LogCategory::CPU, LogLevel::Trace, || {
                format!(
                    "CHIP-8: Key 0x{:X} {}",
                    key,
                    if pressed { "pressed" } else { "released" }
                )
            });
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
        let mode_str = match self.mode {
            Chip8Mode::Chip8 => "CHIP-8",
            Chip8Mode::SuperChip => "Super-CHIP",
            Chip8Mode::XoChip => "XO-CHIP",
            Chip8Mode::Chip8Hires => "CHIP-8 Hires",
            Chip8Mode::MegaChip => "Mega-CHIP",
        };
        let resolution_str = format!("{}x{}", self.display_width, self.display_height);

        DebugInfo {
            pc: self.pc,
            i: self.i,
            sp: self.sp,
            v0: self.v[0],
            vf: self.v[0xF],
            delay_timer: self.delay_timer,
            sound_timer: self.sound_timer,
            mode: mode_str.to_string(),
            resolution: resolution_str,
        }
    }

    /// Get complete inspector data for GUI debugging tools
    pub fn get_inspector_data(&self) -> InspectorData {
        let mode_str = match self.mode {
            Chip8Mode::Chip8 => "CHIP-8",
            Chip8Mode::SuperChip => "Super-CHIP",
            Chip8Mode::XoChip => "XO-CHIP",
            Chip8Mode::Chip8Hires => "CHIP-8 Hires",
            Chip8Mode::MegaChip => "Mega-CHIP",
        };

        InspectorData {
            v_registers: self.v,
            i: self.i,
            pc: self.pc,
            sp: self.sp,
            stack: self.stack,
            delay_timer: self.delay_timer,
            sound_timer: self.sound_timer,
            display_plane0: self.display_planes[0].clone(),
            display_plane1: self.display_planes[1].clone(),
            display_width: self.display_width,
            display_height: self.display_height,
            mode: mode_str.to_string(),
            selected_plane: self.selected_plane,
            high_res: self.high_res,
            waiting_for_key: self.waiting_for_key.is_some(),
            keys: self.keys,
        }
    }

    /// Check if sound should be playing
    pub fn is_sound_playing(&self) -> bool {
        self.sound_timer > 0
    }

    emu_core::impl_instruction_tracer_methods!();

    /// Check if instruction tracing is enabled
    pub fn is_instruction_tracing_enabled(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }

    emu_core::impl_breakpoint_methods!();

    /// Enable or disable breakpoints
    pub fn set_breakpoints_enabled(&mut self, enabled: bool) {
        self.breakpoint_manager.set_enabled(enabled);
    }

    /// Get the breakpoint manager
    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
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
    pub mode: String,
    pub resolution: String,
}

#[derive(Debug, Clone)]
pub struct InspectorData {
    pub v_registers: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub display_plane0: Vec<bool>,
    pub display_plane1: Vec<bool>,
    pub display_width: usize,
    pub display_height: usize,
    pub mode: String,
    pub selected_plane: u8,
    pub high_res: bool,
    pub waiting_for_key: bool,
    pub keys: [bool; 16],
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
            let pc_before = self.pc as u32;
            self.execute_instruction();
            self.cycles_this_frame += 1;

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }
            }
        }

        // Update timers (they count down at 60Hz)
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }

        // Convert display to frame
        let mut frame = Frame::new(self.display_width as u32, self.display_height as u32);

        // Combine planes into color output for XO-CHIP, or use monochrome for CHIP-8/Super-CHIP
        if self.mode == Chip8Mode::XoChip {
            // XO-CHIP: 4 colors using 2 bit planes
            // 00 = background (black), 01 = color 1, 10 = color 2, 11 = color 3
            for (i, pixel) in frame.pixels.iter_mut().enumerate() {
                let plane0 = self.display_planes[0][i];
                let plane1 = self.display_planes[1][i];
                *pixel = match (plane1, plane0) {
                    (false, false) => 0xFF000000, // Black (background)
                    (false, true) => 0xFF00FF00,  // Green (plane 0)
                    (true, false) => 0xFFFF0000,  // Red (plane 1)
                    (true, true) => 0xFFFFFF00,   // Yellow (both planes)
                };
            }
        } else {
            // CHIP-8/Super-CHIP: Monochrome (use plane 0 only)
            for (i, pixel) in frame.pixels.iter_mut().enumerate() {
                *pixel = if self.display_planes[0][i] {
                    0xFFFFFFFF
                } else {
                    0xFF000000
                };
            }
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
            "memory": STANDARD.encode(&self.memory),
            "stack": self.stack,
            "delay_timer": self.delay_timer,
            "sound_timer": self.sound_timer,
            "mode": match self.mode {
                Chip8Mode::Chip8 => 0,
                Chip8Mode::SuperChip => 1,
                Chip8Mode::XoChip => 2,
                Chip8Mode::Chip8Hires => 3,
                Chip8Mode::MegaChip => 4,
            },
            "high_res": self.high_res,
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
                if memory_bytes.len() <= self.memory.len() {
                    self.memory[..memory_bytes.len()].copy_from_slice(&memory_bytes);
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

        // Restore mode and resolution if saved
        if let Some(mode_val) = v["mode"].as_u64() {
            let mode = match mode_val {
                1 => Chip8Mode::SuperChip,
                2 => Chip8Mode::XoChip,
                3 => Chip8Mode::Chip8Hires,
                4 => Chip8Mode::MegaChip,
                _ => Chip8Mode::Chip8,
            };
            if mode != self.mode {
                self.set_mode(mode);
            }
        }

        if let Some(high_res) = v["high_res"].as_bool() {
            if high_res && !self.high_res {
                self.set_high_res();
            } else if !high_res && self.high_res {
                self.set_low_res();
            }
        }

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

        let max_size = self.memory.len() - PROGRAM_START;
        if data.len() > max_size {
            return Err(Chip8Error::ProgramTooLarge {
                size: data.len(),
                max: max_size,
            });
        }

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "CHIP-8: Mounting program - Size: {} bytes, Max: {} bytes",
                data.len(),
                max_size
            )
        });

        // Clear previous program
        for byte in &mut self.memory[PROGRAM_START..] {
            *byte = 0;
        }

        // Load new program
        self.memory[PROGRAM_START..PROGRAM_START + data.len()].copy_from_slice(data);

        // Reset system state
        self.pc = PROGRAM_START as u16;
        self.sp = 0;
        self.v.fill(0);
        self.i = 0;
        self.stack.fill(0);
        for plane in &mut self.display_planes {
            plane.fill(false);
        }
        self.delay_timer = 0;
        self.sound_timer = 0;
        self.keys.fill(false);
        self.waiting_for_key = None;
        self.key_pressed_while_waiting = None;

        self.program_loaded = true;

        log(LogCategory::CPU, LogLevel::Info, || {
            "CHIP-8: Program loaded successfully, system reset to PROGRAM_START".to_string()
        });

        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Program" {
            return Err(Chip8Error::InvalidMountPoint(mount_point_id.to_string()));
        }

        log(LogCategory::Bus, LogLevel::Info, || {
            "CHIP-8: Unmounting program".to_string()
        });

        for byte in &mut self.memory[PROGRAM_START..] {
            *byte = 0;
        }
        self.program_loaded = false;
        self.reset();

        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Program" && self.program_loaded
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
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
        system.display_planes[0].fill(true);
        system.memory[PROGRAM_START] = 0x00;
        system.memory[PROGRAM_START + 1] = 0xE0;
        system.program_loaded = true;

        system.execute_instruction();
        assert!(system.display_planes[0].iter().all(|&p| !p));
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
            assert_eq!(frame.width, DISPLAY_WIDTH_LOW as u32);
            assert_eq!(frame.height, DISPLAY_HEIGHT_LOW as u32);
            assert_eq!(frame.pixels.len(), DISPLAY_WIDTH_LOW * DISPLAY_HEIGHT_LOW);
        }

        // The test ROM should have drawn some pixels
        // Check that at least some pixels are white (0xFFFFFFFF)
        let white_pixels = system.display_planes[0].iter().filter(|&&p| p).count();
        assert!(white_pixels > 0, "Test ROM should have drawn some pixels");
    }

    #[test]
    fn test_scroll_down() {
        let mut system = Chip8System::new_with_mode(Chip8Mode::SuperChip);
        system.program_loaded = true;

        // Set up a simple pattern: first row has pixels
        for x in 0..system.display_width {
            system.display_planes[0][x] = true;
        }

        // Scroll down by 2 rows
        system.scroll_down(2);

        // First two rows should be blank
        for x in 0..system.display_width {
            assert!(!system.display_planes[0][x], "Row 0 should be blank");
            assert!(
                !system.display_planes[0][system.display_width + x],
                "Row 1 should be blank"
            );
        }

        // Third row (index 2) should have the pixels that were in row 0
        for x in 0..system.display_width {
            assert!(
                system.display_planes[0][2 * system.display_width + x],
                "Row 2 should have pixels"
            );
        }
    }

    #[test]
    fn test_scroll_up() {
        let mut system = Chip8System::new_with_mode(Chip8Mode::SuperChip);
        system.program_loaded = true;

        // Set up a simple pattern: last row has pixels
        let last_row_start = (system.display_height - 1) * system.display_width;
        for x in 0..system.display_width {
            system.display_planes[0][last_row_start + x] = true;
        }

        // Scroll up by 2 rows
        system.scroll_up(2);

        // Last two rows should be blank
        let second_last_row = (system.display_height - 2) * system.display_width;
        let last_row = (system.display_height - 1) * system.display_width;
        for x in 0..system.display_width {
            assert!(
                !system.display_planes[0][second_last_row + x],
                "Second-last row should be blank"
            );
            assert!(
                !system.display_planes[0][last_row + x],
                "Last row should be blank"
            );
        }

        // Third-to-last row should have the pixels that were in the last row
        let third_last_row = (system.display_height - 3) * system.display_width;
        for x in 0..system.display_width {
            assert!(
                system.display_planes[0][third_last_row + x],
                "Third-to-last row should have pixels"
            );
        }
    }

    #[test]
    fn test_scroll_right() {
        let mut system = Chip8System::new_with_mode(Chip8Mode::SuperChip);
        system.program_loaded = true;

        // Set up a simple pattern: first column has pixels
        for y in 0..system.display_height {
            system.display_planes[0][y * system.display_width] = true;
        }

        // Scroll right by 4 pixels
        system.scroll_right(4);

        // First four columns should be blank
        for y in 0..system.display_height {
            for x in 0..4 {
                assert!(
                    !system.display_planes[0][y * system.display_width + x],
                    "Column {} should be blank",
                    x
                );
            }
        }

        // Fifth column (index 4) should have the pixels that were in column 0
        for y in 0..system.display_height {
            assert!(
                system.display_planes[0][y * system.display_width + 4],
                "Column 4 should have pixels"
            );
        }
    }

    #[test]
    fn test_scroll_left() {
        let mut system = Chip8System::new_with_mode(Chip8Mode::SuperChip);
        system.program_loaded = true;

        // Set up a simple pattern: last column has pixels
        for y in 0..system.display_height {
            system.display_planes[0][y * system.display_width + (system.display_width - 1)] = true;
        }

        // Scroll left by 4 pixels
        system.scroll_left(4);

        // Last four columns should be blank
        for y in 0..system.display_height {
            for x in (system.display_width - 4)..system.display_width {
                assert!(
                    !system.display_planes[0][y * system.display_width + x],
                    "Column {} should be blank",
                    x
                );
            }
        }

        // Fifth-to-last column should have the pixels that were in the last column
        for y in 0..system.display_height {
            assert!(
                system.display_planes[0][y * system.display_width + (system.display_width - 5)],
                "Column {} should have pixels",
                system.display_width - 5
            );
        }
    }
}
