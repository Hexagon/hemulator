pub mod display_filter;
mod hemu_project;
mod rom_detect;
mod save_state;
mod settings;
mod system_adapter;
mod ui_render;
pub mod video_processor;
pub mod window_backend;
pub mod egui_ui;

use emu_core::{types::Frame, System};
use egui_ui::EguiApp;
use hemu_project::HemuProject;
use rodio::{OutputStream, Source};
use rom_detect::{detect_rom_type, SystemType};
use save_state::GameSaves;
use settings::Settings;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};
use window_backend::{string_to_key, Key, Sdl2EguiBackend, WindowBackend};

/// Runtime state for tracking currently loaded project and mounts
/// This replaces the mount_points field in Settings which has been deprecated
struct RuntimeState {
    /// Currently loaded .hemu project file path (if any)
    current_project_path: Option<PathBuf>,
    /// Current mount points (mount_id -> file_path)
    /// This is runtime-only and not persisted to config.json
    current_mounts: HashMap<String, String>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            current_project_path: None,
            current_mounts: HashMap::new(),
        }
    }

    fn set_mount(&mut self, mount_id: String, path: String) {
        self.current_mounts.insert(mount_id, path);
    }

    #[allow(dead_code)]
    fn get_mount(&self, mount_id: &str) -> Option<&String> {
        self.current_mounts.get(mount_id)
    }

    #[allow(dead_code)]
    fn clear_mounts(&mut self) {
        self.current_mounts.clear();
    }

    #[allow(dead_code)]
    fn set_project_path(&mut self, path: PathBuf) {
        self.current_project_path = Some(path);
    }

    #[allow(dead_code)]
    fn clear_project_path(&mut self) {
        self.current_project_path = None;
    }

    fn get_project_filename(&self) -> Option<String> {
        self.current_project_path.as_ref().and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
    }
}

// System wrapper enum to support multiple emulated systems
// Box large variants to prevent stack overflow
#[allow(clippy::upper_case_acronyms)]
enum EmulatorSystem {
    NES(Box<emu_nes::NesSystem>),
    GameBoy(Box<emu_gb::GbSystem>),
    Atari2600(Box<emu_atari2600::Atari2600System>),
    PC(Box<emu_pc::PcSystem>),
    SNES(Box<emu_snes::SnesSystem>),
    N64(Box<emu_n64::N64System>),
}

