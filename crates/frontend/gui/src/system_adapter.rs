//! System-specific debug information and configuration
//!
//! This module provides a unified interface for different system types,
//! exposing their specific debug information and configuration options
//! to the egui UI in a type-safe and modular way.

/// Unified debug information for all systems
#[derive(Clone)]
pub struct SystemDebugInfo {
    pub system_type: String,
    pub fields: Vec<(String, String)>,
}

impl SystemDebugInfo {
    pub fn new(system_type: String) -> Self {
        Self {
            system_type,
            fields: Vec::new(),
        }
    }

    pub fn add_field(&mut self, label: String, value: String) {
        self.fields.push((label, value));
    }

    pub fn from_nes(info: &emu_nes::DebugInfo) -> Self {
        let mut debug_info = Self::new("NES".to_string());
        debug_info.add_field(
            "Mapper".to_string(),
            format!("{} ({})", info.mapper_name, info.mapper_number),
        );
        debug_info.add_field("Timing".to_string(), format!("{:?}", info.timing_mode));
        debug_info.add_field("PRG Banks".to_string(), format!("{}", info.prg_banks));
        debug_info.add_field("CHR Banks".to_string(), format!("{}", info.chr_banks));
        debug_info
    }

    pub fn from_gb(info: &emu_gb::DebugInfo) -> Self {
        let mut debug_info = Self::new("Game Boy".to_string());
        debug_info.add_field("PC".to_string(), format!("${:04X}", info.pc));
        debug_info.add_field("SP".to_string(), format!("${:04X}", info.sp));
        debug_info.add_field("AF".to_string(), format!("${:04X}", info.af));
        debug_info.add_field("BC".to_string(), format!("${:04X}", info.bc));
        debug_info.add_field("DE".to_string(), format!("${:04X}", info.de));
        debug_info.add_field("HL".to_string(), format!("${:04X}", info.hl));
        debug_info.add_field("IME".to_string(), format!("{}", info.ime));
        debug_info.add_field("Halted".to_string(), format!("{}", info.halted));
        debug_info.add_field("LY".to_string(), format!("{}", info.ly));
        debug_info.add_field("LCDC".to_string(), format!("${:02X}", info.lcdc));
        debug_info
    }

    pub fn from_atari2600(info: &emu_atari2600::DebugInfo) -> Self {
        let mut debug_info = Self::new("Atari 2600".to_string());
        debug_info.add_field("ROM Size".to_string(), format!("{} bytes", info.rom_size));
        debug_info.add_field("Banking Scheme".to_string(), info.banking_scheme.clone());
        debug_info.add_field("Current Bank".to_string(), format!("{}", info.current_bank));
        debug_info.add_field("Scanline".to_string(), format!("{}", info.scanline));
        debug_info
    }

    pub fn from_pc(info: &emu_pc::DebugInfo) -> Self {
        let mut debug_info = Self::new("PC".to_string());
        debug_info.add_field(
            "CS:IP".to_string(),
            format!("{:04X}:{:04X}", info.cs, info.ip),
        );
        debug_info.add_field("AX".to_string(), format!("${:04X}", info.ax));
        debug_info.add_field("BX".to_string(), format!("${:04X}", info.bx));
        debug_info.add_field("CX".to_string(), format!("${:04X}", info.cx));
        debug_info.add_field("DX".to_string(), format!("${:04X}", info.dx));
        debug_info.add_field("SP".to_string(), format!("${:04X}", info.sp));
        debug_info.add_field("BP".to_string(), format!("${:04X}", info.bp));
        debug_info.add_field("SI".to_string(), format!("${:04X}", info.si));
        debug_info.add_field("DI".to_string(), format!("${:04X}", info.di));
        debug_info.add_field("Flags".to_string(), format!("${:04X}", info.flags));
        debug_info.add_field("Cycles".to_string(), format!("{}", info.cycles));
        debug_info
    }

    pub fn from_snes(info: &emu_snes::DebugInfo) -> Self {
        let mut debug_info = Self::new("SNES".to_string());
        debug_info.add_field("ROM Size".to_string(), format!("{} bytes", info.rom_size));
        debug_info.add_field("SMC Header".to_string(), format!("{}", info.has_smc_header));
        debug_info.add_field(
            "PBR:PC".to_string(),
            format!("{:02X}:{:04X}", info.pbr, info.pc),
        );
        debug_info.add_field(
            "Emulation Mode".to_string(),
            format!("{}", info.emulation_mode),
        );
        debug_info
    }

    pub fn from_n64(info: &emu_n64::DebugInfo) -> Self {
        let mut debug_info = Self::new("N64".to_string());
        debug_info.add_field("ROM Name".to_string(), info.rom_name.clone());
        debug_info.add_field("ROM Size".to_string(), format!("{} MB", info.rom_size_mb));
        debug_info.add_field("PC".to_string(), format!("${:016X}", info.pc));
        debug_info.add_field("RSP Microcode".to_string(), info.rsp_microcode.clone());
        debug_info.add_field(
            "RSP Vertex Count".to_string(),
            format!("{}", info.rsp_vertex_count),
        );
        debug_info.add_field("RDP Status".to_string(), format!("{:?}", info.rdp_status));
        debug_info.add_field(
            "Framebuffer".to_string(),
            info.framebuffer_resolution.clone(),
        );
        debug_info
    }
}

/// Configuration options for different system types
#[allow(clippy::upper_case_acronyms)]
#[allow(dead_code)]
pub enum SystemConfig {
    NES(NesConfig),
    GameBoy(GameBoyConfig),
    Atari2600(Atari2600Config),
    PC(PcConfig),
    SNES(SnesConfig),
    N64(N64Config),
}

#[allow(dead_code)]
#[derive(Default)]
pub struct NesConfig {
    pub ppu_show_sprites: bool,
    pub ppu_show_background: bool,
    pub apu_channels_enabled: [bool; 5], // Pulse1, Pulse2, Triangle, Noise, DMC
}

#[allow(dead_code)]
#[derive(Default)]
pub struct GameBoyConfig {
    pub ppu_show_sprites: bool,
    pub ppu_show_background: bool,
    pub ppu_show_window: bool,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Atari2600Config {
    pub tia_show_playfield: bool,
    pub tia_show_sprites: bool,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct PcConfig {
    pub show_bda_inspector: bool,
    pub video_adapter: String,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct SnesConfig {
    pub ppu_mode: u8,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct N64Config {
    pub rdp_enable_texturing: bool,
    pub rsp_show_microcode: bool,
}

impl SystemConfig {
    #[allow(dead_code)]
    pub fn system_name(&self) -> &str {
        match self {
            SystemConfig::NES(_) => "NES",
            SystemConfig::GameBoy(_) => "Game Boy",
            SystemConfig::Atari2600(_) => "Atari 2600",
            SystemConfig::PC(_) => "PC",
            SystemConfig::SNES(_) => "SNES",
            SystemConfig::N64(_) => "N64",
        }
    }
}
