pub mod display_filter;
mod hemu_project;
mod menu;
mod popup_window;
mod rom_detect;
mod save_state;
mod selector;
mod settings;
mod status_bar;
mod system_adapter;
mod ui_render;
pub mod video_processor;
pub mod window_backend;
pub mod egui_ui;

use emu_core::{types::Frame, System};
use hemu_project::HemuProject;
use menu::{MenuAction, MenuBar};
use popup_window::PopupWindowManager;
use rodio::{OutputStream, Source};
use rom_detect::{detect_rom_type, SystemType};
use save_state::GameSaves;
use selector::SelectorManager;
use settings::Settings;
use status_bar::StatusBar;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};
use window_backend::{string_to_key, Key, Sdl2Backend, WindowBackend};

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

    // Determine whether to use OpenGL or software rendering
    let use_opengl = settings.video_backend == "opengl";

    let mut window: Box<dyn WindowBackend> = match Sdl2Backend::new(
        "Hemulator - Multi-System Emulator",
        window_width,
        window_height,
        use_opengl,
    ) {
        Ok(w) => Box::new(w),
        Err(e) => {
            eprintln!("Failed to create window: {}", e);
            return;
        }
    };

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

    // Buffer for rendering
    let mut buffer = if rom_loaded {
        vec![0; width * height]
    } else {
        ui_render::create_splash_screen_with_status(width, height, &status_message)
    };

    // Popup window manager (replaces old overlay system)
    let mut popup_manager = PopupWindowManager::new();

    // Selector manager (for slot/speed/disk format selection)
    let mut selector_manager = SelectorManager::new();

    // Timing trackers
    let mut last_frame = Instant::now();

    // FPS tracking (used by the debug overlay)
    let mut frame_times: Vec<Duration> = Vec::with_capacity(60);
    let mut current_fps = 60.0;

    // Audio sample rate
    const SAMPLE_RATE: usize = 44100;

    // Load saves for current ROM if available
    let mut game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    // Initialize menu bar and status bar
    let mut menu_bar = MenuBar::new();
    let mut status_bar = StatusBar::new();
    status_bar.system_name = sys.system_name().to_string();
    status_bar.message = status_message.clone();
    status_bar.paused = settings.emulation_speed == 0.0;
    status_bar.speed = settings.emulation_speed as f32;

    // Initialize mount points menu
    menu_bar.update_mount_points(&sys.mount_points(), &runtime_state.current_mounts);

    // Track logging state
    let mut logging_active = false;

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

    // Main event loop
    // Host key (Right Alt) must be held to use emulator controls (F1-F12, ESC to exit)
    // Without host key, all keys are passed to the emulated system
    while window.is_open() {
        // Poll events at the start of each frame
        window.poll_events();

        // Handle menu clicks and hover - collect data first to avoid borrow issues
        let (mouse_clicks, mouse_position): (Vec<(i32, i32)>, (i32, i32)) =
            if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
                (
                    sdl2_backend.get_mouse_clicks().to_vec(),
                    sdl2_backend.get_mouse_position(),
                )
            } else {
                (Vec::new(), (0, 0))
            };

        // Handle menu hover for visual feedback and auto-menu-switching
        let (mx, my) = mouse_position;
        if mx >= 0 && my >= 0 {
            menu_bar.handle_hover(mx as usize, my as usize);
        }

        for (x, y) in mouse_clicks {
            // Ignore clicks with negative coordinates to avoid wrapping when casting to usize
            if x < 0 || y < 0 {
                continue;
            }

            // Check if popup window consumed the click first
            if popup_manager.handle_click(x, y) {
                continue; // Popup consumed the click, don't process menu
            }

            if let Some(action) = menu_bar.handle_click(x as usize, y as usize) {
                // Process menu action
                match action {
                    MenuAction::OpenRom => {
                        // Open ROM file dialog and load the ROM
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "ROM Files",
                                &[
                                    "nes", "gb", "gbc", "bin", "a26", "smc", "sfc", "z64", "n64",
                                    "com", "exe",
                                ],
                            )
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            let path_str = path.to_string_lossy().to_string();
                            match std::fs::read(&path) {
                                Ok(data) => match detect_rom_type(&data) {
                                    Ok(SystemType::NES) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut nes_sys = emu_nes::NesSystem::default();
                                        if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                            eprintln!("Failed to load NES ROM: {}", e);
                                            status_message = format!("Error: {}", e);
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                                            status_bar.system_name = "NES".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.last_rom_path = Some(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            status_message = "NES ROM loaded".to_string();
                                            status_bar.message = status_message.clone();
                                            println!("Loaded NES ROM: {}", path_str);
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
                                            status_bar.system_name = "Game Boy".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.last_rom_path = Some(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            status_message = "Game Boy ROM loaded".to_string();
                                            status_bar.message = status_message.clone();
                                            println!("Loaded Game Boy ROM: {}", path_str);
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
                                            status_bar.system_name = "Atari 2600".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.last_rom_path = Some(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            status_message = "Atari 2600 ROM loaded".to_string();
                                            status_bar.message = status_message.clone();
                                            println!("Loaded Atari 2600 ROM: {}", path_str);
                                        }
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
                                            status_bar.system_name = "SNES".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.last_rom_path = Some(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            status_message = "SNES ROM loaded".to_string();
                                            status_bar.message = status_message.clone();
                                            println!("Loaded SNES ROM: {}", path_str);
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
                                            status_bar.system_name = "N64".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.last_rom_path = Some(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            status_message = "N64 ROM loaded".to_string();
                                            status_bar.message = status_message.clone();
                                            println!("Loaded N64 ROM: {}", path_str);
                                        }
                                    }
                                    Ok(SystemType::PC) => {
                                        status_message =
                                            "PC system: Use Mount Points for disk images"
                                                .to_string();
                                        status_bar.message = status_message.clone();
                                        rom_hash = None;
                                        let pc_sys = emu_pc::PcSystem::new();
                                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                                        status_bar.system_name = "PC".to_string();
                                        println!("Initialized PC system");
                                    }
                                    Err(e) => {
                                        eprintln!("Unsupported ROM: {}", e);
                                        status_message = format!("Unsupported ROM: {}", e);
                                        status_bar.message = status_message.clone();
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to read ROM file: {}", e);
                                    status_message = format!("Failed to read ROM: {}", e);
                                    status_bar.message = status_message.clone();
                                }
                            }
                        }
                    }
                    MenuAction::OpenProject => {
                        // Open .hemu project file
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Hemulator Project", &["hemu"])
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            println!("Open project: {}", path.display());
                            // TODO: Implement full project loading logic similar to old F7 handler
                            status_message =
                                "Project loading not yet fully implemented".to_string();
                            status_bar.message = status_message.clone();
                        }
                    }
                    MenuAction::SaveProject => {
                        save_project(&sys, &runtime_state, &settings, &mut status_message);
                    }
                    MenuAction::Exit => {
                        break;
                    }
                    MenuAction::Reset => {
                        let can_reset = rom_loaded || matches!(&sys, EmulatorSystem::PC(_));
                        if can_reset {
                            sys.reset();
                            println!("System reset");
                            status_message = "System reset".to_string();
                            status_bar.message = status_message.clone();
                        }
                    }
                    MenuAction::Pause => {
                        settings.emulation_speed = 0.0;
                        status_message = "Paused".to_string();
                        status_bar.paused = true;
                        status_bar.speed = 0.0;
                        status_bar.message = status_message.clone();
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save speed setting: {}", e);
                        }
                    }
                    MenuAction::Speed(speed_setting) => {
                        settings.emulation_speed = speed_setting.to_float() as f64;
                        status_bar.paused = settings.emulation_speed == 0.0;
                        status_bar.speed = settings.emulation_speed as f32;
                        status_message =
                            format!("Speed: {}%", (settings.emulation_speed * 100.0) as u32);
                        status_bar.message = status_message.clone();
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save speed setting: {}", e);
                        }
                    }
                    MenuAction::SaveState(slot) => {
                        if rom_loaded && sys.supports_save_states() {
                            let state_data = sys.save_state();
                            if let Ok(state_bytes) = serde_json::to_vec(&state_data) {
                                if let Some(ref hash) = rom_hash {
                                    if let Err(e) = game_saves.save_slot(slot, &state_bytes, hash) {
                                        eprintln!("Failed to save state: {}", e);
                                        status_message = format!("Failed to save state: {}", e);
                                    } else {
                                        println!("Saved state to slot {}", slot);
                                        status_message = format!("State saved to slot {}", slot);
                                    }
                                } else {
                                    status_message = "Cannot save state: no ROM loaded".to_string();
                                }
                            } else {
                                status_message = "Failed to serialize state".to_string();
                            }
                            status_bar.message = status_message.clone();
                        }
                    }
                    MenuAction::LoadState(slot) => {
                        if rom_loaded && sys.supports_save_states() {
                            if let Some(ref hash) = rom_hash {
                                match game_saves.load_slot(slot, hash) {
                                    Ok(state_bytes) => {
                                        if let Ok(state_data) =
                                            serde_json::from_slice::<serde_json::Value>(
                                                &state_bytes,
                                            )
                                        {
                                            if let Err(e) = sys.load_state(&state_data) {
                                                eprintln!("Failed to load state: {}", e);
                                                status_message =
                                                    format!("Failed to load state: {}", e);
                                            } else {
                                                println!("Loaded state from slot {}", slot);
                                                status_message =
                                                    format!("State loaded from slot {}", slot);
                                            }
                                        } else {
                                            status_message =
                                                "Failed to deserialize state".to_string();
                                        }
                                    }
                                    Err(e) => {
                                        status_message =
                                            format!("No save state in slot {}: {}", slot, e);
                                    }
                                }
                            } else {
                                status_message = "Cannot load state: no ROM loaded".to_string();
                            }
                            status_bar.message = status_message.clone();
                        }
                    }
                    MenuAction::Screenshot => {
                        match save_screenshot(&buffer, width, height, sys.system_name()) {
                            Ok(path) => {
                                println!("Screenshot saved to: {}", path);
                                status_message = "Screenshot saved".to_string();
                                status_bar.message = status_message.clone();
                            }
                            Err(e) => eprintln!("Failed to save screenshot: {}", e),
                        }
                    }
                    MenuAction::DebugInfo => {
                        popup_manager.toggle_debug();
                        selector_manager.close();
                    }
                    MenuAction::CrtFilter(filter) => {
                        settings.display_filter = filter;
                        if let Some(sdl2_backend) =
                            window.as_any_mut().downcast_mut::<Sdl2Backend>()
                        {
                            sdl2_backend.set_filter(settings.display_filter);
                        }
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save CRT filter setting: {}", e);
                        }
                        println!("CRT Filter: {}", settings.display_filter.name());
                        status_message = format!("CRT Filter: {}", settings.display_filter.name());
                        status_bar.message = status_message.clone();
                    }
                    MenuAction::Help => {
                        popup_manager.toggle_help();
                        selector_manager.close();
                    }
                    MenuAction::About => {
                        // TODO: Show about dialog
                        println!("Hemulator - Multi-System Emulator");
                    }
                    MenuAction::NewProject => {
                        // Create a new project by prompting for system type
                        // For now, show an info message - full implementation would show a system selector dialog
                        status_message =
                            "New Project: Please use File -> Open ROM to load a ROM file"
                                .to_string();
                        status_bar.message = status_message.clone();
                        println!("New Project - Use File -> Open ROM to load a ROM");
                    }
                    MenuAction::Resume => {
                        settings.emulation_speed = 1.0;
                        status_message = "Resumed".to_string();
                        status_bar.paused = false;
                        status_bar.speed = 1.0;
                        status_bar.message = status_message.clone();
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save speed setting: {}", e);
                        }
                    }
                    MenuAction::StartLogging => {
                        // Start logging to file
                        use std::path::PathBuf;
                        let log_path = PathBuf::from("log.txt");
                        match emu_core::logging::LogConfig::global().set_log_file(log_path) {
                            Ok(()) => {
                                logging_active = true;
                                status_message = "Logging started (log.txt)".to_string();
                                status_bar.message = status_message.clone();
                                println!("Logging enabled - writing to log.txt");
                            }
                            Err(e) => {
                                status_message = format!("Failed to start logging: {}", e);
                                status_bar.message = status_message.clone();
                                eprintln!("Failed to start logging: {}", e);
                            }
                        }
                    }
                    MenuAction::StopLogging => {
                        // Stop logging
                        emu_core::logging::LogConfig::global().clear_log_file();
                        logging_active = false;
                        status_message = "Logging stopped".to_string();
                        status_bar.message = status_message.clone();
                        println!("Logging disabled");
                    }
                    MenuAction::MountFile(mount_id) => {
                        // Find the mount point info
                        let mount_points = sys.mount_points();
                        if let Some(mp) = mount_points.iter().find(|m| m.id == mount_id) {
                            if let Some(path) = create_file_dialog(mp).pick_file() {
                                match std::fs::read(&path) {
                                    Ok(data) => {
                                        if let Err(e) = sys.mount(&mount_id, &data) {
                                            eprintln!("Failed to mount {}: {}", mount_id, e);
                                            status_message = format!("Mount failed: {}", e);
                                        } else {
                                            runtime_state.set_mount(
                                                mount_id.clone(),
                                                path.to_string_lossy().to_string(),
                                            );
                                            status_message = format!("Mounted {}", mp.name);
                                            // Update menu to reflect new mount state
                                            menu_bar.update_mount_points(
                                                &sys.mount_points(),
                                                &runtime_state.current_mounts,
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to read file: {}", e);
                                        status_message = format!("Read error: {}", e);
                                    }
                                }
                                status_bar.message = status_message.clone();
                            }
                        }
                    }
                    MenuAction::EjectMount(mount_id) => {
                        if let Err(e) = sys.unmount(&mount_id) {
                            eprintln!("Failed to eject {}: {}", mount_id, e);
                            status_message = format!("Eject failed: {}", e);
                        } else {
                            runtime_state.current_mounts.remove(&mount_id);
                            status_message = format!("Ejected {}", mount_id);
                            // Update menu to reflect new mount state
                            menu_bar.update_mount_points(
                                &sys.mount_points(),
                                &runtime_state.current_mounts,
                            );
                        }
                        status_bar.message = status_message.clone();
                    }
                    MenuAction::StartMachine => {
                        // Start/resume machine
                        if settings.emulation_speed == 0.0 {
                            settings.emulation_speed = 1.0;
                            status_bar.paused = false;
                            status_bar.speed = 1.0;
                        }
                    }
                    MenuAction::StopMachine => {
                        // Stop/pause machine
                        settings.emulation_speed = 0.0;
                        status_bar.paused = true;
                        status_bar.speed = 0.0;
                    }
                }
            }
        }

        // Check if host key (from settings) is held
        let host_modifier_key =
            window_backend::string_to_key(&settings.input.host_modifier).unwrap_or(Key::RightCtrl); // fallback to RightCtrl if invalid
        let host_key_held = window.is_key_down(host_modifier_key);

        // Check if this system requires host key for function keys
        let needs_host_key = sys.requires_host_key_for_function_keys();

        // Only exit on ESC if host key is held (when required by system), else allow ESC always
        // But first check if any overlay is open - close it instead of exiting
        if (needs_host_key && host_key_held && window.is_key_down(Key::Escape))
            || (!needs_host_key && window.is_key_down(Key::Escape))
        {
            // Close overlays first, only exit if no overlay is open
            if selector_manager.is_open() || popup_manager.has_open_popup() {
                selector_manager.close();
                popup_manager.close_all();
            } else {
                break;
            }
        }

        // Toggle help overlay (F1)
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F1, false))
            || (!needs_host_key && window.is_key_pressed(Key::F1, false))
        {
            popup_manager.toggle_help();
            selector_manager.close(); // Close selector if open
                                      // Close disk format selector if open
        }

        // Toggle debug overlay (F10)
        let can_debug = rom_loaded || needs_host_key; // Allow debug for PC even without ROM
        if ((needs_host_key && host_key_held && window.is_key_pressed(Key::F10, false))
            || (!needs_host_key && window.is_key_pressed(Key::F10, false)))
            && can_debug
        {
            popup_manager.toggle_debug();
            selector_manager.close(); // Close selector if open

            // Dump debug info to console when opening debug overlay
            if popup_manager.is_debug_open() {
                println!("\n=== Debug Info Dump ===");

                // Try NES debug info first
                if let Some(debug_info) = sys.get_debug_info_nes() {
                    let timing_str = match debug_info.timing_mode {
                        emu_core::apu::TimingMode::Ntsc => "NTSC",
                        emu_core::apu::TimingMode::Pal => "PAL",
                    };
                    println!("System: NES");
                    println!(
                        "Mapper: {} (#{:03})",
                        debug_info.mapper_name, debug_info.mapper_number
                    );
                    println!("Timing: {}", timing_str);
                    println!(
                        "PRG Banks: {} ({}KB total)",
                        debug_info.prg_banks,
                        debug_info.prg_banks * 16
                    );
                    println!(
                        "CHR Banks: {} ({}KB total)",
                        debug_info.chr_banks,
                        if debug_info.chr_banks == 0 {
                            "RAM".to_string()
                        } else {
                            format!("{}", debug_info.chr_banks * 8)
                        }
                    );
                    let stats = sys.get_runtime_stats();
                    println!("Frame: {}", stats.frame_index);
                    println!("PC: 0x{:04X}", stats.pc);
                }
                // Try Atari 2600 debug info
                else if let Some(debug_info) = sys.get_debug_info_atari2600() {
                    println!("System: Atari 2600");
                    println!("ROM Size: {} bytes", debug_info.rom_size);
                    println!("Banking: {}", debug_info.banking_scheme);
                    println!("Current Bank: {}", debug_info.current_bank);
                    println!("Scanline: {}", debug_info.scanline);
                }
                // Try Game Boy debug info
                else if let Some(debug_info) = sys.get_debug_info_gb() {
                    println!("System: Game Boy");
                    println!("PC: 0x{:04X}", debug_info.pc);
                    println!("SP: 0x{:04X}", debug_info.sp);
                    println!("AF: 0x{:04X}", debug_info.af);
                    println!("BC: 0x{:04X}", debug_info.bc);
                    println!("DE: 0x{:04X}", debug_info.de);
                    println!("HL: 0x{:04X}", debug_info.hl);
                    println!("IME: {}", if debug_info.ime { "ON" } else { "OFF" });
                    println!("Halted: {}", if debug_info.halted { "YES" } else { "NO" });
                    println!("LY: {} (scanline)", debug_info.ly);
                    println!("LCDC: 0x{:02X}", debug_info.lcdc);
                }
                // Try N64 debug info
                else if let Some(debug_info) = sys.get_debug_info_n64() {
                    println!("System: Nintendo 64");
                    println!("ROM: {}", debug_info.rom_name);
                    println!("ROM Size: {:.2} MB", debug_info.rom_size_mb);
                    println!("PC: 0x{:016X}", debug_info.pc);
                    println!("RSP Microcode: {}", debug_info.rsp_microcode);
                    println!("RSP Vertices: {}", debug_info.rsp_vertex_count);
                    println!("RDP Status: 0x{:08X}", debug_info.rdp_status);
                    println!("Framebuffer: {}", debug_info.framebuffer_resolution);
                }
                // Try SNES debug info
                else if let Some(debug_info) = sys.get_debug_info_snes() {
                    println!("System: SNES");
                    println!("ROM Size: {} KB", debug_info.rom_size / 1024);
                    println!(
                        "Header: {}",
                        if debug_info.has_smc_header {
                            "SMC (512 bytes)"
                        } else {
                            "None"
                        }
                    );
                    println!("PC: 0x{:02X}:{:04X}", debug_info.pbr, debug_info.pc);
                    println!(
                        "Mode: {}",
                        if debug_info.emulation_mode {
                            "Emulation (6502)"
                        } else {
                            "Native (65C816)"
                        }
                    );
                }

                println!("FPS: {:.1}", current_fps);
                println!("Video Backend: {}", settings.video_backend);
                println!("======================\n");
            }
        }

        // Cycle CRT filter (F11) - only when host key is held
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F11, false))
            || (!needs_host_key && window.is_key_pressed(Key::F11, false))
        {
            settings.display_filter = settings.display_filter.next();
            // Update the backend's filter setting
            if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
                sdl2_backend.set_filter(settings.display_filter);
            }
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save CRT filter setting: {}", e);
            }
            println!("CRT Filter: {}", settings.display_filter.name());
        }

        // New keyboard shortcuts (Ctrl+key combinations)
        let ctrl_held = window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
        let shift_held = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);

        // Ctrl+O - Open ROM
        if ctrl_held && !shift_held && window.is_key_pressed(Key::O, false) {
            // Trigger F3 handler (mount points)
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    "ROM Files",
                    &[
                        "nes", "gb", "gbc", "bin", "a26", "smc", "sfc", "z64", "n64", "com", "exe",
                    ],
                )
                .add_filter("All Files", &["*"])
                .pick_file()
            {
                // Load ROM logic would go here - for now, trigger same as F3
                println!("Open ROM: {}", path.display());
            }
        }

        // Ctrl+S - Save Project
        if ctrl_held && !shift_held && window.is_key_pressed(Key::S, false) {
            // Trigger F8 handler (save project)
            save_project(&sys, &runtime_state, &settings, &mut status_message);
        }

        // Ctrl+R - Reset
        if ctrl_held && !shift_held && window.is_key_pressed(Key::R, false) {
            let can_reset = rom_loaded || matches!(&sys, EmulatorSystem::PC(_));
            if can_reset {
                sys.reset();
                println!("System reset");
                status_message = "System reset".to_string();
                status_bar.message = status_message.clone();
            }
        }

        // Ctrl+P - Pause/Resume
        if ctrl_held && !shift_held && window.is_key_pressed(Key::P, false) {
            if settings.emulation_speed == 0.0 {
                settings.emulation_speed = 1.0;
                status_message = "Resumed".to_string();
            } else {
                settings.emulation_speed = 0.0;
                status_message = "Paused".to_string();
            }
            status_bar.paused = settings.emulation_speed == 0.0;
            status_bar.speed = settings.emulation_speed as f32;
            status_bar.message = status_message.clone();
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save speed setting: {}", e);
            }
        }

        // Ctrl+1-5 - Save state slots
        if ctrl_held && !shift_held && rom_loaded && sys.supports_save_states() {
            for i in 1..=5 {
                let key = match i {
                    1 => Key::Key1,
                    2 => Key::Key2,
                    3 => Key::Key3,
                    4 => Key::Key4,
                    5 => Key::Key5,
                    _ => continue,
                };

                if window.is_key_pressed(key, false) {
                    let state_data = sys.save_state();
                    // Serialize to bytes
                    if let Ok(state_bytes) = serde_json::to_vec(&state_data) {
                        if let Some(ref hash) = rom_hash {
                            if let Err(e) = game_saves.save_slot(i, &state_bytes, hash) {
                                eprintln!("Failed to save state: {}", e);
                                status_message = format!("Failed to save state: {}", e);
                            } else {
                                println!("Saved state to slot {}", i);
                                status_message = format!("State saved to slot {}", i);
                            }
                        } else {
                            status_message = "Cannot save state: no ROM loaded".to_string();
                        }
                    } else {
                        status_message = "Failed to serialize state".to_string();
                    }
                    status_bar.message = status_message.clone();
                    break;
                }
            }
        }

        // Ctrl+Shift+1-5 - Load state slots
        if ctrl_held && shift_held && rom_loaded && sys.supports_save_states() {
            for i in 1..=5 {
                let key = match i {
                    1 => Key::Key1,
                    2 => Key::Key2,
                    3 => Key::Key3,
                    4 => Key::Key4,
                    5 => Key::Key5,
                    _ => continue,
                };

                if window.is_key_pressed(key, false) {
                    if let Some(ref hash) = rom_hash {
                        match game_saves.load_slot(i, hash) {
                            Ok(state_bytes) => {
                                // Deserialize from bytes
                                if let Ok(state_data) =
                                    serde_json::from_slice::<serde_json::Value>(&state_bytes)
                                {
                                    if let Err(e) = sys.load_state(&state_data) {
                                        eprintln!("Failed to load state: {}", e);
                                        status_message = format!("Failed to load state: {}", e);
                                    } else {
                                        println!("Loaded state from slot {}", i);
                                        status_message = format!("State loaded from slot {}", i);
                                    }
                                } else {
                                    status_message = "Failed to deserialize state".to_string();
                                }
                            }
                            Err(e) => {
                                status_message = format!("No save state in slot {}: {}", i, e);
                            }
                        }
                    } else {
                        status_message = "Cannot load state: no ROM loaded".to_string();
                    }
                    status_bar.message = status_message.clone();
                    break;
                }
            }
        }

        // Handle speed selector
        if false {
            // Check for speed selection (0-5) or cancel (ESC)
            let mut selected_speed: Option<f64> = None;

            if window.is_key_pressed(Key::Key0, false) {
                selected_speed = Some(0.0); // Pause
            } else if window.is_key_pressed(Key::Key1, false) {
                selected_speed = Some(0.25);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_speed = Some(0.5);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_speed = Some(1.0);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_speed = Some(2.0);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_speed = Some(10.0);
            }

            if let Some(speed) = selected_speed {
                settings.emulation_speed = speed;
                if let Err(e) = settings.save() {
                    eprintln!("Warning: Failed to save speed setting: {}", e);
                }
                println!("Emulation speed: {}x", speed);
            }
        }

        // Handle slot selector
        if selector_manager.is_open() {
            // For PC system in SAVE mode, show disk persist menu instead of save state slots
            if matches!(&sys, EmulatorSystem::PC(_))
                && selector_manager
                    .active_selector
                    .as_ref()
                    .map(|s| s.selector_type == selector::SelectorType::SaveSlot)
                    .unwrap_or(false)
            {
                // PC disk persist menu
                let mount_points = sys.mount_points();
                let mounted: Vec<bool> = mount_points
                    .iter()
                    .map(|mp| sys.get_disk_image(&mp.id).is_some())
                    .collect();

                // Check for selection (1 = persist all, 2+ = individual mounts)
                let mut selected_option: Option<usize> = None;

                if window.is_key_pressed(Key::Key1, false) {
                    selected_option = Some(0); // Persist all
                } else if window.is_key_pressed(Key::Key2, false) {
                    selected_option = Some(1);
                } else if window.is_key_pressed(Key::Key3, false) {
                    selected_option = Some(2);
                } else if window.is_key_pressed(Key::Key4, false) {
                    selected_option = Some(3);
                } else if window.is_key_pressed(Key::Key5, false) {
                    selected_option = Some(4);
                } else if window.is_key_pressed(Key::Key6, false) {
                    selected_option = Some(5);
                }

                if let Some(option) = selected_option {
                    selector_manager.close();

                    if option == 0 {
                        // Persist all images
                        let mut saved_count = 0;
                        for (i, mp) in mount_points.iter().enumerate() {
                            if i < mounted.len() && mounted[i] {
                                if let Some(disk_data) = sys.get_disk_image(&mp.id) {
                                    if let Some(path) = runtime_state.get_mount(&mp.id) {
                                        match std::fs::write(path, disk_data) {
                                            Ok(_) => {
                                                println!("Saved {} to {}", mp.name, path);
                                                saved_count += 1;
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Failed to save {} to {}: {}",
                                                    mp.name, path, e
                                                );
                                            }
                                        }
                                    } else {
                                        eprintln!("No path stored for {}", mp.name);
                                    }
                                }
                            }
                        }
                        println!("Persisted {} disk image(s)", saved_count);
                    } else {
                        // Persist individual image
                        // Map option 1-5 to mount point index, skipping unmounted ones
                        let mut current_option = 1;
                        for (i, mp) in mount_points.iter().enumerate() {
                            if i < mounted.len() && mounted[i] {
                                if current_option == option {
                                    if let Some(disk_data) = sys.get_disk_image(&mp.id) {
                                        if let Some(path) = runtime_state.get_mount(&mp.id) {
                                            match std::fs::write(path, disk_data) {
                                                Ok(_) => println!("Saved {} to {}", mp.name, path),
                                                Err(e) => eprintln!(
                                                    "Failed to save {} to {}: {}",
                                                    mp.name, path, e
                                                ),
                                            }
                                        } else {
                                            eprintln!("No path stored for {}", mp.name);
                                        }
                                    }
                                    break;
                                }
                                current_option += 1;
                            }
                        }
                    }
                }
            } else {
                // Regular save/load state handling for other systems
                // Check for slot selection (1-5) or cancel (ESC)
                let mut selected_slot: Option<u8> = None;

                if window.is_key_pressed(Key::Key1, false) {
                    selected_slot = Some(1);
                } else if window.is_key_pressed(Key::Key2, false) {
                    selected_slot = Some(2);
                } else if window.is_key_pressed(Key::Key3, false) {
                    selected_slot = Some(3);
                } else if window.is_key_pressed(Key::Key4, false) {
                    selected_slot = Some(4);
                } else if window.is_key_pressed(Key::Key5, false) {
                    selected_slot = Some(5);
                }

                if let Some(slot) = selected_slot {
                    selector_manager.close();

                    if let Some(ref hash) = rom_hash {
                        if selector_manager
                            .active_selector
                            .as_ref()
                            .map(|s| s.selector_type == selector::SelectorType::SaveSlot)
                            .unwrap_or(false)
                        {
                            // Check if system supports save states
                            if !sys.supports_save_states() {
                                eprintln!("Save states are not supported for this system");
                            } else {
                                // Save state
                                let state = sys.save_state();
                                match serde_json::to_vec(&state) {
                                    Ok(data) => match game_saves.save_slot(slot, &data, hash) {
                                        Ok(_) => println!("Saved state to slot {}", slot),
                                        Err(e) => {
                                            eprintln!("Failed to save to slot {}: {}", slot, e)
                                        }
                                    },
                                    Err(e) => eprintln!("Failed to serialize state: {}", e),
                                }
                            }
                        } else {
                            // Load state
                            if !sys.supports_save_states() {
                                eprintln!("Save states are not supported for this system");
                            } else {
                                match game_saves.load_slot(slot, hash) {
                                    Ok(data) => {
                                        match serde_json::from_slice::<serde_json::Value>(&data) {
                                            Ok(state) => match sys.load_state(&state) {
                                                Ok(_) => {
                                                    println!("Loaded state from slot {}", slot)
                                                }
                                                Err(e) => {
                                                    eprintln!("Failed to load state: {}", e)
                                                }
                                            },
                                            Err(e) => {
                                                eprintln!("Failed to parse save state: {}", e)
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to load from slot {}: {}", slot, e)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle mount point selector
        if false {
            let mount_points = sys.mount_points();

            // Check for mount point selection
            let mut selected_index: Option<usize> = None;

            if window.is_key_pressed(Key::Key1, false) {
                selected_index = Some(0);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_index = Some(1);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_index = Some(2);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_index = Some(3);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_index = Some(4);
            } else if window.is_key_pressed(Key::Key6, false) {
                selected_index = Some(5);
            } else if window.is_key_pressed(Key::Key7, false) {
                selected_index = Some(6);
            } else if window.is_key_pressed(Key::Key8, false) {
                selected_index = Some(7);
            } else if window.is_key_pressed(Key::Key9, false) {
                // Key 9: Show disk format selector for creating new blank disk (PC only)
                if sys.system_name() == "pc" {}
            }

            if let Some(idx) = selected_index {
                if idx < mount_points.len() {
                    // Now show file dialog for the selected mount point
                    let mp_info = &mount_points[idx];

                    if let Some(path) = create_file_dialog(mp_info).pick_file() {
                        let path_str = path.to_string_lossy().to_string();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                match sys.mount(&mp_info.id, &data) {
                                    Ok(_) => {
                                        rom_loaded = true;
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        runtime_state
                                            .set_mount(mp_info.id.clone(), path_str.clone());
                                        settings.last_rom_path = Some(path_str.clone()); // Keep for backward compat
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        game_saves = if let Some(ref hash) = rom_hash {
                                            GameSaves::load(hash)
                                        } else {
                                            GameSaves::default()
                                        };

                                        // Update resolution to match the system's native resolution
                                        // This is critical when loading a ROM into a system with different
                                        // resolution than the initial system (e.g., loading Atari ROM after
                                        // starting with NES system)
                                        let (new_width, new_height) = sys.resolution();
                                        width = new_width;
                                        height = new_height;
                                        buffer = vec![0; width * height];

                                        println!(
                                            "Loaded media into {}: {}",
                                            mp_info.name, path_str
                                        );
                                        // Update POST screen for PC system
                                        sys.update_post_screen();
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to mount media into {}: {}",
                                            mp_info.name, e
                                        );
                                        status_message = format!("Failed to mount: {}", e);
                                        buffer = ui_render::create_splash_screen_with_status(
                                            width,
                                            height,
                                            &status_message,
                                        );
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

            // Render mount point selector
            let mount_buffer = ui_render::create_mount_point_selector(
                width,
                height,
                &mount_points,
                sys.system_name(),
            );
            if let Err(e) = window.update_with_buffer(&mount_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        // Handle disk format selector
        if false {
            // Check for format selection (1-7)
            let mut selected_format: Option<usize> = None;

            if window.is_key_pressed(Key::Key1, false) {
                selected_format = Some(0);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_format = Some(1);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_format = Some(2);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_format = Some(3);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_format = Some(4);
            } else if window.is_key_pressed(Key::Key6, false) {
                selected_format = Some(5);
            } else if window.is_key_pressed(Key::Key7, false) {
                selected_format = Some(6);
            } else if window.is_key_pressed(Key::Key8, false) {
                selected_format = Some(7);
            }

            if let Some(fmt_idx) = selected_format {
                // Create the blank disk based on selected format
                let (disk_data, default_name, description) = match fmt_idx {
                    0 => (
                        emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy360K),
                        "floppy_360k.img",
                        "360KB Floppy",
                    ),
                    1 => (
                        emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy720K),
                        "floppy_720k.img",
                        "720KB Floppy",
                    ),
                    2 => (
                        emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_2M),
                        "floppy_1_2m.img",
                        "1.2MB Floppy",
                    ),
                    3 => (
                        emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_44M),
                        "floppy_1_44m.img",
                        "1.44MB Floppy",
                    ),
                    4 => (
                        emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive20M),
                        "hdd_20m.img",
                        "20MB Hard Drive",
                    ),
                    5 => (
                        emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive250M),
                        "hdd_250m.img",
                        "250MB Hard Drive",
                    ),
                    6 => (
                        emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive1G),
                        "hdd_1g.img",
                        "1GB Hard Drive",
                    ),
                    7 => (
                        emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive20G),
                        "hdd_20g.img",
                        "20GB Hard Drive",
                    ),
                    _ => {
                        eprintln!("Invalid disk format index: {}", fmt_idx);
                        continue;
                    }
                };

                // Show save dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Disk Image", &["img"])
                    .add_filter("All Files", &["*"])
                    .set_file_name(default_name)
                    .save_file()
                {
                    match std::fs::write(&path, &disk_data) {
                        Ok(_) => {
                            println!("Created {} disk image: {}", description, path.display());
                            status_message = format!("Created {} disk image", description);
                        }
                        Err(e) => {
                            eprintln!("Failed to save disk image: {}", e);
                            status_message = format!("Failed to save disk: {}", e);
                        }
                    }
                }
            }

            // Render disk format selector
            let format_buffer = ui_render::create_disk_format_selector(width, height);
            if let Err(e) = window.update_with_buffer(&format_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        // Prepare popup window overlays
        let mut help_overlay: Option<Vec<u32>> = None;
        if let Some(ref help_window) = popup_manager.help_window {
            help_overlay = Some(help_window.render(width, height, &settings));
        }

        // Prepare debug overlay buffer when requested
        let mut debug_overlay: Option<Vec<u32>> = None;
        if let Some(ref mut debug_window) = popup_manager.debug_window {
            // For now, use simple debug window rendering
            // TODO: Properly extract and pass debug info from different systems
            debug_overlay = Some(debug_window.render(
                width,
                height,
                None,
                current_fps,
                &settings.video_backend,
            ));
        }

        // Legacy debug overlay for systems not yet migrated to popup window
        // This will be removed once all systems are migrated
        if debug_overlay.is_none() && rom_loaded && popup_manager.is_debug_open() {
            // Try NES debug info first
            if let Some(debug_info) = sys.get_debug_info_nes() {
                let timing_str = match debug_info.timing_mode {
                    emu_core::apu::TimingMode::Ntsc => "NTSC",
                    emu_core::apu::TimingMode::Pal => "PAL",
                };
                debug_overlay = Some(ui_render::create_debug_overlay(
                    width,
                    height,
                    &debug_info.mapper_name,
                    debug_info.mapper_number,
                    timing_str,
                    debug_info.prg_banks,
                    debug_info.chr_banks,
                    current_fps,
                    sys.get_runtime_stats(),
                    &settings.video_backend,
                ));
            }
            // Try Atari 2600 debug info
            else if let Some(debug_info) = sys.get_debug_info_atari2600() {
                debug_overlay = Some(ui_render::create_atari2600_debug_overlay(
                    width,
                    height,
                    debug_info.rom_size,
                    &debug_info.banking_scheme,
                    debug_info.current_bank,
                    debug_info.scanline,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try Game Boy debug info
            else if let Some(debug_info) = sys.get_debug_info_gb() {
                debug_overlay = Some(ui_render::create_gb_debug_overlay(
                    width,
                    height,
                    debug_info.pc,
                    debug_info.sp,
                    debug_info.af,
                    debug_info.bc,
                    debug_info.de,
                    debug_info.hl,
                    debug_info.ime,
                    debug_info.halted,
                    debug_info.ly,
                    debug_info.lcdc,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try N64 debug info if NES didn't match
            else if let Some(debug_info) = sys.get_debug_info_n64() {
                debug_overlay = Some(ui_render::create_n64_debug_overlay(
                    width,
                    height,
                    &debug_info.rom_name,
                    debug_info.rom_size_mb,
                    debug_info.pc,
                    &debug_info.rsp_microcode,
                    debug_info.rsp_vertex_count,
                    debug_info.rdp_status,
                    &debug_info.framebuffer_resolution,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try SNES debug info
            else if let Some(debug_info) = sys.get_debug_info_snes() {
                debug_overlay = Some(ui_render::create_snes_debug_overlay(
                    width,
                    height,
                    debug_info.rom_size,
                    debug_info.has_smc_header,
                    debug_info.pc,
                    debug_info.pbr,
                    debug_info.emulation_mode,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try PC debug info
            else if let Some(debug_info) = sys.get_debug_info_pc() {
                debug_overlay = Some(ui_render::create_pc_debug_overlay(
                    width,
                    height,
                    debug_info.cs,
                    debug_info.ip,
                    debug_info.ax,
                    debug_info.bx,
                    debug_info.cx,
                    debug_info.dx,
                    debug_info.sp,
                    debug_info.bp,
                    debug_info.si,
                    debug_info.di,
                    debug_info.flags,
                    debug_info.cycles,
                    current_fps,
                    &settings.video_backend,
                ));
            }
        }

        // Check for screenshot key (F4) - only when host key is held
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F4, false))
            || (!needs_host_key && window.is_key_pressed(Key::F4, false))
        {
            match save_screenshot(&buffer, width, height, sys.system_name()) {
                Ok(path) => println!("Screenshot saved to: {}", path),
                Err(e) => eprintln!("Failed to save screenshot: {}", e),
            }
        }

        // Handle controller input / emulation step when ROM is loaded.
        // Debug overlay should NOT pause the game, but selectors should.
        // Speed selector and 0x speed also pause the game.
        // For PC systems, always render frames (to show POST screen even when no disk is loaded)
        let should_step = (rom_loaded || matches!(&sys, EmulatorSystem::PC(_)))
            && !popup_manager.is_help_open()
            && !selector_manager.is_open()
            && settings.emulation_speed > 0.0;

        if should_step {
            // Handle keyboard input for PC system
            if matches!(&sys, EmulatorSystem::PC(_)) {
                // PC system: Use SDL2 scancodes directly for accurate physical key mapping
                // This bypasses the Key enum and directly maps SDL2 scancodes to PC scancodes
                if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
                    if let EmulatorSystem::PC(pc_sys) = &mut sys {
                        // Handle pressed scancodes (only if host key is not held)
                        if !host_key_held {
                            for &scancode in sdl2_backend.get_sdl2_scancodes_pressed() {
                                pc_sys.key_press_sdl2(scancode);
                            }
                            for &scancode in sdl2_backend.get_sdl2_scancodes_released() {
                                pc_sys.key_release_sdl2(scancode);
                            }
                        }
                    }
                }
            } else {
                // Controller-based systems (NES, GB, Atari, SNES, etc.)
                if matches!(&sys, EmulatorSystem::SNES(_)) {
                    // SNES uses 16-bit controller state
                    let ctrl0 = get_snes_controller_state(window.as_ref(), &settings.input.player1);
                    let ctrl1 = get_snes_controller_state(window.as_ref(), &settings.input.player2);
                    sys.set_controller_16(0, ctrl0);
                    sys.set_controller_16(1, ctrl1);
                } else {
                    // Other systems use 8-bit controller state
                    let ctrl0 = get_controller_state(window.as_ref(), &settings.input.player1);
                    let ctrl1 = get_controller_state(window.as_ref(), &settings.input.player2);
                    // Note: Player 3 and 4 would be ctrl2 and ctrl3 for systems that support them
                    sys.set_controller(0, ctrl0);
                    sys.set_controller(1, ctrl1);
                }
            }

            // Step one frame and display
            match sys.step_frame() {
                Ok(f) => {
                    buffer = f.pixels; // Move instead of clone

                    // Apply CRT filter if not showing overlays
                    if !popup_manager.is_help_open() && !selector_manager.is_open() {
                        settings.display_filter.apply(&mut buffer, width, height);
                    }

                    // Audio generation: generate samples based on actual frame rate
                    // NTSC: ~60.1 FPS (≈734 samples), PAL: ~50.0 FPS (≈882 samples)
                    let timing = sys.timing();
                    let samples_per_frame =
                        (SAMPLE_RATE as f64 / timing.frame_rate_hz()).round() as usize;
                    let audio_samples = sys.get_audio_samples(samples_per_frame);
                    for s in audio_samples {
                        let _ = audio_tx.try_send(s);
                    }
                }
                Err(e) => eprintln!("Frame generation error: {:?}", e),
            }
        }

        // Prepare frame to present (with overlays if any)
        let slot_selector_buffer;
        let debug_composed_buffer;

        let frame_to_present: &[u32] = if selector_manager.is_open() {
            // Render slot selector overlay
            // For PC system in SAVE mode, show disk persist menu
            if matches!(&sys, EmulatorSystem::PC(_))
                && selector_manager
                    .active_selector
                    .as_ref()
                    .map(|s| s.selector_type == selector::SelectorType::SaveSlot)
                    .unwrap_or(false)
            {
                let mount_points = sys.mount_points();
                let mounted: Vec<bool> = mount_points
                    .iter()
                    .map(|mp| sys.get_disk_image(&mp.id).is_some())
                    .collect();
                slot_selector_buffer =
                    ui_render::create_pc_save_selector(width, height, &mount_points, &mounted);
                &slot_selector_buffer
            } else {
                let has_saves = [
                    game_saves.slots.contains_key(&1),
                    game_saves.slots.contains_key(&2),
                    game_saves.slots.contains_key(&3),
                    game_saves.slots.contains_key(&4),
                    game_saves.slots.contains_key(&5),
                ];
                let mode_str = if selector_manager
                    .active_selector
                    .as_ref()
                    .map(|s| s.selector_type == selector::SelectorType::SaveSlot)
                    .unwrap_or(false)
                {
                    "SAVE"
                } else {
                    "LOAD"
                };
                slot_selector_buffer =
                    ui_render::create_slot_selector_overlay(width, height, mode_str, &has_saves);
                &slot_selector_buffer
            }
        } else if let Some(ref overlay) = debug_overlay {
            // Blend debug overlay with game buffer
            debug_composed_buffer = blend_over(&buffer, overlay);
            &debug_composed_buffer
        } else if let Some(ref overlay) = help_overlay {
            overlay.as_slice()
        } else {
            &buffer
        };

        // Update status bar state
        status_bar.fps = current_fps as f32;
        status_bar.paused = settings.emulation_speed == 0.0;
        status_bar.speed = settings.emulation_speed as f32;

        // Update rendering backend
        status_bar.rendering_backend = if settings.video_backend == "opengl" {
            "OpenGL".to_string()
        } else {
            "Software".to_string()
        };

        // Update IP from system-specific debug info
        status_bar.ip = sys.get_instruction_pointer();

        // Update CPU frequency info
        status_bar.cpu_freq_target = sys.get_cpu_freq_target();
        status_bar.cpu_freq_actual = sys.get_cpu_freq_actual();

        // Update cycles from runtime stats (for systems that support it)
        let stats = sys.get_runtime_stats();
        if stats.cpu_cycles > 0 {
            status_bar.cycles = Some(stats.cpu_cycles as u64);
        } else {
            status_bar.cycles = None;
        }

        // Update menu state based on current emulator state
        menu_bar.update_menu_state(
            rom_loaded,
            settings.emulation_speed == 0.0,
            sys.supports_save_states(),
            logging_active,
        );

        // Update window title with project filename if available
        let title = if let Some(project_name) = runtime_state.get_project_filename() {
            format!("Hemulator - {}", project_name)
        } else {
            "Hemulator - Multi-System Emulator".to_string()
        };
        if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
            let _ = sdl2_backend.set_title(&title);
        }

        // Render frame with UI elements at window resolution
        // UI elements (menu bar, status bar) are always fixed pixel size regardless of game resolution
        if !frame_to_present.is_empty() {
            let (window_width, window_height) = window.get_size();

            const MENU_HEIGHT: usize = 24;
            const STATUS_HEIGHT: usize = 20;

            // Create a buffer at window resolution
            let mut window_buffer = vec![0xFF000000; window_width * window_height];

            // Calculate game display area (middle section between menu and status bar)
            let game_display_y = MENU_HEIGHT;
            let game_display_height = window_height.saturating_sub(MENU_HEIGHT + STATUS_HEIGHT);

            // Scale and blit game content to the middle section
            if game_display_height > 0 {
                // Calculate scaling to fit game in available space while maintaining aspect ratio
                let scale_x = window_width as f32 / width as f32;
                let scale_y = game_display_height as f32 / height as f32;
                let scale = scale_x.min(scale_y);

                let scaled_width = (width as f32 * scale) as usize;
                let scaled_height = (height as f32 * scale) as usize;

                // Center the game horizontally
                let offset_x = (window_width.saturating_sub(scaled_width)) / 2;
                let offset_y =
                    game_display_y + (game_display_height.saturating_sub(scaled_height)) / 2;

                // Optimized nearest-neighbor scaling using integer arithmetic
                // Pre-compute inverse scale as fixed-point (16.16) to avoid per-pixel division
                let scale_inv = (65536.0 / scale) as usize;

                for sy in 0..scaled_height {
                    // Use fixed-point arithmetic instead of float division
                    let src_y = (sy * scale_inv) >> 16;
                    if src_y >= height {
                        continue;
                    }

                    for sx in 0..scaled_width {
                        let src_x = (sx * scale_inv) >> 16;
                        if src_x >= width {
                            continue;
                        }

                        let dst_x = offset_x + sx;
                        let dst_y = offset_y + sy;

                        if dst_x < window_width && dst_y < window_height {
                            let src_idx = src_y * width + src_x;
                            let dst_idx = dst_y * window_width + dst_x;

                            if src_idx < frame_to_present.len() && dst_idx < window_buffer.len() {
                                window_buffer[dst_idx] = frame_to_present[src_idx];
                            }
                        }
                    }
                }
            }

            // Render menu bar at top (fixed 24px height at window resolution)
            menu_bar.render(&mut window_buffer, window_width, window_height);

            // Render status bar at bottom (fixed 20px height at window resolution)
            status_bar.render(&mut window_buffer, window_width, window_height);

            // Update window with the composite buffer
            if let Err(e) = window.update_with_buffer(&window_buffer, window_width, window_height) {
                eprintln!("Window update error: {}", e);
                break;
            }
        }

        // Dynamic frame pacing based on timing mode (NTSC ~60.1 FPS, PAL ~50.0 FPS)
        let frame_dt = last_frame.elapsed();

        frame_times.push(frame_dt);
        if frame_times.len() > 60 {
            frame_times.remove(0);
        }

        if !frame_times.is_empty() {
            let total_time: Duration = frame_times.iter().sum();
            let avg_frame_time = total_time.as_secs_f64() / frame_times.len() as f64;
            if avg_frame_time > 0.0 {
                current_fps = 1.0 / avg_frame_time;
            }
        }

        // Get target frame time from system timing mode, adjusted by emulation speed
        // In benchmark mode, disable frame limiting to measure raw performance
        let target_frame_time = if cli_args.benchmark {
            Duration::from_secs(0) // No frame limiting in benchmark mode
        } else if rom_loaded && settings.emulation_speed > 0.0 {
            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            Duration::from_secs_f64(1.0 / (frame_rate * settings.emulation_speed))
        } else if settings.emulation_speed > 0.0 {
            // Default to NTSC timing when no ROM loaded
            Duration::from_secs_f64(
                1.0 / (emu_core::apu::TimingMode::Ntsc.frame_rate_hz() * settings.emulation_speed),
            )
        } else {
            // When paused (0x speed), use a longer sleep time
            Duration::from_millis(100)
        };

        if frame_dt < target_frame_time {
            std::thread::sleep(target_frame_time - frame_dt);
        }
        last_frame = Instant::now();

        // Save window size if it changed
        let (new_width, new_height) = window.get_size();
        if new_width != settings.window_width || new_height != settings.window_height {
            settings.window_width = new_width;
            settings.window_height = new_height;
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save window size: {}", e);
            }
        }
    }
}