impl EmulatorSystem {
    fn step_frame(&mut self) -> Result<Frame, Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari2600(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PC(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SNES(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::N64(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    fn reset(&mut self) {
        match self {
            EmulatorSystem::NES(sys) => sys.reset(),
            EmulatorSystem::GameBoy(sys) => sys.reset(),
            EmulatorSystem::Atari2600(sys) => sys.reset(),
            EmulatorSystem::PC(sys) => sys.reset(),
            EmulatorSystem::SNES(sys) => sys.reset(),
            EmulatorSystem::N64(sys) => sys.reset(),
        }
    }

    fn mount(
        &mut self,
        mount_point_id: &str,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari2600(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PC(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SNES(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::N64(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    fn mount_points(&self) -> Vec<emu_core::MountPointInfo> {
        match self {
            EmulatorSystem::NES(sys) => sys.mount_points(),
            EmulatorSystem::GameBoy(sys) => sys.mount_points(),
            EmulatorSystem::Atari2600(sys) => sys.mount_points(),
            EmulatorSystem::PC(sys) => sys.mount_points(),
            EmulatorSystem::SNES(sys) => sys.mount_points(),
            EmulatorSystem::N64(sys) => sys.mount_points(),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari2600(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PC(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SNES(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::N64(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    #[allow(dead_code)]
    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::GameBoy(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::Atari2600(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::PC(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::SNES(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::N64(sys) => sys.is_mounted(mount_point_id),
        }
    }

    fn supports_save_states(&self) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.supports_save_states(),
            EmulatorSystem::GameBoy(sys) => sys.supports_save_states(),
            EmulatorSystem::Atari2600(sys) => sys.supports_save_states(),
            EmulatorSystem::PC(sys) => sys.supports_save_states(),
            EmulatorSystem::SNES(sys) => sys.supports_save_states(),
            EmulatorSystem::N64(sys) => sys.supports_save_states(),
        }
    }

    fn save_state(&self) -> serde_json::Value {
        match self {
            EmulatorSystem::NES(sys) => sys.save_state(),
            EmulatorSystem::GameBoy(sys) => sys.save_state(),
            EmulatorSystem::Atari2600(sys) => sys.save_state(),
            EmulatorSystem::PC(sys) => sys.save_state(),
            EmulatorSystem::SNES(sys) => sys.save_state(),
            EmulatorSystem::N64(sys) => sys.save_state(),
        }
    }

    fn load_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        match self {
            EmulatorSystem::NES(sys) => sys.load_state(state),
            EmulatorSystem::GameBoy(sys) => sys.load_state(state),
            EmulatorSystem::Atari2600(sys) => sys.load_state(state),
            EmulatorSystem::PC(sys) => sys.load_state(state),
            EmulatorSystem::SNES(sys) => sys.load_state(state),
            EmulatorSystem::N64(sys) => sys.load_state(state),
        }
    }

    // System-specific methods
    fn set_controller(&mut self, port: usize, state: u8) {
        match self {
            EmulatorSystem::NES(sys) => sys.set_controller(port, state),
            EmulatorSystem::GameBoy(sys) => {
                // Game Boy only has one controller (port)
                // We'll map the standard button IDs to Game Boy buttons
                // Game Boy buttons: Right, Left, Up, Down, A, B, Select, Start (bits 0-7)
                if port == 0 {
                    // Convert from standard mapping (A, B, Select, Start, Up, Down, Left, Right)
                    // to Game Boy mapping (Right, Left, Up, Down, A, B, Select, Start)
                    // Note: Game Boy uses active-low logic (0 = pressed, 1 = released)
                    let gb_state = ((state & 0x80) >> 7)  // Right (bit 7 -> bit 0)
                        | ((state & 0x40) >> 5)           // Left (bit 6 -> bit 1)
                        | ((state & 0x10) >> 2)           // Up (bit 4 -> bit 2)
                        | ((state & 0x20) >> 2)           // Down (bit 5 -> bit 3)
                        | ((state & 0x01) << 4)           // A (bit 0 -> bit 4)
                        | ((state & 0x02) << 4)           // B (bit 1 -> bit 5)
                        | ((state & 0x04) << 4)           // Select (bit 2 -> bit 6)
                        | ((state & 0x08) << 4); // Start (bit 3 -> bit 7)
                                                 // Invert for Game Boy's active-low logic (0 = pressed)
                    sys.set_controller(!gb_state);
                }
            }
            EmulatorSystem::Atari2600(sys) => sys.set_controller(port, state),
            EmulatorSystem::PC(_) => {} // PC doesn't use controller input
            EmulatorSystem::SNES(_) => {} // SNES controller support stub
            EmulatorSystem::N64(sys) => {
                // N64 controller mapping
                // GUI state bits: 0=A, 1=B, 2=Select, 3=Start, 4=Up, 5=Down, 6=Left, 7=Right
                // Map to N64 controller with proper button mapping
                // Note: N64 uses active-high logic (1 = pressed, bit set means button pressed)
                let mut n64_state = emu_n64::ControllerState::default();

                // Map standard buttons (A, B, Start)
                n64_state.buttons.a = (state & 0x01) != 0; // Bit 0
                n64_state.buttons.b = (state & 0x02) != 0; // Bit 1
                n64_state.buttons.start = (state & 0x08) != 0; // Bit 3

                // Map D-pad
                n64_state.buttons.d_up = (state & 0x10) != 0; // Bit 4
                n64_state.buttons.d_down = (state & 0x20) != 0; // Bit 5
                n64_state.buttons.d_left = (state & 0x40) != 0; // Bit 6
                n64_state.buttons.d_right = (state & 0x80) != 0; // Bit 7

                // Note: Select button (bit 2) is not used on N64
                // Z, L, R, and C-buttons would need additional key mappings

                // Set controller state based on port
                match port {
                    0 => sys.set_controller1(n64_state),
                    1 => sys.set_controller2(n64_state),
                    2 => sys.set_controller3(n64_state),
                    3 => sys.set_controller4(n64_state),
                    _ => {}
                }
            }
        }
    }

    fn set_controller_16(&mut self, port: usize, state: u16) {
        if let EmulatorSystem::SNES(sys) = self {
            sys.set_controller(port, state)
        }
        // Other systems use 8-bit set_controller
    }

    fn get_debug_info_nes(&self) -> Option<emu_nes::DebugInfo> {
        match self {
            EmulatorSystem::NES(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_n64(&self) -> Option<emu_n64::DebugInfo> {
        match self {
            EmulatorSystem::N64(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_atari2600(&self) -> Option<emu_atari2600::DebugInfo> {
        match self {
            EmulatorSystem::Atari2600(sys) => sys.debug_info(),
            _ => None,
        }
    }

    fn get_debug_info_snes(&self) -> Option<emu_snes::DebugInfo> {
        match self {
            EmulatorSystem::SNES(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_pc(&self) -> Option<emu_pc::DebugInfo> {
        match self {
            EmulatorSystem::PC(sys) => Some(sys.debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_gb(&self) -> Option<emu_gb::DebugInfo> {
        match self {
            EmulatorSystem::GameBoy(sys) => Some(sys.debug_info()),
            _ => None,
        }
    }

    /// Get instruction pointer (IP/PC) from any system
    fn get_instruction_pointer(&self) -> Option<u32> {
        match self {
            EmulatorSystem::NES(_) => {
                let stats = self.get_runtime_stats();
                if stats.pc > 0 {
                    Some(stats.pc as u32)
                } else {
                    None
                }
            }
            EmulatorSystem::GameBoy(sys) => {
                let debug = sys.debug_info();
                Some(debug.pc as u32)
            }
            EmulatorSystem::Atari2600(_) => {
                // Atari 2600 doesn't expose PC in a simple way
                None
            }
            EmulatorSystem::PC(sys) => {
                let debug = sys.debug_info();
                // For x86, IP is 16-bit but we can show full linear address CS:IP
                Some(((debug.cs as u32) << 4) + debug.ip)
            }
            EmulatorSystem::SNES(sys) => {
                let debug = sys.get_debug_info();
                // SNES has PBR:PC (24-bit address)
                Some(((debug.pbr as u32) << 16) | (debug.pc as u32))
            }
            EmulatorSystem::N64(sys) => {
                let debug = sys.get_debug_info();
                // N64 PC is 64-bit, truncate to 32-bit for display
                Some(debug.pc as u32)
            }
        }
    }

    /// Get target CPU frequency in MHz (historical/configured value)
    fn get_cpu_freq_target(&self) -> Option<f64> {
        match self {
            EmulatorSystem::NES(_) => Some(1.79), // NTSC NES CPU (1.789773 MHz)
            EmulatorSystem::GameBoy(_) => Some(4.19), // Game Boy CPU (4.194304 MHz)
            EmulatorSystem::Atari2600(_) => Some(1.19), // Atari 2600 6507 (1.19 MHz)
            EmulatorSystem::PC(sys) => Some(sys.cpu_speed_mhz()), // Variable based on CPU model
            EmulatorSystem::SNES(_) => Some(3.58), // SNES 65C816 (3.58 MHz)
            EmulatorSystem::N64(_) => Some(93.75), // N64 R4300i (93.75 MHz)
        }
    }

    /// Get actual CPU frequency in MHz (measured from cycle count)
    /// Returns None if we can't calculate it yet
    fn get_cpu_freq_actual(&self) -> Option<f64> {
        // For now, return None - actual frequency would require tracking cycles over time
        // This could be implemented by tracking cycles per second in the main loop
        None
    }

    fn get_runtime_stats(&self) -> emu_nes::RuntimeStats {
        match self {
            EmulatorSystem::NES(sys) => sys.get_runtime_stats(),
            EmulatorSystem::GameBoy(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Atari2600(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::PC(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::SNES(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::N64(_) => emu_nes::RuntimeStats::default(),
        }
    }

    fn timing(&self) -> emu_core::apu::TimingMode {
        match self {
            EmulatorSystem::NES(sys) => sys.timing(),
            EmulatorSystem::GameBoy(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Atari2600(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::PC(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::SNES(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::N64(_) => emu_core::apu::TimingMode::Ntsc,
        }
    }

    fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        match self {
            EmulatorSystem::NES(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GameBoy(sys) => sys.get_audio_samples(count),
            EmulatorSystem::Atari2600(sys) => sys.get_audio_samples(count),
            EmulatorSystem::PC(_) => vec![0; count], // TODO: Implement audio for PC
            EmulatorSystem::SNES(_) => vec![0; count], // TODO: Implement audio for SNES
            EmulatorSystem::N64(_) => vec![0; count], // TODO: Implement audio for N64
        }
    }

    fn resolution(&self) -> (usize, usize) {
        match self {
            EmulatorSystem::NES(_) => (256, 240),
            EmulatorSystem::GameBoy(_) => (160, 144),
            EmulatorSystem::Atari2600(_) => (160, 192),
            EmulatorSystem::PC(_) => (640, 400),
            EmulatorSystem::SNES(_) => (256, 224),
            EmulatorSystem::N64(_) => (320, 240),
        }
    }

    fn system_name(&self) -> &str {
        match self {
            EmulatorSystem::NES(_) => "nes",
            EmulatorSystem::GameBoy(_) => "gameboy",
            EmulatorSystem::Atari2600(_) => "atari2600",
            EmulatorSystem::PC(_) => "pc",
            EmulatorSystem::SNES(_) => "snes",
            EmulatorSystem::N64(_) => "n64",
        }
    }

    /// Update POST screen for PC system
    fn update_post_screen(&mut self) {
        if let EmulatorSystem::PC(sys) = self {
            sys.update_post_screen();
        }
    }

    /// Get disk image for saving (PC only)
    fn get_disk_image(&self, mount_id: &str) -> Option<&[u8]> {
        if let EmulatorSystem::PC(sys) = self {
            match mount_id {
                "FloppyA" => sys.get_floppy_a(),
                "FloppyB" => sys.get_floppy_b(),
                "HardDrive" => sys.get_hard_drive(),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if this system requires the host key to be held for function keys
    /// Only PC system requires this to allow ESC and function keys to pass through to the emulated system
    fn requires_host_key_for_function_keys(&self) -> bool {
        matches!(self, EmulatorSystem::PC(_))
    }
}

fn key_mapping_to_button(key: Key, mapping: &settings::KeyMapping) -> Option<u8> {
    // Map key to button based on mapping
    if Some(key) == string_to_key(&mapping.a) {
        Some(0)
    } else if Some(key) == string_to_key(&mapping.b) {
        Some(1)
    } else if Some(key) == string_to_key(&mapping.select) {
        Some(2)
    } else if Some(key) == string_to_key(&mapping.start) {
        Some(3)
    } else if Some(key) == string_to_key(&mapping.up) {
        Some(4)
    } else if Some(key) == string_to_key(&mapping.down) {
        Some(5)
    } else if Some(key) == string_to_key(&mapping.left) {
        Some(6)
    } else if Some(key) == string_to_key(&mapping.right) {
        Some(7)
    } else if Some(key) == string_to_key(&mapping.x) {
        Some(8)
    } else if Some(key) == string_to_key(&mapping.y) {
        Some(9)
    } else if Some(key) == string_to_key(&mapping.l) {
        Some(10)
    } else if Some(key) == string_to_key(&mapping.r) {
        Some(11)
    } else {
        None
    }
}

/// Get controller state for a player from current keyboard state (8-bit for NES/GB/Atari)
fn get_controller_state(window: &dyn WindowBackend, mapping: &settings::KeyMapping) -> u8 {
    let keys_to_check: Vec<Key> = vec![
        string_to_key(&mapping.a),
        string_to_key(&mapping.b),
        string_to_key(&mapping.select),
        string_to_key(&mapping.start),
        string_to_key(&mapping.up),
        string_to_key(&mapping.down),
        string_to_key(&mapping.left),
        string_to_key(&mapping.right),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut state: u8 = 0;
    for k in keys_to_check.iter() {
        if window.is_key_down(*k) {
            if let Some(bit) = key_mapping_to_button(*k, mapping) {
                state |= 1u8 << bit;
            }
        }
    }
    state
}

/// Get SNES controller state from current keyboard state (16-bit)
///
/// SNES controllers have 12 buttons laid out as a 16-bit value:
/// Bit positions: B Y Select Start Up Down Left Right A X L R 0 0 0 0
///
/// This function maps the common button IDs (0-11) used by the frontend to the
/// SNES hardware bit positions according to the official SNES controller specification.
///
/// Button ID mapping (from frontend):
/// - 0: A button
/// - 1: B button  
/// - 2: Select
/// - 3: Start
/// - 4: Up (D-pad)
/// - 5: Down (D-pad)
/// - 6: Left (D-pad)
/// - 7: Right (D-pad)
/// - 8: X button
/// - 9: Y button
/// - 10: L shoulder
/// - 11: R shoulder
///
/// SNES hardware bit positions (MSB to LSB):
/// - Bit 15: B button
/// - Bit 14: Y button
/// - Bit 13: Select
/// - Bit 12: Start
/// - Bit 11: Up
/// - Bit 10: Down
/// - Bit 9: Left
/// - Bit 8: Right
/// - Bit 7: A button
/// - Bit 6: X button
/// - Bit 5: L shoulder
/// - Bit 4: R shoulder
/// - Bits 3-0: Unused (always 0)
fn get_snes_controller_state(window: &dyn WindowBackend, mapping: &settings::KeyMapping) -> u16 {
    let keys_to_check: Vec<Key> = vec![
        string_to_key(&mapping.a),
        string_to_key(&mapping.b),
        string_to_key(&mapping.select),
        string_to_key(&mapping.start),
        string_to_key(&mapping.up),
        string_to_key(&mapping.down),
        string_to_key(&mapping.left),
        string_to_key(&mapping.right),
        string_to_key(&mapping.x),
        string_to_key(&mapping.y),
        string_to_key(&mapping.l),
        string_to_key(&mapping.r),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut state: u16 = 0;
    for k in keys_to_check.iter() {
        if window.is_key_down(*k) {
            // Map button IDs (0-11) to SNES button positions
            // NES/common layout: A(0), B(1), Select(2), Start(3), Up(4), Down(5), Left(6), Right(7), X(8), Y(9), L(10), R(11)
            // SNES layout: B(15), Y(14), Select(13), Start(12), Up(11), Down(10), Left(9), Right(8), A(7), X(6), L(5), R(4)
            if let Some(button_id) = key_mapping_to_button(*k, mapping) {
                let snes_bit = match button_id {
                    0 => 7,  // A -> bit 7
                    1 => 15, // B -> bit 15
                    2 => 13, // Select -> bit 13
                    3 => 12, // Start -> bit 12
                    4 => 11, // Up -> bit 11
                    5 => 10, // Down -> bit 10
                    6 => 9,  // Left -> bit 9
                    7 => 8,  // Right -> bit 8
                    8 => 6,  // X -> bit 6
                    9 => 14, // Y -> bit 14
                    10 => 5, // L -> bit 5
                    11 => 4, // R -> bit 4
                    _ => continue,
                };
                state |= 1u16 << snes_bit;
            }
        }
    }
    state
}

/// Streaming audio source backed by a channel. When there's no data, it outputs silence to avoid
/// underruns.
struct StreamSource {
    rx: Receiver<i16>,
    sample_rate: u32,
}

impl Iterator for StreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.rx.try_recv().unwrap_or(0);
        Some(s as f32 / 32768.0)
    }
}

impl Source for StreamSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Save current emulation state to a .hemu project file
/// Works for all systems, not just PC
fn save_project(
    sys: &EmulatorSystem,
    runtime_state: &RuntimeState,
    settings: &Settings,
    status_message: &mut String,
) {
    // Show file save dialog
    let default_name = format!("{}_project.hemu", sys.system_name());
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Hemulator Project", &["hemu"])
        .set_file_name(&default_name)
        .save_file()
    {
        let mut project = HemuProject::new(sys.system_name().to_string());

        // Copy current mount points from runtime state
        // Filter to only include mounts relevant to this system
        // Get system name first to avoid borrowing issue
        let system_name = sys.system_name();
        let relevant_mounts: Vec<&str> = match system_name {
            "pc" => vec!["BIOS", "FloppyA", "FloppyB", "HardDrive"],
            "nes" | "gameboy" | "atari2600" | "snes" | "n64" => vec!["Cartridge"],
            _ => vec![],
        };

        for (mount_id, mount_path) in &runtime_state.current_mounts {
            if relevant_mounts.contains(&mount_id.as_str()) {
                project.set_mount(mount_id.clone(), mount_path.clone());
            }
        }

        // Set display settings from current window state
        project.set_display_settings(
            settings.window_width,
            settings.window_height,
            settings.display_filter,
        );

        // For PC system, also save PC-specific configuration
        if let EmulatorSystem::PC(pc_sys) = sys {
            // Get boot priority from PC system
            let priority = pc_sys.boot_priority();
            let priority_str = match priority {
                emu_pc::BootPriority::FloppyFirst => "FloppyFirst",
                emu_pc::BootPriority::HardDriveFirst => "HardDriveFirst",
                emu_pc::BootPriority::FloppyOnly => "FloppyOnly",
                emu_pc::BootPriority::HardDriveOnly => "HardDriveOnly",
            };
            project.set_boot_priority(priority_str.to_string());

            // Get CPU model from PC system
            let cpu_model = pc_sys.cpu_model();
            let cpu_str = match cpu_model {
                emu_core::cpu_8086::CpuModel::Intel8086 => "Intel8086",
                emu_core::cpu_8086::CpuModel::Intel8088 => "Intel8088",
                emu_core::cpu_8086::CpuModel::Intel80186 => "Intel80186",
                emu_core::cpu_8086::CpuModel::Intel80188 => "Intel80188",
                emu_core::cpu_8086::CpuModel::Intel80286 => "Intel80286",
                emu_core::cpu_8086::CpuModel::Intel80386 => "Intel80386",
                emu_core::cpu_8086::CpuModel::Intel80486 => "Intel80486",
                emu_core::cpu_8086::CpuModel::Intel80486SX => "Intel80486SX",
                emu_core::cpu_8086::CpuModel::Intel80486DX2 => "Intel80486DX2",
                emu_core::cpu_8086::CpuModel::Intel80486SX2 => "Intel80486SX2",
                emu_core::cpu_8086::CpuModel::Intel80486DX4 => "Intel80486DX4",
                emu_core::cpu_8086::CpuModel::IntelPentium => "IntelPentium",
                emu_core::cpu_8086::CpuModel::IntelPentiumMMX => "IntelPentiumMMX",
            };
            project.set_cpu_model(cpu_str.to_string());

            // Get memory size from PC system
            let memory_kb = pc_sys.memory_kb();
            project.set_memory_kb(memory_kb);

            // Get video mode from PC system
            let video_name = pc_sys.video_adapter_name();
            let video_mode = if video_name.contains("VGA") {
                "VGA"
            } else if video_name.contains("EGA") {
                "EGA"
            } else {
                "CGA"
            };
            project.set_video_mode(video_mode.to_string());
        }

        match project.save(&path) {
            Ok(_) => {
                println!("Project saved to: {}", path.display());
                *status_message = format!(
                    "Project saved: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(e) => {
                eprintln!("Failed to save project: {}", e);
                *status_message = format!("Failed to save project: {}", e);
            }
        }
    }
}

/// Save a screenshot to the screenshots directory
/// Format: screenshots/<system-name>/YYYYMMDDHHMMSSRRR.png
/// where RRR is a random number between 000 and 999
fn save_screenshot(
    buffer: &[u32],
    width: usize,
    height: usize,
    system_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    use chrono::Local;
    use png::Encoder;
    use rand::Rng;

    // Get current local time
    let now = Local::now();

    // Generate random number 000-999
    let random = rand::thread_rng().gen_range(0..1000);

    // Create filename: YYYYMMDDHHMMSSRRR.png
    let filename = format!("{}{:03}.png", now.format("%Y%m%d%H%M%S"), random);

    // Create screenshots directory structure
    let screenshots_dir = PathBuf::from("screenshots").join(system_name);
    fs::create_dir_all(&screenshots_dir)?;

    let filepath = screenshots_dir.join(&filename);

    // Convert RGBA buffer to RGB
    let mut rgb_data = Vec::with_capacity(width * height * 3);
    for pixel in buffer {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        rgb_data.push(r);
        rgb_data.push(g);
        rgb_data.push(b);
    }

    // Write PNG file
    let file = fs::File::create(&filepath)?;
    let mut encoder = Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb_data)?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Create a file dialog with individual filters for each file type plus an "All Files" option
/// This improves the user experience by allowing them to filter by specific file types
fn create_file_dialog(mount_point: &emu_core::MountPointInfo) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();

    // Add individual filters for each extension
    for ext in &mount_point.extensions {
        // Create a user-friendly name for the filter
        let filter_name = match ext.as_str() {
            "nes" => "NES ROM (*.nes)".to_string(),
            "unf" => "UNIF ROM (*.unf)".to_string(),
            "gb" => "Game Boy ROM (*.gb)".to_string(),
            "gbc" => "Game Boy Color ROM (*.gbc)".to_string(),
            "a26" => "Atari 2600 ROM (*.a26)".to_string(),
            "bin" => "Binary ROM (*.bin)".to_string(),
            "com" => "DOS COM Executable (*.com)".to_string(),
            "exe" => "DOS EXE Executable (*.exe)".to_string(),
            _ => {
                // For unknown extensions, create a generic filter
                format!("{} File (*.{})", ext.to_uppercase(), ext)
            }
        };

        dialog = dialog.add_filter(&filter_name, &[ext.as_str()]);
    }

    // Add "All supported files" filter with all extensions
    let extensions: Vec<&str> = mount_point.extensions.iter().map(|s| s.as_str()).collect();
    dialog = dialog.add_filter("All Supported Files", &extensions);

    // Add "All Files" filter
    dialog = dialog.add_filter("All Files (*.*)", &["*"]);

    dialog
}

/// Command-line arguments for the emulator
#[derive(Debug, Default)]
struct CliArgs {
    rom_path: Option<String>,
    system: Option<String>, // System to start (pc, nes, gb, atari2600, snes, n64)
    slot1: Option<String>,  // BIOS or primary file
    slot2: Option<String>,  // FloppyA
    slot3: Option<String>,  // FloppyB
    slot4: Option<String>,  // HardDrive
    slot5: Option<String>,  // Reserved for future use
    create_blank_disk: Option<(String, String)>, // (path, format)
    show_help: bool,        // Show help message
    show_version: bool,     // Show version
    benchmark: bool,        // Benchmark mode: disable frame limiter to measure raw performance
    // Logging configuration
    log_level: Option<String>,      // Global log level
    log_cpu: Option<String>,        // CPU log level
    log_bus: Option<String>,        // Bus log level
    log_ppu: Option<String>,        // PPU log level
    log_apu: Option<String>,        // APU log level
    log_interrupts: Option<String>, // Interrupt log level
    log_stubs: Option<String>,      // Stub/unimplemented log level
    log_file: Option<String>,       // Log file path
}

impl CliArgs {
    /// Parse command-line arguments
    fn parse() -> Self {
        let mut args = CliArgs::default();
        let mut arg_iter = env::args().skip(1);

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    args.show_help = true;
                }
                "--version" | "-v" => {
                    args.show_version = true;
                }
                "--benchmark" => {
                    args.benchmark = true;
                }
                "--system" | "-S" => {
                    if let Some(system) = arg_iter.next() {
                        args.system = Some(system);
                    } else {
                        eprintln!(
                            "Error: --system requires a value (pc, nes, gb, atari2600, snes, n64)."
                        );
                        std::process::exit(1);
                    }
                }
                "--slot1" => {
                    args.slot1 = arg_iter.next();
                }
                "--slot2" => {
                    args.slot2 = arg_iter.next();
                }
                "--slot3" => {
                    args.slot3 = arg_iter.next();
                }
                "--slot4" => {
                    args.slot4 = arg_iter.next();
                }
                "--slot5" => {
                    args.slot5 = arg_iter.next();
                }
                "--create-blank-disk" => {
                    if let Some(path) = arg_iter.next() {
                        if let Some(format) = arg_iter.next() {
                            args.create_blank_disk = Some((path, format));
                        }
                    }
                }
                // Logging configuration
                "--log-level" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_level = Some(level);
                    } else {
                        eprintln!("Error: --log-level requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-cpu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_cpu = Some(level);
                    } else {
                        eprintln!("Error: --log-cpu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-bus" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_bus = Some(level);
                    } else {
                        eprintln!("Error: --log-bus requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-ppu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_ppu = Some(level);
                    } else {
                        eprintln!("Error: --log-ppu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-apu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_apu = Some(level);
                    } else {
                        eprintln!("Error: --log-apu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-interrupts" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_interrupts = Some(level);
                    } else {
                        eprintln!("Error: --log-interrupts requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-stubs" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_stubs = Some(level);
                    } else {
                        eprintln!("Error: --log-stubs requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-file" => {
                    if let Some(path) = arg_iter.next() {
                        args.log_file = Some(path);
                    } else {
                        eprintln!(
                            "Error: --log-file requires a file path (e.g., 'debug_trace.log')."
                        );
                        std::process::exit(1);
                    }
                }
                _ => {
                    // First non-flag argument is treated as ROM path for backward compatibility
                    if args.rom_path.is_none() && !arg.starts_with("--") {
                        args.rom_path = Some(arg);
                    }
                }
            }
        }

        args
    }

    /// Print usage information
    fn print_usage() {
        eprintln!(
            "Hemulator - Multi-System Emulator v{}",
            env!("CARGO_PKG_VERSION")
        );
        eprintln!();
        eprintln!("Usage: hemu [OPTIONS] [FILE]");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  [FILE]                   ROM file or .hemu project file to load");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -h, --help               Show this help message");
        eprintln!("  -v, --version            Show version information");
        eprintln!(
            "  --benchmark              Disable frame limiter to measure raw emulation performance"
        );
        eprintln!(
            "  -S, --system <SYSTEM>    Start clean system (pc, nes, gb, atari2600, snes, n64)"
        );
        eprintln!("  --slot1 <file>           Load file into slot 1 (BIOS for PC)");
        eprintln!("  --slot2 <file>           Load file into slot 2 (Floppy A for PC)");
        eprintln!("  --slot3 <file>           Load file into slot 3 (Floppy B for PC)");
        eprintln!("  --slot4 <file>           Load file into slot 4 (Hard Drive for PC)");
        eprintln!("  --slot5 <file>           Load file into slot 5 (reserved)");
        eprintln!("  --create-blank-disk <path> <format>");
        eprintln!("                           Create a blank disk image");
        eprintln!();
        eprintln!("Logging Options:");
        eprintln!("  --log-level <LEVEL>      Set global log level (off, error, warn, info, debug, trace)");
        eprintln!("  --log-cpu <LEVEL>        Set CPU log level");
        eprintln!("  --log-bus <LEVEL>        Set bus/memory log level");
        eprintln!("  --log-ppu <LEVEL>        Set PPU/graphics log level");
        eprintln!("  --log-apu <LEVEL>        Set APU/audio log level");
        eprintln!("  --log-interrupts <LEVEL> Set interrupt log level");
        eprintln!("  --log-stubs <LEVEL>      Set unimplemented feature log level");
        eprintln!("  --log-file <PATH>        Write logs to file instead of stderr");
        eprintln!();
        eprintln!("Disk formats:");
        eprintln!("  360k, 720k, 1.2m, 1.44m  Floppy disk formats");
        eprintln!("  20m, 250m, 1g, 20g       Hard drive formats");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  hemu game.nes                                  # Load NES ROM (auto-detect)");
        eprintln!(
            "  hemu --benchmark game.nes                      # Benchmark mode (no frame limiter)"
        );
        eprintln!(
            "  hemu test.com                                  # Load DOS COM file (auto-detect)"
        );
        eprintln!("  hemu --system pc test.bin                      # Load binary to PC FloppyB");
        eprintln!(
            "  hemu --system nes game.bin                     # Load binary as NES cartridge"
        );
        eprintln!("  hemu project.hemu                              # Load project file");
        eprintln!("  hemu --system pc                               # Start clean PC system");
        eprintln!("  hemu --log-cpu debug game.nes                  # Load with CPU debug logging");
        eprintln!(
            "  hemu --log-level info game.nes                 # Load with global info logging"
        );
        eprintln!("  hemu --log-cpu trace --log-file trace.log game.nes # Log CPU trace to file");
        eprintln!(
            "  hemu --slot2 disk.img                          # Load PC with floppy in drive A"
        );
        eprintln!(
            "  hemu --slot2 boot.img --slot4 hdd.img         # Load PC with floppy and hard drive"
        );
        eprintln!("  hemu --create-blank-disk floppy.img 1.44m      # Create 1.44MB floppy image");
        eprintln!(
            "  hemu --create-blank-disk hdd.img 20m           # Create 20MB hard drive image"
        );
    }

    /// Print version information
    fn print_version() {
        println!("Hemulator v{}", env!("CARGO_PKG_VERSION"));
        println!("Multi-System Emulator");
        println!("Supported systems: NES, Game Boy, Atari 2600, PC/DOS, SNES, N64");
    }
}

fn main() {
    // Parse command-line arguments
    let cli_args = CliArgs::parse();

    // Handle --help
    if cli_args.show_help {
        CliArgs::print_usage();
        std::process::exit(0);
    }

    // Handle --version
    if cli_args.show_version {
        CliArgs::print_version();
        std::process::exit(0);
    }

    // Handle --create-blank-disk command
    if let Some((path, format_str)) = &cli_args.create_blank_disk {
        match format_str.to_lowercase().as_str() {
            "360k" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy360K);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 360KB floppy disk: {}", path);
                std::process::exit(0);
            }
            "720k" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy720K);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 720KB floppy disk: {}", path);
                std::process::exit(0);
            }
            "1.2m" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_2M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 1.2MB floppy disk: {}", path);
                std::process::exit(0);
            }
            "1.44m" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_44M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 1.44MB floppy disk: {}", path);
                std::process::exit(0);
            }
            "20m" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive20M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 20MB hard drive image: {}", path);
                std::process::exit(0);
            }
            "250m" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive250M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 250MB hard drive image: {}", path);
                std::process::exit(0);
            }
            "1g" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive1G);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 1GB hard drive image: {}", path);
                std::process::exit(0);
            }
            "20g" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive20G);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 20GB hard drive image: {}", path);
                std::process::exit(0);
            }
            _ => {
                eprintln!("Error: Unknown disk format '{}'", format_str);
                eprintln!();
                CliArgs::print_usage();
                std::process::exit(1);
            }
        }
    }

    // Initialize the new logging system from command-line arguments
    let log_config = emu_core::logging::LogConfig::global();

    // Parse and set log levels from CLI args
    if let Some(ref level_str) = cli_args.log_level {
        if let Some(level) = emu_core::logging::LogLevel::from_str(level_str) {
            log_config.set_global_level(level);
            eprintln!("Global log level: {:?}", level);
        } else {
            eprintln!("Warning: Invalid log level '{}', using 'off'", level_str);
        }
    }

    // Configure category-specific log levels
    for (opt_level_str, category, name) in [
        (
            &cli_args.log_cpu,
            emu_core::logging::LogCategory::CPU,
            "CPU",
        ),
        (
            &cli_args.log_bus,
            emu_core::logging::LogCategory::Bus,
            "Bus",
        ),
        (
            &cli_args.log_ppu,
            emu_core::logging::LogCategory::PPU,
            "PPU",
        ),
        (
            &cli_args.log_apu,
            emu_core::logging::LogCategory::APU,
            "APU",
        ),
        (
            &cli_args.log_interrupts,
            emu_core::logging::LogCategory::Interrupts,
            "Interrupts",
        ),
        (
            &cli_args.log_stubs,
            emu_core::logging::LogCategory::Stubs,
            "Stubs",
        ),
    ] {
        if let Some(ref level_str) = opt_level_str {
            if let Some(level) = emu_core::logging::LogLevel::from_str(level_str) {
                log_config.set_level(category, level);
                eprintln!("{} log level: {:?}", name, level);
            } else {
                eprintln!(
                    "Warning: Invalid {} log level '{}', using 'off'",
                    name, level_str
                );
            }
        }
    }

    // Configure log file if specified
    if let Some(ref log_file_path) = cli_args.log_file {
        use std::path::PathBuf;
        let path = PathBuf::from(log_file_path);
        match log_config.set_log_file(path) {
            Ok(()) => {
                eprintln!("Logging to file: {}", log_file_path);
            }
            Err(e) => {
                eprintln!("Error: Failed to open log file '{}': {}", log_file_path, e);
                std::process::exit(1);
            }
        }
    }

    // Print benchmark mode message
    if cli_args.benchmark {
        eprintln!("==========================================");
        eprintln!("  BENCHMARK MODE: Frame limiter disabled");
        eprintln!("  Press F10 to see raw FPS performance");
        eprintln!("==========================================");
        eprintln!();
    }

    // Load settings
    let mut settings = Settings::load();

    // Save settings immediately to ensure config.json exists
    // (if it didn't exist, Settings::load() created defaults)
    if let Err(e) = settings.save() {
        eprintln!("Warning: Failed to save config.json: {}", e);
    }

    // Create runtime state for tracking current project and mounts
    let mut runtime_state = RuntimeState::new();

    // Determine what to load based on CLI args
    let rom_path = cli_args.rom_path.or_else(|| settings.last_rom_path.clone());

    // Validate that we have something to load or a system to start
    if cli_args.system.is_none() && rom_path.is_none() {
        eprintln!("Error: Must specify either a system (--system) or a file to load.");
        eprintln!();
        CliArgs::print_usage();
        std::process::exit(1);
    }

    let mut sys: EmulatorSystem;
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;
    let mut status_message = String::new();

    // Initialize system based on --system parameter if specified
    if let Some(ref system_name) = cli_args.system {
        match system_name.to_lowercase().as_str() {
            "nes" => {
                sys = EmulatorSystem::NES(Box::default());
                status_message = "Clean NES system started".to_string();
                println!("Started clean NES system");

                // If a file is provided with --system nes, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::NES(nes_sys) = &mut sys {
                                    if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load NES ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "NES ROM loaded".to_string();
                                        println!("Loaded NES ROM: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            "gb" | "gameboy" => {
                sys = EmulatorSystem::GameBoy(Box::new(emu_gb::GbSystem::new()));
                status_message = "Clean Game Boy system started".to_string();
                println!("Started clean Game Boy system");

                // If a file is provided with --system gb, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::GameBoy(gb_sys) = &mut sys {
                                    if let Err(e) = gb_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load Game Boy ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "Game Boy ROM loaded".to_string();
                                        println!("Loaded Game Boy ROM: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            "atari2600" | "atari" => {
                sys = EmulatorSystem::Atari2600(Box::new(emu_atari2600::Atari2600System::new()));
                status_message = "Clean Atari 2600 system started".to_string();
                println!("Started clean Atari 2600 system");

                // If a file is provided with --system atari2600, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::Atari2600(atari_sys) = &mut sys {
                                    if let Err(e) = atari_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load Atari 2600 ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "Atari 2600 ROM loaded".to_string();
                                        println!("Loaded Atari 2600 ROM: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            "pc" => {
                sys = EmulatorSystem::PC(Box::new(emu_pc::PcSystem::new()));
                status_message = "Clean PC system started".to_string();
                println!("Started clean PC system");

                // If a file is provided with --system pc, mount it to FloppyB
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                if let EmulatorSystem::PC(pc_sys) = &mut sys {
                                    if let Err(e) = pc_sys.mount("FloppyB", &data) {
                                        eprintln!("Failed to mount file to FloppyB: {}", e);
                                        status_message = format!("Error: {}", e);
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("FloppyB".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = format!("File loaded to FloppyB: {}", p);
                                        println!("Mounted file to FloppyB: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            "snes" => {
                sys = EmulatorSystem::SNES(Box::new(emu_snes::SnesSystem::new()));
                status_message = "Clean SNES system started".to_string();
                println!("Started clean SNES system");

                // If a file is provided with --system snes, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::SNES(snes_sys) = &mut sys {
                                    if let Err(e) = snes_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load SNES ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "SNES ROM loaded".to_string();
                                        println!("Loaded SNES ROM: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            "n64" => {
                sys = EmulatorSystem::N64(Box::new(emu_n64::N64System::new()));
                status_message = "Clean N64 system started".to_string();
                println!("Started clean N64 system");

                // If a file is provided with --system n64, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::N64(n64_sys) = &mut sys {
                                    if let Err(e) = n64_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load N64 ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        settings.last_rom_path = Some(p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "N64 ROM loaded".to_string();
                                        println!("Loaded N64 ROM: {}", p);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }
            _ => {
                eprintln!("Error: Unknown system '{}'", system_name);
                eprintln!("Valid systems: pc, nes, gb, atari2600, snes, n64");
                std::process::exit(1);
            }
        }
    } else {
        // No --system specified, default to NES for now (will be replaced by file loading below)
        sys = EmulatorSystem::NES(Box::default());
    }

    // Try to load ROM/project file if path is available
    // Check if it's a .hemu project file first (before reading as ROM)
    // Skip if already loaded via --system
    if let Some(p) = &rom_path {
        if rom_loaded {
            // Already loaded via --system, skip auto-detection
        } else if p.to_lowercase().ends_with(".hemu") {
            println!("Detected .hemu project file: {}", p);
            match HemuProject::load(p) {
                Ok(project) => {
                    if project.system != "pc" {
                        eprintln!("Currently only PC system .hemu projects are supported");
                        eprintln!("Project is for: {}", project.system);
                    } else {
                        // Parse configuration from project
                        let cpu_model = if let Some(cpu_str) = project.get_cpu_model() {
                            match cpu_str.as_str() {
                                "Intel8086" => emu_core::cpu_8086::CpuModel::Intel8086,
                                "Intel8088" => emu_core::cpu_8086::CpuModel::Intel8088,
                                "Intel80186" => emu_core::cpu_8086::CpuModel::Intel80186,
                                "Intel80188" => emu_core::cpu_8086::CpuModel::Intel80188,
                                "Intel80286" => emu_core::cpu_8086::CpuModel::Intel80286,
                                "Intel80386" => emu_core::cpu_8086::CpuModel::Intel80386,
                                "Intel80486" => emu_core::cpu_8086::CpuModel::Intel80486,
                                "Intel80486SX" => emu_core::cpu_8086::CpuModel::Intel80486SX,
                                "Intel80486DX2" => emu_core::cpu_8086::CpuModel::Intel80486DX2,
                                "Intel80486SX2" => emu_core::cpu_8086::CpuModel::Intel80486SX2,
                                "Intel80486DX4" => emu_core::cpu_8086::CpuModel::Intel80486DX4,
                                "IntelPentium" => emu_core::cpu_8086::CpuModel::IntelPentium,
                                "IntelPentiumMMX" => emu_core::cpu_8086::CpuModel::IntelPentiumMMX,
                                _ => {
                                    eprintln!(
                                        "Unknown CPU model: {}, using default Intel8086",
                                        cpu_str
                                    );
                                    emu_core::cpu_8086::CpuModel::Intel8086
                                }
                            }
                        } else {
                            emu_core::cpu_8086::CpuModel::Intel8086
                        };
                        println!("CPU model: {:?}", cpu_model);

                        let memory_kb = project.get_memory_kb().unwrap_or(640);
                        println!("Memory: {}KB", memory_kb);

                        // Create video adapter based on project configuration
                        let video_adapter: Box<dyn emu_pc::VideoAdapter> =
                            if let Some(video_str) = project.get_video_mode() {
                                match video_str.as_str() {
                                    "EGA" => {
                                        println!("Video mode: EGA");
                                        Box::new(emu_pc::SoftwareEgaAdapter::new())
                                    }
                                    "VGA" => {
                                        println!("Video mode: VGA");
                                        Box::new(emu_pc::SoftwareVgaAdapter::new())
                                    }
                                    "CGA" => {
                                        println!("Video mode: CGA");
                                        Box::new(emu_pc::SoftwareCgaAdapter::new())
                                    }
                                    _ => {
                                        println!("Video mode: CGA (unknown mode, defaulting)");
                                        Box::new(emu_pc::SoftwareCgaAdapter::new())
                                    }
                                }
                            } else {
                                println!("Video mode: CGA (default)");
                                Box::new(emu_pc::SoftwareCgaAdapter::new())
                            };

                        // Create PC system with configuration
                        let mut pc_sys =
                            emu_pc::PcSystem::with_config(cpu_model, memory_kb, video_adapter);

                        // Load boot priority if specified
                        if let Some(priority_str) = project.boot_priority.as_ref() {
                            let priority = match priority_str.as_str() {
                                "FloppyFirst" => emu_pc::BootPriority::FloppyFirst,
                                "HardDriveFirst" => emu_pc::BootPriority::HardDriveFirst,
                                "FloppyOnly" => emu_pc::BootPriority::FloppyOnly,
                                "HardDriveOnly" => emu_pc::BootPriority::HardDriveOnly,
                                _ => emu_pc::BootPriority::FloppyFirst,
                            };
                            pc_sys.set_boot_priority(priority);
                            println!("Set boot priority: {:?}", priority);
                        }

                        // Mount all files from the project
                        let project_dir = std::path::Path::new(p)
                            .parent()
                            .unwrap_or(std::path::Path::new("."));
                        for (mount_id, relative_path) in &project.mounts {
                            let full_path = project_dir.join(relative_path);
                            match fs::read(&full_path) {
                                Ok(data) => {
                                    if let Err(e) = pc_sys.mount(mount_id, &data) {
                                        eprintln!("Failed to mount {}: {}", mount_id, e);
                                    } else {
                                        runtime_state.set_mount(
                                            mount_id.clone(),
                                            full_path.to_string_lossy().to_string(),
                                        );
                                        println!("Mounted {}: {}", mount_id, relative_path);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to read {}: {}", relative_path, e);
                                }
                            }
                        }

                        // Update POST screen with mount status
                        pc_sys.update_post_screen();

                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                        rom_loaded = true; // Allow POST screen to be displayed
                        status_message = "PC virtual machine loaded".to_string();
                        println!("Switched to PC system");

                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load .hemu project: {}", e);
                }
            }
        } else {
            // Regular ROM file detection (not a .hemu file)
            match std::fs::read(p) {
                Ok(data) => match detect_rom_type(&data) {
                    Ok(SystemType::NES) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut nes_sys = emu_nes::NesSystem::default();
                        // Use the mount point system to load the cartridge
                        if let Err(e) = nes_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load NES ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone()); // Keep for backward compat
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "NES ROM loaded".to_string();
                            println!("Loaded NES ROM: {}", p);
                        }
                    }
                    Ok(SystemType::Atari2600) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut a2600_sys = emu_atari2600::Atari2600System::new();
                        if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load Atari 2600 ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "Atari 2600 ROM loaded".to_string();
                            println!("Loaded Atari 2600 ROM: {}", p);
                        }
                    }
                    Ok(SystemType::GameBoy) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut gb_sys = emu_gb::GbSystem::new();
                        if let Err(e) = gb_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load Game Boy ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::GameBoy(Box::new(gb_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "Game Boy ROM loaded".to_string();
                            println!("Loaded Game Boy ROM: {}", p);
                        }
                    }
                    Ok(SystemType::PC) => {
                        // PC executables should be on disk images, not loaded directly
                        // Create a new PC system and let user mount disk images via F3
                        status_message =
                            "PC system detected. Use F3 to mount disk images.".to_string();
                        rom_hash = None; // PC systems don't use ROM hash
                        let pc_sys = emu_pc::PcSystem::new();
                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                        // Don't save mount points for PC since they use disk images
                        eprintln!("PC executable detected. Please mount disk images using F3.");
                        println!("Initialized PC system. Mount disk images to proceed.");
                    }
                    Ok(SystemType::SNES) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut snes_sys = emu_snes::SnesSystem::new();
                        if let Err(e) = snes_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load SNES ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::SNES(Box::new(snes_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "SNES ROM loaded".to_string();
                            println!("Loaded SNES ROM: {}", p);
                        }
                    }
                    Ok(SystemType::N64) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut n64_sys = emu_n64::N64System::new();
                        if let Err(e) = n64_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load N64 ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::N64(Box::new(n64_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "N64 ROM loaded".to_string();
                            println!("Loaded N64 ROM: {}", p);
                        }
                    }
                    Err(e) => {
                        eprintln!("Unsupported ROM: {}", e);
                        status_message = format!("Unsupported ROM: {}", e);
                    }
                }, // closes inner match detect_rom_type
                Err(e) => {
                    eprintln!("Failed to read ROM file: {}", e);
                }
            }
        } // closes else block for non-.hemu files
    } // closes if let Some(p) = &rom_path

    // Handle slot-based loading (primarily for PC system)
    // If any slot arguments are provided, auto-select PC mode if no ROM was loaded
    let has_slot_args = cli_args.slot1.is_some()
        || cli_args.slot2.is_some()
        || cli_args.slot3.is_some()
        || cli_args.slot4.is_some()
        || cli_args.slot5.is_some();

    if has_slot_args && !rom_loaded {
        // Auto-select PC mode when slot files are provided
        let pc_sys = emu_pc::PcSystem::new();
        sys = EmulatorSystem::PC(Box::new(pc_sys));
        rom_loaded = true;
        println!("Auto-selected PC mode for slot-based loading");
    }

    // Load slot files for PC system
    if let EmulatorSystem::PC(ref mut pc_sys) = sys {
        // Slot 1: BIOS
        if let Some(ref slot1_path) = cli_args.slot1 {
            match fs::read(slot1_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("BIOS", &data) {
                        eprintln!("Failed to mount BIOS from slot 1: {}", e);
                    } else {
                        runtime_state.set_mount("BIOS".to_string(), slot1_path.clone());
                        println!("Loaded BIOS from slot 1: {}", slot1_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 1 file: {}", e),
            }
        }

        // Slot 2: Floppy A
        if let Some(ref slot2_path) = cli_args.slot2 {
            match fs::read(slot2_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("FloppyA", &data) {
                        eprintln!("Failed to mount Floppy A from slot 2: {}", e);
                    } else {
                        runtime_state.set_mount("FloppyA".to_string(), slot2_path.clone());
                        println!("Loaded Floppy A from slot 2: {}", slot2_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 2 file: {}", e),
            }
        }

        // Slot 3: Floppy B
        if let Some(ref slot3_path) = cli_args.slot3 {
            match fs::read(slot3_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("FloppyB", &data) {
                        eprintln!("Failed to mount Floppy B from slot 3: {}", e);
                    } else {
                        runtime_state.set_mount("FloppyB".to_string(), slot3_path.clone());
                        println!("Loaded Floppy B from slot 3: {}", slot3_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 3 file: {}", e),
            }
        }

        // Slot 4: Hard Drive
        if let Some(ref slot4_path) = cli_args.slot4 {
            match fs::read(slot4_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("HardDrive", &data) {
                        eprintln!("Failed to mount Hard Drive from slot 4: {}", e);
                    } else {
                        runtime_state.set_mount("HardDrive".to_string(), slot4_path.clone());
                        println!("Loaded Hard Drive from slot 4: {}", slot4_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 4 file: {}", e),
            }
        }

        // Slot 5: Reserved for future use
        if cli_args.slot5.is_some() {
            eprintln!("Warning: Slot 5 is reserved for future use and will be ignored");
        }

        // Save settings if any slot was loaded
        if has_slot_args {
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save settings: {}", e);
            }
        }
    }

    // Get resolution from the system
    let (mut width, mut height) = sys.resolution();

    // Window size is user-resizable and persisted; buffer size stays at native resolution.
    let window_width = settings.window_width.max(width);
    let window_height = settings.window_height.max(height);

    // Create egui backend
    let mut egui_backend = match Sdl2EguiBackend::new(
        "Hemulator - Multi-System Emulator",
        window_width as u32,
        window_height as u32,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create egui window: {}", e);
            return;
        }
    };

    // Initialize egui app
    let mut egui_app = EguiApp::new();
    egui_app.property_pane.system_name = sys.system_name().to_string();
    egui_app.property_pane.rendering_backend = "OpenGL (egui)".to_string();
    egui_app.status_bar.set_message(status_message.clone());

    // Initialize audio output
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Warning: Failed to initialize audio: {}. Audio will be disabled.",
                e
            );
            return;
        }
    };
    let (audio_tx, audio_rx) = sync_channel::<i16>(44100 * 2);
    if let Err(e) = stream_handle.play_raw(
        StreamSource {
            rx: audio_rx,
            sample_rate: 44100,
        }
        .convert_samples(),
    ) {
        eprintln!("Warning: Failed to start audio playback: {}", e);
    }

    // Timing trackers
    let mut last_frame = Instant::now();

    // FPS tracking
    let mut frame_times: Vec<Duration> = Vec::with_capacity(60);
    let mut current_fps = 60.0;

    // Audio sample rate
    const SAMPLE_RATE: usize = 44100;

    // Load saves for current ROM if available
    let _game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    fn blend_over(base: &[u32], overlay: &[u32]) -> Vec<u32> {
        debug_assert_eq!(base.len(), overlay.len());
        let mut out = Vec::with_capacity(base.len());
        for (b, o) in base.iter().copied().zip(overlay.iter().copied()) {
            let a = (o >> 24) & 0xFF;
            if a == 0 {
                out.push(b);
                continue;
            }
            if a == 255 {
                out.push(0xFF00_0000 | (o & 0x00FF_FFFF));
                continue;
            }

            let inv = 255 - a;
            let br = (b >> 16) & 0xFF;
            let bg = (b >> 8) & 0xFF;
            let bb = b & 0xFF;

            let or = (o >> 16) & 0xFF;
            let og = (o >> 8) & 0xFF;
            let ob = o & 0xFF;

            let r = (or * a + br * inv) / 255;
            let g = (og * a + bg * inv) / 255;
            let b = (ob * a + bb * inv) / 255;

            out.push(0xFF00_0000 | (r << 16) | (g << 8) | b);
        }
        out
    }

    // Main event loop with egui
    loop {
        // Handle SDL2 events and update egui input
        if !egui_backend.handle_events() {
            break; // Window closed
        }

        // Begin egui frame
        egui_backend.begin_frame();

        // Update egui app state
        egui_app.property_pane.fps = current_fps;
        egui_app.property_pane.paused = settings.emulation_speed == 0.0;
        egui_app.property_pane.speed = settings.emulation_speed as f32;
        egui_app.property_pane.cpu_freq_target = sys.get_cpu_freq_target();
        egui_app.property_pane.emulation_speed_percent = (settings.emulation_speed * 100.0) as i32;

        // Update debug info if debug tab is visible
        if egui_app.tab_manager.debug_visible {
            use system_adapter::SystemDebugInfo;
            let debug_info = match &sys {
                EmulatorSystem::NES(s) => SystemDebugInfo::from_nes(&s.get_debug_info()),
                EmulatorSystem::GameBoy(s) => SystemDebugInfo::from_gb(&s.debug_info()),
                EmulatorSystem::Atari2600(s) => {
                    if let Some(info) = s.debug_info() {
                        SystemDebugInfo::from_atari2600(&info)
                    } else {
                        SystemDebugInfo::new("Atari 2600".to_string())
                    }
                }
                EmulatorSystem::PC(s) => SystemDebugInfo::from_pc(&s.debug_info()),
                EmulatorSystem::SNES(s) => SystemDebugInfo::from_snes(&s.get_debug_info()),
                EmulatorSystem::N64(s) => SystemDebugInfo::from_n64(&s.get_debug_info()),
            };
            egui_app.tab_manager.update_debug_info(debug_info);
        }

        // Render egui UI
        egui_app.ui(egui_backend.egui_ctx());

        // Handle menu actions
        if let Some(action) = egui_app.menu_bar.take_action() {
            use egui_ui::menu_bar::MenuAction;
            match action {
                MenuAction::OpenRom => {
                    // Open ROM file dialog
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("ROM Files", &["nes", "gb", "gbc", "bin", "a26", "smc", "sfc", "z64", "n64", "com", "exe"])
                        .add_filter("All Files", &["*"])
                        .pick_file()
                    {
                        // TODO: Implement ROM loading logic (move from old main loop)
                        egui_app.status_bar.set_message(format!("Opening ROM: {}", path.display()));
                    }
                }
                MenuAction::Reset => {
                    sys.reset();
                    egui_app.status_bar.set_message("System reset".to_string());
                }
                MenuAction::Pause => {
                    settings.emulation_speed = 0.0;
                    egui_app.status_bar.set_message("Paused".to_string());
                }
                MenuAction::Resume => {
                    settings.emulation_speed = 1.0;
                    egui_app.status_bar.set_message("Resumed".to_string());
                }
                MenuAction::Screenshot => {
                    // TODO: Implement screenshot
                    egui_app.status_bar.set_message("Screenshot not yet implemented".to_string());
                }
                MenuAction::ShowHelp => {
                    egui_app.tab_manager.show_help_tab();
                }
                MenuAction::About => {
                    egui_app.status_bar.set_message("Hemulator Multi-System Emulator".to_string());
                }
                _ => {}
            }
        }

        // Handle emulation speed changes from property pane
        settings.emulation_speed = (egui_app.property_pane.emulation_speed_percent as f64) / 100.0;

        // Step emulation frame if ROM is loaded and not paused
        if rom_loaded && settings.emulation_speed > 0.0 {
            // Step the frame
            match sys.step_frame() {
                Ok(frame) => {
                    // Update emulator texture with new frame
                    egui_app.update_emulator_texture(
                        egui_backend.egui_ctx(),
                        &frame.pixels,
                        frame.width as usize,
                        frame.height as usize,
                    );

                    // Handle audio
                    let timing = sys.timing();
                    let frame_rate = timing.frame_rate_hz();
                    let samples_per_frame = (SAMPLE_RATE as f64 / frame_rate) as usize;
                    let audio_samples = sys.get_audio_samples(samples_per_frame);
                    for sample in audio_samples {
                        let _ = audio_tx.try_send(sample);
                    }
                }
                Err(e) => {
                    eprintln!("Emulation error: {}", e);
                }
            }

            // Handle keyboard input for emulator
            if !matches!(&sys, EmulatorSystem::PC(_)) {
                // For non-PC systems, use standard controller mapping
                let controller_state = get_controller_state(&egui_backend, &settings.input.player1);
                let snes_state = get_snes_controller_state(&egui_backend, &settings.input.player1);
                match &mut sys {
                    EmulatorSystem::SNES(s) => s.set_controller(0, snes_state),
                    _ => sys.set_controller(0, controller_state),
                }
            } else {
                // PC systems handle keyboard directly via scancodes
                let pressed = egui_backend.get_sdl2_scancodes_pressed();
                let released = egui_backend.get_sdl2_scancodes_released();
                if let EmulatorSystem::PC(pc_sys) = &mut sys {
                    for &scancode in pressed {
                        pc_sys.key_press_sdl2(scancode as u32);
                    }
                    for &scancode in released {
                        pc_sys.key_release_sdl2(scancode as u32);
                    }
                }
            }
        }

        // End egui frame and render
        egui_backend.end_frame();

        // FPS tracking
        let frame_dt = last_frame.elapsed();
        frame_times.push(frame_dt);
        if frame_times.len() > 60 {
            frame_times.remove(0);
        }
        if !frame_times.is_empty() {
            let total_time: Duration = frame_times.iter().sum();
            let avg_frame_time = total_time.as_secs_f64() / frame_times.len() as f64;
            if avg_frame_time > 0.0 {
                current_fps = (1.0 / avg_frame_time) as f32;
            }
        }

        // Frame timing
        let target_frame_time = if rom_loaded && settings.emulation_speed > 0.0 {
            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            Duration::from_secs_f64(1.0 / (frame_rate * settings.emulation_speed))
        } else {
            Duration::from_millis(16) // ~60 FPS when idle
        };

        if frame_dt < target_frame_time {
            std::thread::sleep(target_frame_time - frame_dt);
        }
        last_frame = Instant::now();
    }

}
