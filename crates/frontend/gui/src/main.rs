pub mod display_filter;
pub mod egui_ui;
mod hemu_project;
pub mod input;
pub mod input_mapper;
pub mod rom_detect; // Made public so egui_ui can use rom_detect::SystemType (ROM type metadata for the UI)
mod save_state;
mod settings;
mod system_adapter;
mod ui_render;
pub mod video_processor;
pub mod window_backend;

use egui_ui::EguiApp;
use emu_core::{types::Frame, System};
use hemu_project::HemuProject;
use rodio::{DeviceSinkBuilder, Source};
use rom_detect::{
    detect_rom_type_with_extension, is_ps1_bios_file, pc_disk_mount_target, SystemType,
};
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
struct RuntimeState {
    /// Currently loaded .hemu project file path (if any)
    current_project_path: Option<PathBuf>,
    /// Current mount points (mount_id -> file_path)
    /// This is runtime-only and not persisted to config.json
    current_mounts: HashMap<String, String>,
    /// Project-specific input override (when using per-project config)
    /// None means using global config.json settings
    input_override: Option<settings::InputConfig>,
}

impl RuntimeState {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            current_project_path: None,
            current_mounts: HashMap::new(),
            input_override: None,
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
    GBA(Box<hemu_gba::GbaSystem>),
    Atari2600(Box<emu_atari2600::Atari2600System>),
    PC(Box<emu_pc::PcSystem>),
    SNES(Box<emu_snes::SnesSystem>),
    N64(Box<emu_n64::N64System>),
    SMS(Box<emu_sms::SmsSystem>),
    Chip8(Box<emu_chip8::Chip8System>),
    ColecoVision(Box<emu_colecovision::ColecoVisionSystem>),
    SG1000(Box<emu_sg1000::Sg1000System>),
    PS1(Box<emu_ps1::Ps1System>),
    GameAndWatch(Box<emu_gameandwatch::GameAndWatchSystem>),
    Atari5200(Box<emu_atari5200::Atari5200System>),
    MegaDrive(Box<emu_megadrive::MegaDriveSystem>),
}

#[allow(dead_code)]
impl EmulatorSystem {
    fn step_frame(&mut self) -> Result<Frame, Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GBA(sys) => sys
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
            EmulatorSystem::SMS(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Chip8(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::ColecoVision(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SG1000(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PS1(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameAndWatch(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari5200(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::MegaDrive(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    fn reset(&mut self) {
        match self {
            EmulatorSystem::NES(sys) => sys.reset(),
            EmulatorSystem::GameBoy(sys) => sys.reset(),
            EmulatorSystem::GBA(sys) => sys.reset(),
            EmulatorSystem::Atari2600(sys) => sys.reset(),
            EmulatorSystem::PC(sys) => sys.reset(),
            EmulatorSystem::SNES(sys) => sys.reset(),
            EmulatorSystem::N64(sys) => sys.reset(),
            EmulatorSystem::SMS(sys) => sys.reset(),
            EmulatorSystem::Chip8(sys) => sys.reset(),
            EmulatorSystem::ColecoVision(sys) => sys.reset(),
            EmulatorSystem::SG1000(sys) => sys.reset(),
            EmulatorSystem::PS1(sys) => sys.reset(),
            EmulatorSystem::GameAndWatch(sys) => sys.reset(),
            EmulatorSystem::Atari5200(sys) => sys.reset(),
            EmulatorSystem::MegaDrive(sys) => sys.reset(),
        }
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        match self {
            EmulatorSystem::NES(sys) => sys.debugger(),
            EmulatorSystem::GameBoy(sys) => sys.debugger(),
            EmulatorSystem::GBA(sys) => sys.debugger(),
            EmulatorSystem::Atari2600(sys) => sys.debugger(),
            EmulatorSystem::PC(sys) => sys.debugger(),
            EmulatorSystem::SNES(sys) => sys.debugger(),
            EmulatorSystem::N64(sys) => sys.debugger(),
            EmulatorSystem::SMS(sys) => sys.debugger(),
            EmulatorSystem::Chip8(sys) => sys.debugger(),
            EmulatorSystem::ColecoVision(sys) => sys.debugger(),
            EmulatorSystem::SG1000(sys) => sys.debugger(),
            EmulatorSystem::PS1(sys) => sys.debugger(),
            EmulatorSystem::GameAndWatch(sys) => sys.debugger(),
            EmulatorSystem::Atari5200(sys) => sys.debugger(),
            EmulatorSystem::MegaDrive(sys) => sys.debugger(),
        }
    }

    fn get_total_cycles(&self) -> u64 {
        match self {
            EmulatorSystem::NES(sys) => sys.get_total_cycles(),
            EmulatorSystem::GameBoy(sys) => sys.get_total_cycles(),
            EmulatorSystem::GBA(sys) => sys.get_total_cycles(),
            EmulatorSystem::Atari2600(sys) => sys.get_total_cycles(),
            EmulatorSystem::PC(sys) => sys.get_total_cycles(),
            EmulatorSystem::SNES(sys) => sys.get_total_cycles(),
            EmulatorSystem::N64(sys) => sys.get_total_cycles(),
            EmulatorSystem::SMS(sys) => sys.get_total_cycles(),
            EmulatorSystem::Chip8(sys) => sys.get_total_cycles(),
            EmulatorSystem::ColecoVision(sys) => sys.get_total_cycles(),
            EmulatorSystem::SG1000(sys) => sys.get_total_cycles(),
            EmulatorSystem::PS1(sys) => sys.get_total_cycles(),
            EmulatorSystem::GameAndWatch(sys) => sys.get_total_cycles(),
            EmulatorSystem::Atari5200(sys) => sys.get_total_cycles(),
            EmulatorSystem::MegaDrive(sys) => sys.get_total_cycles(),
        }
    }

    #[allow(dead_code)]
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
            EmulatorSystem::GBA(sys) => sys
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
            EmulatorSystem::SMS(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Chip8(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::ColecoVision(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SG1000(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PS1(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameAndWatch(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari5200(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::MegaDrive(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    #[allow(dead_code)]
    fn mount_points(&self) -> Vec<emu_core::MountPointInfo> {
        match self {
            EmulatorSystem::NES(sys) => sys.mount_points(),
            EmulatorSystem::GameBoy(sys) => sys.mount_points(),
            EmulatorSystem::GBA(sys) => sys.mount_points(),
            EmulatorSystem::Atari2600(sys) => sys.mount_points(),
            EmulatorSystem::PC(sys) => sys.mount_points(),
            EmulatorSystem::SNES(sys) => sys.mount_points(),
            EmulatorSystem::N64(sys) => sys.mount_points(),
            EmulatorSystem::SMS(sys) => sys.mount_points(),
            EmulatorSystem::Chip8(sys) => sys.mount_points(),
            EmulatorSystem::ColecoVision(sys) => sys.mount_points(),
            EmulatorSystem::SG1000(sys) => sys.mount_points(),
            EmulatorSystem::PS1(sys) => sys.mount_points(),
            EmulatorSystem::GameAndWatch(sys) => sys.mount_points(),
            EmulatorSystem::Atari5200(sys) => sys.mount_points(),
            EmulatorSystem::MegaDrive(sys) => sys.mount_points(),
        }
    }

    #[allow(dead_code)]
    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GBA(sys) => sys
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
            EmulatorSystem::SMS(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Chip8(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::ColecoVision(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SG1000(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PS1(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameAndWatch(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari5200(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::MegaDrive(sys) => sys
                .unmount(mount_point_id)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    #[allow(dead_code)]
    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::GameBoy(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::GBA(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::Atari2600(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::PC(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::SNES(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::N64(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::SMS(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::Chip8(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::ColecoVision(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::SG1000(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::PS1(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::GameAndWatch(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::Atari5200(sys) => sys.is_mounted(mount_point_id),
            EmulatorSystem::MegaDrive(sys) => sys.is_mounted(mount_point_id),
        }
    }

    /// Check if all required mount points have media loaded
    fn has_required_mounts(&self) -> bool {
        let mount_points = self.mount_points();
        for mp in mount_points {
            if mp.required && !self.is_mounted(&mp.id) {
                return false;
            }
        }
        true
    }

    fn supports_save_states(&self) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.supports_save_states(),
            EmulatorSystem::GameBoy(sys) => sys.supports_save_states(),
            EmulatorSystem::GBA(sys) => sys.supports_save_states(),
            EmulatorSystem::Atari2600(sys) => sys.supports_save_states(),
            EmulatorSystem::PC(sys) => sys.supports_save_states(),
            EmulatorSystem::SNES(sys) => sys.supports_save_states(),
            EmulatorSystem::N64(sys) => sys.supports_save_states(),
            EmulatorSystem::SMS(sys) => sys.supports_save_states(),
            EmulatorSystem::Chip8(sys) => sys.supports_save_states(),
            EmulatorSystem::ColecoVision(sys) => sys.supports_save_states(),
            EmulatorSystem::SG1000(sys) => sys.supports_save_states(),
            EmulatorSystem::PS1(sys) => sys.supports_save_states(),
            EmulatorSystem::GameAndWatch(sys) => sys.supports_save_states(),
            EmulatorSystem::Atari5200(sys) => sys.supports_save_states(),
            EmulatorSystem::MegaDrive(sys) => sys.supports_save_states(),
        }
    }

    fn save_state(&self) -> serde_json::Value {
        match self {
            EmulatorSystem::NES(sys) => sys.save_state(),
            EmulatorSystem::GameBoy(sys) => sys.save_state(),
            EmulatorSystem::GBA(sys) => sys.save_state(),
            EmulatorSystem::Atari2600(sys) => sys.save_state(),
            EmulatorSystem::PC(sys) => sys.save_state(),
            EmulatorSystem::SNES(sys) => sys.save_state(),
            EmulatorSystem::N64(sys) => sys.save_state(),
            EmulatorSystem::SMS(sys) => sys.save_state(),
            EmulatorSystem::Chip8(sys) => sys.save_state(),
            EmulatorSystem::ColecoVision(sys) => sys.save_state(),
            EmulatorSystem::SG1000(sys) => sys.save_state(),
            EmulatorSystem::PS1(sys) => sys.save_state(),
            EmulatorSystem::GameAndWatch(sys) => sys.save_state(),
            EmulatorSystem::Atari5200(sys) => sys.save_state(),
            EmulatorSystem::MegaDrive(sys) => sys.save_state(),
        }
    }

    fn load_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        match self {
            EmulatorSystem::NES(sys) => sys.load_state(state),
            EmulatorSystem::GameBoy(sys) => sys.load_state(state),
            EmulatorSystem::GBA(sys) => sys.load_state(state),
            EmulatorSystem::Atari2600(sys) => sys.load_state(state),
            EmulatorSystem::PC(sys) => sys.load_state(state),
            EmulatorSystem::SNES(sys) => sys.load_state(state),
            EmulatorSystem::N64(sys) => sys.load_state(state),
            EmulatorSystem::SMS(sys) => sys.load_state(state),
            EmulatorSystem::Chip8(sys) => sys.load_state(state),
            EmulatorSystem::ColecoVision(sys) => sys.load_state(state),
            EmulatorSystem::SG1000(sys) => sys.load_state(state),
            EmulatorSystem::PS1(sys) => sys.load_state(state),
            EmulatorSystem::GameAndWatch(sys) => sys.load_state(state),
            EmulatorSystem::Atari5200(sys) => sys.load_state(state),
            EmulatorSystem::MegaDrive(sys) => sys.load_state(state),
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
            EmulatorSystem::GBA(sys) => {
                if port == 0 {
                    // GBA button layout matches the standard frontend mapping:
                    // Bit 0: A, Bit 1: B, Bit 2: Select, Bit 3: Start
                    // Bit 4: Right (was Up in 8-bit), Bit 5: Left (was Down)
                    // Bit 6: Up (was Left), Bit 7: Down (was Right)
                    //
                    // Frontend state (8-bit):
                    // Bit 0: A, Bit 1: B, Bit 2: Select, Bit 3: Start
                    // Bit 4: Up, Bit 5: Down, Bit 6: Left, Bit 7: Right
                    //
                    // GBA KEYINPUT (10-bit):
                    // Bit 0: A, Bit 1: B, Bit 2: Select, Bit 3: Start
                    // Bit 4: Right, Bit 5: Left, Bit 6: Up, Bit 7: Down
                    // Bit 8: R, Bit 9: L
                    let gba_state: u16 = (state as u16 & 0x0F)      // A, B, Select, Start stay
                        | (((state >> 7) & 1) as u16) << 4          // Right (bit 7 -> bit 4)
                        | (((state >> 6) & 1) as u16) << 5          // Left (bit 6 -> bit 5)
                        | (((state >> 4) & 1) as u16) << 6          // Up (bit 4 -> bit 6)
                        | (((state >> 5) & 1) as u16) << 7; // Down (bit 5 -> bit 7)
                                                            // L/R buttons would need extra key bindings; not mapped from 8-bit state
                    sys.set_controller(gba_state);
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
            EmulatorSystem::SMS(sys) => {
                // SMS has 2 controller ports
                // Remap from NES-format (active-high) to SMS port $DC format (active-low)
                // NES bits: A(0) B(1) Select(2) Start(3) Up(4) Down(5) Left(6) Right(7)
                // SMS bits: Up(0) Down(1) Left(2) Right(3) B1(4) B2(5) [6-7 unused for port 1]
                let mut sms_state: u8 = 0xFF; // All released (active-low)
                if state & 0x10 != 0 {
                    sms_state &= !0x01;
                } // Up
                if state & 0x20 != 0 {
                    sms_state &= !0x02;
                } // Down
                if state & 0x40 != 0 {
                    sms_state &= !0x04;
                } // Left
                if state & 0x80 != 0 {
                    sms_state &= !0x08;
                } // Right
                if state & 0x02 != 0 {
                    sms_state &= !0x10;
                } // B -> Button 1 (fire)
                if state & 0x01 != 0 {
                    sms_state &= !0x20;
                } // A -> Button 2
                if port == 0 {
                    sys.set_controller_1(sms_state);
                } else if port == 1 {
                    sys.set_controller_2(sms_state);
                }
            }
            EmulatorSystem::Chip8(_) => {
                // Chip8 uses 16-bit controller state via set_controller_16
                // This 8-bit set_controller is not used for Chip8
            }
            EmulatorSystem::ColecoVision(sys) => {
                // ColecoVision has 2 controller ports
                if port == 0 {
                    sys.set_controller(1, state);
                } else if port == 1 {
                    sys.set_controller(2, state);
                }
            }
            EmulatorSystem::SG1000(sys) => {
                // SG-1000 has 2 controller ports
                if port == 0 {
                    sys.set_controller(1, state);
                } else if port == 1 {
                    sys.set_controller(2, state);
                }
            }
            EmulatorSystem::PS1(_) => {
                // PS1 controller not yet implemented
            }
            EmulatorSystem::GameAndWatch(_) => {
                // Game & Watch uses 16-bit controller via set_controller_16
            }
            EmulatorSystem::Atari5200(sys) => sys.set_controller(port, state),
            EmulatorSystem::MegaDrive(sys) => {
                // Mega Drive 3-button pad (active LOW: 0=pressed, 1=released)
                // MD bits: 0=Up, 1=Down, 2=Left, 3=Right, 4=B, 5=C, 6=A, 7=Start
                // GUI bits: 0=A, 1=B, 2=Select, 3=Start, 4=Up, 5=Down, 6=Left, 7=Right
                let mut md_state: u16 = 0xFFFF; // All released
                if state & 0x10 != 0 {
                    md_state &= !0x01;
                } // Up
                if state & 0x20 != 0 {
                    md_state &= !0x02;
                } // Down
                if state & 0x40 != 0 {
                    md_state &= !0x04;
                } // Left
                if state & 0x80 != 0 {
                    md_state &= !0x08;
                } // Right
                if state & 0x02 != 0 {
                    md_state &= !0x10;
                } // B -> MD B
                if state & 0x04 != 0 {
                    md_state &= !0x20;
                } // Select -> MD C
                if state & 0x01 != 0 {
                    md_state &= !0x40;
                } // A -> MD A
                if state & 0x08 != 0 {
                    md_state &= !0x80;
                } // Start -> MD Start
                if port == 0 {
                    sys.set_controller_1(md_state);
                } else if port == 1 {
                    sys.set_controller_2(md_state);
                }
            }
        }
    }

    fn set_controller_16(&mut self, port: usize, state: u16) {
        match self {
            EmulatorSystem::SNES(sys) => sys.set_controller(port, state),
            EmulatorSystem::Chip8(sys) => sys.set_controller(state),
            EmulatorSystem::GameAndWatch(sys) => sys.set_controller(state),
            _ => {} // Other systems use 8-bit set_controller
        }
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
            EmulatorSystem::GBA(_) => None,
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
            EmulatorSystem::SMS(_) => {
                // Z80 CPU not yet implemented
                None
            }
            EmulatorSystem::Chip8(sys) => {
                let debug = sys.debug_info();
                Some(debug.pc as u32)
            }
            EmulatorSystem::ColecoVision(_) => {
                // Z80 CPU - debugger implementation needed to expose PC
                None
            }
            EmulatorSystem::SG1000(_) => {
                // Z80 CPU - debugger implementation needed to expose PC
                None
            }
            EmulatorSystem::PS1(_) => {
                // R3000A CPU - PC can be exposed here later
                None
            }
            EmulatorSystem::GameAndWatch(sys) => Some(sys.cpu.pc as u32),
            EmulatorSystem::Atari5200(_) => None,
            EmulatorSystem::MegaDrive(_) => None,
        }
    }

    /// Get target CPU frequency in MHz (historical/configured value)
    fn get_cpu_freq_target(&self) -> Option<f64> {
        match self {
            EmulatorSystem::NES(_) => Some(1.79), // NTSC NES CPU (1.789773 MHz)
            EmulatorSystem::GameBoy(_) => Some(4.19), // Game Boy CPU (4.194304 MHz)
            EmulatorSystem::GBA(_) => Some(16.78), // GBA ARM7TDMI (16.78 MHz)
            EmulatorSystem::Atari2600(_) => Some(1.19), // Atari 2600 6507 (1.19 MHz)
            EmulatorSystem::PC(sys) => Some(sys.cpu_speed_mhz()), // Variable based on CPU model
            EmulatorSystem::SNES(_) => Some(3.58), // SNES 65C816 (3.58 MHz)
            EmulatorSystem::N64(_) => Some(93.75), // N64 R4300i (93.75 MHz)
            EmulatorSystem::SMS(_) => Some(3.58), // SMS Z80A (3.58 MHz NTSC)
            EmulatorSystem::Chip8(_) => Some(0.0007), // CHIP-8 runs at ~700 instructions/sec (~0.7 kHz)
            EmulatorSystem::ColecoVision(_) => Some(3.58), // ColecoVision Z80A (3.579545 MHz NTSC)
            EmulatorSystem::SG1000(_) => Some(3.58),  // SG-1000 Z80A (3.579545 MHz NTSC)
            EmulatorSystem::PS1(_) => Some(33.87),    // PS1 R3000A (33.8688 MHz)
            EmulatorSystem::GameAndWatch(_) => Some(0.033), // SM510 (32.768 kHz)
            EmulatorSystem::Atari5200(_) => Some(1.79), // Atari 5200 6502C (1.79 MHz)
            EmulatorSystem::MegaDrive(_) => Some(7.67), // M68000 (7.67 MHz NTSC)
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
            EmulatorSystem::GBA(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Atari2600(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::PC(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::SNES(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::N64(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::SMS(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Chip8(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::ColecoVision(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::SG1000(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::PS1(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::GameAndWatch(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Atari5200(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::MegaDrive(_) => emu_nes::RuntimeStats::default(),
        }
    }

    fn timing(&self) -> emu_core::apu::TimingMode {
        match self {
            EmulatorSystem::NES(sys) => sys.timing(),
            EmulatorSystem::GameBoy(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::GBA(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Atari2600(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::PC(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::SNES(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::N64(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::SMS(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Chip8(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::ColecoVision(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::SG1000(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::PS1(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::GameAndWatch(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Atari5200(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::MegaDrive(_) => emu_core::apu::TimingMode::Ntsc,
        }
    }

    fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        match self {
            EmulatorSystem::NES(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GameBoy(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GBA(sys) => sys.get_audio_samples(count),
            EmulatorSystem::Atari2600(sys) => sys.get_audio_samples(count),
            EmulatorSystem::PC(sys) => sys.get_audio_samples(count),
            EmulatorSystem::SNES(sys) => sys.get_audio_samples(count),
            EmulatorSystem::N64(sys) => sys.get_audio_samples(count),
            EmulatorSystem::Chip8(sys) => {
                // CHIP-8 audio: Simple beep tone when sound timer is active
                if sys.is_sound_playing() {
                    // Generate 440Hz square wave (A4 note)
                    const SAMPLE_RATE: f32 = 44100.0;
                    const FREQUENCY: f32 = 440.0;
                    const AMPLITUDE: i16 = 3000; // Moderate volume to avoid clipping

                    let mut samples = Vec::with_capacity(count);
                    let period_samples = (SAMPLE_RATE / FREQUENCY) as usize;
                    let half_period = period_samples / 2;

                    for i in 0..count {
                        let position = i % period_samples;
                        let sample = if position < half_period {
                            AMPLITUDE
                        } else {
                            -AMPLITUDE
                        };
                        samples.push(sample);
                    }
                    samples
                } else {
                    vec![0; count]
                }
            }
            EmulatorSystem::SMS(sys) => sys.get_audio_samples(count),
            EmulatorSystem::ColecoVision(sys) => sys.get_audio_samples(count),
            EmulatorSystem::SG1000(sys) => sys.get_audio_samples(count),
            EmulatorSystem::PS1(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GameAndWatch(sys) => sys.generate_audio_samples(count),
            EmulatorSystem::Atari5200(sys) => sys.get_audio_samples(count),
            EmulatorSystem::MegaDrive(sys) => sys.get_audio_samples(count),
        }
    }

    fn resolution(&self) -> (usize, usize) {
        match self {
            EmulatorSystem::NES(_) => (256, 240),
            EmulatorSystem::GameBoy(_) => (160, 144),
            EmulatorSystem::GBA(_) => (240, 160),
            EmulatorSystem::Atari2600(_) => (160, 192),
            EmulatorSystem::PC(_) => (320, 200),
            EmulatorSystem::SNES(_) => (256, 224),
            EmulatorSystem::N64(_) => (320, 240),
            EmulatorSystem::SMS(_) => (256, 192),
            EmulatorSystem::Chip8(_) => (64, 32),
            EmulatorSystem::ColecoVision(_) => (256, 192), // TMS9918A resolution
            EmulatorSystem::SG1000(_) => (256, 192),       // TMS9918A resolution
            EmulatorSystem::PS1(_) => (320, 240),          // PS1 standard resolution
            EmulatorSystem::GameAndWatch(_) => (160, 120), // LCD segment grid
            EmulatorSystem::Atari5200(_) => (320, 192),    // ANTIC standard resolution
            EmulatorSystem::MegaDrive(_) => (320, 224),    // Mega Drive standard resolution
        }
    }

    fn system_name(&self) -> &str {
        match self {
            EmulatorSystem::NES(_) => "nes",
            EmulatorSystem::GameBoy(_) => "gameboy",
            EmulatorSystem::GBA(_) => "gba",
            EmulatorSystem::Atari2600(_) => "atari2600",
            EmulatorSystem::PC(_) => "pc",
            EmulatorSystem::SNES(_) => "snes",
            EmulatorSystem::N64(_) => "n64",
            EmulatorSystem::SMS(_) => "sms",
            EmulatorSystem::Chip8(_) => "chip8",
            EmulatorSystem::ColecoVision(_) => "colecovision",
            EmulatorSystem::SG1000(_) => "sg1000",
            EmulatorSystem::PS1(_) => "ps1",
            EmulatorSystem::GameAndWatch(_) => "gameandwatch",
            EmulatorSystem::Atari5200(_) => "atari5200",
            EmulatorSystem::MegaDrive(_) => "megadrive",
        }
    }

    /// Get the SystemType for ROM detection hints
    fn system_type(&self) -> SystemType {
        match self {
            EmulatorSystem::NES(_) => SystemType::NES,
            EmulatorSystem::GameBoy(_) => SystemType::GameBoy,
            EmulatorSystem::GBA(_) => SystemType::GBA,
            EmulatorSystem::Atari2600(_) => SystemType::Atari2600,
            EmulatorSystem::PC(_) => SystemType::PC,
            EmulatorSystem::SNES(_) => SystemType::SNES,
            EmulatorSystem::N64(_) => SystemType::N64,
            EmulatorSystem::SMS(_) => SystemType::SMS,
            EmulatorSystem::Chip8(_) => SystemType::Chip8,
            EmulatorSystem::ColecoVision(_) => SystemType::ColecoVision,
            EmulatorSystem::SG1000(_) => SystemType::SG1000,
            EmulatorSystem::PS1(_) => SystemType::PS1,
            EmulatorSystem::GameAndWatch(_) => SystemType::GameAndWatch,
            EmulatorSystem::Atari5200(_) => SystemType::Atari5200,
            EmulatorSystem::MegaDrive(_) => SystemType::MegaDrive,
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

    /// Get the name of the currently active renderer
    fn get_current_renderer_name(&self) -> String {
        match self {
            EmulatorSystem::NES(nes_sys) => {
                // Get actual renderer name from NES system
                let name = nes_sys.renderer_name();
                if name.contains("OpenGL") {
                    "OpenGL".to_string()
                } else {
                    "Software".to_string()
                }
            }
            EmulatorSystem::GameBoy(_) => "Software".to_string(),
            EmulatorSystem::GBA(_) => "Software".to_string(),
            EmulatorSystem::Atari2600(_) => "Software".to_string(),
            EmulatorSystem::PC(sys) => {
                // PC can use different video adapters
                let adapter_name = sys.video_adapter_name();
                if adapter_name.contains("Hardware") {
                    "OpenGL".to_string()
                } else {
                    "Software".to_string()
                }
            }
            EmulatorSystem::SNES(_) => "Software".to_string(),
            EmulatorSystem::N64(_) => {
                // N64 uses software renderer by default
                // Note: OpenGL renderer would need to be exposed via debug info or separate method
                "Software".to_string()
            }
            EmulatorSystem::SMS(_) => "Software".to_string(),
            EmulatorSystem::Chip8(_) => "Software".to_string(),
            EmulatorSystem::ColecoVision(_) => "Software".to_string(),
            EmulatorSystem::SG1000(_) => "Software".to_string(),
            EmulatorSystem::PS1(_) => "Software".to_string(),
            EmulatorSystem::GameAndWatch(_) => "Software".to_string(),
            EmulatorSystem::Atari5200(_) => "Software".to_string(),
            EmulatorSystem::MegaDrive(_) => "Software".to_string(),
        }
    }

    /// Get the list of available renderers for this system
    /// Returns a vector of renderer names that are available
    fn get_available_renderers(&self) -> Vec<String> {
        match self {
            EmulatorSystem::NES(_) => {
                // OpenGL renderer disabled for now
                vec!["Software".to_string()]
            }
            EmulatorSystem::GameBoy(_) => vec!["Software".to_string()],
            EmulatorSystem::GBA(_) => vec!["Software".to_string()],
            EmulatorSystem::Atari2600(_) => vec!["Software".to_string()],
            EmulatorSystem::PC(_) => {
                // PC has both software and hardware video adapters available
                vec!["Software".to_string(), "OpenGL".to_string()]
            }
            EmulatorSystem::SNES(_) => vec!["Software".to_string()],
            EmulatorSystem::N64(_) => {
                // OpenGL renderer is available when opengl feature is enabled
                #[cfg(feature = "opengl")]
                {
                    vec!["Software".to_string(), "OpenGL".to_string()]
                }
                #[cfg(not(feature = "opengl"))]
                {
                    vec!["Software".to_string()]
                }
            }
            EmulatorSystem::SMS(_) => vec!["Software".to_string()],
            EmulatorSystem::Chip8(_) => vec!["Software".to_string()],
            EmulatorSystem::ColecoVision(_) => vec!["Software".to_string()],
            EmulatorSystem::SG1000(_) => vec!["Software".to_string()],
            EmulatorSystem::PS1(_) => vec!["Software".to_string()],
            EmulatorSystem::GameAndWatch(_) => vec!["Software".to_string()],
            EmulatorSystem::Atari5200(_) => vec!["Software".to_string()],
            EmulatorSystem::MegaDrive(_) => vec!["Software".to_string()],
        }
    }

    /// Check if the current PC is at a breakpoint
    /// Returns Some(pc) if a breakpoint is hit, None otherwise
    fn check_breakpoint(&self) -> Option<u32> {
        match self {
            EmulatorSystem::NES(sys) => sys.check_breakpoint(),
            EmulatorSystem::GameBoy(sys) => sys.check_breakpoint(),
            EmulatorSystem::GBA(_) => None,
            EmulatorSystem::Atari2600(sys) => sys.check_breakpoint(),
            EmulatorSystem::PC(sys) => sys.check_breakpoint(),
            EmulatorSystem::SNES(sys) => sys.check_breakpoint(),
            EmulatorSystem::N64(sys) => sys.check_breakpoint(),
            EmulatorSystem::SMS(sys) => sys.check_breakpoint(),
            EmulatorSystem::Chip8(sys) => sys.check_breakpoint(),
            EmulatorSystem::ColecoVision(sys) => sys.check_breakpoint(),
            EmulatorSystem::SG1000(sys) => sys.check_breakpoint(),
            EmulatorSystem::PS1(_) => None,
            EmulatorSystem::GameAndWatch(_) => None,
            EmulatorSystem::Atari5200(sys) => sys.check_breakpoint(),
            EmulatorSystem::MegaDrive(sys) => sys.check_breakpoint(),
        }
    }

    /// Get all active breakpoints
    fn get_breakpoints(&self) -> Vec<emu_core::breakpoints::Breakpoint> {
        match self {
            EmulatorSystem::NES(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::GameBoy(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::GBA(_) => Vec::new(),
            EmulatorSystem::Atari2600(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::PC(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::SNES(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::N64(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::SMS(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::Chip8(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::ColecoVision(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::SG1000(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::PS1(_) => Vec::new(),
            EmulatorSystem::GameAndWatch(_) => Vec::new(),
            EmulatorSystem::Atari5200(sys) => sys.get_breakpoint_manager().get_all(),
            EmulatorSystem::MegaDrive(sys) => sys.get_breakpoint_manager().get_all(),
        }
    }

    /// Get the instruction tracer for dumping trace to file
    fn get_instruction_tracer(&self) -> Option<&emu_core::instruction_tracer::InstructionTracer> {
        match self {
            EmulatorSystem::NES(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::GameBoy(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::GBA(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::Atari2600(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::PC(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::SNES(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::N64(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::SMS(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::Chip8(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::ColecoVision(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::SG1000(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::PS1(_) => None,
            EmulatorSystem::GameAndWatch(_) => None,
            EmulatorSystem::Atari5200(sys) => Some(sys.get_instruction_tracer()),
            EmulatorSystem::MegaDrive(sys) => Some(sys.get_instruction_tracer()),
        }
    }

    /// Get mutable access to the instruction tracer
    fn instruction_tracer_mut(
        &mut self,
    ) -> Option<&mut emu_core::instruction_tracer::InstructionTracer> {
        match self {
            EmulatorSystem::NES(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::GameBoy(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::GBA(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::Atari2600(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::PC(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::SNES(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::N64(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::SMS(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::Chip8(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::ColecoVision(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::SG1000(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::PS1(_) => None,
            EmulatorSystem::GameAndWatch(_) => None,
            EmulatorSystem::Atari5200(sys) => Some(sys.get_instruction_tracer_mut()),
            EmulatorSystem::MegaDrive(sys) => Some(sys.get_instruction_tracer_mut()),
        }
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

/// Get CHIP-8 controller state from current keyboard state (16-bit for 16-key hexadecimal keypad)
///
/// CHIP-8 has a 16-key hexadecimal keypad (0x0-0xF):
/// Original layout:
///   1 2 3 C
///   4 5 6 D
///   7 8 9 E
///   A 0 B F
///
/// Commonly mapped to QWERTY keyboard:
///   1 2 3 4  ->  1 2 3 C
///   Q W E R  ->  4 5 6 D
///   A S D F  ->  7 8 9 E
///   Z X C V  ->  A 0 B F
///
/// Returns a 16-bit value where bit N represents key N (0x0-0xF)
fn get_chip8_controller_state(window: &dyn WindowBackend) -> u16 {
    let mut state: u16 = 0;

    // Map keyboard keys to CHIP-8 hex keypad
    // Row 1: 1 2 3 4 -> keys 1 2 3 C
    if window.is_key_down(Key::Key1) {
        state |= 1 << 0x1;
    }
    if window.is_key_down(Key::Key2) {
        state |= 1 << 0x2;
    }
    if window.is_key_down(Key::Key3) {
        state |= 1 << 0x3;
    }
    if window.is_key_down(Key::Key4) {
        state |= 1 << 0xC;
    }

    // Row 2: Q W E R -> keys 4 5 6 D
    if window.is_key_down(Key::Q) {
        state |= 1 << 0x4;
    }
    if window.is_key_down(Key::W) {
        state |= 1 << 0x5;
    }
    if window.is_key_down(Key::E) {
        state |= 1 << 0x6;
    }
    if window.is_key_down(Key::R) {
        state |= 1 << 0xD;
    }

    // Row 3: A S D F -> keys 7 8 9 E
    if window.is_key_down(Key::A) {
        state |= 1 << 0x7;
    }
    if window.is_key_down(Key::S) {
        state |= 1 << 0x8;
    }
    if window.is_key_down(Key::D) {
        state |= 1 << 0x9;
    }
    if window.is_key_down(Key::F) {
        state |= 1 << 0xE;
    }

    // Row 4: Z X C V -> keys A 0 B F
    if window.is_key_down(Key::Z) {
        state |= 1 << 0xA;
    }
    if window.is_key_down(Key::X) {
        state |= 1 << 0x0;
    }
    if window.is_key_down(Key::C) {
        state |= 1 << 0xB;
    }
    if window.is_key_down(Key::V) {
        state |= 1 << 0xF;
    }

    state
}

/// Get Game & Watch controller state using .mgw button encoding (8-bit).
///
/// Button encoding matches the LCD-Game-Shrinker / gw-libretro format:
///   Bit 0: LEFT   (Arrow Left)
///   Bit 1: UP     (Arrow Up)
///   Bit 2: RIGHT  (Arrow Right)
///   Bit 3: DOWN   (Arrow Down)
///   Bit 4: A      (Z key — Game A / primary action)
///   Bit 5: B      (X key — Game B / secondary action)
///   Bit 6: TIME   (T key)
///   Bit 7: GAME   (G key — Game select / Alarm)
fn get_gw_controller_state(window: &dyn WindowBackend) -> u16 {
    let mut state: u16 = 0;

    if window.is_key_down(Key::Left) {
        state |= 0x01;
    }
    if window.is_key_down(Key::Up) {
        state |= 0x02;
    }
    if window.is_key_down(Key::Right) {
        state |= 0x04;
    }
    if window.is_key_down(Key::Down) {
        state |= 0x08;
    }
    if window.is_key_down(Key::Z) {
        state |= 0x10; // A
    }
    if window.is_key_down(Key::X) {
        state |= 0x20; // B
    }
    if window.is_key_down(Key::T) {
        state |= 0x40; // TIME
    }
    if window.is_key_down(Key::G) {
        state |= 0x80; // GAME
    }

    state
}

/// Get ColecoVision controller state from current keyboard state (8-bit)
///
/// ColecoVision controller layout:
/// - Joystick: 4 directions (bits 4-7)
/// - Fire Button A (left side): bit 0
/// - Fire Button B (right side): bit 1
/// - Start: bit 3
/// - Select: bit 2
///
/// Note: The ColecoVision hardware also has a 12-key numeric keypad (0-9, *, #)
/// which is read separately from the main controller state. For now, this function
/// only handles the joystick and fire buttons.
///
/// Player 1 mapping (Problem statement: Arrow keys for joystick, Left Shift and Enter for buttons A and B):
/// - Arrow Keys: Joystick directions
/// - Left Shift: Fire Button A
/// - Enter: Fire Button B
///
/// Player 2 mapping (IJKL for joystick, Right Shift and P for buttons):
/// - I/J/K/L: Joystick directions (I=Up, K=Down, J=Left, L=Right)
/// - Right Shift: Fire Button A
/// - P: Fire Button B
fn get_colecovision_controller_state(
    window: &dyn WindowBackend,
    mapping: &settings::KeyMapping,
) -> u8 {
    let mut state: u8 = 0;

    // Fire buttons (A and B)
    if let Some(key) = string_to_key(&mapping.a) {
        if window.is_key_down(key) {
            state |= 1 << 0; // Bit 0: Fire Button A
        }
    }
    if let Some(key) = string_to_key(&mapping.b) {
        if window.is_key_down(key) {
            state |= 1 << 1; // Bit 1: Fire Button B
        }
    }

    // Select and Start (though not commonly used in ColecoVision)
    if let Some(key) = string_to_key(&mapping.select) {
        if window.is_key_down(key) {
            state |= 1 << 2; // Bit 2: Select
        }
    }
    if let Some(key) = string_to_key(&mapping.start) {
        if window.is_key_down(key) {
            state |= 1 << 3; // Bit 3: Start
        }
    }

    // Joystick directions
    if let Some(key) = string_to_key(&mapping.up) {
        if window.is_key_down(key) {
            state |= 1 << 4; // Bit 4: Up
        }
    }
    if let Some(key) = string_to_key(&mapping.down) {
        if window.is_key_down(key) {
            state |= 1 << 5; // Bit 5: Down
        }
    }
    if let Some(key) = string_to_key(&mapping.left) {
        if window.is_key_down(key) {
            state |= 1 << 6; // Bit 6: Left
        }
    }
    if let Some(key) = string_to_key(&mapping.right) {
        if window.is_key_down(key) {
            state |= 1 << 7; // Bit 7: Right
        }
    }

    state
}

/// Streaming audio source backed by a channel. When there's no data, it outputs silence to avoid
/// underruns.
struct StreamSource {
    rx: Receiver<i16>,
    sample_rate: u32,
    channels: u16,
}

impl Iterator for StreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.rx.try_recv().unwrap_or(0);
        Some(s as f32 / 32768.0)
    }
}

impl Source for StreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::new(self.channels).unwrap()
    }

    fn sample_rate(&self) -> std::num::NonZero<u32> {
        std::num::NonZero::new(self.sample_rate).unwrap()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Save current emulation state to a .hemu project file
/// Works for all systems, not just PC
#[allow(dead_code)]
fn save_project(
    sys: &EmulatorSystem,
    runtime_state: &RuntimeState,
    settings: &Settings,
    status_message: &mut String,
    current_project_path: Option<&PathBuf>,
) -> Option<String> {
    // Show file save dialog with default path if available
    let default_name = format!("{}_project.hemu", sys.system_name());
    let mut dialog = rfd::FileDialog::new().add_filter("Hemulator Project", &["hemu"]);

    // If there's a current project path, use it as the default location
    if let Some(current_path) = current_project_path {
        if let Some(dir) = current_path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file_name) = current_path.file_name() {
            dialog = dialog.set_file_name(file_name.to_string_lossy().as_ref());
        }
    } else {
        dialog = dialog.set_file_name(&default_name);
    }

    if let Some(path) = dialog.save_file() {
        let mut project = HemuProject::new(sys.system_name().to_string());

        // Copy current mount points from runtime state
        // Filter to only include mounts relevant to this system
        // Get system name first to avoid borrowing issue
        let system_name = sys.system_name();
        let relevant_mounts: Vec<&str> = match system_name {
            "pc" => vec!["BIOS", "FloppyA", "FloppyB", "HardDrive"],
            "atari5200" => vec!["BIOS", "Cartridge"],
            "nes" | "gameboy" | "gba" | "atari2600" | "snes" | "n64" | "megadrive" => {
                vec!["Cartridge"]
            }
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

        // Save project-specific input override if it exists
        if let Some(ref input_override) = runtime_state.input_override {
            project.set_input_override(input_override.clone());
        }

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
                let path_str = path.to_string_lossy().to_string();
                println!("Project saved to: {}", path.display());
                *status_message = format!(
                    "Project saved: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                return Some(path_str);
            }
            Err(e) => {
                eprintln!("Failed to save project: {}", e);
                *status_message = format!("Failed to save project: {}", e);
            }
        }
    }
    None
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
    // Get current local time
    let now = Local::now();

    // Generate random number 000-999
    let random = rand::random_range(0u32..1000);

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

/// Enable OpenGL renderer for N64 systems if the opengl feature is enabled
/// This should be called after creating any new N64System instance
/// NOTE: This function is now a stub since N64 systems are initialized with GL context
/// at construction time. The renderer is determined during system creation.
#[cfg(feature = "opengl")]
fn enable_n64_opengl_renderer(
    sys: &mut EmulatorSystem,
    _backend: &Sdl2EguiBackend,
) -> Option<String> {
    if let EmulatorSystem::N64(n64_sys) = sys {
        // N64 systems already have their renderer initialized at construction
        // Just return the current renderer name
        Some(n64_sys.renderer_name().to_string())
    } else {
        None
    }
}

/// Stub for when opengl feature is not enabled
#[cfg(not(feature = "opengl"))]
fn enable_n64_opengl_renderer(
    _sys: &mut EmulatorSystem,
    _backend: &Sdl2EguiBackend,
) -> Option<String> {
    None
}

/// Create a file dialog with individual filters for each file type plus an "All Files" option
/// This improves the user experience by allowing them to filter by specific file types
#[allow(dead_code)]
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
    bios_path: Option<String>, // BIOS file path (for systems that support BIOS)
    system: Option<String>,    // System to start (pc, nes, gb, atari2600, snes, n64)
    slot1: Option<String>,     // BIOS or primary file
    slot2: Option<String>,     // FloppyA
    slot3: Option<String>,     // FloppyB
    slot4: Option<String>,     // HardDrive
    slot5: Option<String>,     // Reserved for future use
    create_blank_disk: Option<(String, String)>, // (path, format)
    show_help: bool,           // Show help message
    show_version: bool,        // Show version
    benchmark: bool,           // Benchmark mode: disable frame limiter to measure raw performance
    no_gui: bool,              // No-GUI mode: run in a plain SDL2 window without egui overlay
    // Logging configuration
    log_level: Option<String>,      // Global log level
    log_cpu: Option<String>,        // CPU log level
    log_bus: Option<String>,        // Bus log level
    log_ppu: Option<String>,        // PPU log level
    log_apu: Option<String>,        // APU log level
    log_interrupts: Option<String>, // Interrupt log level
    log_stubs: Option<String>,      // Stub/unimplemented log level
    log_file: Option<String>,       // Log file path
    // Debug dump configuration
    debug_dump_pc: Option<u32>,      // PC value to trigger debug dump
    debug_dump_cycles: Option<u64>,  // Cycle count to trigger debug dump
    debug_dump_file: Option<String>, // Output file for debug dump (default: debug_dump.txt)
    // Instruction tracing configuration
    trace_instructions: bool,        // Enable instruction tracing
    trace_limit: Option<usize>,      // Max instructions to keep in trace buffer
    trace_dump_file: Option<String>, // File to dump trace on breakpoint/exit
    // Breakpoint configuration
    breakpoints: Vec<u32>,       // List of execution breakpoint addresses
    read_breakpoints: Vec<u32>,  // List of read breakpoint addresses
    write_breakpoints: Vec<u32>, // List of write breakpoint addresses
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
                "--no-gui" => {
                    args.no_gui = true;
                }
                "--system" | "-S" => {
                    if let Some(system) = arg_iter.next() {
                        args.system = Some(system);
                    } else {
                        eprintln!(
                            "Error: --system requires a value (pc, nes, gb, gba, atari2600, atari5200, snes, n64)."
                        );
                        std::process::exit(1);
                    }
                }
                "--bios" => {
                    if let Some(path) = arg_iter.next() {
                        args.bios_path = Some(path);
                    } else {
                        eprintln!("Error: --bios requires a file path.");
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
                "--debug-dump-pc" => {
                    if let Some(value) = arg_iter.next() {
                        let pc = if value.starts_with("0x") || value.starts_with("0X") {
                            u32::from_str_radix(&value[2..], 16)
                        } else {
                            value.parse::<u32>()
                        };
                        match pc {
                            Ok(pc_value) => args.debug_dump_pc = Some(pc_value),
                            Err(_) => {
                                eprintln!("Error: --debug-dump-pc requires a valid number (hex: 0x8000 or decimal: 32768).");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --debug-dump-pc requires a PC value.");
                        std::process::exit(1);
                    }
                }
                "--debug-dump-cycles" => {
                    if let Some(value) = arg_iter.next() {
                        match value.parse::<u64>() {
                            Ok(cycles) => args.debug_dump_cycles = Some(cycles),
                            Err(_) => {
                                eprintln!("Error: --debug-dump-cycles requires a valid number.");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --debug-dump-cycles requires a cycle count.");
                        std::process::exit(1);
                    }
                }
                "--debug-dump-file" => {
                    if let Some(path) = arg_iter.next() {
                        args.debug_dump_file = Some(path);
                    } else {
                        eprintln!("Error: --debug-dump-file requires a file path.");
                        std::process::exit(1);
                    }
                }
                "--trace-instructions" => {
                    args.trace_instructions = true;
                }
                "--trace-limit" => {
                    if let Some(value) = arg_iter.next() {
                        match value.parse::<usize>() {
                            Ok(limit) => args.trace_limit = Some(limit),
                            Err(_) => {
                                eprintln!("Error: --trace-limit requires a valid number.");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --trace-limit requires a number.");
                        std::process::exit(1);
                    }
                }
                "--trace-dump-file" => {
                    if let Some(path) = arg_iter.next() {
                        args.trace_dump_file = Some(path);
                    } else {
                        eprintln!("Error: --trace-dump-file requires a file path.");
                        std::process::exit(1);
                    }
                }
                "--breakpoint" | "-b" => {
                    if let Some(value) = arg_iter.next() {
                        let addr = if value.starts_with("0x") || value.starts_with("0X") {
                            u32::from_str_radix(&value[2..], 16)
                        } else {
                            value.parse::<u32>()
                        };
                        match addr {
                            Ok(address) => args.breakpoints.push(address),
                            Err(_) => {
                                eprintln!("Error: --breakpoint requires a valid address (hex: 0x8000 or decimal: 32768).");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --breakpoint requires an address.");
                        std::process::exit(1);
                    }
                }
                "--read-breakpoint" | "-r" => {
                    if let Some(value) = arg_iter.next() {
                        let addr = if value.starts_with("0x") || value.starts_with("0X") {
                            u32::from_str_radix(&value[2..], 16)
                        } else {
                            value.parse::<u32>()
                        };
                        match addr {
                            Ok(address) => args.read_breakpoints.push(address),
                            Err(_) => {
                                eprintln!("Error: --read-breakpoint requires a valid address (hex: 0x2000 or decimal: 8192).");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --read-breakpoint requires an address.");
                        std::process::exit(1);
                    }
                }
                "--write-breakpoint" | "-w" => {
                    if let Some(value) = arg_iter.next() {
                        let addr = if value.starts_with("0x") || value.starts_with("0X") {
                            u32::from_str_radix(&value[2..], 16)
                        } else {
                            value.parse::<u32>()
                        };
                        match addr {
                            Ok(address) => args.write_breakpoints.push(address),
                            Err(_) => {
                                eprintln!("Error: --write-breakpoint requires a valid address (hex: 0x2000 or decimal: 8192).");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --write-breakpoint requires an address.");
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
            "  --no-gui                 Run in a plain SDL2 window without the egui overlay (faster startup, minimal UI)"
        );
        eprintln!(
            "  -S, --system <SYSTEM>    Start clean system (pc, nes, gb, gba, atari2600, atari5200, snes, n64)"
        );
        eprintln!("  --bios <file>            Load BIOS file (for PS1, ColecoVision, SMS, PC)");
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
        eprintln!("Debug Dump Options:");
        eprintln!("  --debug-dump-pc <PC>     Dump debug info when PC reaches value (hex: 0x8000 or decimal)");
        eprintln!("  --debug-dump-cycles <N>  Dump debug info after N cycles");
        eprintln!(
            "  --debug-dump-file <PATH> Output file for debug dump (default: debug_dump.txt)"
        );
        eprintln!("                           Dumps full disassembly and memory contents");
        eprintln!();
        eprintln!("Instruction Tracing Options:");
        eprintln!("  --trace-instructions     Enable instruction tracing");
        eprintln!("                           Supported for all systems: NES, Game Boy, GBA, Atari 2600, SMS, SNES, CHIP-8, N64, ColecoVision, SG-1000, and PC.");
        eprintln!(
            "  --trace-limit <N>        Max instructions to keep in trace buffer (default: 10,000)"
        );
        eprintln!("                           Note: Configurable at runtime via --trace-limit (default: 10,000 instructions).");
        eprintln!("                           Higher limits provide more history but consume more RAM (~32 bytes per instruction).");
        eprintln!("  --trace-dump-file <PATH> File to dump trace (default: trace_dump.txt)");
        eprintln!("                           Automatically dumps when breakpoint is hit or debug dump is triggered.");
        eprintln!(
            "  -b, --breakpoint <ADDR>  Set execution breakpoint at address (can be used multiple times)"
        );
        eprintln!("                           Supported for all systems except GBA. Stops execution and dumps trace when hit.");
        eprintln!(
            "  -r, --read-breakpoint <ADDR>  Set read breakpoint at address (can be used multiple times)"
        );
        eprintln!("                           Breaks when memory at address is read. Useful for debugging memory corruption.");
        eprintln!(
            "  -w, --write-breakpoint <ADDR> Set write breakpoint at address (can be used multiple times)"
        );
        eprintln!("                           Breaks when memory at address is written. Useful for tracking variable changes.");
        eprintln!();
        eprintln!("Disk formats:");
        eprintln!("  360k, 720k, 1.2m, 1.44m  Floppy disk formats");
        eprintln!("  20m, 250m, 1g, 20g       Hard drive formats");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  hemu game.nes                                  # Load NES ROM (auto-detect)");
        eprintln!(
            "  hemu --no-gui game.nes                         # Load NES ROM without GUI overlay"
        );
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
            "  hemu --debug-dump-pc 0x8000 game.nes           # Dump debug info when PC=0x8000"
        );
        eprintln!(
            "  hemu --debug-dump-cycles 10000 game.nes        # Dump debug info after 10000 cycles"
        );
        eprintln!("  hemu --trace-instructions --breakpoint 0x8100 game.sfc");
        eprintln!(
            "                                                 # Trace SNES execution until breakpoint (auto-dumps trace)"
        );
        eprintln!("  hemu --trace-instructions --trace-limit 5000 game.nes");
        eprintln!(
            "                                                 # Keep last 5000 instructions in trace"
        );
        eprintln!(
            "  hemu --slot2 disk.img                          # Load PC with floppy in drive A"
        );
        eprintln!(
            "  hemu --slot2 boot.img --slot4 hdd.img         # Load PC with floppy and hard drive"
        );
        eprintln!("  hemu --bios coleco.rom game.col                # Load ColecoVision with BIOS");
        eprintln!("  hemu --bios bios.sms game.sms                  # Load SMS with BIOS");
        eprintln!("  hemu --bios 5200.rom game.a52                  # Load Atari 5200 with BIOS");
        eprintln!("  hemu --bios custom.bin --slot2 boot.img       # Load PC with custom BIOS");
        eprintln!("  hemu --create-blank-disk floppy.img 1.44m      # Create 1.44MB floppy image");
        eprintln!(
            "  hemu --create-blank-disk hdd.img 20m           # Create 20MB hard drive image"
        );
    }

    /// Print version information
    fn print_version() {
        println!("Hemulator v{}", env!("CARGO_PKG_VERSION"));
        println!("Multi-System Emulator");
        println!("Supported systems: NES, Game Boy, Atari 2600, Atari 5200, Mega Drive, PC/DOS, SNES, N64");
    }
}

/// Create a NES system with the appropriate renderer based on settings
fn create_nes_system(
    video_backend: &str,
    _gl_context: Option<glow::Context>,
) -> emu_nes::NesSystem {
    #[allow(unused_mut)] // mut needed when opengl feature is enabled
    let mut nes_sys = emu_nes::NesSystem::default();

    // Enable OpenGL renderer if requested and context is available
    if video_backend == "opengl" {
        #[cfg(feature = "opengl")]
        if let Some(gl) = _gl_context {
            match nes_sys.enable_opengl_renderer(gl) {
                Ok(()) => {
                    eprintln!("NES: Using OpenGL (hardware) renderer");
                }
                Err(e) => {
                    eprintln!(
                        "Failed to enable OpenGL NES renderer: {}, using software",
                        e
                    );
                }
            }
        } else {
            eprintln!(
                "Warning: OpenGL renderer requested but GL context not available - using software"
            );
        }
        #[cfg(not(feature = "opengl"))]
        {
            eprintln!("Warning: OpenGL feature not enabled - using software renderer");
        }
    }

    nes_sys
}

/// Create an N64 system with OpenGL hardware renderer
/// Returns an error if GL context is not available or renderer initialization fails
fn create_n64_system(
    gl_context: Option<glow::Context>,
    settings: &Settings,
) -> Result<emu_n64::N64System, String> {
    if let Some(gl) = gl_context {
        let mut system = emu_n64::N64System::new(gl)?;

        // Apply system-specific settings from config.json "extra" field
        // Example: "n64_frame_cycles": 100000 in config.json
        if let Some(cycles_value) = settings.extra.get("n64_frame_cycles") {
            if let Some(cycles) = cycles_value.as_u64() {
                system.set_frame_cycles(cycles as u32);
            }
        }

        Ok(system)
    } else {
        Err("OpenGL context required for N64 emulation: no GL context was created by the frontend or windowing system. \
Please ensure your system supports hardware-accelerated OpenGL and that the graphics/video backend is configured to create an OpenGL context for the N64 renderer.".to_string())
    }
}

/// Create an Atari 2600 system
fn create_atari2600_system(_settings: &Settings) -> emu_atari2600::Atari2600System {
    emu_atari2600::Atari2600System::new()
}

fn create_atari5200_system(_settings: &Settings) -> emu_atari5200::Atari5200System {
    emu_atari5200::Atari5200System::new()
}

fn create_megadrive_system(_settings: &Settings) -> emu_megadrive::MegaDriveSystem {
    emu_megadrive::MegaDriveSystem::new()
}

/// Helper function to load BIOS from CLI argument or auto-search in ROM directory
/// Returns (bios_data, bios_path) if found, None otherwise
fn load_bios(
    cli_bios_path: Option<&String>,
    rom_path_for_search: Option<&str>,
    bios_filenames: &[&str],
    expected_size: Option<usize>,
) -> Option<(Vec<u8>, String)> {
    // First, try CLI-provided BIOS path
    if let Some(bios_path) = cli_bios_path {
        match std::fs::read(bios_path) {
            Ok(bios_data) => {
                // Verify size if specified
                if let Some(size) = expected_size {
                    if bios_data.len() != size {
                        eprintln!(
                            "Warning: BIOS file {} has unexpected size {} (expected {})",
                            bios_path,
                            bios_data.len(),
                            size
                        );
                        return None;
                    }
                }
                println!("Loaded BIOS from: {}", bios_path);
                return Some((bios_data, bios_path.clone()));
            }
            Err(e) => {
                eprintln!("Error loading BIOS from {}: {}", bios_path, e);
                return None;
            }
        }
    }

    // If no CLI path provided, try auto-search in ROM directory
    if let Some(rom_path) = rom_path_for_search {
        if let Ok(rom_path_abs) = std::path::Path::new(rom_path).canonicalize() {
            if let Some(parent_dir) = rom_path_abs.parent() {
                for candidate in bios_filenames {
                    let bios_path = parent_dir.join(candidate);
                    if bios_path.exists() {
                        if let Ok(bios_data) = std::fs::read(&bios_path) {
                            // Verify size if specified
                            if let Some(size) = expected_size {
                                if bios_data.len() != size {
                                    continue; // Try next candidate
                                }
                            }
                            println!("Auto-detected BIOS from: {}", bios_path.display());
                            return Some((bios_data, bios_path.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Helper to update mount_info in tab_manager after mount/unmount operations
fn update_tab_mount_info(
    egui_app: &mut egui_ui::EguiApp,
    sys: &EmulatorSystem,
    runtime_state: &RuntimeState,
) {
    let mount_points = sys.mount_points();
    let mount_info: Vec<egui_ui::MountInfo> = mount_points
        .into_iter()
        .map(|mp| {
            let mounted_file = runtime_state.current_mounts.get(&mp.id).cloned();
            egui_ui::MountInfo {
                id: mp.id,
                name: mp.name,
                extensions: mp.extensions,
                required: mp.required,
                mounted_file,
            }
        })
        .collect();
    egui_app.tab_manager.update_mount_info(mount_info);
}

/// Helper to configure UI state after system creation
/// This consolidates common state updates that happen for all systems
fn configure_system_ui(
    egui_app: &mut egui_ui::EguiApp,
    sys: &EmulatorSystem,
    system_name: &str,
    rom_loaded: &mut bool,
    status_message: &str,
    runtime_state: &RuntimeState,
) {
    *rom_loaded = true;
    egui_app.property_pane.system_name = system_name.to_string();
    egui_app.property_pane.rendering_backend = sys.get_current_renderer_name();
    egui_app.property_pane.available_renderers = sys.get_available_renderers();
    egui_app.status_bar.set_message(status_message.to_string());

    // Update tab_manager state immediately so welcome screen shows correctly
    egui_app.set_system_loaded(*rom_loaded, system_name);

    // Update mount_info immediately so the welcome screen can check if cartridge is needed
    update_tab_mount_info(egui_app, sys, runtime_state);
}

/// Helper function to create EnhancedDebugState from a Debugger
fn create_enhanced_debug_state(
    system_name: &str,
    debugger: &dyn emu_core::debug::Debugger,
    system: &EmulatorSystem,
) -> system_adapter::EnhancedDebugState {
    let cpu_state = debugger.get_cpu_state();
    let memory_regions = debugger.get_memory_regions();

    // Disassemble instructions around the current PC.
    //
    // Note: this is an *approximate* centering:
    // - We start 32 bytes before PC (clamped to 0 via `saturating_sub`).
    // - We request 20 instructions from that starting address.
    //
    // On CPUs with variable-length instructions (e.g. 1–3 bytes), this does
    // not guarantee that PC will appear exactly in the middle of the rendered
    // disassembly, and early addresses (PC < 32) cannot be centered at all.
    // This trade-off keeps the implementation simple while still providing
    // useful context around the current PC.
    let pc = cpu_state.pc;
    let disassembly = debugger.disassemble_range(pc.saturating_sub(32), 20);

    let mut enhanced_state = system_adapter::EnhancedDebugState::new(system_name);
    enhanced_state.cpu_state = Some(cpu_state);
    enhanced_state.memory_regions = memory_regions;
    enhanced_state.disassembly = disassembly;
    enhanced_state.current_pc = pc;
    enhanced_state.breakpoints = system.get_breakpoints();

    enhanced_state
}

/// Apply CLI debugging options to a system
fn apply_debug_options(sys: &mut EmulatorSystem, cli_args: &CliArgs) {
    // Enable instruction tracing if requested
    if cli_args.trace_instructions {
        match sys {
            EmulatorSystem::NES(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::GameBoy(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::GBA(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::Atari2600(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::Chip8(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::SMS(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::SNES(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::N64(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::PC(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::ColecoVision(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::SG1000(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::PS1(_) => {
                // PS1 instruction tracing not yet implemented
            }
            EmulatorSystem::GameAndWatch(_) => {
                // Game & Watch instruction tracing not yet implemented
            }
            EmulatorSystem::Atari5200(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
            EmulatorSystem::MegaDrive(s) => {
                s.set_instruction_tracing(true);
                if let Some(limit) = cli_args.trace_limit {
                    s.get_instruction_tracer_mut().set_max_history(limit);
                }
            }
        }
    }

    // Add breakpoints
    for &addr in &cli_args.breakpoints {
        match sys {
            EmulatorSystem::NES(s) => s.add_breakpoint(addr),
            EmulatorSystem::GameBoy(s) => s.add_breakpoint(addr),
            EmulatorSystem::GBA(_) => {}
            EmulatorSystem::Atari2600(s) => s.add_breakpoint(addr),
            EmulatorSystem::Chip8(s) => s.add_breakpoint(addr),
            EmulatorSystem::SMS(s) => s.add_breakpoint(addr),
            EmulatorSystem::SNES(s) => s.add_breakpoint(addr),
            EmulatorSystem::N64(s) => s.add_breakpoint(addr),
            EmulatorSystem::PC(s) => s.add_breakpoint(addr),
            EmulatorSystem::ColecoVision(s) => s.add_breakpoint(addr),
            EmulatorSystem::SG1000(s) => s.add_breakpoint(addr),
            EmulatorSystem::PS1(_) => {} // PS1 breakpoints not yet implemented
            EmulatorSystem::GameAndWatch(_) => {} // Game & Watch breakpoints not yet implemented
            EmulatorSystem::Atari5200(s) => s.add_breakpoint(addr),
            EmulatorSystem::MegaDrive(s) => s.add_breakpoint(addr),
        }
    }

    // Add read breakpoints
    for &addr in &cli_args.read_breakpoints {
        match sys {
            EmulatorSystem::NES(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::GameBoy(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::GBA(_) => {}
            EmulatorSystem::Atari2600(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::Chip8(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::SMS(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::SNES(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::N64(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::PC(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::ColecoVision(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::SG1000(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::PS1(_) => {}
            EmulatorSystem::GameAndWatch(_) => {}
            EmulatorSystem::Atari5200(s) => s.add_read_breakpoint(addr),
            EmulatorSystem::MegaDrive(s) => s.add_read_breakpoint(addr),
        }
    }

    // Add write breakpoints
    for &addr in &cli_args.write_breakpoints {
        match sys {
            EmulatorSystem::NES(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::GameBoy(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::GBA(_) => {}
            EmulatorSystem::Atari2600(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::Chip8(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::SMS(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::SNES(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::N64(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::PC(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::ColecoVision(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::SG1000(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::PS1(_) => {}
            EmulatorSystem::GameAndWatch(_) => {}
            EmulatorSystem::Atari5200(s) => s.add_write_breakpoint(addr),
            EmulatorSystem::MegaDrive(s) => s.add_write_breakpoint(addr),
        }
    }
}

/// Generate a comprehensive debug dump with disassembly and memory contents
/// Also captures a screenshot of the current frame state
fn generate_debug_dump(
    system_adapter: &EmulatorSystem,
    output_file: &str,
    cycle_count: u64,
    frame_buffer: Option<&(Vec<u32>, usize, usize)>,
) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(output_file)?;

    writeln!(file, "===============================================")?;
    writeln!(file, "       HEMULATOR DEBUG DUMP")?;
    writeln!(file, "===============================================")?;
    writeln!(file)?;
    writeln!(
        file,
        "Timestamp: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )?;
    writeln!(file, "Cycle Count: {}", cycle_count)?;

    // Save screenshot if frame buffer is available
    if let Some((buffer, width, height)) = frame_buffer {
        let system_name = system_adapter.system_name();
        match save_screenshot(buffer, *width, *height, system_name) {
            Ok(path) => {
                writeln!(file, "Screenshot: {}", path)?;
            }
            Err(e) => {
                writeln!(file, "Screenshot: Failed to save ({}) ", e)?;
            }
        }
    } else {
        writeln!(file, "Screenshot: No frame buffer available")?;
    }

    writeln!(file)?;

    // Get debugger if available
    if let Some(debugger) = system_adapter.debugger() {
        let cpu_state = debugger.get_cpu_state();

        writeln!(file, "===============================================")?;
        writeln!(file, "       CPU STATE")?;
        writeln!(file, "===============================================")?;
        writeln!(file)?;
        writeln!(file, "Program Counter: ${:04X}", cpu_state.pc)?;
        writeln!(file)?;

        writeln!(file, "Registers:")?;
        for reg in &cpu_state.registers {
            writeln!(
                file,
                "  {} = ${:0width$X}",
                reg.name,
                reg.value,
                width = (reg.width / 4) as usize
            )?;
        }
        writeln!(file)?;

        writeln!(file, "Flags:")?;
        for (name, value) in &cpu_state.flags.flags {
            writeln!(file, "  {} = {}", name, if *value { "1" } else { "0" })?;
        }
        writeln!(file)?;

        // Disassembly around current PC
        writeln!(file, "===============================================")?;
        writeln!(file, "       DISASSEMBLY (±100 instructions from PC)")?;
        writeln!(file, "===============================================")?;
        writeln!(file)?;

        let start_addr = cpu_state.pc.saturating_sub(200);
        let instructions = debugger.disassemble_range(start_addr, 200);

        for instr in instructions {
            let marker = if instr.address == cpu_state.pc {
                "▶"
            } else {
                " "
            };
            let bytes_str: String = instr
                .bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");

            let comment = instr
                .comment
                .as_ref()
                .map(|c| format!(" ; {}", c))
                .unwrap_or_default();

            writeln!(
                file,
                "{} {:04X}  {:12}  {}{}",
                marker, instr.address, bytes_str, instr.mnemonic, comment
            )?;
        }
        writeln!(file)?;

        // Memory dump
        writeln!(file, "===============================================")?;
        writeln!(file, "       MEMORY REGIONS")?;
        writeln!(file, "===============================================")?;
        writeln!(file)?;

        let regions = debugger.get_memory_regions();
        for region in &regions {
            writeln!(
                file,
                "--- {} (${:04X}-${:04X}, {} bytes) ---",
                region.name,
                region.start,
                region.end,
                region.size()
            )?;
            writeln!(file, "Description: {}", region.description)?;
            writeln!(
                file,
                "Access: {}",
                match (region.readable, region.writable) {
                    (true, true) => "R/W",
                    (true, false) => "R",
                    (false, true) => "W",
                    (false, false) => "-",
                }
            )?;
            writeln!(file)?;

            if region.readable {
                // Dump memory in hex format (16 bytes per line)
                let size = region.size().min(256 * 1024) as usize; // Limit to 256KB per region
                if let Some(data) = debugger.read_memory(region.start, size) {
                    for (offset, chunk) in data.chunks(16).enumerate() {
                        let addr: u32 = region.start + (offset * 16) as u32;
                        write!(file, "{:04X}:  ", addr)?;

                        // Hex bytes
                        for (i, byte) in chunk.iter().enumerate() {
                            write!(file, "{:02X} ", byte)?;
                            if i == 7 {
                                write!(file, " ")?; // Extra space at halfway point
                            }
                        }

                        // Padding for incomplete lines
                        for i in chunk.len()..16 {
                            write!(file, "   ")?;
                            if i == 7 {
                                write!(file, " ")?;
                            }
                        }

                        // ASCII representation
                        write!(file, " |")?;
                        for byte in chunk {
                            let ch: char = if *byte >= 32 && *byte < 127 {
                                *byte as char
                            } else {
                                '.'
                            };
                            write!(file, "{}", ch)?;
                        }
                        writeln!(file, "|")?;
                    }
                } else {
                    writeln!(file, "(Memory read failed)")?;
                }
            } else {
                writeln!(file, "(Not readable)")?;
            }
            writeln!(file)?;
        }
    } else {
        writeln!(file, "Debug interface not available for this system.")?;
    }

    writeln!(file, "===============================================")?;
    writeln!(file, "       END OF DEBUG DUMP")?;
    writeln!(file, "===============================================")?;

    Ok(())
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

    // Configure rate limit from settings
    log_config.set_rate_limit(settings.log_rate_limit);

    // Create runtime state for tracking current project and mounts
    let mut runtime_state = RuntimeState::new();

    // Determine what to load based on CLI args
    let rom_path = cli_args.rom_path.as_ref().cloned();

    let mut sys: EmulatorSystem = EmulatorSystem::NES(Box::default());
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;
    let mut status_message = String::new();
    // Deferred N64 ROM loading: N64 requires GL context, which is only available
    // after the event loop starts. Store ROM data + path here for deferred creation.
    let mut pending_n64_rom: Option<(Vec<u8>, String)> = None;

    // Initialize system based on --system parameter if specified
    if let Some(ref system_name) = cli_args.system {
        match system_name.to_lowercase().as_str() {
            "nes" => {
                sys = EmulatorSystem::NES(Box::default());
                rom_loaded = true; // Mark system as loaded even without ROM
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
                rom_loaded = true; // Mark system as loaded even without ROM
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
            "gba" | "gameboyadvance" => {
                sys = EmulatorSystem::GBA(Box::new(hemu_gba::GbaSystem::new()));
                rom_loaded = true; // Mark system as loaded even without ROM
                status_message = "Clean GBA system started".to_string();
                println!("Started clean GBA system");

                // If a file is provided with --system gba, load it directly
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::GBA(gba_sys) = &mut sys {
                                    if let Err(e) = gba_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load GBA ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "GBA ROM loaded".to_string();
                                        println!("Loaded GBA ROM: {}", p);
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
                sys = EmulatorSystem::Atari2600(Box::new(create_atari2600_system(&settings)));
                rom_loaded = true; // Mark system as loaded even without ROM
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
            "atari5200" | "atari52" => {
                sys = EmulatorSystem::Atari5200(Box::new(create_atari5200_system(&settings)));
                rom_loaded = true;
                status_message = "Clean Atari 5200 system started".to_string();
                println!("Started clean Atari 5200 system");

                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::Atari5200(atari_sys) = &mut sys {
                                    if let Err(e) = atari_sys.mount("Cartridge", &data) {
                                        eprintln!("Failed to load Atari 5200 ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "Atari 5200 ROM loaded".to_string();
                                        println!("Loaded Atari 5200 ROM: {}", p);
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
            "megadrive" | "genesis" | "md" | "gen" => {
                sys = EmulatorSystem::MegaDrive(Box::new(create_megadrive_system(&settings)));
                rom_loaded = true;
                status_message = "Clean Mega Drive system started".to_string();
                println!("Started clean Mega Drive system");

                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::MegaDrive(md_sys) = &mut sys {
                                    if let Err(e) = md_sys.mount("cartridge", &data) {
                                        eprintln!("Failed to load Mega Drive ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        status_message = "Mega Drive ROM loaded".to_string();
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
                rom_loaded = true; // Mark system as loaded even without ROM
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
                rom_loaded = true; // Mark system as loaded even without ROM
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
                // N64 requires a real GL context for its renderer, which isn't
                // available until the event loop starts. Defer creation.
                status_message = "N64 system (pending GL init)".to_string();
                println!("N64 system will be created when GL context is available");

                // If a file is provided with --system n64, store it for deferred loading
                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                pending_n64_rom = Some((data, p.clone()));
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                } else {
                    // --system n64 with no ROM: still defer creation
                    pending_n64_rom = Some((vec![], String::new()));
                }
            }
            "gameandwatch" | "gw" => {
                sys = EmulatorSystem::GameAndWatch(Box::new(
                    emu_gameandwatch::GameAndWatchSystem::new(),
                ));
                rom_loaded = true;
                status_message = "Clean Game & Watch system started".to_string();
                println!("Started clean Game & Watch system");

                if let Some(ref p) = rom_path {
                    if !p.to_lowercase().ends_with(".hemu") {
                        match std::fs::read(p) {
                            Ok(data) => {
                                rom_hash = Some(GameSaves::rom_hash(&data));
                                if let EmulatorSystem::GameAndWatch(gw_sys) = &mut sys {
                                    if let Err(e) = gw_sys.mount("Program", &data) {
                                        eprintln!("Failed to load Game & Watch ROM: {}", e);
                                        status_message = format!("Error: {}", e);
                                        rom_hash = None;
                                    } else {
                                        rom_loaded = true;
                                        runtime_state.set_mount("Program".to_string(), p.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        status_message = "Game & Watch ROM loaded".to_string();
                                        println!("Loaded Game & Watch ROM: {}", p);
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
                eprintln!("Valid systems: pc, nes, gb, gba, atari2600, atari5200, megadrive, snes, n64, gameandwatch");
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

                        // Load project-specific input override if it exists
                        if let Some(input_override) = project.get_input_override() {
                            runtime_state.input_override = Some(input_override.clone());
                            println!("Loaded project-specific input configuration");
                        }

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
                Ok(data) => {
                    // Check file extension first for CHIP-8 (since it overlaps with PC COM files in size)
                    let extension = std::path::Path::new(p)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());

                    // Use extension-aware detection with preferred system hint
                    // If --system was specified, use that as preferred system for ambiguous formats
                    let preferred_system = if cli_args.system.is_some() && rom_loaded {
                        Some(sys.system_type())
                    } else {
                        None
                    };

                    let system_type = detect_rom_type_with_extension(
                        &data,
                        extension.as_deref(),
                        preferred_system,
                    );

                    match system_type {
                        Ok(SystemType::NES) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut nes_sys = emu_nes::NesSystem::default();
                            // Use the mount point system to load the cartridge
                            if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load NES ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                // Enable OpenGL renderer if requested (note: GL context not available at startup)
                                // OpenGL can be enabled later when switching renderers
                                rom_loaded = true;
                                sys = EmulatorSystem::NES(Box::new(nes_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "NES ROM loaded".to_string();
                                println!("Loaded NES ROM: {}", p);
                            }
                        }
                        Ok(SystemType::Atari2600) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut a2600_sys = create_atari2600_system(&settings);
                            if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load Atari 2600 ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                            }
                        }
                        Ok(SystemType::Atari5200) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut a5200_sys = create_atari5200_system(&settings);

                            // Atari 5200 BIOS is optional - try CLI arg first, then auto-search
                            let bios_candidates = [
                                "5200.rom",
                                "5200.bin",
                                "ataribas.rom",
                                "bios.rom",
                                "bios.bin",
                            ];

                            let bios_result = load_bios(
                                cli_args.bios_path.as_ref(),
                                Some(p),
                                &bios_candidates,
                                None, // 5200 BIOS can be 2KB or 4KB
                            );

                            if let Some((bios_data, bios_path)) = bios_result {
                                if a5200_sys.mount("BIOS", &bios_data).is_ok() {
                                    runtime_state.set_mount("BIOS".to_string(), bios_path);
                                } else {
                                    eprintln!("Failed to mount Atari 5200 BIOS");
                                }
                            }

                            if let Err(e) = a5200_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load Atari 5200 ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::Atari5200(Box::new(a5200_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                            }
                        }
                        Ok(SystemType::MegaDrive) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut md_sys = create_megadrive_system(&settings);
                            if let Err(e) = md_sys.mount("cartridge", &data) {
                                eprintln!("Failed to load Mega Drive ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::MegaDrive(Box::new(md_sys));
                                runtime_state.set_mount("cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
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
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "Game Boy ROM loaded".to_string();
                                println!("Loaded Game Boy ROM: {}", p);
                            }
                        }
                        Ok(SystemType::GBA) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut gba_sys = hemu_gba::GbaSystem::new();
                            if let Err(e) = gba_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load GBA ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::GBA(Box::new(gba_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "GBA ROM loaded".to_string();
                                println!("Loaded GBA ROM: {}", p);
                            }
                        }
                        Ok(SystemType::PC) => {
                            rom_hash = None; // PC systems don't use ROM hash
                            let mut pc_sys = emu_pc::PcSystem::new();

                            // Determine the correct mount point for .img/.ima disk images;
                            // other PC files (.com/.exe) simply start a bare system.
                            let ext_str = extension.as_deref().unwrap_or("");
                            let (mount_id, msg) = pc_disk_mount_target(ext_str, data.len());
                            if mount_id.is_empty() {
                                status_message = msg.to_string();
                                if !matches!(ext_str, "img" | "ima") {
                                    println!(
                                        "Initialized PC system. Mount disk images to proceed."
                                    );
                                }
                            } else if let Err(e) = pc_sys.mount(mount_id, &data) {
                                eprintln!("Failed to mount {}: {}", mount_id, e);
                                status_message = format!("Error: {}", e);
                            } else {
                                runtime_state.set_mount(mount_id.to_string(), p.clone());
                                status_message = format!("{}: {}", msg, p);
                                println!("Mounted {}: {}", mount_id, p);
                            }

                            sys = EmulatorSystem::PC(Box::new(pc_sys));
                            rom_loaded = true; // Mark as loaded so slot-args block reuses this system
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
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
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "SNES ROM loaded".to_string();
                                println!("Loaded SNES ROM: {}", p);
                            }
                        }
                        Ok(SystemType::N64) => {
                            // N64 requires GL context — defer creation to event loop
                            pending_n64_rom = Some((data.clone(), p.clone()));
                            status_message = "N64 ROM detected (pending GL init)".to_string();
                            println!("N64 ROM will be loaded when GL context is available: {}", p);
                        }
                        Ok(SystemType::SMS) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut sms_sys = emu_sms::SmsSystem::new();

                            // SMS BIOS is optional - try CLI arg first, then auto-search
                            let bios_candidates =
                                ["bios.sms", "sms.rom", "sms.bin", "bios.rom", "bios.bin"];

                            let bios_result = load_bios(
                                cli_args.bios_path.as_ref(),
                                Some(p),
                                &bios_candidates,
                                None, // SMS BIOS size can vary
                            );

                            if let Some((bios_data, bios_path)) = bios_result {
                                if sms_sys.mount("bios", &bios_data).is_ok() {
                                    runtime_state.set_mount("bios".to_string(), bios_path);
                                } else {
                                    eprintln!("Failed to mount SMS BIOS");
                                }
                            }

                            // Load the cartridge
                            if let Err(e) = sms_sys.mount("cartridge", &data) {
                                eprintln!("Failed to load SMS ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::SMS(Box::new(sms_sys));
                                runtime_state.set_mount("cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "SMS ROM loaded".to_string();
                                println!("Loaded SMS ROM: {}", p);
                            }
                        }
                        Ok(SystemType::Chip8) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut chip8_sys = emu_chip8::Chip8System::new();
                            if let Err(e) = chip8_sys.mount("Program", &data) {
                                eprintln!("Failed to load CHIP-8 ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::Chip8(Box::new(chip8_sys));
                                runtime_state.set_mount("Program".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "CHIP-8 program loaded".to_string();
                                println!("Loaded CHIP-8 program: {}", p);
                            }
                        }
                        Ok(SystemType::ColecoVision) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut coleco_sys = emu_colecovision::ColecoVisionSystem::new();

                            // ColecoVision requires BIOS - try CLI arg first, then auto-search
                            let bios_candidates = [
                                "ColecoVision BIOS (1982).col",
                                "coleco.rom",
                                "coleco.bin",
                                "bios.rom",
                                "bios.bin",
                            ];

                            let bios_result = load_bios(
                                cli_args.bios_path.as_ref(),
                                Some(p),
                                &bios_candidates,
                                Some(8192), // ColecoVision BIOS must be 8KB
                            );

                            let bios_loaded = if let Some((bios_data, bios_path)) = bios_result {
                                if coleco_sys.mount("BIOS", &bios_data).is_ok() {
                                    runtime_state.set_mount("BIOS".to_string(), bios_path);
                                    true
                                } else {
                                    eprintln!("Failed to mount ColecoVision BIOS");
                                    false
                                }
                            } else {
                                false
                            };

                            if !bios_loaded {
                                eprintln!("Warning: ColecoVision BIOS not found. System will not boot properly.");
                            }

                            // Load the cartridge
                            if let Err(e) = coleco_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load ColecoVision ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::ColecoVision(Box::new(coleco_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                if bios_loaded {
                                    status_message = "ColecoVision cartridge loaded".to_string();
                                    println!("Loaded ColecoVision cartridge: {}", p);
                                } else {
                                    status_message = "ColecoVision cartridge loaded (BIOS missing - will not boot)".to_string();
                                    println!("Loaded ColecoVision cartridge: {} (BIOS missing)", p);
                                }
                            }
                        }
                        Ok(SystemType::SG1000) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut sg1000_sys = emu_sg1000::Sg1000System::new();
                            if let Err(e) = sg1000_sys.mount("Cartridge", &data) {
                                eprintln!("Failed to load SG-1000 ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::SG1000(Box::new(sg1000_sys));
                                runtime_state.set_mount("Cartridge".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "SG-1000 cartridge loaded".to_string();
                                println!("Loaded SG-1000 cartridge: {}", p);
                            }
                        }
                        Ok(SystemType::PS1) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut ps1_sys = emu_ps1::Ps1System::new();

                            // Check if the loaded file IS the BIOS itself
                            let loaded_file_is_bios = is_ps1_bios_file(&data);

                            if loaded_file_is_bios {
                                // User opened the BIOS file directly — mount it as BIOS
                                if ps1_sys.mount("bios", &data).is_ok() {
                                    runtime_state.set_mount("bios".to_string(), p.clone());
                                    sys = EmulatorSystem::PS1(Box::new(ps1_sys));
                                    rom_loaded = true;
                                    status_message =
                                        "PS1 BIOS loaded — ready (load a game or run BIOS)"
                                            .to_string();
                                    println!("Loaded PS1 BIOS: {}", p);
                                } else {
                                    eprintln!("Failed to mount PS1 BIOS from: {}", p);
                                    status_message = "Failed to load PS1 BIOS".to_string();
                                }
                            } else {
                                // Loaded a game/disc image — search for BIOS separately
                                let bios_candidates = [
                                    "scph1001.bin",
                                    "scph5501.bin",
                                    "scph5500.bin",
                                    "scph5502.bin",
                                    "scph7001.bin",
                                    "scph7502.bin",
                                    "ps1.bin",
                                    "psx.bin",
                                    "bios.bin",
                                    "bios.rom",
                                ];

                                let bios_result = load_bios(
                                    cli_args.bios_path.as_ref(),
                                    Some(p),
                                    &bios_candidates,
                                    Some(512 * 1024), // PS1 BIOS must be 512KB
                                );

                                let bios_loaded_ok =
                                    if let Some((bios_data, bios_path)) = bios_result {
                                        if ps1_sys.mount("bios", &bios_data).is_ok() {
                                            runtime_state.set_mount("bios".to_string(), bios_path);
                                            true
                                        } else {
                                            eprintln!("Failed to mount PS1 BIOS");
                                            false
                                        }
                                    } else {
                                        false
                                    };

                                if !bios_loaded_ok {
                                    eprintln!(
                                        "Warning: PS1 BIOS not found. System will not boot properly."
                                    );
                                    eprintln!("  Use --bios <file> or place a 512KB BIOS in the ROM directory");
                                }

                                sys = EmulatorSystem::PS1(Box::new(ps1_sys));

                                // If data is a PS-X EXE, mount it as disc
                                if data.len() >= 8 && &data[0..8] == b"PS-X EXE" {
                                    if let Err(e) = sys.mount("disc", &data) {
                                        eprintln!("Failed to load PS-X EXE: {}", e);
                                    }
                                } else {
                                    // Mount as raw disc image
                                    if let Err(e) = sys.mount("disc", &data) {
                                        eprintln!("Failed to load PS1 disc image: {}", e);
                                    }
                                }
                                rom_loaded = true;
                                runtime_state.set_mount("disc".to_string(), p.clone());
                                if bios_loaded_ok {
                                    status_message = "PS1 disc loaded".to_string();
                                } else {
                                    status_message =
                                        "PS1 disc loaded (BIOS missing - will not boot)"
                                            .to_string();
                                }
                                println!("Loaded PS1 disc: {}", p);
                            }
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                        }
                        Ok(SystemType::GameAndWatch) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            let mut gw_sys = emu_gameandwatch::GameAndWatchSystem::new();
                            if let Err(e) = gw_sys.mount("Program", &data) {
                                eprintln!("Failed to load Game & Watch ROM: {}", e);
                                status_message = format!("Error: {}", e);
                                rom_hash = None;
                            } else {
                                rom_loaded = true;
                                sys = EmulatorSystem::GameAndWatch(Box::new(gw_sys));
                                runtime_state.set_mount("Program".to_string(), p.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                status_message = "Game & Watch program loaded".to_string();
                                println!("Loaded Game & Watch ROM: {}", p);
                            }
                        }
                        Err(e) => {
                            eprintln!("Unsupported ROM: {}", e);
                            status_message = format!("Unsupported ROM: {}", e);
                        }
                    } // closes match system_type
                } // closes Ok(data)
                Err(e) => {
                    eprintln!("Failed to read ROM file: {}", e);
                }
            } // closes match std::fs::read
        } // closes else block for non-.hemu files
    } // closes if let Some(p) = &rom_path

    // Apply debug options after ROM loading
    if rom_loaded {
        apply_debug_options(&mut sys, &cli_args);
    }

    // Handle slot-based loading (primarily for PC system)
    // If any slot arguments or bios argument are provided, auto-select PC mode if no ROM was loaded
    let has_slot_args = cli_args.slot1.is_some()
        || cli_args.slot2.is_some()
        || cli_args.slot3.is_some()
        || cli_args.slot4.is_some()
        || cli_args.slot5.is_some()
        || cli_args.bios_path.is_some();

    if has_slot_args && !rom_loaded {
        // Auto-select PC mode when slot files are provided
        let pc_sys = emu_pc::PcSystem::new();
        sys = EmulatorSystem::PC(Box::new(pc_sys));
        rom_loaded = true;
        println!("Auto-selected PC mode for slot-based loading");
    }

    // Load slot files for PC system
    if let EmulatorSystem::PC(ref mut pc_sys) = sys {
        // BIOS: Load from --bios first, then fall back to --slot1
        let bios_source = cli_args.bios_path.as_ref().or(cli_args.slot1.as_ref());
        if let Some(bios_path) = bios_source {
            match fs::read(bios_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("BIOS", &data) {
                        eprintln!("Failed to mount BIOS: {}", e);
                    } else {
                        runtime_state.set_mount("BIOS".to_string(), bios_path.clone());
                        println!("Loaded BIOS from: {}", bios_path);
                    }
                }
                Err(e) => eprintln!("Failed to read BIOS file: {}", e),
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

    // Apply debug options after slot loading
    if has_slot_args {
        apply_debug_options(&mut sys, &cli_args);
    }

    // Get resolution from the system
    let (width, height) = sys.resolution();

    // Window size is user-resizable and persisted; buffer size stays at native resolution.
    let window_width = settings.window_width.max(width);
    let window_height = settings.window_height.max(height);

    // ===== HEADLESS MODE FOR DEBUG DUMP =====
    // If debug dump is requested or breakpoints are set, run without GUI for faster execution
    if cli_args.debug_dump_pc.is_some()
        || cli_args.debug_dump_cycles.is_some()
        || !cli_args.breakpoints.is_empty()
    {
        eprintln!("Running in headless mode for debug dump...");

        if !rom_loaded {
            eprintln!("Error: No ROM loaded. Debug dump requires a loaded ROM.");
            std::process::exit(1);
        }

        let mut total_cycles: u64 = 0;
        let dump_file = cli_args
            .debug_dump_file
            .as_deref()
            .unwrap_or("debug_dump.txt");

        // Determine trigger condition
        let trigger_pc = cli_args.debug_dump_pc;
        let trigger_cycles = cli_args.debug_dump_cycles;

        eprintln!("Debug dump will be triggered:");
        if let Some(pc) = trigger_pc {
            eprintln!("  - When PC = ${:04X}", pc);
        }
        if let Some(cycles) = trigger_cycles {
            eprintln!("  - After {} cycles", cycles);
        }
        if !cli_args.breakpoints.is_empty() {
            eprintln!("  - When any breakpoint is hit:");
            for &bp in &cli_args.breakpoints {
                eprintln!("    - ${:06X}", bp);
            }
        }
        eprintln!("  - Output file: {}", dump_file);
        eprintln!();

        // Apply debug options (tracing, breakpoints) before starting emulation
        apply_debug_options(&mut sys, &cli_args);

        // Track the latest frame for screenshot on dump
        let mut latest_frame_buffer: Option<(Vec<u32>, usize, usize)> = None;

        // Run emulation loop until trigger condition is met
        loop {
            // Step one frame
            match sys.step_frame() {
                Ok(frame) => {
                    // Store the latest frame for screenshot (move pixels instead of cloning)
                    latest_frame_buffer =
                        Some((frame.pixels, frame.width as usize, frame.height as usize));

                    // Get actual CPU cycles from the system
                    total_cycles = sys.get_total_cycles();

                    // Check for trigger conditions
                    let breakpoint_hit = sys.check_breakpoint();
                    let should_dump =
                        if let (Some(pc_trigger), Some(debugger)) = (trigger_pc, sys.debugger()) {
                            let cpu_state = debugger.get_cpu_state();
                            cpu_state.pc == pc_trigger
                        } else {
                            false
                        } || if let Some(cycle_trigger) = trigger_cycles {
                            total_cycles >= cycle_trigger
                        } else {
                            false
                        } || breakpoint_hit.is_some();

                    if should_dump {
                        if let Some(bp_pc) = breakpoint_hit {
                            eprintln!(
                                "Breakpoint hit at PC=${:06X} after {} cycles",
                                bp_pc, total_cycles
                            );
                        } else {
                            eprintln!("Trigger condition met at {} cycles", total_cycles);
                        }
                        eprintln!("Generating debug dump...");

                        // Dump instruction trace if tracing was enabled
                        if let Some(tracer) = sys.get_instruction_tracer() {
                            if tracer.is_enabled() {
                                let trace_file = cli_args
                                    .trace_dump_file
                                    .as_deref()
                                    .unwrap_or("trace_dump.txt");
                                eprintln!("Dumping instruction trace to {}...", trace_file);
                                match tracer.dump_to_file(trace_file) {
                                    Ok(()) => {
                                        eprintln!("✓ Instruction trace written to {}", trace_file);
                                    }
                                    Err(e) => {
                                        eprintln!("✗ Failed to write trace dump: {}", e);
                                    }
                                }
                            }
                        }

                        match generate_debug_dump(
                            &sys,
                            dump_file,
                            total_cycles,
                            latest_frame_buffer.as_ref(),
                        ) {
                            Ok(()) => {
                                eprintln!("✓ Debug dump written successfully to {}", dump_file);
                                std::process::exit(0);
                            }
                            Err(e) => {
                                eprintln!("✗ Failed to write debug dump: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }

                    // Progress indicator every 1000 cycles
                    if total_cycles.is_multiple_of(1000) {
                        eprint!("\rCycles: {}...", total_cycles);
                    }
                }
                Err(e) => {
                    eprintln!("\nEmulation error after {} cycles: {}", total_cycles, e);
                    eprintln!("Generating debug dump at error point...");

                    match generate_debug_dump(
                        &sys,
                        dump_file,
                        total_cycles,
                        latest_frame_buffer.as_ref(),
                    ) {
                        Ok(()) => {
                            eprintln!("✓ Debug dump written to {}", dump_file);
                            std::process::exit(1);
                        }
                        Err(dump_err) => {
                            eprintln!("✗ Failed to write debug dump: {}", dump_err);
                            std::process::exit(1);
                        }
                    }
                }
            }

            // Safety limit to prevent infinite loops (can be removed or increased)
            if total_cycles > 200_000_000 {
                eprintln!("\nReached cycle limit (200M) without triggering dump.");
                eprintln!(
                    "Current PC: ${:04X}",
                    sys.debugger().map(|d| d.get_cpu_state().pc).unwrap_or(0)
                );
                std::process::exit(1);
            }
        }
    }
    // ===== END HEADLESS MODE =====

    // ===== NO-GUI MODE =====
    // If --no-gui is requested, run in a plain SDL2 window without egui overhead
    if cli_args.no_gui {
        // N64 requires an OpenGL context which --no-gui does not set up
        if matches!(&sys, EmulatorSystem::N64(_)) {
            eprintln!("Error: --no-gui does not support N64 (requires an OpenGL context). Use the full GUI instead.");
            std::process::exit(1);
        }

        let title = format!("Hemulator - {}", sys.system_name());
        let mut window =
            match window_backend::Sdl2Backend::new(&title, window_width, window_height, false) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to create window: {}", e);
                    return;
                }
            };

        // Initialize audio output
        let _stream = match DeviceSinkBuilder::open_default_sink() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: Failed to initialize audio (exiting): {}.", e);
                std::process::exit(1);
            }
        };
        let (audio_tx, audio_rx) = sync_channel::<i16>(44100 * 2);
        _stream.mixer().add(StreamSource {
            rx: audio_rx,
            sample_rate: 44100,
            channels: 2,
        });

        const SAMPLE_RATE: usize = 44100;
        let mut audio_sample_remainder: f64 = 0.0;
        let mut last_frame = Instant::now();

        loop {
            window.poll_events();
            if !window.is_open() {
                break;
            }

            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            let target_frame_duration = Duration::from_secs_f64(1.0 / frame_rate);

            match sys.step_frame() {
                Ok(frame) => {
                    // Handle audio
                    let samples_per_frame_f =
                        (SAMPLE_RATE as f64 / frame_rate) + audio_sample_remainder;
                    let samples_per_frame = samples_per_frame_f.floor() as usize;
                    audio_sample_remainder = samples_per_frame_f - samples_per_frame as f64;
                    let audio_samples = sys.get_audio_samples(samples_per_frame);
                    let expected_stereo = samples_per_frame * 2;
                    if audio_samples.len() == expected_stereo {
                        for sample in audio_samples {
                            let _ = audio_tx.try_send(sample);
                        }
                    } else {
                        for i in 0..samples_per_frame {
                            let sample = audio_samples.get(i).copied().unwrap_or(0);
                            let _ = audio_tx.try_send(sample);
                            let _ = audio_tx.try_send(sample);
                        }
                    }

                    // Render frame
                    if let Err(e) = window.update_with_buffer(
                        &frame.pixels,
                        frame.width as usize,
                        frame.height as usize,
                    ) {
                        eprintln!("Render error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Emulation error: {}", e);
                    break;
                }
            }

            // Handle controller input
            if !matches!(&sys, EmulatorSystem::PC(_)) {
                let controller_state = get_controller_state(&window, &settings.input.player1);
                let snes_state = get_snes_controller_state(&window, &settings.input.player1);
                let chip8_state = get_chip8_controller_state(&window);
                let gw_state = get_gw_controller_state(&window);
                let coleco_p1_state =
                    get_colecovision_controller_state(&window, &settings.input.player1);
                let coleco_p2_state =
                    get_colecovision_controller_state(&window, &settings.input.player2);

                match &mut sys {
                    EmulatorSystem::SNES(s) => s.set_controller(0, snes_state),
                    EmulatorSystem::Chip8(s) => s.set_controller(chip8_state),
                    EmulatorSystem::GameAndWatch(s) => s.set_controller(gw_state),
                    EmulatorSystem::ColecoVision(s) => {
                        s.set_controller(1, coleco_p1_state);
                        s.set_controller(2, coleco_p2_state);
                    }
                    _ => sys.set_controller(0, controller_state),
                }
            } else {
                let pressed = window.get_sdl2_scancodes_pressed().clone();
                let released = window.get_sdl2_scancodes_released().clone();
                if let EmulatorSystem::PC(pc_sys) = &mut sys {
                    for scancode in &pressed {
                        pc_sys.key_press_sdl2(*scancode);
                    }
                    for scancode in &released {
                        pc_sys.key_release_sdl2(*scancode);
                    }
                }
            }

            // Frame timing
            if !cli_args.benchmark {
                let elapsed = last_frame.elapsed();
                if elapsed < target_frame_duration {
                    std::thread::sleep(target_frame_duration - elapsed);
                }
            }
            last_frame = Instant::now();
        }
        return;
    }
    // ===== END NO-GUI MODE =====

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

    // Ensure egui_extras image loaders are installed for this egui context.
    // Some platforms/paths may create contexts elsewhere; installing here
    // is a defensive measure so `egui::include_image!` works at runtime.
    egui_extras::install_image_loaders(egui_backend.egui_ctx());

    // Initialize egui app
    let mut egui_app = EguiApp::new();
    egui_app.property_pane.system_name = sys.system_name().to_string();
    egui_app.set_system_loaded(rom_loaded, sys.system_name()); // Initialize menu state based on whether system is loaded

    // Upgrade renderer to OpenGL if settings request it and system was loaded
    // Note: OpenGL renderer upgrade temporarily disabled due to GL context refactoring
    // if rom_loaded && settings.video_backend == "opengl" {
    //     // GL context handling needs to be restored
    // }

    // Set property pane renderer display based on settings preference, not current renderer
    egui_app.property_pane.rendering_backend = if settings.video_backend == "opengl" {
        "OpenGL".to_string()
    } else {
        "Software".to_string()
    };
    egui_app.property_pane.available_renderers = sys.get_available_renderers();
    // Initialize menu bar display filter state from settings
    egui_app.menu_bar.current_filter = settings.display_filter;
    egui_app.status_bar.set_message(status_message.clone());
    // Initialize recent files menu
    egui_app.update_recent_files(settings.get_recent_files().to_vec());

    // Enable OpenGL rendering for N64 if the system is N64
    if let Some(renderer_name) = enable_n64_opengl_renderer(&mut sys, &egui_backend) {
        egui_app.property_pane.rendering_backend = renderer_name;
    }

    // Initialize audio output
    let _stream = match DeviceSinkBuilder::open_default_sink() {
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
    _stream.mixer().add(StreamSource {
        rx: audio_rx,
        sample_rate: 44100,
        channels: 2,
    });

    // Timing trackers - reset when ROM is loaded
    let mut emulation_start_time = Instant::now(); // Time when emulation started
    let mut total_emulated_time = Duration::ZERO; // Total time emulated so far
    let mut last_frame = Instant::now();

    // FPS tracking - display FPS only
    let mut display_frame_times: Vec<Duration> = Vec::with_capacity(60);
    let mut current_fps = 60.0; // Display FPS

    // GUI update throttling
    let mut frame_counter: u64 = 0;
    const GUI_UPDATE_INTERVAL: u64 = 15; // Update GUI every 15th frame

    // Track when emulation becomes active to reset timing
    let mut was_emulation_active = false;

    // Track emulation speed changes to reset timing
    let mut previous_emulation_speed = settings.emulation_speed;
    const SPEED_CHANGE_THRESHOLD: f64 = 0.001; // Minimum change to detect speed adjustment

    // Debug dump tracking
    let mut total_cycles: u64 = 0;
    let mut debug_dump_triggered = false;

    // Audio sample rate
    const SAMPLE_RATE: usize = 44100;
    let mut audio_sample_remainder: f64 = 0.0;

    // Load saves for current ROM if available
    let mut _game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    // Store latest frame buffer for screenshots
    let mut latest_frame_buffer: Option<(Vec<u32>, usize, usize)> = None;

    #[allow(dead_code)]
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

    // Track ROM loaded state to run certain updates only on transitions
    let mut prev_rom_loaded = rom_loaded;

    // Main event loop with egui
    loop {
        // Handle deferred N64 creation (needs GL context from event loop)
        if let Some((rom_data, rom_path_str)) = pending_n64_rom.take() {
            let gl_ctx = egui_backend.gl_context();
            match create_n64_system(gl_ctx, &settings) {
                Ok(mut n64_sys) => {
                    if !rom_data.is_empty() {
                        rom_hash = Some(GameSaves::rom_hash(&rom_data));
                        if let Err(e) = n64_sys.mount("Cartridge", &rom_data) {
                            eprintln!("Failed to load N64 ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            runtime_state.set_mount("Cartridge".to_string(), rom_path_str.clone());
                            settings.add_recent_file(rom_path_str);
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            egui_app.update_recent_files(settings.get_recent_files().to_vec());
                            status_message = "N64 ROM loaded".to_string();
                            println!("N64 system created with GL context, ROM loaded");
                        }
                    } else {
                        rom_loaded = true;
                        status_message = "Clean N64 system started".to_string();
                        println!("N64 system created with GL context");
                    }
                    sys = EmulatorSystem::N64(Box::new(n64_sys));
                    egui_app.property_pane.system_name = "N64".to_string();
                    egui_app.property_pane.rendering_backend = sys.get_current_renderer_name();
                    egui_app.property_pane.available_renderers = sys.get_available_renderers();
                    egui_app.status_bar.set_message(status_message.clone());
                }
                Err(e) => {
                    eprintln!("Failed to create N64 system: {}", e);
                    egui_app
                        .status_bar
                        .set_error(format!("Failed to create N64 system: {}", e));
                }
            }
        }

        // Detect transition: ROM has just been loaded or unloaded this frame
        let rom_state_changed = rom_loaded != prev_rom_loaded;

        // Update inspector tabs based on current system (only when ROM state changes)
        if rom_state_changed {
            if rom_loaded {
                egui_app.dock_layout.update_system(sys.system_type());
                // Enable GUI message capture for the log tab
                emu_core::logging::LogConfig::global().enable_gui_capture();
            } else {
                egui_app.dock_layout.clear_system();
            }
            prev_rom_loaded = rom_loaded;
        }

        // Only increment frame counter when emulation is active
        if rom_loaded && settings.emulation_speed > 0.0 {
            frame_counter = frame_counter.wrapping_add(1);
        }
        // Update GUI more frequently when paused or no ROM loaded
        let should_update_gui = if rom_loaded && settings.emulation_speed > 0.0 {
            frame_counter.is_multiple_of(GUI_UPDATE_INTERVAL)
        } else {
            true // Always update when paused or no ROM
        };
        // Handle SDL2 events and update egui input
        if !egui_backend.handle_events() {
            break; // Window closed
        }

        // Begin egui frame
        egui_backend.begin_frame();

        // Update egui app state (only periodically to reduce overhead)
        if should_update_gui {
            egui_app.property_pane.update_fps(current_fps);
            egui_app.property_pane.paused = settings.emulation_speed == 0.0;
            egui_app.property_pane.speed = settings.emulation_speed as f32;
            egui_app.property_pane.cpu_freq_target = sys.get_cpu_freq_target();
            egui_app.property_pane.emulation_speed_percent =
                (settings.emulation_speed * 100.0) as i32;

            // Update input device counts from backend
            egui_app.property_pane.num_gamepads_detected = egui_backend.num_gamepads();
            egui_app.property_pane.num_joysticks_detected = egui_backend.num_joysticks();

            // Update input configuration from settings
            egui_app.property_pane.mouse_enabled = settings.input.mouse_enabled;
            egui_app.property_pane.mouse_sensitivity = settings.input.mouse_sensitivity;

            // Determine input config source
            if runtime_state.input_override.is_some() {
                egui_app.property_pane.input_config_source = egui_ui::InputConfigSource::Project;
            } else {
                egui_app.property_pane.input_config_source = egui_ui::InputConfigSource::Global;
            }

            // Update target FPS from system timing
            if rom_loaded {
                let timing = sys.timing();
                egui_app.property_pane.target_fps = timing.frame_rate_hz() as f32;
            }

            // Update mount points from current system
            // For PC systems, show mount points even when rom_loaded is false
            // because PC can boot from disk images without a ROM file
            let is_pc_system = matches!(sys, EmulatorSystem::PC(_));
            if rom_loaded || is_pc_system {
                use egui_ui::property_pane::MountPoint;
                let mount_points_info = sys.mount_points();
                egui_app.property_pane.mount_points = mount_points_info
                    .iter()
                    .map(|mp| MountPoint {
                        id: mp.id.clone(),
                        name: mp.name.clone(),
                        mounted_file: runtime_state.get_mount(&mp.id).map(|s| {
                            // Show just the filename, not the full path
                            std::path::Path::new(s)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(s)
                                .to_string()
                        }),
                    })
                    .collect();
            } else {
                egui_app.property_pane.mount_points.clear();
            }

            // Update PC-specific property pane fields if PC is loaded
            if rom_loaded {
                if let EmulatorSystem::PC(pc_sys) = &sys {
                    // Read BDA values
                    use egui_ui::property_pane::PcBdaValues;
                    let bda = pc_sys.read_bda_values();
                    egui_app.property_pane.pc_bda_values = Some(PcBdaValues {
                        equipment_word: bda.equipment_word,
                        memory_size_kb: bda.memory_size_kb,
                        video_mode: bda.video_mode,
                        video_columns: bda.video_columns,
                        num_serial_ports: bda.num_serial_ports,
                        num_parallel_ports: bda.num_parallel_ports,
                        num_hard_drives: bda.num_hard_drives,
                    });

                    // Set PC CPU model for dropdown
                    let cpu_model_str = match pc_sys.cpu_model() {
                        emu_core::cpu_8086::CpuModel::Intel8086 => "Intel 8086",
                        emu_core::cpu_8086::CpuModel::Intel8088 => "Intel 8088",
                        emu_core::cpu_8086::CpuModel::Intel80186 => "Intel 80186",
                        emu_core::cpu_8086::CpuModel::Intel80188 => "Intel 80188",
                        emu_core::cpu_8086::CpuModel::Intel80286 => "Intel 80286",
                        emu_core::cpu_8086::CpuModel::Intel80386 => "Intel 80386",
                        emu_core::cpu_8086::CpuModel::Intel80486 => "Intel 80486",
                        emu_core::cpu_8086::CpuModel::Intel80486SX => "Intel 80486SX",
                        emu_core::cpu_8086::CpuModel::Intel80486DX2 => "Intel 80486DX2",
                        emu_core::cpu_8086::CpuModel::Intel80486SX2 => "Intel 80486SX2",
                        emu_core::cpu_8086::CpuModel::Intel80486DX4 => "Intel 80486DX4",
                        emu_core::cpu_8086::CpuModel::IntelPentium => "Intel Pentium",
                        emu_core::cpu_8086::CpuModel::IntelPentiumMMX => "Intel Pentium MMX",
                    };
                    egui_app.property_pane.pc_cpu_model = Some(cpu_model_str.to_string());

                    // Set PC memory for dropdown
                    egui_app.property_pane.pc_memory_kb = Some(pc_sys.memory_kb());
                } else {
                    // Clear PC-specific fields for non-PC systems
                    egui_app.property_pane.pc_bda_values = None;
                    egui_app.property_pane.pc_cpu_model = None;
                    egui_app.property_pane.pc_memory_kb = None;
                }
            }

            // Update PC config tab if PC is loaded (deprecated, but keep for backward compat)
            if rom_loaded {
                // PC Config tab is deprecated - removed
            }

            // Update menu bar state for new menu features
            let mount_points = sys.mount_points();
            let has_required_mount = mount_points
                .iter()
                .any(|mp| mp.required && sys.is_mounted(&mp.id));
            egui_app.menu_bar.rom_loaded = has_required_mount;
            egui_app.menu_bar.single_mount_system =
                mount_points.iter().filter(|mp| mp.required).count() == 1;
            egui_app.menu_bar.current_speed = egui_app.property_pane.emulation_speed_percent;
        }

        // Update debug info if inspector is visible (contains Debug tab)
        if egui_app.dock_layout.inspector_visible {
            use system_adapter::SystemDebugInfo;
            let debug_info = match &sys {
                EmulatorSystem::NES(s) => SystemDebugInfo::from_nes(&s.get_debug_info()),
                EmulatorSystem::GameBoy(s) => SystemDebugInfo::from_gb(&s.debug_info()),
                EmulatorSystem::GBA(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("GBA", debugger)
                    } else {
                        SystemDebugInfo::new("GBA".to_string())
                    }
                }
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
                EmulatorSystem::SMS(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("Sega Master System", debugger)
                    } else {
                        SystemDebugInfo::new("Sega Master System".to_string())
                    }
                }
                EmulatorSystem::Chip8(s) => SystemDebugInfo::from_chip8(&s.debug_info()),
                EmulatorSystem::ColecoVision(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("ColecoVision", debugger)
                    } else {
                        SystemDebugInfo::new("ColecoVision".to_string())
                    }
                }
                EmulatorSystem::SG1000(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("SG-1000", debugger)
                    } else {
                        SystemDebugInfo::new("SG-1000".to_string())
                    }
                }
                EmulatorSystem::PS1(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("PS1", debugger)
                    } else {
                        SystemDebugInfo::new("PS1".to_string())
                    }
                }
                EmulatorSystem::GameAndWatch(s) => {
                    if let Some(debugger) = s.debugger() {
                        SystemDebugInfo::from_debugger("Game & Watch", debugger)
                    } else {
                        SystemDebugInfo::new("Game & Watch".to_string())
                    }
                }
                EmulatorSystem::Atari5200(_) => SystemDebugInfo::new("Atari 5200".to_string()),
                EmulatorSystem::MegaDrive(_) => SystemDebugInfo::new("Mega Drive".to_string()),
            };
            egui_app.tab_manager.update_debug_info(debug_info);

            // Populate enhanced debug state using the Debugger trait (if available)
            use emu_core::debug::Debugger;
            let enhanced_state_opt = match &sys {
                EmulatorSystem::NES(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("NES", debugger, &sys))
                }
                EmulatorSystem::SMS(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("SMS", debugger, &sys))
                }
                EmulatorSystem::SNES(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("SNES", debugger, &sys))
                }
                EmulatorSystem::GameBoy(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("Game Boy", debugger, &sys))
                }
                EmulatorSystem::GBA(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("GBA", debugger, &sys))
                }
                EmulatorSystem::Atari2600(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("Atari 2600", debugger, &sys))
                }
                EmulatorSystem::PC(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("PC", debugger, &sys))
                }
                EmulatorSystem::N64(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("N64", debugger, &sys))
                }
                EmulatorSystem::Chip8(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("CHIP-8", debugger, &sys))
                }
                EmulatorSystem::ColecoVision(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("ColecoVision", debugger, &sys))
                }
                EmulatorSystem::SG1000(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("SG-1000", debugger, &sys))
                }
                EmulatorSystem::PS1(_) => None,
                EmulatorSystem::GameAndWatch(s) => {
                    let debugger: &dyn Debugger = s.as_ref();
                    Some(create_enhanced_debug_state("Game & Watch", debugger, &sys))
                }
                EmulatorSystem::Atari5200(_) => None,
                EmulatorSystem::MegaDrive(_) => None,
            };

            if let Some(enhanced_state) = enhanced_state_opt {
                egui_app
                    .tab_manager
                    .update_enhanced_debug_state(enhanced_state.clone());

                // Update cached memory for the current view
                let debugger: Option<&dyn Debugger> = match &sys {
                    EmulatorSystem::NES(s) => Some(s.as_ref()),
                    EmulatorSystem::SMS(s) => Some(s.as_ref()),
                    EmulatorSystem::SNES(s) => Some(s.as_ref()),
                    EmulatorSystem::GameBoy(s) => Some(s.as_ref()),
                    EmulatorSystem::GBA(s) => Some(s.as_ref()),
                    EmulatorSystem::Atari2600(s) => Some(s.as_ref()),
                    EmulatorSystem::PC(s) => Some(s.as_ref()),
                    EmulatorSystem::N64(s) => Some(s.as_ref()),
                    EmulatorSystem::Chip8(s) => Some(s.as_ref()),
                    EmulatorSystem::ColecoVision(s) => Some(s.as_ref()),
                    EmulatorSystem::SG1000(s) => Some(s.as_ref()),
                    EmulatorSystem::PS1(_) => None,
                    EmulatorSystem::GameAndWatch(s) => Some(s.as_ref()),
                    EmulatorSystem::Atari5200(_) => None,
                    EmulatorSystem::MegaDrive(_) => None,
                };

                if let Some(debugger) = debugger {
                    // Read memory for the current view (512 bytes centered around current address)
                    let memory_address = egui_app.tab_manager.memory_view_address;
                    let bytes_to_read = 512;

                    // Align to 16-byte boundary
                    let aligned_address = (memory_address / 16) * 16;

                    if let Some(memory_data) = debugger.read_memory(aligned_address, bytes_to_read)
                    {
                        egui_app
                            .tab_manager
                            .update_cached_memory(memory_data, aligned_address);
                    }
                }
            }
        }

        // Update tile viewer data if inspector is visible (contains Tiles tab)
        if egui_app.dock_layout.inspector_visible {
            match &sys {
                EmulatorSystem::NES(s) => {
                    let nes_data = s.get_tile_viewer_data();
                    let tile_data = egui_ui::SystemTileData::NES(egui_ui::NesTileData {
                        chr_data: nes_data.chr_data,
                        palette: nes_data.palette,
                        master_palette: nes_data.master_palette,
                        oam: nes_data.oam,
                        vram: nes_data.vram,
                        chr_is_ram: nes_data.chr_is_ram,
                        ppuctrl: nes_data.ppuctrl,
                        ppumask: nes_data.ppumask,
                        scroll_x: nes_data.scroll_x,
                        scroll_y: nes_data.scroll_y,
                        mirroring: nes_data.mirroring,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);

                    // Update cartridge info
                    if let Some(cart_info) = s.get_cartridge_info() {
                        let cart_data = egui_ui::CartridgeData {
                            system_name: "NES".to_string(),
                            crc32: cart_info.crc32,
                            rom_size: cart_info.prg_size + cart_info.chr_size,
                            nes_mapper: Some(cart_info.mapper),
                            nes_submapper: Some(cart_info.submapper),
                            nes_mapper_name: Some(cart_info.mapper_name),
                            nes_mirroring: Some(cart_info.mirroring),
                            nes_timing: Some(format!("{:?}", cart_info.timing)),
                            nes_prg_size: Some(cart_info.prg_size),
                            nes_chr_size: Some(cart_info.chr_size),
                            nes_header_mapper: Some(cart_info.header_mapper),
                            nes_header_submapper: Some(cart_info.header_submapper),
                            nes_header_mirroring: Some(cart_info.header_mirroring),
                            nes_db_mapper_override: cart_info.db_mapper_override,
                            nes_db_mirroring_override: cart_info.db_mirroring_override,
                            nes_board_name: cart_info.board_name,
                            snes_has_smc_header: None,
                            snes_mapping_mode: None,
                            snes_chip_type: None,
                        };
                        egui_app.tab_manager.update_cartridge_data(cart_data);
                    }
                }
                EmulatorSystem::GameBoy(s) => {
                    let gb_data = s.get_tile_viewer_data();
                    let tile_data = egui_ui::SystemTileData::GameBoy(egui_ui::GbTileData {
                        vram_bank0: gb_data.vram_bank0,
                        vram_bank1: gb_data.vram_bank1,
                        oam: gb_data.oam,
                        bg_palettes: gb_data.bg_palettes,
                        obj_palettes: gb_data.obj_palettes,
                        lcdc: gb_data.lcdc,
                        scx: gb_data.scx,
                        scy: gb_data.scy,
                        wx: gb_data.wx,
                        wy: gb_data.wy,
                        is_cgb_mode: gb_data.is_cgb_mode,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);
                }
                EmulatorSystem::GBA(s) => {
                    let gba_data = s.get_tile_viewer_data();
                    let tile_data = egui_ui::SystemTileData::GBA(egui_ui::GbaTileData {
                        vram: gba_data.vram,
                        palette_ram: gba_data.palette_ram,
                        oam: gba_data.oam,
                        master_palette: gba_data.master_palette,
                        dispcnt: gba_data.dispcnt,
                        bg0cnt: gba_data.bg0cnt,
                        bg1cnt: gba_data.bg1cnt,
                        bg2cnt: gba_data.bg2cnt,
                        bg3cnt: gba_data.bg3cnt,
                        bg_scroll: gba_data.bg_scroll,
                        bldcnt: gba_data.bldcnt,
                        bldalpha: gba_data.bldalpha,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);

                    // Update cartridge info if available
                    if let Some(header) = s.cartridge_header() {
                        let cart_data = egui_ui::CartridgeData {
                            system_name: "GBA".to_string(),
                            crc32: 0, // TODO: Calculate CRC32 if needed
                            rom_size: s.rom_size(),
                            nes_mapper: None,
                            nes_submapper: None,
                            nes_mapper_name: None,
                            nes_mirroring: None,
                            nes_timing: None,
                            nes_prg_size: None,
                            nes_chr_size: None,
                            nes_header_mapper: None,
                            nes_header_submapper: None,
                            nes_header_mirroring: None,
                            nes_db_mapper_override: false,
                            nes_db_mirroring_override: false,
                            nes_board_name: Some(format!(
                                "{} - {}",
                                header.title, header.game_code
                            )),
                            snes_has_smc_header: None,
                            snes_mapping_mode: None,
                            snes_chip_type: None,
                        };
                        egui_app.tab_manager.update_cartridge_data(cart_data);
                    }
                }
                EmulatorSystem::SMS(s) => {
                    let sms_data = s.get_tile_viewer_data();
                    let tile_data = egui_ui::SystemTileData::SMS(egui_ui::SmsTileData {
                        vram: sms_data.vram,
                        cram: sms_data.cram,
                        palette: sms_data.palette,
                        registers: sms_data.registers,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);
                }
                EmulatorSystem::ColecoVision(s) => {
                    let coleco_data = s.get_tile_viewer_data();
                    let tile_data =
                        egui_ui::SystemTileData::ColecoVision(egui_ui::ColecoVisionTileData {
                            vram: coleco_data.vram,
                            palette: coleco_data.palette,
                            registers: coleco_data.registers,
                        });
                    egui_app.tab_manager.update_system_tile_data(tile_data);
                }
                EmulatorSystem::SNES(s) => {
                    let snes_data = s.get_tile_viewer_data();
                    let tile_data = egui_ui::SystemTileData::SNES(egui_ui::SnesTileData {
                        vram: snes_data.vram,
                        cgram: snes_data.cgram,
                        oam: snes_data.oam,
                        palette: snes_data.palette,
                        bg_mode: snes_data.bg_mode,
                        screen_enabled: snes_data.screen_enabled,
                        bg1sc: snes_data.bg1sc,
                        bg2sc: snes_data.bg2sc,
                        bg3sc: snes_data.bg3sc,
                        bg4sc: snes_data.bg4sc,
                        bg12nba: snes_data.bg12nba,
                        bg34nba: snes_data.bg34nba,
                        bg1_hofs: snes_data.bg1_hofs,
                        bg1_vofs: snes_data.bg1_vofs,
                        bg2_hofs: snes_data.bg2_hofs,
                        bg2_vofs: snes_data.bg2_vofs,
                        bg3_hofs: snes_data.bg3_hofs,
                        bg3_vofs: snes_data.bg3_vofs,
                        bg4_hofs: snes_data.bg4_hofs,
                        bg4_vofs: snes_data.bg4_vofs,
                        tm: snes_data.tm,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);

                    // Update cartridge info
                    if let Some(cart_info) = s.get_cartridge_info() {
                        let cart_data = egui_ui::CartridgeData {
                            system_name: "SNES".to_string(),
                            crc32: cart_info.crc32,
                            rom_size: cart_info.rom_size,
                            nes_mapper: None,
                            nes_submapper: None,
                            nes_mapper_name: None,
                            nes_mirroring: None,
                            nes_timing: None,
                            nes_prg_size: None,
                            nes_chr_size: None,
                            nes_header_mapper: None,
                            nes_header_submapper: None,
                            nes_header_mirroring: None,
                            nes_db_mapper_override: false,
                            nes_db_mirroring_override: false,
                            nes_board_name: None,
                            snes_has_smc_header: Some(cart_info.has_smc_header),
                            snes_mapping_mode: Some(cart_info.mapping_mode),
                            snes_chip_type: Some(cart_info.chip_type),
                        };
                        egui_app.tab_manager.update_cartridge_data(cart_data);
                    }
                }
                EmulatorSystem::Atari2600(s) => {
                    if let Some(inspector_data) = s.get_inspector_data() {
                        let tile_data =
                            egui_ui::SystemTileData::Atari2600(egui_ui::Atari2600TileData {
                                pf0: inspector_data.pf0,
                                pf1: inspector_data.pf1,
                                pf2: inspector_data.pf2,
                                playfield_reflect: inspector_data.playfield_reflect,
                                playfield_score_mode: inspector_data.playfield_score_mode,
                                playfield_priority: inspector_data.playfield_priority,
                                grp0: inspector_data.grp0,
                                grp1: inspector_data.grp1,
                                player0_x: inspector_data.player0_x,
                                player1_x: inspector_data.player1_x,
                                player0_reflect: inspector_data.player0_reflect,
                                player1_reflect: inspector_data.player1_reflect,
                                nusiz0: inspector_data.nusiz0,
                                nusiz1: inspector_data.nusiz1,
                                enam0: inspector_data.enam0,
                                enam1: inspector_data.enam1,
                                missile0_x: inspector_data.missile0_x,
                                missile1_x: inspector_data.missile1_x,
                                enabl: inspector_data.enabl,
                                ball_x: inspector_data.ball_x,
                                ball_size: inspector_data.ball_size,
                                colubk: inspector_data.colubk,
                                colupf: inspector_data.colupf,
                                colup0: inspector_data.colup0,
                                colup1: inspector_data.colup1,
                                master_palette: emu_atari2600::tia::Tia::get_ntsc_palette(),
                                cxm0p: inspector_data.cxm0p,
                                cxm1p: inspector_data.cxm1p,
                                cxp0fb: inspector_data.cxp0fb,
                                cxp1fb: inspector_data.cxp1fb,
                                cxm0fb: inspector_data.cxm0fb,
                                cxm1fb: inspector_data.cxm1fb,
                                cxblpf: inspector_data.cxblpf,
                                cxppmm: inspector_data.cxppmm,
                                vblank: inspector_data.vblank,
                                vsync: inspector_data.vsync,
                            });
                        egui_app.tab_manager.update_system_tile_data(tile_data);
                    }
                }
                EmulatorSystem::PC(s) => {
                    // Update PC BDA data for inspector
                    let bda_data = s.read_bda_inspector_data();
                    let pc_bda = egui_ui::PcBdaData {
                        equipment_word: bda_data.equipment_word,
                        memory_size_kb: bda_data.memory_size_kb,
                        video_mode: bda_data.video_mode,
                        video_columns: bda_data.video_columns,
                        num_serial_ports: bda_data.num_serial_ports,
                        num_parallel_ports: bda_data.num_parallel_ports,
                        num_hard_drives: bda_data.num_hard_drives,
                        bda_raw: bda_data.bda_raw,
                        ebda_raw: bda_data.ebda_raw,
                        ebda_segment: bda_data.ebda_segment,
                    };
                    egui_app.tab_manager.update_pc_bda_data(pc_bda);
                }
                EmulatorSystem::Chip8(s) => {
                    let inspector_data = s.get_inspector_data();
                    let tile_data = egui_ui::SystemTileData::Chip8(egui_ui::Chip8TileData {
                        v_registers: inspector_data.v_registers,
                        i: inspector_data.i,
                        pc: inspector_data.pc,
                        sp: inspector_data.sp,
                        stack: inspector_data.stack,
                        delay_timer: inspector_data.delay_timer,
                        sound_timer: inspector_data.sound_timer,
                        display_plane0: inspector_data.display_plane0,
                        display_plane1: inspector_data.display_plane1,
                        display_width: inspector_data.display_width,
                        display_height: inspector_data.display_height,
                        mode: inspector_data.mode,
                        selected_plane: inspector_data.selected_plane,
                        high_res: inspector_data.high_res,
                        waiting_for_key: inspector_data.waiting_for_key,
                        keys: inspector_data.keys,
                    });
                    egui_app.tab_manager.update_system_tile_data(tile_data);
                }
                EmulatorSystem::PS1(s) => {
                    let gpu_data = s.get_gpu_inspector_data();
                    let ps1_gpu = egui_ui::Ps1GpuData {
                        gpustat: gpu_data.gpustat,
                        display_vram_x: gpu_data.display_vram_x,
                        display_vram_y: gpu_data.display_vram_y,
                        display_horiz_start: gpu_data.display_horiz_start,
                        display_horiz_end: gpu_data.display_horiz_end,
                        display_vert_start: gpu_data.display_vert_start,
                        display_vert_end: gpu_data.display_vert_end,
                        hres: gpu_data.hres_str,
                        vres: gpu_data.vres_str,
                        is_pal: gpu_data.is_pal,
                        display_24bit: gpu_data.display_24bit,
                        interlace: gpu_data.interlace,
                        display_disabled: gpu_data.display_disabled,
                        draw_area_left: gpu_data.draw_area_left,
                        draw_area_top: gpu_data.draw_area_top,
                        draw_area_right: gpu_data.draw_area_right,
                        draw_area_bottom: gpu_data.draw_area_bottom,
                        draw_offset_x: gpu_data.draw_offset_x,
                        draw_offset_y: gpu_data.draw_offset_y,
                        texpage_x: gpu_data.texpage_x,
                        texpage_y: gpu_data.texpage_y,
                        tex_depth: gpu_data.tex_depth_str,
                        semi_transparency: gpu_data.semi_transparency_str,
                        dithering: gpu_data.dithering,
                        set_mask_bit: gpu_data.set_mask_bit,
                        check_mask_bit: gpu_data.check_mask_bit,
                        tex_window_mask_x: gpu_data.tex_window_mask_x,
                        tex_window_mask_y: gpu_data.tex_window_mask_y,
                        tex_window_offset_x: gpu_data.tex_window_offset_x,
                        tex_window_offset_y: gpu_data.tex_window_offset_y,
                        scanline: gpu_data.scanline,
                        in_vblank: gpu_data.in_vblank,
                        irq: gpu_data.irq,
                    };
                    egui_app.tab_manager.update_ps1_gpu_data(ps1_gpu);
                }
                _ => {
                    // Other systems don't have tile viewers yet
                }
            }

            // Update mount point information for Mounts tab
            let mount_points = sys.mount_points();
            let mount_info: Vec<egui_ui::MountInfo> = mount_points
                .into_iter()
                .map(|mp| {
                    let mounted_file = runtime_state.current_mounts.get(&mp.id).cloned();
                    egui_ui::MountInfo {
                        id: mp.id,
                        name: mp.name,
                        extensions: mp.extensions,
                        required: mp.required,
                        mounted_file,
                    }
                })
                .collect();
            egui_app.tab_manager.update_mount_info(mount_info);
        }

        // Update menu bar system loaded state before rendering UI
        egui_app.set_system_loaded(rom_loaded, sys.system_name());

        // Render egui UI
        egui_app.ui(egui_backend.egui_ctx(), settings.scaling_mode);

        // Handle menu actions
        if let Some(action) = egui_app.menu_bar.take_action() {
            use egui_ui::menu_bar::MenuAction;
            match action {
                MenuAction::NewProjectSystem(system_name) => {
                    // Create a new system based on the selected type
                    // Clear any existing system state
                    rom_loaded = false;
                    rom_hash = None;
                    runtime_state.clear_mounts();
                    _game_saves = GameSaves::default();

                    // Clear debug and inspector state when switching systems
                    egui_app.tab_manager.clear_debug_state();

                    match system_name.as_str() {
                        "NES" => {
                            let gl_ctx = egui_backend.gl_context();
                            let nes_sys = create_nes_system(&settings.video_backend, gl_ctx);
                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "NES",
                                &mut rom_loaded,
                                "Created new NES system",
                                &runtime_state,
                            );
                        }
                        "Game Boy" => {
                            sys = EmulatorSystem::GameBoy(Box::new(emu_gb::GbSystem::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "Game Boy",
                                &mut rom_loaded,
                                "Created new Game Boy system",
                                &runtime_state,
                            );
                        }
                        "GBA" => {
                            sys = EmulatorSystem::GBA(Box::new(hemu_gba::GbaSystem::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "GBA",
                                &mut rom_loaded,
                                "Created new GBA system",
                                &runtime_state,
                            );
                        }
                        "Atari 2600" => {
                            sys = EmulatorSystem::Atari2600(Box::new(create_atari2600_system(
                                &settings,
                            )));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "Atari 2600",
                                &mut rom_loaded,
                                "Created new Atari 2600 system",
                                &runtime_state,
                            );
                        }
                        "Atari 5200" => {
                            sys = EmulatorSystem::Atari5200(Box::new(create_atari5200_system(
                                &settings,
                            )));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "Atari 5200",
                                &mut rom_loaded,
                                "Created new Atari 5200 system",
                                &runtime_state,
                            );
                        }
                        "Mega Drive" => {
                            sys = EmulatorSystem::MegaDrive(Box::new(create_megadrive_system(
                                &settings,
                            )));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "Mega Drive",
                                &mut rom_loaded,
                                "Created new Mega Drive system",
                                &runtime_state,
                            );
                        }
                        "SMS" => {
                            sys = EmulatorSystem::SMS(Box::new(emu_sms::SmsSystem::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "SMS",
                                &mut rom_loaded,
                                "Created new SMS system",
                                &runtime_state,
                            );
                        }
                        "ColecoVision" => {
                            sys = EmulatorSystem::ColecoVision(Box::new(
                                emu_colecovision::ColecoVisionSystem::new(),
                            ));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "ColecoVision",
                                &mut rom_loaded,
                                "Created new ColecoVision system",
                                &runtime_state,
                            );
                        }
                        "SG-1000" => {
                            sys = EmulatorSystem::SG1000(Box::new(emu_sg1000::Sg1000System::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "SG-1000",
                                &mut rom_loaded,
                                "Created new SG-1000 system",
                                &runtime_state,
                            );
                        }
                        "PS1" => {
                            sys = EmulatorSystem::PS1(Box::new(emu_ps1::Ps1System::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "PS1",
                                &mut rom_loaded,
                                "Created new PS1 system",
                                &runtime_state,
                            );
                        }
                        "CHIP-8" => {
                            sys = EmulatorSystem::Chip8(Box::new(emu_chip8::Chip8System::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "CHIP-8",
                                &mut rom_loaded,
                                "Created new CHIP-8 system",
                                &runtime_state,
                            );
                        }
                        "SNES" => {
                            sys = EmulatorSystem::SNES(Box::new(emu_snes::SnesSystem::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "SNES",
                                &mut rom_loaded,
                                "Created new SNES system",
                                &runtime_state,
                            );
                        }
                        "N64" => {
                            let gl_ctx = egui_backend.gl_context();
                            match create_n64_system(gl_ctx, &settings) {
                                Ok(n64_sys) => {
                                    sys = EmulatorSystem::N64(Box::new(n64_sys));
                                    configure_system_ui(
                                        &mut egui_app,
                                        &sys,
                                        "N64",
                                        &mut rom_loaded,
                                        "Created new N64 system",
                                        &runtime_state,
                                    );
                                }
                                Err(e) => {
                                    egui_app
                                        .status_bar
                                        .set_error(format!("Failed to create N64 system: {}", e));
                                    // Ensure rom_loaded stays false on error
                                }
                            }
                        }
                        "PC" => {
                            sys = EmulatorSystem::PC(Box::new(emu_pc::PcSystem::new()));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "PC",
                                &mut rom_loaded,
                                "Created new PC system",
                                &runtime_state,
                            );
                        }
                        "Game & Watch" => {
                            sys = EmulatorSystem::GameAndWatch(Box::new(
                                emu_gameandwatch::GameAndWatchSystem::new(),
                            ));
                            configure_system_ui(
                                &mut egui_app,
                                &sys,
                                "Game & Watch",
                                &mut rom_loaded,
                                "Created new Game & Watch system",
                                &runtime_state,
                            );
                        }
                        _ => {
                            egui_app
                                .status_bar
                                .set_error(format!("Unknown system: {}", system_name));
                            // Ensure rom_loaded stays false for unknown systems
                        }
                    }
                }
                MenuAction::NewProjectAutoDetect => {
                    // Open ROM file dialog with comprehensive extension support
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "All ROM Files",
                            &[
                                "nes", "unf", "gb", "gbc", "gba", "bin", "a26", "a52", "smc",
                                "sfc", "z64", "n64", "v64", "com", "exe", "sms", "ch8", "c8",
                                "col", "sg", "sc", "gw", "gnw", "mgw", "md", "gen", "smd",
                            ],
                        )
                        .add_filter("NES ROMs", &["nes", "unf"])
                        .add_filter("Game Boy ROMs", &["gb", "gbc"])
                        .add_filter("GBA ROMs", &["gba"])
                        .add_filter("Atari 2600 ROMs", &["a26", "bin"])
                        .add_filter("Atari 5200 ROMs", &["a52", "bin"])
                        .add_filter("Mega Drive ROMs", &["md", "gen", "smd", "bin"])
                        .add_filter("SNES ROMs", &["smc", "sfc", "bin"])
                        .add_filter("N64 ROMs", &["z64", "n64", "v64", "bin"])
                        .add_filter("SMS ROMs", &["sms", "bin"])
                        .add_filter("ColecoVision ROMs", &["col", "bin"])
                        .add_filter("SG-1000 ROMs", &["sg", "sc", "bin"])
                        .add_filter("CHIP-8 Programs", &["ch8", "c8"])
                        .add_filter("Game & Watch ROMs", &["gw", "gnw", "mgw"])
                        .add_filter("PC Executables", &["com", "exe", "bin"])
                        .add_filter("All Files", &["*"])
                        .pick_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                // Get file extension for extension-aware detection
                                let extension = path.extension().and_then(|e| e.to_str());

                                // Use current system as hint for ambiguous formats
                                let preferred_system = if rom_loaded {
                                    Some(sys.system_type())
                                } else {
                                    None
                                };

                                // Clear debug and inspector state when loading a new ROM
                                // This handles both system switches and ROM changes within the same system
                                egui_app.tab_manager.clear_debug_state();

                                match detect_rom_type_with_extension(
                                    &data,
                                    extension,
                                    preferred_system,
                                ) {
                                    Ok(SystemType::NES) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let gl_ctx = egui_backend.gl_context();
                                        let mut nes_sys =
                                            create_nes_system(&settings.video_backend, gl_ctx);
                                        if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                            egui_app.status_bar.set_error(format!(
                                                "Failed to load NES ROM: {}",
                                                e
                                            ));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                                            egui_app.property_pane.system_name = "NES".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            // Add to recent files
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_success(
                                                "NES ROM loaded successfully".to_string(),
                                            );
                                            // Update resolution
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::GameBoy) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gb_sys = emu_gb::GbSystem::new();
                                        if let Err(e) = gb_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GameBoy(Box::new(gb_sys));
                                            egui_app.property_pane.system_name =
                                                "Game Boy".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            // Add to recent files
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Game Boy ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::GBA) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gba_sys = hemu_gba::GbaSystem::new();
                                        if let Err(e) = gba_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GBA(Box::new(gba_sys));
                                            egui_app.property_pane.system_name = "GBA".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("GBA ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Atari2600) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut a2600_sys = create_atari2600_system(&settings);
                                        if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                                            egui_app.property_pane.system_name =
                                                "Atari 2600".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            // Add to recent files
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Atari 2600 ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Atari5200) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut a5200_sys = create_atari5200_system(&settings);

                                        // Atari 5200 BIOS - optional auto-search
                                        let bios_candidates = [
                                            "5200.rom",
                                            "5200.bin",
                                            "ataribas.rom",
                                            "bios.rom",
                                            "bios.bin",
                                        ];
                                        let bios_result = load_bios(
                                            None,
                                            Some(&path_str),
                                            &bios_candidates,
                                            None,
                                        );
                                        if let Some((bios_data, bios_path)) = bios_result {
                                            if a5200_sys.mount("BIOS", &bios_data).is_ok() {
                                                runtime_state
                                                    .set_mount("BIOS".to_string(), bios_path);
                                            }
                                        }

                                        if let Err(e) = a5200_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Atari5200(Box::new(a5200_sys));
                                            egui_app.property_pane.system_name =
                                                "Atari 5200".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Atari 5200 ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::MegaDrive) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut md_sys = create_megadrive_system(&settings);
                                        if let Err(e) = md_sys.mount("cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::MegaDrive(Box::new(md_sys));
                                            egui_app.property_pane.system_name =
                                                "Mega Drive".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Mega Drive ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::PC) => {
                                        rom_hash = None; // PC systems don't use ROM hash
                                        let mut pc_sys = emu_pc::PcSystem::new();

                                        // Determine correct mount point from extension / size
                                        let ext_str =
                                            extension.map(|e| e.to_lowercase()).unwrap_or_default();
                                        let (mount_id, msg) =
                                            pc_disk_mount_target(&ext_str, data.len());

                                        if mount_id.is_empty() {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::PC(Box::new(pc_sys));
                                            egui_app.property_pane.system_name = "PC".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(msg.to_string());
                                            let _ = sys.resolution();
                                        } else if let Err(e) = pc_sys.mount(mount_id, &data) {
                                            egui_app.status_bar.set_error(format!("Error: {}", e));
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::PC(Box::new(pc_sys));
                                            egui_app.property_pane.system_name = "PC".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state
                                                .set_mount(mount_id.to_string(), path_str.clone());
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(msg.to_string());
                                            let _ = sys.resolution();
                                        }
                                    }
                                    Ok(SystemType::SNES) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut snes_sys = emu_snes::SnesSystem::new();
                                        if let Err(e) = snes_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SNES(Box::new(snes_sys));
                                            egui_app.property_pane.system_name = "SNES".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            // Add to recent files
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("SNES ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::N64) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let gl_ctx = egui_backend.gl_context();
                                        match create_n64_system(gl_ctx, &settings) {
                                            Ok(mut n64_sys) => {
                                                if let Err(e) = n64_sys.mount("Cartridge", &data) {
                                                    egui_app
                                                        .status_bar
                                                        .set_message(format!("Error: {}", e));
                                                    rom_hash = None;
                                                } else {
                                                    rom_loaded = true;
                                                    sys = EmulatorSystem::N64(Box::new(n64_sys));

                                                    // Enable OpenGL renderer for N64
                                                    if let Some(renderer_name) =
                                                        enable_n64_opengl_renderer(
                                                            &mut sys,
                                                            &egui_backend,
                                                        )
                                                    {
                                                        egui_app.property_pane.rendering_backend =
                                                            renderer_name;
                                                    } else {
                                                        egui_app.property_pane.rendering_backend =
                                                            sys.get_current_renderer_name();
                                                    }

                                                    egui_app.property_pane.system_name =
                                                        "N64".to_string();
                                                    // Set renderer display based on settings preference
                                                    egui_app.property_pane.rendering_backend =
                                                        if settings.video_backend == "opengl" {
                                                            "Hardware".to_string()
                                                        } else {
                                                            "Software".to_string()
                                                        };
                                                    egui_app.property_pane.available_renderers =
                                                        sys.get_available_renderers();
                                                    runtime_state.set_mount(
                                                        "Cartridge".to_string(),
                                                        path_str.clone(),
                                                    );
                                                    // Add to recent files
                                                    settings.add_recent_file(path_str.clone());
                                                    if let Err(e) = settings.save() {
                                                        eprintln!(
                                                            "Warning: Failed to save settings: {}",
                                                            e
                                                        );
                                                    }
                                                    egui_app.update_recent_files(
                                                        settings.get_recent_files().to_vec(),
                                                    );
                                                    egui_app
                                                        .status_bar
                                                        .set_message("N64 ROM loaded".to_string());
                                                    let _ = sys.resolution();
                                                    // Load save states for this ROM
                                                    if let Some(ref hash) = rom_hash {
                                                        _game_saves = GameSaves::load(hash);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                egui_app.status_bar.set_message(format!(
                                                    "Failed to create N64 system: {}",
                                                    e
                                                ));
                                                rom_hash = None;
                                            }
                                        }
                                    }
                                    Ok(SystemType::SMS) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut sms_sys = emu_sms::SmsSystem::new();
                                        if let Err(e) = sms_sys.mount("cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SMS(Box::new(sms_sys));
                                            egui_app.property_pane.system_name = "SMS".to_string();
                                            runtime_state.set_mount(
                                                "cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app
                                                .status_bar
                                                .set_message("SMS ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Chip8) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut chip8_sys = emu_chip8::Chip8System::new();
                                        if let Err(e) = chip8_sys.mount("Program", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Chip8(Box::new(chip8_sys));
                                            egui_app.property_pane.system_name =
                                                "CHIP-8".to_string();
                                            runtime_state
                                                .set_mount("Program".to_string(), path_str.clone());
                                            // Add to recent files
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("CHIP-8 program loaded".to_string());
                                            let _ = sys.resolution();
                                            // Load save states for this ROM
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::ColecoVision) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut coleco_sys =
                                            emu_colecovision::ColecoVisionSystem::new();
                                        if let Err(e) = coleco_sys.mount("Cartridge", &data) {
                                            egui_app.status_bar.set_message(format!(
                                                "Error: {} (Note: ColecoVision requires BIOS)",
                                                e
                                            ));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys =
                                                EmulatorSystem::ColecoVision(Box::new(coleco_sys));
                                            egui_app.property_pane.system_name =
                                                "ColecoVision".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "ColecoVision cartridge loaded (BIOS required)"
                                                    .to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::SG1000) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut sg1000_sys = emu_sg1000::Sg1000System::new();
                                        if let Err(e) = sg1000_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SG1000(Box::new(sg1000_sys));
                                            egui_app.property_pane.system_name =
                                                "SG-1000".to_string();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                path_str.clone(),
                                            );
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "SG-1000 cartridge loaded".to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::PS1) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let ps1_sys = emu_ps1::Ps1System::new();
                                        sys = EmulatorSystem::PS1(Box::new(ps1_sys));
                                        // If data is a PS-X EXE, mount it as disc
                                        if data.len() >= 8 && &data[0..8] == b"PS-X EXE" {
                                            if let Err(e) = sys.mount("disc", &data) {
                                                eprintln!("Failed to load PS-X EXE: {}", e);
                                            }
                                        }
                                        rom_loaded = true;
                                        egui_app.property_pane.system_name = "PS1".to_string();
                                        runtime_state
                                            .set_mount("disc".to_string(), path_str.clone());
                                        settings.add_recent_file(path_str.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        egui_app.update_recent_files(
                                            settings.get_recent_files().to_vec(),
                                        );
                                        egui_app
                                            .status_bar
                                            .set_message("PS1 disc loaded".to_string());
                                        let _ = sys.resolution();
                                        if let Some(ref hash) = rom_hash {
                                            _game_saves = GameSaves::load(hash);
                                        }
                                    }
                                    Ok(SystemType::GameAndWatch) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gw_sys =
                                            emu_gameandwatch::GameAndWatchSystem::new();
                                        if let Err(e) = gw_sys.mount("Program", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GameAndWatch(Box::new(gw_sys));
                                            egui_app.property_pane.system_name =
                                                "Game & Watch".to_string();
                                            runtime_state
                                                .set_mount("Program".to_string(), path_str.clone());
                                            settings.add_recent_file(path_str.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "Game & Watch program loaded".to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        egui_app
                                            .status_bar
                                            .set_error(format!("Failed to detect ROM type: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                egui_app
                                    .status_bar
                                    .set_error(format!("Failed to read file: {}", e));
                            }
                        }
                    }
                }
                MenuAction::OpenRecentFile(file_path) => {
                    // Determine if this is a .hemu project or a ROM file
                    let path = PathBuf::from(&file_path);

                    if file_path.ends_with(".hemu") {
                        // Load as a project file
                        match HemuProject::load(&file_path) {
                            Ok(project) => {
                                if project.system != "pc" {
                                    egui_app.status_bar.set_message(format!(
                                        "Currently only PC system .hemu projects are supported. Project is for: {}",
                                        project.system
                                    ));
                                } else {
                                    // Parse configuration from project
                                    let cpu_model = if let Some(cpu_str) = project.get_cpu_model() {
                                        match cpu_str.as_str() {
                                            "Intel8086" => emu_core::cpu_8086::CpuModel::Intel8086,
                                            "Intel8088" => emu_core::cpu_8086::CpuModel::Intel8088,
                                            "Intel80186" => {
                                                emu_core::cpu_8086::CpuModel::Intel80186
                                            }
                                            "Intel80188" => {
                                                emu_core::cpu_8086::CpuModel::Intel80188
                                            }
                                            "Intel80286" => {
                                                emu_core::cpu_8086::CpuModel::Intel80286
                                            }
                                            "Intel80386" => {
                                                emu_core::cpu_8086::CpuModel::Intel80386
                                            }
                                            "Intel80486" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486
                                            }
                                            "Intel80486SX" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486SX
                                            }
                                            "Intel80486DX2" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486DX2
                                            }
                                            "Intel80486SX2" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486SX2
                                            }
                                            "Intel80486DX4" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486DX4
                                            }
                                            "IntelPentium" => {
                                                emu_core::cpu_8086::CpuModel::IntelPentium
                                            }
                                            "IntelPentiumMMX" => {
                                                emu_core::cpu_8086::CpuModel::IntelPentiumMMX
                                            }
                                            _ => {
                                                eprintln!("Unknown CPU model: {}, using default Intel8086", cpu_str);
                                                emu_core::cpu_8086::CpuModel::Intel8086
                                            }
                                        }
                                    } else {
                                        emu_core::cpu_8086::CpuModel::Intel8086 // Default
                                    };

                                    let memory_kb = project.get_memory_kb().unwrap_or(640);

                                    // Create video adapter based on project configuration
                                    let video_adapter: Box<dyn emu_pc::VideoAdapter> =
                                        if let Some(video_str) = project.get_video_mode() {
                                            match video_str.as_str() {
                                                "EGA" => {
                                                    Box::new(emu_pc::SoftwareEgaAdapter::new())
                                                }
                                                "VGA" => {
                                                    Box::new(emu_pc::SoftwareVgaAdapter::new())
                                                }
                                                "CGA" => {
                                                    Box::new(emu_pc::SoftwareCgaAdapter::new())
                                                }
                                                _ => Box::new(emu_pc::SoftwareCgaAdapter::new()),
                                            }
                                        } else {
                                            Box::new(emu_pc::SoftwareCgaAdapter::new())
                                        };

                                    // Create PC system with configuration
                                    let mut pc_sys = emu_pc::PcSystem::with_config(
                                        cpu_model,
                                        memory_kb,
                                        video_adapter,
                                    );

                                    // Set boot priority
                                    let boot_priority = project
                                        .get_boot_priority()
                                        .map(|s| s.as_str())
                                        .unwrap_or("FloppyFirst");
                                    let priority = match boot_priority {
                                        "HardDriveFirst" => emu_pc::BootPriority::HardDriveFirst,
                                        "FloppyOnly" => emu_pc::BootPriority::FloppyOnly,
                                        "HardDriveOnly" => emu_pc::BootPriority::HardDriveOnly,
                                        _ => emu_pc::BootPriority::FloppyFirst,
                                    };
                                    pc_sys.set_boot_priority(priority);

                                    // Mount files from project
                                    // Resolve paths relative to the .hemu file's directory
                                    let project_dir =
                                        path.parent().unwrap_or_else(|| std::path::Path::new("."));
                                    for (mount_id, relative_path) in &project.mounts {
                                        let full_path = project_dir.join(relative_path);
                                        match fs::read(&full_path) {
                                            Ok(data) => {
                                                if let Err(e) = pc_sys.mount(mount_id, &data) {
                                                    eprintln!(
                                                        "Failed to mount {}: {}",
                                                        mount_id, e
                                                    );
                                                } else {
                                                    runtime_state.set_mount(
                                                        mount_id.clone(),
                                                        full_path.to_string_lossy().to_string(),
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Failed to read {}: {}",
                                                    relative_path, e
                                                );
                                            }
                                        }
                                    }

                                    // Update POST screen with mount status
                                    pc_sys.update_post_screen();
                                    // Clear debug and inspector state when loading a project
                                    egui_app.tab_manager.clear_debug_state();

                                    sys = EmulatorSystem::PC(Box::new(pc_sys));
                                    rom_loaded = true;
                                    egui_app.property_pane.system_name = "PC".to_string();
                                    egui_app.property_pane.rendering_backend =
                                        sys.get_current_renderer_name();
                                    egui_app.property_pane.available_renderers =
                                        sys.get_available_renderers();

                                    // Add project to recent files
                                    settings.add_recent_file(file_path.clone());
                                    if let Err(e) = settings.save() {
                                        eprintln!("Warning: Failed to save settings: {}", e);
                                    }
                                    egui_app
                                        .update_recent_files(settings.get_recent_files().to_vec());

                                    egui_app.status_bar.set_message(format!(
                                        "Project loaded: {}",
                                        path.file_name().unwrap_or_default().to_string_lossy()
                                    ));
                                }
                            }
                            Err(e) => {
                                egui_app
                                    .status_bar
                                    .set_message(format!("Failed to load project: {}", e));
                            }
                        }
                    } else {
                        // Load as a ROM file
                        match fs::read(&path) {
                            Ok(data) => {
                                // Get file extension for extension-aware detection
                                let extension = path.extension().and_then(|e| e.to_str());

                                // Use current system as hint for ambiguous formats
                                let preferred_system = if rom_loaded {
                                    Some(sys.system_type())
                                } else {
                                    None
                                };

                                // Clear debug and inspector state when loading a new ROM
                                egui_app.tab_manager.clear_debug_state();

                                match detect_rom_type_with_extension(
                                    &data,
                                    extension,
                                    preferred_system,
                                ) {
                                    Ok(SystemType::NES) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut nes_sys = emu_nes::NesSystem::default();
                                        if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                            egui_app.status_bar.set_error(format!(
                                                "Failed to load NES ROM: {}",
                                                e
                                            ));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;

                                            // Apply renderer preference if OpenGL is requested
                                            #[cfg(feature = "opengl")]
                                            if settings.video_backend == "opengl" {
                                                if let Some(gl) = egui_backend.gl_context() {
                                                    if let Err(e) =
                                                        nes_sys.enable_opengl_renderer(gl)
                                                    {
                                                        eprintln!(
                                                            "Failed to enable OpenGL renderer: {}",
                                                            e
                                                        );
                                                        egui_app.tab_manager.add_log(format!(
                                                        "Failed to enable OpenGL renderer, using Software: {}",
                                                        e
                                                    ));
                                                    }
                                                }
                                            }

                                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                                            egui_app.property_pane.system_name = "NES".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            // Add to recent files (already in list since it was clicked from recent files)
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_success(
                                                "NES ROM loaded successfully".to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::GameBoy) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gb_sys = emu_gb::GbSystem::new();
                                        if let Err(e) = gb_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GameBoy(Box::new(gb_sys));
                                            egui_app.property_pane.system_name =
                                                "Game Boy".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Game Boy ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::GBA) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gba_sys = hemu_gba::GbaSystem::new();
                                        if let Err(e) = gba_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GBA(Box::new(gba_sys));
                                            egui_app.property_pane.system_name = "GBA".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("GBA ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Atari2600) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut a2600_sys = create_atari2600_system(&settings);
                                        if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                                            egui_app.property_pane.system_name =
                                                "Atari 2600".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Atari 2600 ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Atari5200) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut a5200_sys = create_atari5200_system(&settings);

                                        // Atari 5200 BIOS - optional auto-search
                                        let bios_candidates = [
                                            "5200.rom",
                                            "5200.bin",
                                            "ataribas.rom",
                                            "bios.rom",
                                            "bios.bin",
                                        ];
                                        let bios_result = load_bios(
                                            None,
                                            Some(&file_path),
                                            &bios_candidates,
                                            None,
                                        );
                                        if let Some((bios_data, bios_path)) = bios_result {
                                            if a5200_sys.mount("BIOS", &bios_data).is_ok() {
                                                runtime_state
                                                    .set_mount("BIOS".to_string(), bios_path);
                                            }
                                        }

                                        if let Err(e) = a5200_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Atari5200(Box::new(a5200_sys));
                                            egui_app.property_pane.system_name =
                                                "Atari 5200".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Atari 5200 ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::MegaDrive) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut md_sys = create_megadrive_system(&settings);
                                        if let Err(e) = md_sys.mount("cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::MegaDrive(Box::new(md_sys));
                                            egui_app.property_pane.system_name =
                                                "Mega Drive".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("Mega Drive ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::PC) => {
                                        rom_hash = None; // PC systems don't use ROM hash
                                        let mut pc_sys = emu_pc::PcSystem::new();

                                        // Determine correct mount point from extension / size
                                        let ext_str =
                                            extension.map(|e| e.to_lowercase()).unwrap_or_default();
                                        let (mount_id, msg) =
                                            pc_disk_mount_target(&ext_str, data.len());

                                        if mount_id.is_empty() {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::PC(Box::new(pc_sys));
                                            egui_app.property_pane.system_name = "PC".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(msg.to_string());
                                            let _ = sys.resolution();
                                        } else if let Err(e) = pc_sys.mount(mount_id, &data) {
                                            egui_app.status_bar.set_error(format!("Error: {}", e));
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::PC(Box::new(pc_sys));
                                            egui_app.property_pane.system_name = "PC".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state
                                                .set_mount(mount_id.to_string(), file_path.clone());
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(msg.to_string());
                                            let _ = sys.resolution();
                                        }
                                    }
                                    Ok(SystemType::SNES) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut snes_sys = emu_snes::SnesSystem::new();
                                        if let Err(e) = snes_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SNES(Box::new(snes_sys));
                                            egui_app.property_pane.system_name = "SNES".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("SNES ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::N64) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let gl_ctx = egui_backend.gl_context();

                                        match create_n64_system(gl_ctx, &settings) {
                                            Ok(mut n64_sys) => {
                                                if let Err(e) = n64_sys.mount("Cartridge", &data) {
                                                    egui_app
                                                        .status_bar
                                                        .set_message(format!("Error: {}", e));
                                                    rom_hash = None;
                                                } else {
                                                    rom_loaded = true;
                                                    sys = EmulatorSystem::N64(Box::new(n64_sys));
                                                    egui_app.property_pane.system_name =
                                                        "N64".to_string();
                                                    egui_app.property_pane.rendering_backend =
                                                        sys.get_current_renderer_name();
                                                    egui_app.property_pane.available_renderers =
                                                        sys.get_available_renderers();
                                                    runtime_state.set_mount(
                                                        "Cartridge".to_string(),
                                                        file_path.clone(),
                                                    );
                                                    settings.add_recent_file(file_path.clone());
                                                    if let Err(e) = settings.save() {
                                                        eprintln!(
                                                            "Warning: Failed to save settings: {}",
                                                            e
                                                        );
                                                    }
                                                    egui_app.update_recent_files(
                                                        settings.get_recent_files().to_vec(),
                                                    );
                                                    egui_app
                                                        .status_bar
                                                        .set_message("N64 ROM loaded".to_string());
                                                    let _ = sys.resolution();
                                                    if let Some(ref hash) = rom_hash {
                                                        _game_saves = GameSaves::load(hash);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                egui_app.status_bar.set_message(format!(
                                                    "Failed to create N64 system: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                    Ok(SystemType::SMS) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut sms_sys = emu_sms::SmsSystem::new();
                                        if let Err(e) = sms_sys.mount("cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SMS(Box::new(sms_sys));
                                            egui_app.property_pane.system_name = "SMS".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("SMS ROM loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::Chip8) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut chip8_sys = emu_chip8::Chip8System::new();
                                        if let Err(e) = chip8_sys.mount("Program", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::Chip8(Box::new(chip8_sys));
                                            egui_app.property_pane.system_name =
                                                "CHIP-8".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Program".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app
                                                .status_bar
                                                .set_message("CHIP-8 program loaded".to_string());
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::ColecoVision) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut coleco_sys =
                                            emu_colecovision::ColecoVisionSystem::new();
                                        if let Err(e) = coleco_sys.mount("Cartridge", &data) {
                                            egui_app.status_bar.set_message(format!(
                                                "Error: {} (Note: ColecoVision requires BIOS)",
                                                e
                                            ));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys =
                                                EmulatorSystem::ColecoVision(Box::new(coleco_sys));
                                            egui_app.property_pane.system_name =
                                                "ColecoVision".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "ColecoVision cartridge loaded (BIOS required)"
                                                    .to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::SG1000) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut sg1000_sys = emu_sg1000::Sg1000System::new();
                                        if let Err(e) = sg1000_sys.mount("Cartridge", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::SG1000(Box::new(sg1000_sys));
                                            egui_app.property_pane.system_name =
                                                "SG-1000".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Cartridge".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "SG-1000 cartridge loaded".to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Ok(SystemType::PS1) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let ps1_sys = emu_ps1::Ps1System::new();
                                        sys = EmulatorSystem::PS1(Box::new(ps1_sys));
                                        if data.len() >= 8 && &data[0..8] == b"PS-X EXE" {
                                            if let Err(e) = sys.mount("disc", &data) {
                                                eprintln!("Failed to load PS-X EXE: {}", e);
                                            }
                                        }
                                        rom_loaded = true;
                                        egui_app.property_pane.system_name = "PS1".to_string();
                                        egui_app.property_pane.rendering_backend =
                                            sys.get_current_renderer_name();
                                        egui_app.property_pane.available_renderers =
                                            sys.get_available_renderers();
                                        runtime_state
                                            .set_mount("disc".to_string(), file_path.clone());
                                        settings.add_recent_file(file_path.clone());
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        egui_app.update_recent_files(
                                            settings.get_recent_files().to_vec(),
                                        );
                                        egui_app
                                            .status_bar
                                            .set_message("PS1 disc loaded".to_string());
                                        let _ = sys.resolution();
                                        if let Some(ref hash) = rom_hash {
                                            _game_saves = GameSaves::load(hash);
                                        }
                                    }
                                    Ok(SystemType::GameAndWatch) => {
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        let mut gw_sys =
                                            emu_gameandwatch::GameAndWatchSystem::new();
                                        if let Err(e) = gw_sys.mount("Program", &data) {
                                            egui_app
                                                .status_bar
                                                .set_message(format!("Error: {}", e));
                                            rom_hash = None;
                                        } else {
                                            rom_loaded = true;
                                            sys = EmulatorSystem::GameAndWatch(Box::new(gw_sys));
                                            egui_app.property_pane.system_name =
                                                "Game & Watch".to_string();
                                            egui_app.property_pane.rendering_backend =
                                                sys.get_current_renderer_name();
                                            egui_app.property_pane.available_renderers =
                                                sys.get_available_renderers();
                                            runtime_state.set_mount(
                                                "Program".to_string(),
                                                file_path.clone(),
                                            );
                                            settings.add_recent_file(file_path.clone());
                                            if let Err(e) = settings.save() {
                                                eprintln!(
                                                    "Warning: Failed to save settings: {}",
                                                    e
                                                );
                                            }
                                            egui_app.update_recent_files(
                                                settings.get_recent_files().to_vec(),
                                            );
                                            egui_app.status_bar.set_message(
                                                "Game & Watch program loaded".to_string(),
                                            );
                                            let _ = sys.resolution();
                                            if let Some(ref hash) = rom_hash {
                                                _game_saves = GameSaves::load(hash);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        egui_app
                                            .status_bar
                                            .set_error(format!("Unknown ROM format: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                egui_app
                                    .status_bar
                                    .set_error(format!("Failed to read file: {}", e));
                            }
                        }
                    }
                }
                MenuAction::ClearRecentFiles => {
                    settings.clear_recent_files();
                    if let Err(e) = settings.save() {
                        eprintln!("Warning: Failed to save settings: {}", e);
                    }
                    egui_app.update_recent_files(Vec::new());
                    egui_app
                        .status_bar
                        .set_message("Recent files cleared".to_string());
                }
                MenuAction::Reset => {
                    if rom_loaded {
                        sys.reset();
                        egui_app.status_bar.set_message("System reset".to_string());
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::Pause => {
                    if rom_loaded {
                        settings.emulation_speed = 0.0;
                        egui_app.property_pane.emulation_speed_percent = 0;
                        egui_app.status_bar.set_message("Paused".to_string());
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::Resume => {
                    if rom_loaded {
                        settings.emulation_speed = 1.0;
                        egui_app.property_pane.emulation_speed_percent = 100;
                        egui_app.status_bar.set_message("Resumed".to_string());
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::Step => {
                    // Step one frame when paused
                    if rom_loaded && settings.emulation_speed == 0.0 {
                        match sys.step_frame() {
                            Ok(frame) => {
                                latest_frame_buffer = Some((
                                    frame.pixels.clone(),
                                    frame.width as usize,
                                    frame.height as usize,
                                ));
                                egui_app
                                    .status_bar
                                    .set_message("Stepped one frame".to_string());
                            }
                            Err(e) => {
                                eprintln!("Error stepping frame: {:?}", e);
                            }
                        }
                    }
                }
                MenuAction::SaveState(slot) => {
                    if rom_loaded {
                        if let Some(ref hash) = rom_hash {
                            if sys.supports_save_states() {
                                let state = sys.save_state();
                                let state_json = serde_json::to_string(&state).unwrap_or_default();
                                if let Err(e) =
                                    _game_saves.save_slot(slot, state_json.as_bytes(), hash)
                                {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Error saving state: {}", e));
                                } else {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Saved to slot {}", slot));
                                    egui_app
                                        .tab_manager
                                        .add_log(format!("State saved to slot {}", slot));
                                }
                            } else {
                                egui_app.status_bar.set_message(
                                    "Save states not supported for this system".to_string(),
                                );
                            }
                        }
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::LoadState(slot) => {
                    if rom_loaded {
                        if let Some(ref hash) = rom_hash {
                            if sys.supports_save_states() {
                                match _game_saves.load_slot(slot, hash) {
                                    Ok(data) => {
                                        if let Ok(state_str) = String::from_utf8(data) {
                                            if let Ok(state) = serde_json::from_str(&state_str) {
                                                if let Err(e) = sys.load_state(&state) {
                                                    egui_app.status_bar.set_message(format!(
                                                        "Error loading state: {}",
                                                        e
                                                    ));
                                                } else {
                                                    egui_app.status_bar.set_message(format!(
                                                        "Loaded from slot {}",
                                                        slot
                                                    ));
                                                    egui_app.tab_manager.add_log(format!(
                                                        "State loaded from slot {}",
                                                        slot
                                                    ));
                                                }
                                            } else {
                                                egui_app
                                                    .status_bar
                                                    .set_message("Invalid state data".to_string());
                                            }
                                        } else {
                                            egui_app
                                                .status_bar
                                                .set_message("Invalid state encoding".to_string());
                                        }
                                    }
                                    Err(e) => {
                                        egui_app
                                            .status_bar
                                            .set_message(format!("Error loading state: {}", e));
                                    }
                                }
                            } else {
                                egui_app.status_bar.set_message(
                                    "Save states not supported for this system".to_string(),
                                );
                            }
                        }
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::SetSpeed(speed) => {
                    settings.emulation_speed = speed as f64 / 100.0;
                    egui_app.property_pane.emulation_speed_percent = speed;
                    egui_app.menu_bar.current_speed = speed;
                    egui_app
                        .status_bar
                        .set_message(format!("Speed set to {}%", speed));
                }
                MenuAction::EjectCartridge => {
                    // Find the first required mount point and eject it
                    let mount_points = sys.mount_points();
                    if let Some(mp) = mount_points.iter().find(|mp| mp.required) {
                        if let Err(e) = sys.unmount(&mp.id) {
                            egui_app
                                .status_bar
                                .set_message(format!("Error ejecting: {}", e));
                        } else {
                            runtime_state.current_mounts.remove(&mp.id);
                            rom_hash = None;
                            egui_app
                                .status_bar
                                .set_message("Cartridge ejected".to_string());
                            egui_app.tab_manager.add_log(format!("Ejected {}", mp.name));
                            update_tab_mount_info(&mut egui_app, &sys, &runtime_state);
                        }
                    }
                }
                MenuAction::SetDisplayFilter(filter) => {
                    settings.display_filter = filter;
                    egui_app.menu_bar.current_filter = filter;
                    egui_app
                        .status_bar
                        .set_message(format!("Filter: {}", filter.name()));
                }
                MenuAction::ToggleMetrics => {
                    egui_app.property_pane.metrics_visible =
                        !egui_app.property_pane.metrics_visible;
                    egui_app.menu_bar.metrics_visible = egui_app.property_pane.metrics_visible;
                }
                MenuAction::ToggleControllerSettings => {
                    egui_app.property_pane.controller_visible =
                        !egui_app.property_pane.controller_visible;
                    egui_app.menu_bar.controller_visible =
                        egui_app.property_pane.controller_visible;
                }
                MenuAction::ToggleMountPoints => {
                    egui_app.property_pane.mounts_visible = !egui_app.property_pane.mounts_visible;
                    egui_app.menu_bar.mounts_visible = egui_app.property_pane.mounts_visible;
                }
                MenuAction::Screenshot => {
                    // Take screenshot of current frame
                    if rom_loaded {
                        if let Some((ref buffer, width, height)) = latest_frame_buffer {
                            let system_name = egui_app.property_pane.system_name.replace(" ", "_");
                            match save_screenshot(buffer, width, height, &system_name) {
                                Ok(filename) => {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Screenshot saved: {}", filename));
                                    egui_app
                                        .tab_manager
                                        .add_log(format!("Screenshot saved: {}", filename));
                                }
                                Err(e) => {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Error saving screenshot: {}", e));
                                    egui_app
                                        .tab_manager
                                        .add_log(format!("Error saving screenshot: {}", e));
                                }
                            }
                        } else {
                            egui_app
                                .status_bar
                                .set_message("No frame to capture".to_string());
                        }
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                MenuAction::ShowHelp => {
                    if let Err(e) = open::that("https://hemulator.56k.guru/user") {
                        egui_app
                            .status_bar
                            .set_error(format!("Failed to open help URL: {}", e));
                    } else {
                        egui_app
                            .status_bar
                            .set_message("Opened help in default browser".to_string());
                    }
                }

                MenuAction::About => {
                    egui_app.tab_manager.show_about_tab();
                }
                MenuAction::ScalingOriginal => {
                    settings.scaling_mode = settings::ScalingMode::Original;
                    egui_app
                        .status_bar
                        .set_message("Scaling: Original".to_string());
                }
                MenuAction::ScalingFit => {
                    settings.scaling_mode = settings::ScalingMode::Fit;
                    egui_app.status_bar.set_message("Scaling: Fit".to_string());
                }
                MenuAction::ScalingStretch => {
                    settings.scaling_mode = settings::ScalingMode::Stretch;
                    egui_app
                        .status_bar
                        .set_message("Scaling: Stretch".to_string());
                }
                MenuAction::Fullscreen => {
                    settings.fullscreen = !settings.fullscreen;
                    settings.fullscreen_with_gui = false;
                    if let Err(e) = egui_backend.set_fullscreen(settings.fullscreen) {
                        eprintln!("Failed to toggle fullscreen: {}", e);
                        egui_app
                            .status_bar
                            .set_message(format!("Fullscreen error: {}", e));
                    } else {
                        let msg = if settings.fullscreen {
                            "Fullscreen enabled"
                        } else {
                            "Fullscreen disabled"
                        };
                        egui_app.status_bar.set_message(msg.to_string());
                    }
                }
                MenuAction::FullscreenWithGui => {
                    settings.fullscreen = !settings.fullscreen;
                    settings.fullscreen_with_gui = settings.fullscreen;
                    if let Err(e) = egui_backend.set_fullscreen(settings.fullscreen) {
                        eprintln!("Failed to toggle fullscreen: {}", e);
                        egui_app
                            .status_bar
                            .set_message(format!("Fullscreen error: {}", e));
                    } else {
                        let msg = if settings.fullscreen {
                            "Fullscreen (With GUI) enabled"
                        } else {
                            "Fullscreen disabled"
                        };
                        egui_app.status_bar.set_message(msg.to_string());
                    }
                }
                MenuAction::ShowInspector => {
                    // Toggle inspector dock visibility
                    egui_app.dock_layout.toggle_inspector();
                    let msg = if egui_app.dock_layout.inspector_visible {
                        "Inspector panel shown"
                    } else {
                        "Inspector panel hidden"
                    };
                    egui_app.status_bar.set_message(msg.to_string());
                }
                MenuAction::OpenProject => {
                    // Open .hemu project file dialog
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Hemulator Project", &["hemu"])
                        .add_filter("All Files", &["*"])
                        .pick_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        match HemuProject::load(&path_str) {
                            Ok(project) => {
                                if project.system != "pc" {
                                    egui_app.status_bar.set_message(format!(
                                        "Currently only PC system .hemu projects are supported. Project is for: {}",
                                        project.system
                                    ));
                                } else {
                                    // Parse configuration from project
                                    let cpu_model = if let Some(cpu_str) = project.get_cpu_model() {
                                        match cpu_str.as_str() {
                                            "Intel8086" => emu_core::cpu_8086::CpuModel::Intel8086,
                                            "Intel8088" => emu_core::cpu_8086::CpuModel::Intel8088,
                                            "Intel80186" => {
                                                emu_core::cpu_8086::CpuModel::Intel80186
                                            }
                                            "Intel80188" => {
                                                emu_core::cpu_8086::CpuModel::Intel80188
                                            }
                                            "Intel80286" => {
                                                emu_core::cpu_8086::CpuModel::Intel80286
                                            }
                                            "Intel80386" => {
                                                emu_core::cpu_8086::CpuModel::Intel80386
                                            }
                                            "Intel80486" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486
                                            }
                                            "Intel80486SX" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486SX
                                            }
                                            "Intel80486DX2" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486DX2
                                            }
                                            "Intel80486SX2" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486SX2
                                            }
                                            "Intel80486DX4" => {
                                                emu_core::cpu_8086::CpuModel::Intel80486DX4
                                            }
                                            "IntelPentium" => {
                                                emu_core::cpu_8086::CpuModel::IntelPentium
                                            }
                                            "IntelPentiumMMX" => {
                                                emu_core::cpu_8086::CpuModel::IntelPentiumMMX
                                            }
                                            _ => {
                                                eprintln!("Unknown CPU model: {}, using default Intel8086", cpu_str);
                                                emu_core::cpu_8086::CpuModel::Intel8086
                                            }
                                        }
                                    } else {
                                        emu_core::cpu_8086::CpuModel::Intel8086
                                    };

                                    let memory_kb = project.get_memory_kb().unwrap_or(640);

                                    // Create video adapter based on project configuration
                                    let video_adapter: Box<dyn emu_pc::VideoAdapter> =
                                        if let Some(video_str) = project.get_video_mode() {
                                            match video_str.as_str() {
                                                "EGA" => {
                                                    Box::new(emu_pc::SoftwareEgaAdapter::new())
                                                }
                                                "VGA" => {
                                                    Box::new(emu_pc::SoftwareVgaAdapter::new())
                                                }
                                                "CGA" => {
                                                    Box::new(emu_pc::SoftwareCgaAdapter::new())
                                                }
                                                _ => Box::new(emu_pc::SoftwareCgaAdapter::new()),
                                            }
                                        } else {
                                            Box::new(emu_pc::SoftwareCgaAdapter::new())
                                        };

                                    // Create PC system with configuration
                                    let mut pc_sys = emu_pc::PcSystem::with_config(
                                        cpu_model,
                                        memory_kb,
                                        video_adapter,
                                    );

                                    // Load boot priority if specified
                                    if let Some(priority_str) = project.boot_priority.as_ref() {
                                        let priority = match priority_str.as_str() {
                                            "FloppyFirst" => emu_pc::BootPriority::FloppyFirst,
                                            "HardDriveFirst" => {
                                                emu_pc::BootPriority::HardDriveFirst
                                            }
                                            "FloppyOnly" => emu_pc::BootPriority::FloppyOnly,
                                            "HardDriveOnly" => emu_pc::BootPriority::HardDriveOnly,
                                            _ => emu_pc::BootPriority::FloppyFirst,
                                        };
                                        pc_sys.set_boot_priority(priority);
                                    }

                                    // Mount all files from the project
                                    let project_dir = std::path::Path::new(&path_str)
                                        .parent()
                                        .unwrap_or(std::path::Path::new("."));
                                    for (mount_id, relative_path) in &project.mounts {
                                        let full_path = project_dir.join(relative_path);
                                        match fs::read(&full_path) {
                                            Ok(data) => {
                                                if let Err(e) = pc_sys.mount(mount_id, &data) {
                                                    eprintln!(
                                                        "Failed to mount {}: {}",
                                                        mount_id, e
                                                    );
                                                } else {
                                                    runtime_state.set_mount(
                                                        mount_id.clone(),
                                                        full_path.to_string_lossy().to_string(),
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Failed to read {}: {}",
                                                    relative_path, e
                                                );
                                            }
                                        }
                                    }

                                    // Update POST screen with mount status
                                    pc_sys.update_post_screen();
                                    // Clear debug and inspector state when loading a project
                                    egui_app.tab_manager.clear_debug_state();

                                    sys = EmulatorSystem::PC(Box::new(pc_sys));
                                    rom_loaded = true;
                                    egui_app.property_pane.system_name = "PC".to_string();
                                    egui_app.property_pane.rendering_backend =
                                        sys.get_current_renderer_name();
                                    egui_app.property_pane.available_renderers =
                                        sys.get_available_renderers();

                                    // Add project to recent files
                                    settings.add_recent_file(path_str.clone());
                                    if let Err(e) = settings.save() {
                                        eprintln!("Warning: Failed to save settings: {}", e);
                                    }
                                    egui_app
                                        .update_recent_files(settings.get_recent_files().to_vec());

                                    // Track current project path
                                    runtime_state.set_project_path(path.clone());

                                    egui_app.status_bar.set_message(format!(
                                        "Project loaded: {}",
                                        path.file_name().unwrap_or_default().to_string_lossy()
                                    ));
                                }
                            }
                            Err(e) => {
                                egui_app
                                    .status_bar
                                    .set_message(format!("Failed to load project: {}", e));
                            }
                        }
                    }
                }
                MenuAction::SaveProject => {
                    // Save current emulation state to a .hemu project file
                    if rom_loaded {
                        if let Some(saved_path) = save_project(
                            &sys,
                            &runtime_state,
                            &settings,
                            &mut status_message,
                            runtime_state.current_project_path.as_ref(),
                        ) {
                            // Add saved project to recent files
                            settings.add_recent_file(saved_path.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            egui_app.update_recent_files(settings.get_recent_files().to_vec());

                            // Track current project path
                            runtime_state.set_project_path(PathBuf::from(&saved_path));
                        }
                        egui_app.status_bar.set_message(status_message.clone());
                    } else {
                        egui_app
                            .status_bar
                            .set_message("No system loaded to save".to_string());
                    }
                }
                MenuAction::Exit => {
                    // Exit the application by breaking out of the main loop
                    break;
                }
            }
        }

        // Handle property pane actions
        if let Some(action) = egui_app.property_pane.take_action() {
            use egui_ui::property_pane::PropertyAction;
            match action {
                PropertyAction::MountFile(mount_id) => {
                    // Find the mount point info to get allowed extensions
                    let mount_points = sys.mount_points();
                    if let Some(mount_info) = mount_points.iter().find(|mp| mp.id == mount_id) {
                        // Create file dialog with appropriate filters
                        let extensions: Vec<&str> =
                            mount_info.extensions.iter().map(|s| s.as_str()).collect();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(&mount_info.name, &extensions)
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            match fs::read(&path) {
                                Ok(data) => {
                                    if let Err(e) = sys.mount(&mount_id, &data) {
                                        egui_app
                                            .status_bar
                                            .set_message(format!("Error mounting: {}", e));
                                    } else {
                                        let path_str = path.to_string_lossy().to_string();
                                        runtime_state.set_mount(mount_id.clone(), path_str.clone());
                                        egui_app.status_bar.set_message(format!(
                                            "Mounted {}",
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("file")
                                        ));
                                        egui_app.tab_manager.add_log(format!(
                                            "Mounted {} to {}",
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("file"),
                                            mount_info.name
                                        ));
                                        // Update mount_info so welcome screen shows emulator
                                        update_tab_mount_info(&mut egui_app, &sys, &runtime_state);
                                    }
                                }
                                Err(e) => {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Error reading file: {}", e));
                                }
                            }
                        }
                    }
                }
                PropertyAction::EjectFile(mount_id) => {
                    if let Err(e) = sys.unmount(&mount_id) {
                        egui_app
                            .status_bar
                            .set_message(format!("Error ejecting: {}", e));
                    } else {
                        runtime_state.current_mounts.remove(&mount_id);
                        egui_app.status_bar.set_message("Ejected".to_string());
                        egui_app
                            .tab_manager
                            .add_log(format!("Ejected {}", mount_id));
                        // Update mount_info so welcome screen shows ready state
                        update_tab_mount_info(&mut egui_app, &sys, &runtime_state);
                    }
                }
                PropertyAction::ConfigureInput => {
                    // Input configuration dialog
                    // Future feature: Allow users to customize keyboard mappings
                    // Currently uses default mappings defined in settings.rs
                    // For now, just show a message that this feature is coming soon
                    egui_app.status_bar.set_message(
                        "Input configuration dialog coming soon. Edit config.json manually for now."
                            .to_string(),
                    );
                    egui_app.tab_manager.add_log(
                        "Input configuration: Feature in development. Use config.json for now."
                            .to_string(),
                    );
                }
                PropertyAction::SetInputSource(source) => {
                    use egui_ui::InputConfigSource;
                    match source {
                        InputConfigSource::Global => {
                            // Clear project-specific input override
                            runtime_state.input_override = None;
                            egui_app.property_pane.input_config_source = InputConfigSource::Global;
                            // Update property pane to show current global settings
                            egui_app.property_pane.mouse_enabled = settings.input.mouse_enabled;
                            egui_app.property_pane.mouse_sensitivity =
                                settings.input.mouse_sensitivity;
                            egui_app
                                .status_bar
                                .set_message("Using global input config".to_string());
                            egui_app
                                .tab_manager
                                .add_log("Switched to global input configuration".to_string());
                        }
                        InputConfigSource::Project => {
                            // Create project-specific input override if not exists
                            if runtime_state.input_override.is_none() {
                                runtime_state.input_override = Some(settings.input.clone());
                            }
                            egui_app.property_pane.input_config_source = InputConfigSource::Project;
                            // Update property pane to show project-specific settings
                            if let Some(ref input_override) = runtime_state.input_override {
                                egui_app.property_pane.mouse_enabled = input_override.mouse_enabled;
                                egui_app.property_pane.mouse_sensitivity =
                                    input_override.mouse_sensitivity;
                            }
                            egui_app.status_bar.set_message(
                                "Using project-specific input config (save project to persist)"
                                    .to_string(),
                            );
                            egui_app.tab_manager.add_log(
                                "Switched to project-specific input configuration".to_string(),
                            );
                        }
                    }
                }
                PropertyAction::SetRenderer(renderer_name) => {
                    // Save renderer preference to settings
                    let backend_name = if renderer_name == "OpenGL" {
                        "opengl"
                    } else {
                        "software"
                    };
                    settings.video_backend = backend_name.to_string();

                    // Save settings immediately
                    if let Err(e) = settings.save() {
                        eprintln!("Failed to save renderer preference: {}", e);
                        egui_app
                            .status_bar
                            .set_error(format!("Failed to save renderer preference: {}", e));
                    } else {
                        // Try to switch renderer immediately
                        let mut switched = false;
                        match &mut sys {
                            #[cfg(feature = "opengl")]
                            EmulatorSystem::NES(nes_sys) => {
                                if renderer_name == "OpenGL" {
                                    // Get GL context from egui backend
                                    if let Some(gl) = egui_backend.gl_context() {
                                        match nes_sys.enable_opengl_renderer(gl) {
                                            Ok(()) => {
                                                switched = true;
                                                egui_app.property_pane.rendering_backend =
                                                    "OpenGL".to_string();
                                                egui_app.status_bar.set_success(
                                                    "Switched to OpenGL renderer".to_string(),
                                                );
                                                egui_app.tab_manager.add_log(
                                                    "NES: Switched to OpenGL hardware renderer"
                                                        .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Failed to enable OpenGL renderer: {}",
                                                    e
                                                );
                                                egui_app.status_bar.set_error(format!(
                                                    "Failed to enable OpenGL renderer: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    } else {
                                        egui_app
                                            .status_bar
                                            .set_error("OpenGL context not available".to_string());
                                    }
                                } else {
                                    // Cannot switch back to software without recreating system
                                    egui_app.status_bar.set_message(
                                        "Renderer preference saved. Reload ROM to switch to Software renderer.".to_string(),
                                    );
                                    egui_app.tab_manager.add_log(
                                        "Renderer preference saved: Software (reload ROM to apply)"
                                            .to_string(),
                                    );
                                }
                            }
                            #[cfg(feature = "opengl")]
                            EmulatorSystem::N64(_n64_sys) => {
                                // N64 renderer is set at system creation time and cannot be changed
                                egui_app.status_bar.set_error(
                                    "N64 renderer cannot be changed after system creation. Restart with a new system to change renderer.".to_string(),
                                );
                                egui_app.tab_manager.add_log(
                                    "N64: Renderer is fixed at system creation time".to_string(),
                                );
                            }
                            _ => {
                                // System doesn't support renderer switching
                                egui_app.status_bar.set_message(format!(
                                    "Renderer preference saved to '{}'",
                                    renderer_name
                                ));
                            }
                        }

                        if !switched
                            && !matches!(sys, EmulatorSystem::NES(_) | EmulatorSystem::N64(_))
                        {
                            egui_app
                                .status_bar
                                .set_message(format!("Renderer set to '{}'", renderer_name));
                            egui_app
                                .tab_manager
                                .add_log(format!("Renderer preference saved: {}", renderer_name));
                        }
                    }
                }
            }
        }

        // Handle emulation speed changes from property pane
        settings.emulation_speed = (egui_app.property_pane.emulation_speed_percent as f64) / 100.0;

        // Display filter is now managed via menu, not property pane

        // Handle log rate limit changes
        let current_rate_limit = emu_core::logging::LogConfig::global().get_rate_limit();
        if settings.log_rate_limit != current_rate_limit {
            settings.log_rate_limit = current_rate_limit;
            // Auto-save settings when rate limit changes
            if let Err(e) = settings.save() {
                eprintln!("Failed to save log rate limit: {}", e);
            }
        }

        // Handle input configuration changes from property pane
        // Sync mouse settings back to the appropriate config (global or project-specific)
        let input_config_changed = settings.input.mouse_enabled
            != egui_app.property_pane.mouse_enabled
            || (settings.input.mouse_sensitivity - egui_app.property_pane.mouse_sensitivity).abs()
                > 0.01;

        if input_config_changed {
            match egui_app.property_pane.input_config_source {
                egui_ui::InputConfigSource::Global => {
                    // Update global settings
                    settings.input.mouse_enabled = egui_app.property_pane.mouse_enabled;
                    settings.input.mouse_sensitivity = egui_app.property_pane.mouse_sensitivity;
                    // Auto-save global config
                    if let Err(e) = settings.save() {
                        eprintln!("Failed to save global input config: {}", e);
                        egui_app
                            .status_bar
                            .set_message(format!("Failed to save config: {}", e));
                    } else {
                        egui_app
                            .status_bar
                            .set_message("Global input config saved".to_string());
                    }
                }
                egui_ui::InputConfigSource::Project => {
                    // Update project-specific override
                    if let Some(ref mut input_override) = runtime_state.input_override {
                        input_override.mouse_enabled = egui_app.property_pane.mouse_enabled;
                        input_override.mouse_sensitivity = egui_app.property_pane.mouse_sensitivity;
                        // Note: Project will be saved when user explicitly saves the project
                        egui_app.status_bar.set_message(
                            "Project input config updated (save project to persist)".to_string(),
                        );
                    }
                }
            }
        }

        // Handle tab actions (e.g., create new project)
        if let Some(action) = egui_app.tab_manager.take_action() {
            use egui_ui::TabAction;
            match action {
                TabAction::CreateNewProject(system_name) => {
                    // Create a new system based on the selected type
                    match system_name.as_str() {
                        "NES" => {
                            sys = EmulatorSystem::NES(Box::default());
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "NES".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new NES system".to_string());
                        }
                        "Game Boy" => {
                            sys = EmulatorSystem::GameBoy(Box::new(emu_gb::GbSystem::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "Game Boy".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new Game Boy system".to_string());
                        }
                        "GBA" => {
                            sys = EmulatorSystem::GBA(Box::new(hemu_gba::GbaSystem::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "GBA".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new GBA system".to_string());
                        }
                        "Atari 2600" => {
                            sys = EmulatorSystem::Atari2600(Box::new(create_atari2600_system(
                                &settings,
                            )));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "Atari 2600".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new Atari 2600 system".to_string());
                        }
                        "PC" => {
                            sys = EmulatorSystem::PC(Box::new(emu_pc::PcSystem::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "PC".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new PC system".to_string());
                        }
                        "SNES" => {
                            sys = EmulatorSystem::SNES(Box::new(emu_snes::SnesSystem::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "SNES".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new SNES system".to_string());
                        }
                        "N64" => {
                            let gl_ctx = egui_backend.gl_context();
                            match create_n64_system(gl_ctx, &settings) {
                                Ok(n64_sys) => {
                                    sys = EmulatorSystem::N64(Box::new(n64_sys));
                                    rom_loaded = true; // Mark system as created even without ROM
                                    rom_hash = None;
                                    runtime_state.clear_mounts();
                                    egui_app.property_pane.system_name = "N64".to_string();
                                    egui_app.property_pane.rendering_backend =
                                        sys.get_current_renderer_name();
                                    egui_app.property_pane.available_renderers =
                                        sys.get_available_renderers();
                                    egui_app
                                        .status_bar
                                        .set_message("Created new N64 system".to_string());
                                }
                                Err(e) => {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Failed to create N64 system: {}", e));
                                }
                            }
                        }
                        "SMS" => {
                            sys = EmulatorSystem::SMS(Box::new(emu_sms::SmsSystem::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "SMS".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new SMS system".to_string());
                        }
                        "Mega Drive" => {
                            sys = EmulatorSystem::MegaDrive(Box::new(create_megadrive_system(
                                &settings,
                            )));
                            rom_loaded = true;
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "Mega Drive".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new Mega Drive system".to_string());
                        }
                        "CHIP-8" => {
                            sys = EmulatorSystem::Chip8(Box::new(emu_chip8::Chip8System::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "CHIP-8".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new CHIP-8 system".to_string());
                        }
                        "ColecoVision" => {
                            sys = EmulatorSystem::ColecoVision(Box::new(
                                emu_colecovision::ColecoVisionSystem::new(),
                            ));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "ColecoVision".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new ColecoVision system".to_string());
                        }
                        "SG-1000" => {
                            sys = EmulatorSystem::SG1000(Box::new(emu_sg1000::Sg1000System::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "SG-1000".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new SG-1000 system".to_string());
                        }
                        "PS1" => {
                            sys = EmulatorSystem::PS1(Box::new(emu_ps1::Ps1System::new()));
                            rom_loaded = true; // Mark system as created even without ROM
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "PS1".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new PS1 system".to_string());
                        }
                        "Game & Watch" => {
                            sys = EmulatorSystem::GameAndWatch(Box::new(
                                emu_gameandwatch::GameAndWatchSystem::new(),
                            ));
                            rom_loaded = true;
                            rom_hash = None;
                            runtime_state.clear_mounts();
                            egui_app.property_pane.system_name = "Game & Watch".to_string();
                            egui_app.property_pane.rendering_backend =
                                sys.get_current_renderer_name();
                            egui_app.property_pane.available_renderers =
                                sys.get_available_renderers();
                            egui_app
                                .status_bar
                                .set_message("Created new Game & Watch system".to_string());
                        }
                        _ => {
                            egui_app
                                .status_bar
                                .set_message(format!("Unknown system: {}", system_name));
                        }
                    }
                }
                TabAction::SelectCartridge => {
                    // Find the first required mount point that's not mounted (cartridge/rom)
                    let mount_points = sys.mount_points();
                    if let Some(mount_info) = mount_points.iter().find(|mp| {
                        mp.required
                            && !sys.is_mounted(&mp.id)
                            && (mp.id.to_lowercase().contains("cartridge")
                                || mp.id.to_lowercase().contains("rom"))
                    }) {
                        // Create file dialog with appropriate filters
                        let extensions: Vec<&str> =
                            mount_info.extensions.iter().map(|s| s.as_str()).collect();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(&mount_info.name, &extensions)
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            match std::fs::read(&path) {
                                Ok(data) => {
                                    rom_hash = Some(GameSaves::rom_hash(&data));
                                    if let Err(e) = sys.mount(&mount_info.id, &data) {
                                        egui_app
                                            .status_bar
                                            .set_message(format!("Error mounting: {}", e));
                                        rom_hash = None;
                                    } else {
                                        let path_str = path.to_string_lossy().to_string();
                                        runtime_state
                                            .set_mount(mount_info.id.clone(), path_str.clone());
                                        egui_app.status_bar.set_message(format!(
                                            "Loaded {}",
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("file")
                                        ));
                                        egui_app.tab_manager.add_log(format!(
                                            "Mounted {} to {}",
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("file"),
                                            mount_info.name
                                        ));
                                        // Update mount_info so welcome screen shows emulator
                                        update_tab_mount_info(&mut egui_app, &sys, &runtime_state);
                                    }
                                }
                                Err(e) => {
                                    egui_app
                                        .status_bar
                                        .set_message(format!("Error reading file: {}", e));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle debug tab actions (pause, resume, step)
        if let Some(debug_action) = egui_app.tab_manager.take_debug_action() {
            use egui_ui::DebugAction;
            match debug_action {
                DebugAction::Pause => {
                    if rom_loaded {
                        settings.emulation_speed = 0.0;
                        egui_app.property_pane.emulation_speed_percent = 0;
                        egui_app.status_bar.set_message("Paused".to_string());
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                DebugAction::Resume => {
                    if rom_loaded {
                        settings.emulation_speed = 1.0;
                        egui_app.property_pane.emulation_speed_percent = 100;
                        egui_app.status_bar.set_message("Resumed".to_string());
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                DebugAction::Step => {
                    // Step one frame when paused
                    if rom_loaded && settings.emulation_speed == 0.0 {
                        match sys.step_frame() {
                            Ok(frame) => {
                                latest_frame_buffer = Some((
                                    frame.pixels.clone(),
                                    frame.width as usize,
                                    frame.height as usize,
                                ));
                                egui_app
                                    .status_bar
                                    .set_message("Stepped one frame".to_string());
                            }
                            Err(e) => {
                                eprintln!("Error stepping frame: {:?}", e);
                            }
                        }
                    }
                }
                DebugAction::StartTrace(filename) => {
                    if rom_loaded {
                        if let Some(tracer) = sys.instruction_tracer_mut() {
                            tracer.clear();
                            tracer.set_enabled(true);
                            egui_app
                                .status_bar
                                .set_message(format!("Started trace: {}", filename));
                        } else {
                            egui_app
                                .status_bar
                                .set_message("Trace not supported for this system".to_string());
                        }
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                DebugAction::StopTrace => {
                    if rom_loaded {
                        if let Some(tracer) = sys.instruction_tracer_mut() {
                            tracer.set_enabled(false);
                            if let Some(filename) = &egui_app.tab_manager.trace_filename {
                                match tracer.dump_to_file(filename) {
                                    Ok(_) => {
                                        egui_app
                                            .status_bar
                                            .set_message(format!("Trace saved to {}", filename));
                                    }
                                    Err(e) => {
                                        egui_app
                                            .status_bar
                                            .set_message(format!("Failed to save trace: {}", e));
                                    }
                                }
                            }
                        }
                    }
                }
                DebugAction::SetGbAudioChannels(mask) => {
                    if rom_loaded {
                        if let EmulatorSystem::GameBoy(sys) = &mut sys {
                            sys.set_audio_channel_mask(mask);
                            egui_app
                                .status_bar
                                .set_message("Updated GB audio channels".to_string());
                        }
                    }
                }
                DebugAction::AddBreakpoint(address, bp_type) => {
                    if rom_loaded {
                        match &mut sys {
                            EmulatorSystem::NES(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::GameBoy(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::Atari2600(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::Chip8(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::SNES(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::N64(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            EmulatorSystem::PC(s) => match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => {
                                    s.add_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Read => {
                                    s.add_read_breakpoint(address)
                                }
                                emu_core::breakpoints::BreakpointType::Write => {
                                    s.add_write_breakpoint(address)
                                }
                            },
                            _ => {} // SMS, ColecoVision, SG1000 don't have breakpoint manager yet
                        }
                        let type_str = match bp_type {
                            emu_core::breakpoints::BreakpointType::Execute => "execution",
                            emu_core::breakpoints::BreakpointType::Read => "read",
                            emu_core::breakpoints::BreakpointType::Write => "write",
                        };
                        egui_app.status_bar.set_message(format!(
                            "Added {} breakpoint at ${:04X}",
                            type_str, address
                        ));
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
                DebugAction::RemoveBreakpoint(address, bp_type) => {
                    if rom_loaded {
                        // We need mutable access, so match on each system
                        let success = match &mut sys {
                            EmulatorSystem::NES(s) => s.remove_breakpoint_by_type(address, bp_type),
                            EmulatorSystem::GameBoy(s) => {
                                s.remove_breakpoint_by_type(address, bp_type)
                            }
                            EmulatorSystem::Atari2600(s) => {
                                s.remove_breakpoint_by_type(address, bp_type)
                            }
                            EmulatorSystem::Chip8(s) => {
                                s.remove_breakpoint_by_type(address, bp_type)
                            }
                            EmulatorSystem::SNES(s) => {
                                s.remove_breakpoint_by_type(address, bp_type)
                            }
                            EmulatorSystem::N64(s) => s.remove_breakpoint_by_type(address, bp_type),
                            EmulatorSystem::PC(s) => s.remove_breakpoint_by_type(address, bp_type),
                            _ => false, // SMS, ColecoVision, SG1000 don't have breakpoint manager yet
                        };

                        if success {
                            let type_str = match bp_type {
                                emu_core::breakpoints::BreakpointType::Execute => "execution",
                                emu_core::breakpoints::BreakpointType::Read => "read",
                                emu_core::breakpoints::BreakpointType::Write => "write",
                            };
                            egui_app.status_bar.set_message(format!(
                                "Removed {} breakpoint at ${:04X}",
                                type_str, address
                            ));
                        } else {
                            egui_app
                                .status_bar
                                .set_message(format!("Breakpoint not found at ${:04X}", address));
                        }
                    } else {
                        egui_app.status_bar.set_message("No ROM loaded".to_string());
                    }
                }
            }
        }

        // Handle host key + fullscreen toggle (switch between Fullscreen and Fullscreen with GUI)
        if let Some(host_key) = string_to_key(&settings.input.host_modifier) {
            if egui_backend.is_key_down(host_key) && egui_backend.is_key_pressed(Key::F11, false) {
                // Toggle between fullscreen modes
                if !settings.fullscreen {
                    // Currently windowed, enable fullscreen with GUI
                    settings.fullscreen = true;
                    settings.fullscreen_with_gui = true;
                    if let Err(e) = egui_backend.set_fullscreen(true) {
                        eprintln!("Failed to enable fullscreen: {}", e);
                    } else {
                        egui_app
                            .status_bar
                            .set_message("Fullscreen (With GUI) enabled".to_string());
                    }
                } else if settings.fullscreen_with_gui {
                    // Currently fullscreen with GUI, switch to fullscreen without GUI
                    settings.fullscreen_with_gui = false;
                    egui_app
                        .status_bar
                        .set_message("Fullscreen enabled".to_string());
                } else {
                    // Currently fullscreen without GUI, disable fullscreen
                    settings.fullscreen = false;
                    if let Err(e) = egui_backend.set_fullscreen(false) {
                        eprintln!("Failed to disable fullscreen: {}", e);
                    } else {
                        egui_app
                            .status_bar
                            .set_message("Fullscreen disabled".to_string());
                    }
                }
            }
        }

        // Step emulation frame if ROM is loaded, all required mounts are satisfied, and not paused
        if rom_loaded && sys.has_required_mounts() && settings.emulation_speed > 0.0 {
            // Reset timing when emulation becomes active or speed changes
            let is_emulation_active = true;
            let speed_changed = (settings.emulation_speed - previous_emulation_speed).abs()
                > SPEED_CHANGE_THRESHOLD;

            if (!was_emulation_active && is_emulation_active) || speed_changed {
                emulation_start_time = Instant::now();
                total_emulated_time = Duration::ZERO;
                previous_emulation_speed = settings.emulation_speed;
            }
            was_emulation_active = is_emulation_active;

            // Calculate time since emulation started
            let time_since_start = emulation_start_time.elapsed();

            // Get target frame time
            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            let target_frame_duration = Duration::from_secs_f64(1.0 / frame_rate);

            // Calculate how many frames we need to emulate to catch up
            // Emulation speed affects the target emulated time, not the frame count
            let emulation_speed = settings.emulation_speed;
            let desired_emulated_time_secs = time_since_start.as_secs_f64() * emulation_speed;
            let current_emulated_time_secs = total_emulated_time.as_secs_f64();
            let time_diff_secs = (desired_emulated_time_secs - current_emulated_time_secs).max(0.0);

            // Determine how many frames to step based on time difference
            // Calculate the actual number of frames we need to catch up
            // We step all necessary frames but only render the last one for smooth visuals
            let frames_behind = (time_diff_secs / target_frame_duration.as_secs_f64()) as usize;
            let frames_to_step = if frames_behind > 0 {
                // Cap frames per iteration to prevent pathological catch-up behavior
                // Higher cap (30) allows faster recovery from lag spikes without audio desync
                let max_frames_per_iteration: usize = 30;
                frames_behind.min(max_frames_per_iteration)
            } else {
                0
            };

            let mut last_frame_opt: Option<emu_core::types::Frame> = None;

            // Step the calculated number of frames
            for _ in 0..frames_to_step {
                // Step the frame
                match sys.step_frame() {
                    Ok(frame) => {
                        last_frame_opt = Some(frame);

                        // Track cycles (approximate - one frame worth)
                        total_cycles += 1; // This is a placeholder - actual cycle count would depend on system

                        // Handle audio for each stepped frame
                        let samples_per_frame_f =
                            (SAMPLE_RATE as f64 / frame_rate) + audio_sample_remainder;
                        let samples_per_frame = samples_per_frame_f.floor() as usize;
                        audio_sample_remainder = samples_per_frame_f - samples_per_frame as f64;
                        let audio_samples = sys.get_audio_samples(samples_per_frame);
                        let expected_mono = samples_per_frame;
                        let expected_stereo = samples_per_frame * 2;
                        if audio_samples.len() == expected_stereo {
                            for sample in audio_samples {
                                let _ = audio_tx.try_send(sample);
                            }
                        } else {
                            for i in 0..expected_mono {
                                let sample = audio_samples.get(i).copied().unwrap_or(0);
                                let _ = audio_tx.try_send(sample);
                                let _ = audio_tx.try_send(sample);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Emulation error: {}", e);
                        break;
                    }
                }
            }

            // Check for debug dump triggers
            if !debug_dump_triggered {
                let should_dump = if let (Some(trigger_pc), Some(debugger)) =
                    (cli_args.debug_dump_pc, sys.debugger())
                {
                    let cpu_state = debugger.get_cpu_state();
                    cpu_state.pc == trigger_pc
                } else {
                    false
                } || if let Some(trigger_cycles) = cli_args.debug_dump_cycles {
                    total_cycles >= trigger_cycles
                } else {
                    false
                };

                if should_dump {
                    let dump_file = cli_args
                        .debug_dump_file
                        .as_deref()
                        .unwrap_or("debug_dump.txt");
                    eprintln!("Debug dump triggered - writing to {}", dump_file);
                    match generate_debug_dump(
                        &sys,
                        dump_file,
                        total_cycles,
                        latest_frame_buffer.as_ref(),
                    ) {
                        Ok(()) => {
                            eprintln!("Debug dump written successfully to {}", dump_file);
                            debug_dump_triggered = true;
                            // Optionally exit after dump
                            // std::process::exit(0);
                        }
                        Err(e) => {
                            eprintln!("Failed to write debug dump: {}", e);
                        }
                    }
                }
            }

            // Accumulate emulated time outside the loop (based on frames actually stepped)
            total_emulated_time += target_frame_duration * frames_to_step as u32;

            // Render only the last frame to the display (always update client screen - requirement 3.2)
            if let Some(mut frame) = last_frame_opt {
                // Apply display filter to the frame
                // For phosphor persistence filter, use previous frame for temporal blending
                if settings.display_filter.requires_frame_history() {
                    // Extract previous frame pixels if available
                    let prev_pixels = latest_frame_buffer
                        .as_ref()
                        .map(|(pixels, _, _)| pixels.as_slice());
                    settings.display_filter.apply_with_history(
                        &mut frame.pixels,
                        prev_pixels,
                        frame.width as usize,
                        frame.height as usize,
                    );
                } else {
                    // For non-temporal filters, use regular apply
                    settings.display_filter.apply(
                        &mut frame.pixels,
                        frame.width as usize,
                        frame.height as usize,
                    );
                }

                // Store frame buffer for screenshots (after filter is applied)
                latest_frame_buffer = Some((
                    frame.pixels.clone(),
                    frame.width as usize,
                    frame.height as usize,
                ));

                // Update emulator texture with filtered frame
                egui_app.update_emulator_texture(
                    egui_backend.egui_ctx(),
                    &frame.pixels,
                    frame.width as usize,
                    frame.height as usize,
                );
            }

            // Handle keyboard input for emulator
            // We check if egui wants input (e.g., text field focused) and only skip controller updates then.
            // This allows controller input to work even when docked panels are visible.
            let egui_wants_input = egui_backend.egui_ctx().wants_keyboard_input();

            if !matches!(&sys, EmulatorSystem::PC(_)) {
                // For non-PC systems, use standard controller mapping (always update, even if egui has focus)
                let controller_state = get_controller_state(&egui_backend, &settings.input.player1);
                let snes_state = get_snes_controller_state(&egui_backend, &settings.input.player1);
                let chip8_state = get_chip8_controller_state(&egui_backend);
                let gw_state = get_gw_controller_state(&egui_backend);

                // ColecoVision needs special handling for 2-player input
                let coleco_p1_state =
                    get_colecovision_controller_state(&egui_backend, &settings.input.player1);
                let coleco_p2_state =
                    get_colecovision_controller_state(&egui_backend, &settings.input.player2);

                match &mut sys {
                    EmulatorSystem::SNES(s) => s.set_controller(0, snes_state),
                    EmulatorSystem::Chip8(s) => s.set_controller(chip8_state),
                    EmulatorSystem::GameAndWatch(s) => s.set_controller(gw_state),
                    EmulatorSystem::ColecoVision(s) => {
                        // Set both players for ColecoVision
                        s.set_controller(1, coleco_p1_state);
                        s.set_controller(2, coleco_p2_state);
                    }
                    _ => sys.set_controller(0, controller_state),
                }
            } else if !egui_wants_input {
                // PC systems handle keyboard directly via scancodes (only when egui doesn't need input)
                let pressed = egui_backend.get_sdl2_scancodes_pressed();
                let released = egui_backend.get_sdl2_scancodes_released();
                if let EmulatorSystem::PC(pc_sys) = &mut sys {
                    for scancode in pressed {
                        pc_sys.key_press_sdl2(*scancode as u32);
                    }
                    for scancode in released {
                        pc_sys.key_release_sdl2(*scancode as u32);
                    }
                }
            }
        } else {
            // Emulation is not active
            was_emulation_active = false;
        }

        // End egui frame and render
        egui_backend.end_frame();

        // Display FPS tracking (separate from emulation FPS - requirement 3.3)
        let frame_dt = last_frame.elapsed();
        display_frame_times.push(frame_dt);
        if display_frame_times.len() > 60 {
            display_frame_times.remove(0);
        }
        if !display_frame_times.is_empty() {
            let total_time: Duration = display_frame_times.iter().sum();
            let avg_frame_time = total_time.as_secs_f64() / display_frame_times.len() as f64;
            if avg_frame_time > 0.0 {
                current_fps = (1.0 / avg_frame_time) as f32;
            }
        }

        // Frame timing - skip sleep in benchmark mode
        if !cli_args.benchmark {
            // Target 120 FPS for display refresh (allows quicker catch-up)
            let target_display_time = Duration::from_secs_f64(1.0 / 120.0);

            // Sleep to maintain consistent display frame rate
            if frame_dt < target_display_time {
                std::thread::sleep(target_display_time - frame_dt);
            }
        }
        last_frame = Instant::now();
    }
}
