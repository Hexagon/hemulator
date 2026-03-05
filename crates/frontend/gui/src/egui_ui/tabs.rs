//! Tab manager for left panel

use crate::settings::ScalingMode;
use crate::system_adapter::{EnhancedDebugState, SystemDebugInfo};
use egui::{ScrollArea, TextureHandle, Ui};
use emu_core::debug::MemoryRegion;
use std::rc::Rc;

/// Application version constant
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Emulator,
    About,
}

/// System-specific tile viewer data
#[derive(Clone)]
pub enum SystemTileData {
    NES(NesTileData),
    GameBoy(GbTileData),
    GBA(GbaTileData),
    SMS(SmsTileData),
    ColecoVision(ColecoVisionTileData),
    SG1000(Sg1000TileData),
    SNES(SnesTileData),
    Atari2600(Atari2600TileData),
    Chip8(Chip8TileData),
}

/// NES cartridge information data
#[derive(Clone)]
/// Generic cartridge information for all cartridge-based systems
pub struct CartridgeData {
    /// System type (e.g., "NES", "Game Boy", "Atari 2600")
    pub system_name: String,
    /// CRC32 checksum of the ROM file
    pub crc32: u32,
    /// ROM size in bytes
    pub rom_size: usize,

    // NES-specific fields (optional, only populated for NES)
    pub nes_mapper: Option<u16>,
    pub nes_submapper: Option<u8>,
    pub nes_mapper_name: Option<String>,
    pub nes_mirroring: Option<String>,
    pub nes_timing: Option<String>,
    pub nes_prg_size: Option<usize>,
    pub nes_chr_size: Option<usize>,
    pub nes_header_mapper: Option<u16>,
    pub nes_header_submapper: Option<u8>,
    pub nes_header_mirroring: Option<String>,
    pub nes_db_mapper_override: bool,
    pub nes_db_mirroring_override: bool,
    pub nes_board_name: Option<String>,

    // SNES-specific fields (optional, only populated for SNES)
    pub snes_has_smc_header: Option<bool>,
    pub snes_mapping_mode: Option<String>,
    pub snes_chip_type: Option<String>,
}

/// NES tile viewer data
#[derive(Clone)]
pub struct NesTileData {
    /// CHR data (pattern tables) - 8KB for NES
    /// Uses Rc to avoid cloning the full CHR data
    pub chr_data: Rc<Vec<u8>>,
    /// Palette data - 32 bytes for NES (4 colors x 8 palettes)
    pub palette: Vec<u8>,
    /// NES master palette for color lookup
    pub master_palette: Vec<u32>,
    /// OAM data - 256 bytes (64 sprites x 4 bytes each)
    pub oam: Vec<u8>,
    /// VRAM data - 2KB nametables
    pub vram: Vec<u8>,
    /// Whether this is CHR-RAM (true) or CHR-ROM (false)
    pub chr_is_ram: bool,
    /// Current PPUCTRL value (for pattern table selection info)
    pub ppuctrl: u8,
    /// Current PPUMASK value
    pub ppumask: u8,
    /// Current scroll values
    pub scroll_x: u8,
    pub scroll_y: u8,
    /// Current mirroring mode
    pub mirroring: String,
}

/// Game Boy tile viewer data
#[derive(Clone)]
pub struct GbTileData {
    pub vram_bank0: Vec<u8>,
    pub vram_bank1: Vec<u8>,
    pub oam: Vec<u8>,
    pub bg_palettes: Vec<u32>,
    pub obj_palettes: Vec<u32>,
    pub lcdc: u8,
    pub scx: u8,
    pub scy: u8,
    pub wx: u8,
    pub wy: u8,
    pub is_cgb_mode: bool,
}

/// GBA tile viewer data
#[derive(Clone)]
pub struct GbaTileData {
    /// VRAM data - 96KB (tile data and tilemaps)
    pub vram: Vec<u8>,
    /// Palette RAM - 1KB (512 colors as RGBA)
    pub palette_ram: Vec<u8>,
    /// OAM data - 1KB (128 sprites × 8 bytes)
    pub oam: Vec<u8>,
    /// Master palette converted to RGBA for display
    pub master_palette: Vec<u32>,

    // PPU state registers
    /// DISPCNT - Display Control
    pub dispcnt: u16,
    /// BG0CNT - BG0 Control
    pub bg0cnt: u16,
    /// BG1CNT - BG1 Control
    pub bg1cnt: u16,
    /// BG2CNT - BG2 Control
    pub bg2cnt: u16,
    /// BG3CNT - BG3 Control
    pub bg3cnt: u16,
    /// BG scroll offsets (X and Y for each BG layer)
    pub bg_scroll: [(u16, u16); 4],
    /// BLDCNT - Color Special Effects Selection
    pub bldcnt: u16,
    /// BLDALPHA - Alpha Blending Coefficients
    pub bldalpha: u16,
}

/// SMS tile viewer data
#[derive(Clone)]
pub struct SmsTileData {
    pub vram: Vec<u8>,
    pub cram: Vec<u8>,
    pub palette: Vec<u32>,
    pub registers: Vec<u8>,
}

/// ColecoVision tile viewer data
#[derive(Clone)]
pub struct ColecoVisionTileData {
    pub vram: Vec<u8>,
    pub palette: Vec<u32>,
    pub registers: Vec<u8>,
}

/// SG-1000 tile viewer data (TMS9918A VDP - same as ColecoVision)
#[derive(Clone)]
pub struct Sg1000TileData {
    pub vram: Vec<u8>,
    pub palette: Vec<u32>,
    pub registers: Vec<u8>,
}

/// SNES tile viewer data
#[derive(Clone)]
pub struct SnesTileData {
    pub vram: Vec<u8>,
    pub cgram: Vec<u8>,
    pub oam: Vec<u8>,
    pub palette: Vec<u32>,
    pub bg_mode: u8,
    pub screen_enabled: bool,
    // Tilemap configuration
    pub bg1sc: u8,
    pub bg2sc: u8,
    pub bg3sc: u8,
    pub bg4sc: u8,
    pub bg12nba: u8,
    pub bg34nba: u8,
    // Scroll positions
    pub bg1_hofs: u16,
    pub bg1_vofs: u16,
    pub bg2_hofs: u16,
    pub bg2_vofs: u16,
    pub bg3_hofs: u16,
    pub bg3_vofs: u16,
    pub bg4_hofs: u16,
    pub bg4_vofs: u16,
    // Main screen designation
    pub tm: u8,
}

/// CHIP-8 inspector data
#[derive(Clone)]
pub struct Chip8TileData {
    /// V registers (V0-VF)
    pub v_registers: [u8; 16],
    /// Index register
    pub i: u16,
    /// Program counter
    pub pc: u16,
    /// Stack pointer
    pub sp: u8,
    /// Stack contents
    pub stack: [u16; 16],
    /// Delay timer
    pub delay_timer: u8,
    /// Sound timer
    pub sound_timer: u8,
    /// Display plane 0 (primary display)
    pub display_plane0: Vec<bool>,
    /// Display plane 1 (for XO-CHIP)
    pub display_plane1: Vec<bool>,
    /// Display width
    pub display_width: usize,
    /// Display height
    pub display_height: usize,
    /// Current mode (Chip8, SuperChip, XoChip, etc.)
    pub mode: String,
    /// Selected drawing plane (bitmask for XO-CHIP)
    pub selected_plane: u8,
    /// High resolution mode enabled
    pub high_res: bool,
    /// Waiting for key press
    pub waiting_for_key: bool,
    /// Key states (16 keys)
    pub keys: [bool; 16],
}

/// Atari 2600 inspector data
#[derive(Clone)]
pub struct Atari2600TileData {
    /// Playfield registers (PF0, PF1, PF2)
    pub pf0: u8,
    pub pf1: u8,
    pub pf2: u8,
    /// Playfield control flags
    pub playfield_reflect: bool,
    pub playfield_score_mode: bool,
    pub playfield_priority: bool,
    /// Player 0 and 1 graphics
    pub grp0: u8,
    pub grp1: u8,
    /// Player positions
    pub player0_x: u8,
    pub player1_x: u8,
    /// Player reflection flags
    pub player0_reflect: bool,
    pub player1_reflect: bool,
    /// Player number and size (NUSIZ)
    pub nusiz0: u8,
    pub nusiz1: u8,
    /// Missile enable flags
    pub enam0: bool,
    pub enam1: bool,
    /// Missile positions
    pub missile0_x: u8,
    pub missile1_x: u8,
    /// Ball enable and position
    pub enabl: bool,
    pub ball_x: u8,
    pub ball_size: u8,
    /// Color registers
    pub colubk: u8, // Background
    pub colupf: u8, // Playfield
    pub colup0: u8, // Player 0
    pub colup1: u8, // Player 1
    /// NTSC palette (128 colors) - static reference to avoid allocations
    pub master_palette: &'static [u32; 128],
    /// Collision detection registers
    pub cxm0p: u8, // Missile 0 to Player
    pub cxm1p: u8,  // Missile 1 to Player
    pub cxp0fb: u8, // Player 0 to Playfield/Ball
    pub cxp1fb: u8, // Player 1 to Playfield/Ball
    pub cxm0fb: u8, // Missile 0 to Playfield/Ball
    pub cxm1fb: u8, // Missile 1 to Playfield/Ball
    pub cxblpf: u8, // Ball to Playfield
    pub cxppmm: u8, // Player/Missile collisions
    /// Video blanking state
    pub vblank: bool,
    pub vsync: bool,
}

/// PC BIOS Data Area (BDA) and Extended BIOS Data Area (EBDA) viewer data
#[derive(Clone)]
pub struct PcBdaData {
    /// Equipment word at 0x0410-0x0411
    pub equipment_word: u16,
    /// Memory size in KB at 0x0413-0x0414
    pub memory_size_kb: u16,
    /// Video mode at 0x0449
    pub video_mode: u8,
    /// Video columns at 0x044A
    pub video_columns: u8,
    /// Number of serial ports (derived from equipment word bits 9-11)
    pub num_serial_ports: u8,
    /// Number of parallel ports (derived from equipment word bits 14-15)
    pub num_parallel_ports: u8,
    /// Number of hard drives at 0x0475
    pub num_hard_drives: u8,
    /// Raw BDA memory (0x0400-0x04FF, 256 bytes)
    pub bda_raw: Vec<u8>,
    /// Raw EBDA memory (1KB from segment stored in BDA via `ebda_segment`)
    pub ebda_raw: Vec<u8>,
    /// EBDA segment address from BDA at 0x040E-0x040F
    pub ebda_segment: u16,
}

/// PS1 GPU inspector data for the debug UI
#[derive(Clone, Debug)]
pub struct Ps1GpuData {
    pub gpustat: u32,
    pub display_vram_x: u32,
    pub display_vram_y: u32,
    pub display_horiz_start: u32,
    pub display_horiz_end: u32,
    pub display_vert_start: u32,
    pub display_vert_end: u32,
    pub hres: String,
    pub vres: String,
    pub is_pal: bool,
    pub display_24bit: bool,
    pub interlace: bool,
    pub display_disabled: bool,
    pub draw_area_left: u32,
    pub draw_area_top: u32,
    pub draw_area_right: u32,
    pub draw_area_bottom: u32,
    pub draw_offset_x: i32,
    pub draw_offset_y: i32,
    pub texpage_x: u32,
    pub texpage_y: u32,
    pub tex_depth: String,
    pub semi_transparency: String,
    pub dithering: bool,
    pub set_mask_bit: bool,
    pub check_mask_bit: bool,
    pub tex_window_mask_x: u8,
    pub tex_window_mask_y: u8,
    pub tex_window_offset_x: u8,
    pub tex_window_offset_y: u8,
    pub scanline: u32,
    pub in_vblank: bool,
    pub irq: bool,
}

/// Mount point information for inspector
#[derive(Clone, Debug)]
pub struct MountInfo {
    /// Mount point ID (e.g., "Cartridge", "Floppy0")
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// File extensions accepted
    pub extensions: Vec<String>,
    /// Whether required
    pub required: bool,
    /// Currently mounted file path (if any)
    pub mounted_file: Option<String>,
}

/// Actions that can be triggered from tabs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabAction {
    CreateNewProject(String), // String is the system name
    SelectCartridge,          // Open file dialog to select cartridge
}

/// Debug actions that can be triggered from the debug tab
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAction {
    Step,                                                         // Step one instruction
    Pause,                                                        // Pause emulation
    Resume,                                                       // Resume emulation
    StartTrace(String), // Start instruction tracing with specified filename
    StopTrace,          // Stop instruction tracing and dump to file
    AddBreakpoint(u32, emu_core::breakpoints::BreakpointType), // Add breakpoint (address, type)
    RemoveBreakpoint(u32, emu_core::breakpoints::BreakpointType), // Remove breakpoint
    SetGbAudioChannels([bool; 4]), // Pulse1, Pulse2, Wave, Noise
}

pub struct TabManager {
    pub active_tab: Tab,
    pub log_messages: Vec<String>,
    pub about_visible: bool,
    pub debug_info: Option<SystemDebugInfo>,
    pub enhanced_debug_state: Option<EnhancedDebugState>,
    pub system_tile_data: Option<SystemTileData>,
    pub pc_bda_data: Option<PcBdaData>,
    pub ps1_gpu_data: Option<Ps1GpuData>,
    pub cartridge_data: Option<CartridgeData>,
    pub mount_info: Vec<MountInfo>,
    pub system_loaded: bool,
    pub system_name: String,
    pub pending_action: Option<TabAction>,
    pub pending_debug_action: Option<DebugAction>,
    pub selected_memory_region_index: usize,
    pub memory_view_address: u32,
    /// Cached memory data for the current view (address -> bytes)
    pub cached_memory: Vec<u8>,
    /// Address range of cached memory
    pub cached_memory_start: u32,
    /// SNES tile viewer state
    pub snes_bpp_mode: u8,
    pub snes_selected_palette: usize,
    /// Instruction trace state
    pub trace_active: bool,
    pub trace_filename: Option<String>,
    /// Breakpoint management state
    pub breakpoint_address_input: String,
    pub breakpoint_type_selected: usize, // 0=Execute, 1=Read, 2=Write
    pub show_breakpoint_panel: bool,
    pub gb_audio_channels: [bool; 4], // Pulse1, Pulse2, Wave, Noise
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Emulator,
            log_messages: Vec::new(),
            about_visible: false,
            debug_info: None,
            enhanced_debug_state: None,
            system_tile_data: None,
            pc_bda_data: None,
            ps1_gpu_data: None,
            cartridge_data: None,
            mount_info: Vec::new(),
            system_loaded: false,
            system_name: String::new(),
            pending_action: None,
            pending_debug_action: None,
            selected_memory_region_index: 0,
            memory_view_address: 0,
            cached_memory: Vec::new(),
            cached_memory_start: 0,
            snes_bpp_mode: 4, // Default to 4bpp
            snes_selected_palette: 0,
            trace_active: false,
            trace_filename: None,
            breakpoint_address_input: String::new(),
            breakpoint_type_selected: 0,
            show_breakpoint_panel: false,
            gb_audio_channels: [true, true, true, true],
        }
    }

    pub fn add_log(&mut self, message: String) {
        self.log_messages.push(message);
        // Keep only last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.remove(0);
        }
    }

    pub fn update_system_tile_data(&mut self, data: SystemTileData) {
        self.system_tile_data = Some(data);
    }

    pub fn update_pc_bda_data(&mut self, data: PcBdaData) {
        self.pc_bda_data = Some(data);
    }

    pub fn update_ps1_gpu_data(&mut self, data: Ps1GpuData) {
        self.ps1_gpu_data = Some(data);
    }

    pub fn update_cartridge_data(&mut self, data: CartridgeData) {
        self.cartridge_data = Some(data);
    }

    pub fn update_mount_info(&mut self, mounts: Vec<MountInfo>) {
        self.mount_info = mounts;
    }

    pub fn show_about_tab(&mut self) {
        self.about_visible = true;
        self.active_tab = Tab::About;
    }

    pub fn update_debug_info(&mut self, info: SystemDebugInfo) {
        self.debug_info = Some(info);
    }

    pub fn update_enhanced_debug_state(&mut self, state: EnhancedDebugState) {
        self.enhanced_debug_state = Some(state);
    }

    /// Update cached memory data for the memory viewer
    pub fn update_cached_memory(&mut self, data: Vec<u8>, start_address: u32) {
        self.cached_memory = data;
        self.cached_memory_start = start_address;
    }

    /// Get and clear any pending action
    pub fn take_action(&mut self) -> Option<TabAction> {
        self.pending_action.take()
    }

    /// Get and clear any pending debug action
    pub fn take_debug_action(&mut self) -> Option<DebugAction> {
        self.pending_debug_action.take()
    }

    /// Clear all debug and inspector state
    ///
    /// This prevents stale debug data from a previous system/ROM from being displayed
    /// in inspector panels. Clears:
    /// - CPU debug info (registers, flags)
    /// - Enhanced debug state (disassembly, memory maps)
    /// - System-specific tile/graphics data (NES CHR, GB tiles, etc.)
    /// - PC BIOS data area information
    /// - Cartridge metadata
    /// - Memory viewer cache and position
    ///
    /// Call this when switching systems or loading a new ROM.
    pub fn clear_debug_state(&mut self) {
        self.debug_info = None;
        self.enhanced_debug_state = None;
        self.system_tile_data = None;
        self.pc_bda_data = None;
        self.ps1_gpu_data = None;
        self.cartridge_data = None;
        self.cached_memory.clear();
        self.cached_memory_start = 0;
        self.selected_memory_region_index = 0;
        self.memory_view_address = 0;
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        emulator_texture: &Option<TextureHandle>,
        scaling_mode: ScalingMode,
    ) {
        // Only show tab bar if there are multiple tabs visible (About)
        let has_extra_tabs = self.about_visible;

        if has_extra_tabs {
            // Tab bar with improved visual styling
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Emulator, "🎮 Emulator");

                // Help tab removed.

                if self.about_visible {
                    ui.selectable_value(&mut self.active_tab, Tab::About, "ℹ️ About");
                    // Use a colored button for the close icon to ensure visibility
                    let close_button = egui::Button::new(
                        egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                    )
                    .small();
                    if ui
                        .add(close_button)
                        .on_hover_text("Close About tab")
                        .clicked()
                    {
                        self.about_visible = false;
                        if self.active_tab == Tab::About {
                            self.active_tab = Tab::Emulator;
                        }
                    }
                }
            });

            ui.separator();
        } else {
            // When only Emulator tab is visible, ensure we're on it
            self.active_tab = Tab::Emulator;
        }

        // Tab content
        match self.active_tab {
            Tab::Emulator => self.render_emulator_tab(ui, emulator_texture, scaling_mode),
            Tab::About => self.render_about_tab(ui),
        }
    }

    fn render_emulator_tab(
        &mut self,
        ui: &mut Ui,
        emulator_texture: &Option<TextureHandle>,
        scaling_mode: ScalingMode,
    ) {
        // Check if a system is loaded but needs a cartridge (required mount not satisfied)
        let needs_cartridge = self.system_loaded
            && self.mount_info.iter().any(|m| {
                m.required
                    && m.mounted_file.is_none()
                    && (m.id.to_lowercase().contains("cartridge")
                        || m.id.to_lowercase().contains("rom"))
            });

        // Show emulator view only if we have a texture AND don't need a cartridge
        // Otherwise show welcome/ready screen
        let should_show_emulator = emulator_texture.is_some() && !needs_cartridge;

        ui.centered_and_justified(|ui| {
            if should_show_emulator {
                let texture = emulator_texture.as_ref().unwrap();
                let available_size = ui.available_size();
                let texture_size = texture.size_vec2();
                let aspect_ratio = texture_size.x / texture_size.y;

                let (display_width, display_height) = match scaling_mode {
                    ScalingMode::Original => {
                        // 1:1 pixel mapping - use original texture size
                        (texture_size.x, texture_size.y)
                    }
                    ScalingMode::Fit => {
                        // Fit to window while maintaining aspect ratio
                        let display_width = available_size.x.min(available_size.y * aspect_ratio);
                        let display_height = display_width / aspect_ratio;
                        (display_width, display_height)
                    }
                    ScalingMode::Stretch => {
                        // Fill entire window, ignoring aspect ratio
                        (available_size.x, available_size.y)
                    }
                };

                let image = egui::Image::from_texture(texture)
                    .fit_to_exact_size(egui::vec2(display_width, display_height));
                ui.add(image);
            } else if needs_cartridge {
                // System is loaded but needs a cartridge - show system-specific header
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);

                    let system_icon = match self.system_name.as_str() {
                        "NES" => "🎮",
                        "SNES" => "🎮",
                        "Game Boy" => "🎮",
                        "GBA" => "🎮",
                        "SMS" => "🎮",
                        "Atari 2600" => "🕹️",
                        "N64" => "🎮",
                        "CHIP-8" => "💻",
                        "ColecoVision" => "🕹️",
                        "SG-1000" => "🎮",
                        _ => "🎮",
                    };
                    ui.heading(
                        egui::RichText::new(format!(
                            "{} {} Emulator Ready",
                            system_icon, self.system_name
                        ))
                        .size(32.0)
                        .strong(),
                    );
                    ui.add_space(15.0);
                    ui.label(
                        egui::RichText::new("Load a cartridge to start emulation")
                            .size(18.0)
                            .weak(),
                    );
                    ui.add_space(50.0);

                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(30, 30, 30))
                        .corner_radius(8.0)
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("📁 Select Cartridge...")
                                                .size(18.0),
                                        )
                                        .min_size(egui::vec2(200.0, 40.0)),
                                    )
                                    .clicked()
                                {
                                    self.pending_action = Some(TabAction::SelectCartridge);
                                }
                            });
                        });
                });
            } else {
                // No system loaded - show welcome screen with project creation instructions
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);

                    ui.add(
                        egui::Image::new(egui::include_image!(
                            "../../../../../assets/icon_256.png"
                        ))
                        .max_width(200.0)
                        .corner_radius(10.0),
                    );
                    ui.add_space(15.0);
                    ui.label(
                        egui::RichText::new("Multi-System Console Emulator")
                            .size(18.0)
                            .weak(),
                    );
                    ui.add_space(50.0);

                    ui.label(egui::RichText::new("Get Started").size(22.0).strong());
                    ui.add_space(20.0);

                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(30, 30, 30))
                        .corner_radius(8.0)
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "File -> New Project -> Auto Detect from ROM...",
                                    )
                                    .size(16.0),
                                );
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new("Load a ROM to start emulating").weak(),
                                );
                                ui.add_space(15.0);
                                ui.label(egui::RichText::new("File -> Open Project").size(16.0));
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("Load a saved .hemu project").weak());
                            });
                        });
                });
            }
        });
    }

    pub fn render_memory_tab(&mut self, ui: &mut Ui) {
        // If we have enhanced debug state, show memory explorer
        if let Some(state) = self.enhanced_debug_state.clone() {
            if state.memory_regions.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("💾").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("Memory Inspector");
                    ui.add_space(10.0);
                    ui.label("No memory regions defined for this system");
                });
                return;
            }

            ui.vertical(|ui| {
                ui.heading("💾 Memory Inspector");
                ui.separator();
                ui.add_space(5.0);

                // Dropdown for memory region selection
                ui.horizontal(|ui| {
                    ui.label("Region:");
                    egui::ComboBox::from_id_salt(format!(
                        "memory_inspector_region_selector_{}",
                        state.system_type
                    ))
                    .selected_text(
                        state
                            .memory_regions
                            .get(self.selected_memory_region_index)
                            .map(|r| r.name.as_str())
                            .unwrap_or("Select region"),
                    )
                    .show_ui(ui, |ui| {
                        for (idx, region) in state.memory_regions.iter().enumerate() {
                            if ui
                                .selectable_value(
                                    &mut self.selected_memory_region_index,
                                    idx,
                                    &region.name,
                                )
                                .clicked()
                            {
                                // Reset view address to region start when changing regions
                                self.memory_view_address = region.start;
                            }
                        }
                    });
                });

                ui.separator();
                ui.add_space(5.0);

                // Show selected region info and hex viewer
                if let Some(region) = state.memory_regions.get(self.selected_memory_region_index) {
                    // Region info
                    ui.horizontal(|ui| {
                        ui.label("Range:");
                        ui.label(
                            egui::RichText::new(format!(
                                "${:04X} - ${:04X}",
                                region.start, region.end
                            ))
                            .monospace(),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        ui.label(
                            egui::RichText::new(format!("{} bytes", region.size())).monospace(),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Access:");
                        let access = match (region.readable, region.writable) {
                            (true, true) => "Read/Write",
                            (true, false) => "Read-only",
                            (false, true) => "Write-only",
                            (false, false) => "No access",
                        };
                        ui.label(egui::RichText::new(access).monospace());
                    });

                    ui.add_space(5.0);
                    ui.separator();

                    // Address navigation
                    ui.horizontal(|ui| {
                        ui.label("Address:");
                        let mut addr_input = format!("{:04X}", self.memory_view_address);
                        if ui.text_edit_singleline(&mut addr_input).changed() {
                            if let Ok(addr) = u32::from_str_radix(&addr_input, 16) {
                                self.memory_view_address = addr.clamp(region.start, region.end);
                            }
                        }
                        if ui.button("⬆").clicked() && self.memory_view_address >= region.start + 16
                        {
                            self.memory_view_address -= 16;
                        }
                        if ui.button("⬇").clicked() && self.memory_view_address + 16 <= region.end
                        {
                            self.memory_view_address += 16;
                        }
                        if ui.button("Page ⬆").clicked()
                            && self.memory_view_address >= region.start + 256
                        {
                            self.memory_view_address -= 256;
                        }
                        if ui.button("Page ⬇").clicked()
                            && self.memory_view_address + 256 <= region.end
                        {
                            self.memory_view_address += 256;
                        }
                    });

                    ui.add_space(5.0);

                    // Hex viewer
                    egui::ScrollArea::vertical()
                        .id_salt(format!("memory_hex_viewer_{}", state.system_type))
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_hex_dump(ui, region, &state.system_type);
                        });
                }
            });
        } else {
            // No debug state available
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("💾").size(48.0));
                ui.add_space(10.0);
                ui.heading("Memory Inspector");
                ui.add_space(10.0);
                ui.label("Load a ROM to see memory contents");
            });
        }
    }

    pub fn render_mounts_tab(&self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("💿 Mount Points");
            ui.separator();
            ui.add_space(10.0);

            if self.mount_info.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("💿").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("No Mount Points");
                    ui.add_space(10.0);
                    ui.label("Load a ROM or create a system to see mount points");
                });
            } else {
                ui.label("Currently available mount points for this system:");
                ui.add_space(10.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for mount in &self.mount_info {
                            egui::Frame::new()
                                .fill(ui.visuals().faint_bg_color)
                                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                                .corner_radius(4.0)
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Icon based on mount status
                                        let icon = if mount.mounted_file.is_some() {
                                            "✅"
                                        } else if mount.required {
                                            "⚠️"
                                        } else {
                                            "⚪"
                                        };
                                        ui.label(egui::RichText::new(icon).size(20.0));

                                        ui.vertical(|ui| {
                                            // Mount point name and ID
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&mount.name)
                                                        .strong()
                                                        .size(16.0),
                                                );
                                                if mount.required {
                                                    ui.label(
                                                        egui::RichText::new("(required)")
                                                            .weak()
                                                            .italics(),
                                                    );
                                                }
                                            });

                                            ui.add_space(5.0);

                                            // Mount point ID
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("ID:").weak().size(12.0),
                                                );
                                                ui.label(
                                                    egui::RichText::new(&mount.id)
                                                        .monospace()
                                                        .size(12.0),
                                                );
                                            });

                                            // Accepted file types
                                            if !mount.extensions.is_empty() {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Accepts:")
                                                            .weak()
                                                            .size(12.0),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(
                                                            mount.extensions.join(", "),
                                                        )
                                                        .monospace()
                                                        .size(12.0),
                                                    );
                                                });
                                            }

                                            ui.add_space(5.0);

                                            // Current mount status
                                            if let Some(ref file_path) = mount.mounted_file {
                                                // Extract just the filename from the path
                                                let filename = std::path::Path::new(file_path)
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or(file_path);

                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Mounted:")
                                                            .color(egui::Color32::from_rgb(
                                                                100, 200, 100,
                                                            ))
                                                            .strong(),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(filename)
                                                            .monospace()
                                                            .color(egui::Color32::from_rgb(
                                                                100, 200, 100,
                                                            )),
                                                    );
                                                });

                                                // Show full path on separate line if different from filename
                                                if filename != file_path {
                                                    ui.label(
                                                        egui::RichText::new(file_path)
                                                            .weak()
                                                            .italics()
                                                            .size(10.0),
                                                    );
                                                }
                                            } else {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new("Status:").color(
                                                        egui::Color32::from_rgb(200, 200, 100),
                                                    ));
                                                    ui.label(
                                                        egui::RichText::new("Empty")
                                                            .weak()
                                                            .italics(),
                                                    );
                                                });
                                            }
                                        });
                                    });
                                });

                            ui.add_space(8.0);
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Summary
                        let mounted_count = self
                            .mount_info
                            .iter()
                            .filter(|m| m.mounted_file.is_some())
                            .count();
                        let required_count = self.mount_info.iter().filter(|m| m.required).count();

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Total mount points:").strong());
                            ui.label(self.mount_info.len().to_string());
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Mounted:").strong());
                            ui.label(mounted_count.to_string());
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Required:").strong());
                            ui.label(required_count.to_string());
                        });
                    });
            }
        });
    }

    // Help tab removed.

    pub fn render_debug_tab(&mut self, ui: &mut Ui) {
        // If we have enhanced debug state, show the comprehensive 3-panel view
        if let Some(state) = self.enhanced_debug_state.clone() {
            self.render_enhanced_debug_view(ui, &state);
        } else if let Some(ref debug_info) = self.debug_info {
            // Fallback to simple legacy view
            self.render_legacy_debug_view(ui, debug_info);
        } else {
            // No debug information available
            let available_height = ui.available_height();
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_height(available_height)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🔧").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("No Debug Information Available");
                        ui.add_space(10.0);
                        ui.label("Load a ROM to see system-specific debug information");
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("Debug info includes CPU state, memory maps, and")
                                .weak(),
                        );
                        ui.label(
                            egui::RichText::new("disassembly for troubleshooting and analysis.")
                                .weak(),
                        );
                    });
                });
        }
    }

    fn render_enhanced_debug_view(&mut self, ui: &mut Ui, state: &EnhancedDebugState) {
        let total_available_height = ui.available_height();

        ui.vertical(|ui| {
            // Header
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.heading(
                    egui::RichText::new(format!("🔧 {} Debugger", state.system_type))
                        .size(20.0)
                        .strong(),
                );
            });

            ui.add_space(5.0);
            ui.separator();

            // Debugger toolbar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Controls:").strong());

                if ui.button("⏸ Pause").clicked() {
                    self.pending_debug_action = Some(DebugAction::Pause);
                }

                if ui.button("▶ Resume").clicked() {
                    self.pending_debug_action = Some(DebugAction::Resume);
                }

                if ui.button("⏭ Step").clicked() {
                    self.pending_debug_action = Some(DebugAction::Step);
                }

                ui.separator();

                // Instruction trace controls
                if !self.trace_active {
                    if ui.button("📝 Start Trace").clicked() {
                        // Generate random filename with timestamp
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let filename = format!("trace_{}.txt", timestamp);
                        self.trace_filename = Some(filename.clone());
                        self.trace_active = true;
                        self.pending_debug_action = Some(DebugAction::StartTrace(filename));
                    }
                } else {
                    if ui.button("⏹ Stop Trace").clicked() {
                        self.trace_active = false;
                        self.pending_debug_action = Some(DebugAction::StopTrace);
                    }
                    if let Some(ref filename) = self.trace_filename {
                        ui.label(egui::RichText::new(format!("Recording to: {}", filename)).weak());
                    }
                }
            });

            ui.add_space(5.0);
            ui.separator();

            if state.system_type == "Game Boy" {
                ui.add_space(5.0);
                egui::CollapsingHeader::new("🔊 Audio Channels")
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut changed = false;
                        changed |= ui
                            .checkbox(&mut self.gb_audio_channels[0], "Pulse 1")
                            .changed();
                        changed |= ui
                            .checkbox(&mut self.gb_audio_channels[1], "Pulse 2")
                            .changed();
                        changed |= ui
                            .checkbox(&mut self.gb_audio_channels[2], "Wave")
                            .changed();
                        changed |= ui
                            .checkbox(&mut self.gb_audio_channels[3], "Noise")
                            .changed();

                        if changed {
                            self.pending_debug_action =
                                Some(DebugAction::SetGbAudioChannels(self.gb_audio_channels));
                        }
                    });
            }

            // Breakpoint Management Panel (collapsible)
            egui::CollapsingHeader::new("🎯 Breakpoints")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Address:");
                        ui.text_edit_singleline(&mut self.breakpoint_address_input);

                        ui.label("Type:");
                        egui::ComboBox::from_id_salt("breakpoint_type")
                            .selected_text(match self.breakpoint_type_selected {
                                0 => "Execute",
                                1 => "Read",
                                2 => "Write",
                                _ => "Execute",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.breakpoint_type_selected,
                                    0,
                                    "Execute",
                                );
                                ui.selectable_value(&mut self.breakpoint_type_selected, 1, "Read");
                                ui.selectable_value(&mut self.breakpoint_type_selected, 2, "Write");
                            });

                        if ui.button("➕ Add").clicked() {
                            // Parse address (support hex with 0x prefix or decimal)
                            let addr_str = self.breakpoint_address_input.trim();
                            let addr = if addr_str.starts_with("0x") || addr_str.starts_with("0X") {
                                u32::from_str_radix(&addr_str[2..], 16)
                            } else {
                                addr_str.parse::<u32>()
                            };

                            if let Ok(address) = addr {
                                let bp_type = match self.breakpoint_type_selected {
                                    0 => emu_core::breakpoints::BreakpointType::Execute,
                                    1 => emu_core::breakpoints::BreakpointType::Read,
                                    2 => emu_core::breakpoints::BreakpointType::Write,
                                    _ => emu_core::breakpoints::BreakpointType::Execute,
                                };
                                self.pending_debug_action =
                                    Some(DebugAction::AddBreakpoint(address, bp_type));
                                self.breakpoint_address_input.clear();
                            }
                        }
                    });

                    ui.add_space(5.0);

                    // Show active breakpoints
                    if let Some(ref debug_state) = self.enhanced_debug_state {
                        if !debug_state.breakpoints.is_empty() {
                            ui.label(egui::RichText::new("Active Breakpoints:").strong());

                            egui::Grid::new("breakpoints_grid")
                                .striped(true)
                                .spacing([10.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Address").strong());
                                    ui.label(egui::RichText::new("Type").strong());
                                    ui.label(""); // For remove button
                                    ui.end_row();

                                    let mut to_remove: Option<(
                                        u32,
                                        emu_core::breakpoints::BreakpointType,
                                    )> = None;
                                    for bp in &debug_state.breakpoints {
                                        ui.label(format!("${:04X}", bp.address));
                                        let type_str = match bp.breakpoint_type {
                                            emu_core::breakpoints::BreakpointType::Execute => {
                                                "Execute"
                                            }
                                            emu_core::breakpoints::BreakpointType::Read => "Read",
                                            emu_core::breakpoints::BreakpointType::Write => "Write",
                                        };
                                        ui.label(type_str);
                                        if ui.button("❌").clicked() {
                                            to_remove = Some((bp.address, bp.breakpoint_type));
                                        }
                                        ui.end_row();
                                    }

                                    if let Some((addr, bp_type)) = to_remove {
                                        self.pending_debug_action =
                                            Some(DebugAction::RemoveBreakpoint(addr, bp_type));
                                    }
                                });
                        } else {
                            ui.label(egui::RichText::new("No breakpoints set").weak().italics());
                        }
                    }
                });

            ui.add_space(5.0);
            ui.separator();
            ui.add_space(5.0);

            // 2-column layout: Disassembly | CPU State
            // Memory explorer is only available via the Inspector Memory tab
            let header_height = 120.0; // Approximate height used by header elements above (including toolbar)
            ui.horizontal_top(|ui| {
                let available_width = ui.available_width();
                let column_width = available_width / 2.0 - 10.0; // 2 columns with spacing
                let content_height = total_available_height - header_height;

                // Left panel: Disassembly
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.set_height(content_height);
                    ui.heading("📜 Disassembly");
                    ui.separator();
                    ScrollArea::vertical()
                        .id_salt(format!("disassembly_scroll_{}", state.system_type))
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_disassembly_panel(ui, state);
                        });
                });

                ui.add_space(10.0);

                // Right panel: CPU State
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.set_height(content_height);
                    ui.heading("🖥️ CPU State");
                    ui.separator();
                    ScrollArea::vertical()
                        .id_salt(format!("cpu_state_scroll_{}", state.system_type))
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_cpu_state_panel(ui, state);
                        });
                });
            });
        });
    }

    fn render_disassembly_panel(&self, ui: &mut Ui, state: &EnhancedDebugState) {
        if state.disassembly.is_empty() {
            ui.label(egui::RichText::new("No disassembly available").weak());
            return;
        }

        // Show disassembled instructions
        egui::Grid::new(format!("disasm_grid_{}", state.system_type))
            .num_columns(3)
            .spacing([5.0, 2.0])
            .striped(false)
            .show(ui, |ui| {
                for instr in &state.disassembly {
                    let is_current = instr.address == state.current_pc;

                    // Highlight current instruction
                    let bg_color = if is_current {
                        egui::Color32::from_rgb(60, 80, 100)
                    } else {
                        ui.visuals().window_fill()
                    };

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Address
                        let addr_text = if is_current {
                            egui::RichText::new(format!("▶ {:04X}", instr.address))
                                .monospace()
                                .strong()
                        } else {
                            egui::RichText::new(format!("  {:04X}", instr.address)).monospace()
                        };
                        ui.label(addr_text);
                    });

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Bytes
                        let bytes_str: String = instr
                            .bytes
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        ui.label(egui::RichText::new(bytes_str).monospace());
                    });

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Mnemonic with optional comment
                        let mnem_with_comment = if let Some(ref comment) = instr.comment {
                            format!("{}  ; {}", instr.mnemonic, comment)
                        } else {
                            instr.mnemonic.clone()
                        };

                        let mnem_text = if is_current {
                            egui::RichText::new(&mnem_with_comment).monospace().strong()
                        } else {
                            egui::RichText::new(&mnem_with_comment).monospace()
                        };
                        ui.label(mnem_text);
                    });

                    ui.end_row();
                }
            });
    }

    fn render_hex_dump(&self, ui: &mut Ui, region: &MemoryRegion, system_type: &str) {
        // Check if we have cached memory data
        if self.cached_memory.is_empty() {
            ui.label(
                egui::RichText::new("No memory data available")
                    .weak()
                    .italics(),
            );
            ui.label(
                egui::RichText::new("Memory will be loaded when available")
                    .weak()
                    .italics(),
            );
            return;
        }

        // Calculate how many rows to display (16 bytes per row)
        let bytes_per_row = 16;
        let rows_to_display = 32; // Display ~512 bytes at a time

        // Calculate starting address aligned to 16-byte boundary
        let aligned_address = (self.memory_view_address / bytes_per_row) * bytes_per_row;

        // Display header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Offset").monospace().strong());
            ui.add_space(10.0);
            for i in 0..16 {
                ui.label(egui::RichText::new(format!("{:X}", i)).monospace().weak());
            }
            ui.add_space(10.0);
            ui.label(egui::RichText::new("ASCII").monospace().strong());
        });

        ui.separator();

        // Display hex dump rows
        egui::Grid::new(format!("hex_dump_grid_{}", system_type))
            .num_columns(18) // Address + 16 bytes + ASCII
            .spacing([8.0, 2.0])
            .striped(false)
            .show(ui, |ui| {
                for row in 0..rows_to_display {
                    let row_addr = aligned_address + (row * bytes_per_row);

                    // Check if this row is within the region
                    if row_addr > region.end {
                        break;
                    }

                    // Display address
                    ui.label(
                        egui::RichText::new(format!("{:04X}:", row_addr))
                            .monospace()
                            .color(egui::Color32::from_rgb(150, 150, 200)),
                    );

                    // Display hex bytes
                    let mut ascii_text = String::new();
                    for byte_offset in 0..bytes_per_row {
                        let addr = row_addr + byte_offset;

                        if addr < region.start || addr > region.end {
                            // Out of region bounds
                            ui.label(egui::RichText::new("  ").monospace());
                            ascii_text.push(' ');
                        } else {
                            // Calculate offset in cached memory
                            let cache_offset = addr.saturating_sub(self.cached_memory_start);

                            if (cache_offset as usize) < self.cached_memory.len() {
                                let byte = self.cached_memory[cache_offset as usize];

                                // Display hex byte
                                ui.label(egui::RichText::new(format!("{:02X}", byte)).monospace());

                                // Build ASCII representation
                                if (0x20..=0x7E).contains(&byte) {
                                    ascii_text.push(byte as char);
                                } else {
                                    ascii_text.push('.');
                                }
                            } else {
                                // Not in cache
                                ui.label(egui::RichText::new("??").monospace().weak());
                                ascii_text.push('?');
                            }
                        }
                    }

                    // Display ASCII column
                    ui.label(
                        egui::RichText::new(ascii_text)
                            .monospace()
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );

                    ui.end_row();
                }
            });
    }

    fn render_cpu_state_panel(&self, ui: &mut Ui, state: &EnhancedDebugState) {
        if let Some(ref cpu_state) = state.cpu_state {
            // Program Counter
            ui.heading("Program Counter");
            ui.label(
                egui::RichText::new(format!("${:04X}", cpu_state.pc))
                    .monospace()
                    .strong(),
            );
            ui.add_space(10.0);

            // Registers
            ui.heading("Registers");
            egui::Grid::new(format!("registers_grid_{}", state.system_type))
                .num_columns(2)
                .spacing([15.0, 5.0])
                .striped(true)
                .show(ui, |ui| {
                    for reg in &cpu_state.registers {
                        ui.label(egui::RichText::new(&reg.name).strong());
                        let value_str = match reg.width {
                            8 => format!("${:02X}", reg.value),
                            16 => format!("${:04X}", reg.value),
                            32 => format!("${:08X}", reg.value),
                            _ => format!("${:X}", reg.value),
                        };
                        ui.label(egui::RichText::new(value_str).monospace());
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);

            // Flags
            ui.heading("Flags");
            egui::Grid::new(format!("flags_grid_{}", state.system_type))
                .num_columns(2)
                .spacing([15.0, 5.0])
                .striped(true)
                .show(ui, |ui| {
                    for (flag_name, flag_value) in &cpu_state.flags.flags {
                        ui.label(egui::RichText::new(flag_name).strong());
                        let flag_str = if *flag_value { "1" } else { "0" };
                        let flag_color = if *flag_value {
                            egui::Color32::from_rgb(100, 255, 100)
                        } else {
                            egui::Color32::from_rgb(150, 150, 150)
                        };
                        ui.label(egui::RichText::new(flag_str).monospace().color(flag_color));
                        ui.end_row();
                    }
                });
        } else {
            ui.label(egui::RichText::new("No CPU state available").weak());
        }
    }

    fn render_legacy_debug_view(&self, ui: &mut Ui, debug_info: &SystemDebugInfo) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading(
                        egui::RichText::new(format!(
                            "🔧 {} Debug Information",
                            debug_info.system_type
                        ))
                        .size(24.0)
                        .strong(),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("System internals and diagnostic information").weak(),
                    );
                });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                egui::Grid::new(format!("debug_grid_{}", debug_info.system_type))
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (label, value) in &debug_info.fields {
                            ui.label(egui::RichText::new(label).strong());
                            ui.label(egui::RichText::new(value).monospace());
                            ui.end_row();
                        }
                    });
            });
    }

    pub fn render_tiles_tab(&self, ui: &mut Ui) {
        let available_height = ui.available_height();
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(available_height)
            .show(ui, |ui| {
                if let Some(ref sys_data) = self.system_tile_data {
                    // Render system-specific tile viewers
                    match sys_data {
                        SystemTileData::NES(nes_data) => {
                            // Header with PPU state info
                            ui.heading("🎨 NES Tile Viewer");
                            ui.separator();

                            // PPU state summary
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "PPUCTRL: ${:02X}",
                                        nes_data.ppuctrl
                                    ))
                                    .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "PPUMASK: ${:02X}",
                                        nes_data.ppumask
                                    ))
                                    .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Scroll: ({}, {})",
                                        nes_data.scroll_x, nes_data.scroll_y
                                    ))
                                    .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!("Mirror: {}", nes_data.mirroring))
                                        .monospace(),
                                );
                            });

                            ui.add_space(5.0);

                            // CHR type indicator
                            let chr_type = if nes_data.chr_is_ram {
                                "CHR-RAM"
                            } else {
                                "CHR-ROM"
                            };
                            let chr_size = nes_data.chr_data.len();
                            ui.label(format!(
                                "{} ({} bytes / {} KB)",
                                chr_type,
                                chr_size,
                                chr_size / 1024
                            ));

                            ui.add_space(10.0);

                            // Pattern Tables section
                            ui.heading("Pattern Tables");
                            ui.separator();

                            // Render both pattern tables side by side
                            ui.horizontal(|ui| {
                                // Pattern Table 0 ($0000-$0FFF)
                                ui.vertical(|ui| {
                                    let bg_table = (nes_data.ppuctrl & 0x10) != 0;
                                    let label = if !bg_table { "◄ BG" } else { "" };
                                    ui.label(format!(
                                        "Pattern Table 0 (CHR $0000-$0FFF) {}",
                                        label
                                    ));
                                    self.render_pattern_table(ui, nes_data, 0);
                                });

                                ui.add_space(20.0);

                                // Pattern Table 1 ($1000-$1FFF)
                                ui.vertical(|ui| {
                                    let bg_table = (nes_data.ppuctrl & 0x10) != 0;
                                    let label = if bg_table { "◄ BG" } else { "" };
                                    ui.label(format!(
                                        "Pattern Table 1 (CHR $1000-$1FFF) {}",
                                        label
                                    ));
                                    self.render_pattern_table(ui, nes_data, 1);
                                });
                            });

                            ui.add_space(15.0);

                            // Palette section
                            ui.heading("Palettes");
                            ui.separator();

                            // Background palettes
                            ui.label("Background Palettes ($3F00-$3F0F):");
                            self.render_palettes(ui, nes_data, 0);

                            ui.add_space(5.0);

                            // Sprite palettes
                            ui.label("Sprite Palettes ($3F10-$3F1F):");
                            self.render_palettes(ui, nes_data, 4);

                            ui.add_space(15.0);

                            // Sprites (OAM) section
                            ui.heading("Sprites (OAM)");
                            ui.separator();

                            // Sprite info
                            let sprite_size = if (nes_data.ppuctrl & 0x20) != 0 {
                                "8x16"
                            } else {
                                "8x8"
                            };
                            let sprite_table = if (nes_data.ppuctrl & 0x08) != 0 { 1 } else { 0 };

                            // Count visible sprites
                            let visible_count = if nes_data.oam.len() >= 256 {
                                (0..64)
                                    .filter(|&i| {
                                        let y = nes_data.oam[i * 4];
                                        if sprite_size == "8x16" {
                                            y < 0xE7
                                        } else {
                                            y < 0xEF
                                        }
                                    })
                                    .count()
                            } else {
                                0
                            };

                            ui.horizontal(|ui| {
                                ui.label(format!("Sprite Size: {}", sprite_size));
                                ui.separator();
                                if sprite_size == "8x8" {
                                    ui.label(format!(
                                        "Pattern Table: {} (CHR ${:04X})",
                                        sprite_table,
                                        sprite_table * 0x1000
                                    ));
                                } else {
                                    ui.label("Pattern Table: Per-sprite (tile bit 0)");
                                }
                                ui.separator();
                                ui.label(format!("Visible: {}/64", visible_count));
                                ui.separator();
                                ui.label(format!("OAM: {} bytes", nes_data.oam.len()));
                            });

                            ui.add_space(5.0);

                            // Render sprite grid
                            self.render_sprites(ui, nes_data);
                        }
                        SystemTileData::GameBoy(gb_data) => {
                            ui.heading(format!(
                                "🎮 Game Boy Tile Viewer ({})",
                                if gb_data.is_cgb_mode { "CGB" } else { "DMG" }
                            ));
                            ui.separator();
                            ui.label(format!("LCDC: ${:02X}", gb_data.lcdc));
                            ui.label(format!(
                                "Scroll: ({}, {}) Window: ({}, {})",
                                gb_data.scx, gb_data.scy, gb_data.wx, gb_data.wy
                            ));
                            ui.label(format!("VRAM: {} KB", gb_data.vram_bank0.len() / 1024));
                            ui.label(format!("OAM: {} bytes (40 sprites)", gb_data.oam.len()));
                            ui.label(format!("BG Palettes: {} colors", gb_data.bg_palettes.len()));
                            ui.label(format!(
                                "OBJ Palettes: {} colors",
                                gb_data.obj_palettes.len()
                            ));
                        }
                        SystemTileData::GBA(gba_data) => {
                            ui.heading("🎮 GBA Tile Viewer");
                            ui.separator();

                            // Display control
                            let bg_mode = gba_data.dispcnt & 0x7;
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("BG Mode: {}", bg_mode))
                                        .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "DISPCNT: ${:04X}",
                                        gba_data.dispcnt
                                    ))
                                    .monospace(),
                                );
                            });

                            ui.add_space(5.0);

                            // Memory sizes
                            ui.label(format!("VRAM: {} KB", gba_data.vram.len() / 1024));
                            ui.label(format!("Palette RAM: {} bytes ({} colors)", gba_data.palette_ram.len(), gba_data.master_palette.len()));
                            ui.label(format!("OAM: {} bytes (128 sprites)", gba_data.oam.len()));

                            ui.add_space(5.0);

                            // Enabled layers
                            ui.label("Enabled Layers:");
                            ui.horizontal(|ui| {
                                for i in 0..4 {
                                    if (gba_data.dispcnt & (1 << (8 + i))) != 0 {
                                        ui.label(format!("BG{}", i));
                                    }
                                }
                                if gba_data.dispcnt & (1 << 12) != 0 {
                                    ui.label("OBJ");
                                }
                            });
                        }
                        SystemTileData::SMS(sms_data) => {
                            ui.heading("🎮 SMS Tile Viewer");
                            ui.separator();
                            ui.label(format!("VRAM: {} KB", sms_data.vram.len() / 1024));
                            ui.label(format!("CRAM: {} bytes", sms_data.cram.len()));
                            ui.label(format!("Palette: {} colors", sms_data.palette.len()));
                            ui.label(format!("Registers: {:?}", sms_data.registers));
                        }
                        SystemTileData::SNES(snes_data) => {
                            // Header with PPU state info
                            ui.heading("🎮 SNES Tile Viewer");
                            ui.separator();

                            // PPU state summary
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("BG Mode: {}", snes_data.bg_mode))
                                        .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Screen: {}",
                                        if snes_data.screen_enabled {
                                            "Enabled"
                                        } else {
                                            "Disabled"
                                        }
                                    ))
                                    .monospace(),
                                );
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "VRAM: {} KB",
                                        snes_data.vram.len() / 1024
                                    ))
                                    .monospace(),
                                );
                            });

                            ui.add_space(10.0);

                            // Tile sections
                            ui.heading("Character Data (VRAM)");
                            ui.separator();

                            // Add BPP selector
                            let mut bpp_mode = self.snes_bpp_mode;
                            ui.horizontal(|ui| {
                                ui.label("Color Depth:");
                                if ui.selectable_label(bpp_mode == 2, "2bpp (4 colors)").clicked() {
                                    bpp_mode = 2;
                                }
                                if ui.selectable_label(bpp_mode == 4, "4bpp (16 colors)").clicked() {
                                    bpp_mode = 4;
                                }
                                if ui.selectable_label(bpp_mode == 8, "8bpp (256 colors)").clicked() {
                                    bpp_mode = 8;
                                }
                            });

                            ui.add_space(5.0);

                            // Add palette selector
                            let mut selected_palette = self.snes_selected_palette;
                            ui.horizontal(|ui| {
                                ui.label("Palette:");
                                for pal in 0..8 {
                                    if ui.selectable_label(selected_palette == pal, format!("{}", pal)).clicked() {
                                        selected_palette = pal;
                                    }
                                }
                            });

                            ui.add_space(10.0);

                            // Render tiles with current settings
                            self.render_snes_tiles_with_settings(ui, snes_data, bpp_mode, selected_palette);

                            ui.add_space(15.0);

                            // Palette section
                            ui.heading("Palettes (CGRAM)");
                            ui.separator();
                            self.render_snes_palettes(ui, snes_data);

                            ui.add_space(15.0);

                            // Sprite section
                            ui.heading("Sprites (OAM)");
                            ui.separator();
                            ui.label(format!("OAM: {} bytes (128 sprites)", snes_data.oam.len()));
                            ui.label("Sprite viewer coming soon...");
                        }
                        SystemTileData::Atari2600(a2600_data) => {
                            ui.heading("🕹️ Atari 2600 Inspector");
                            ui.separator();

                            // Show basic status
                            ui.horizontal(|ui| {
                                ui.label(format!("VSYNC: {}", if a2600_data.vsync { "ON" } else { "OFF" }));
                                ui.separator();
                                ui.label(format!("VBLANK: {}", if a2600_data.vblank { "ON" } else { "OFF" }));
                            });

                            ui.add_space(5.0);
                            ui.label("See Playfield, Sprites, Palette, and Collision tabs for detailed views");
                        }
                        SystemTileData::Chip8(_) => {
                            ui.heading("📺 CHIP-8 Inspector");
                            ui.separator();
                            ui.label("CHIP-8 doesn't use tiles. See Display and Registers tabs for debugging info.");
                        }
                        SystemTileData::ColecoVision(coleco_data) => {
                            ui.heading("🎨 ColecoVision Tile Viewer");
                            ui.separator();
                            ui.label(format!("VRAM: {} bytes", coleco_data.vram.len()));
                            ui.label(format!("Palette: {} colors", coleco_data.palette.len()));
                            ui.label(format!("VDP Registers: {}", coleco_data.registers.len()));
                            ui.add_space(5.0);
                            ui.label("See VDP tab for detailed register information");
                        }
                        SystemTileData::SG1000(sg1000_data) => {
                            ui.heading("🎨 SG-1000 Tile Viewer");
                            ui.separator();
                            ui.label(format!("VRAM: {} bytes", sg1000_data.vram.len()));
                            ui.label(format!("Palette: {} colors", sg1000_data.palette.len()));
                            ui.label(format!("VDP Registers: {}", sg1000_data.registers.len()));
                            ui.add_space(5.0);
                            ui.label("See VDP tab for detailed register information");
                        }
                    }
                } else {
                    // No tile data available
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🎨").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("No Tile Data Available");
                        ui.add_space(10.0);
                        ui.label("Load a ROM to see tile and palette data");
                    });
                }
            });
    }

    pub fn render_tilemaps_tab(&self, ui: &mut Ui) {
        let available_height = ui.available_height();
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(available_height)
            .show(ui, |ui| {
                if let Some(ref sys_data) = self.system_tile_data {
                    match sys_data {
                        SystemTileData::NES(nes_data) => {
                            // Header
                            ui.heading("🗺️ NES Nametable Viewer");
                            ui.separator();

                            // PPU state summary
                            let bg_table = if (nes_data.ppuctrl & 0x10) != 0 { 1 } else { 0 };
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "BG Pattern Table: {} (CHR ${:04X})",
                                    bg_table,
                                    bg_table * 0x1000
                                ));
                                ui.separator();
                                ui.label(format!("Mirroring: {}", nes_data.mirroring));
                                ui.separator();
                                ui.label(format!("VRAM: {} bytes", nes_data.vram.len()));
                            });

                            ui.add_space(5.0);

                            // Render nametable preview
                            self.render_nametables(ui, nes_data);
                        }
                        SystemTileData::GameBoy(gb_data) => {
                            // Header
                            ui.heading("🗺️ Game Boy Tilemap Viewer");
                            ui.separator();

                            // PPU state summary
                            ui.horizontal(|ui| {
                                ui.label(format!("LCDC: 0x{:02X}", gb_data.lcdc));
                                ui.separator();
                                ui.label(format!("SCX: {}, SCY: {}", gb_data.scx, gb_data.scy));
                                ui.separator();
                                ui.label(format!("WX: {}, WY: {}", gb_data.wx, gb_data.wy));
                                ui.separator();
                                ui.label(if gb_data.is_cgb_mode {
                                    "CGB Mode"
                                } else {
                                    "DMG Mode"
                                });
                            });

                            ui.add_space(5.0);

                            // Render tilemap preview
                            self.render_gb_tilemaps(ui, gb_data);
                        }
                        _ => {
                            // Not supported for this system
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                ui.label(egui::RichText::new("🗺️").size(48.0));
                                ui.add_space(10.0);
                                ui.heading("Tilemaps");
                                ui.add_space(10.0);
                                ui.label("Only available for NES and Game Boy");
                            });
                        }
                    }
                } else {
                    // No tile data available
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🗺️").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Tilemaps");
                        ui.add_space(10.0);
                        ui.label("Tilemap viewer for NES and Game Boy");
                        ui.label("Load a ROM to see tilemap data");
                    });
                }
            });
    }

    fn render_pattern_table(&self, ui: &mut Ui, data: &NesTileData, table_num: usize) {
        // Pattern table is 16x16 tiles = 256 tiles
        // Each tile is 8x8 pixels
        // Render as a grid with hover tooltips

        let base_addr = table_num * 0x1000;
        let tile_size = 10.0; // pixels per tile in the viewer (scaled up for visibility)

        // Create the grid frame
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(16.0 * tile_size, 16.0 * tile_size),
            egui::Sense::hover(),
        );

        let rect = response.rect;

        // Draw grid background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

        // Draw each tile
        for tile_row in 0..16 {
            for tile_col in 0..16 {
                let tile_index = tile_row * 16 + tile_col;
                let chr_addr = base_addr + tile_index * 16; // Each tile is 16 bytes

                // Get tile position in the viewer
                let tile_x = rect.min.x + tile_col as f32 * tile_size;
                let tile_y = rect.min.y + tile_row as f32 * tile_size;
                let tile_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(tile_x, tile_y),
                    egui::Vec2::new(tile_size, tile_size),
                );

                // Calculate average brightness for the tile to show a preview
                let mut total_value = 0u32;
                for byte_idx in 0..16 {
                    if chr_addr + byte_idx < data.chr_data.len() {
                        let b = data.chr_data[chr_addr + byte_idx];
                        total_value += b.count_ones();
                    }
                }
                let brightness = ((total_value as f32 / 128.0) * 180.0) as u8;
                let tile_color = egui::Color32::from_rgb(brightness, brightness, brightness);

                painter.rect_filled(tile_rect, 0.0, tile_color);

                // Draw grid lines
                painter.rect_stroke(
                    tile_rect,
                    0.0,
                    egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 60)),
                    egui::StrokeKind::Inside,
                );
            }
        }

        // Handle hover tooltip
        if let Some(hover_pos) = response.hover_pos() {
            let rel_x = hover_pos.x - rect.min.x;
            let rel_y = hover_pos.y - rect.min.y;

            if rel_x >= 0.0 && rel_y >= 0.0 {
                let tile_col = (rel_x / tile_size) as usize;
                let tile_row = (rel_y / tile_size) as usize;

                if tile_col < 16 && tile_row < 16 {
                    let tile_index = tile_row * 16 + tile_col;
                    let chr_addr = base_addr + tile_index * 16;

                    // Show tooltip with memory info
                    let mut low_bytes = String::new();
                    let mut high_bytes = String::new();
                    for i in 0..8 {
                        if chr_addr + i < data.chr_data.len() {
                            low_bytes.push_str(&format!("{:02X} ", data.chr_data[chr_addr + i]));
                        }
                        if chr_addr + 8 + i < data.chr_data.len() {
                            high_bytes
                                .push_str(&format!("{:02X} ", data.chr_data[chr_addr + 8 + i]));
                        }
                    }

                    response.clone().on_hover_ui(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Tile ${:02X} ({})",
                                tile_index, tile_index
                            ))
                            .strong(),
                        );
                        ui.label(format!(
                            "CHR Address: ${:04X}-${:04X}",
                            chr_addr,
                            chr_addr + 15
                        ));
                        ui.label(format!("Pattern Table: {}", table_num));
                        ui.label(format!("Row: {}, Col: {}", tile_row, tile_col));
                        ui.separator();
                        ui.label("Tile Data (Low plane / High plane):");
                        ui.label(
                            egui::RichText::new(format!("Low:  {}", low_bytes.trim()))
                                .monospace()
                                .size(11.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("High: {}", high_bytes.trim()))
                                .monospace()
                                .size(11.0),
                        );
                    });
                }
            }
        }
    }

    fn render_palettes(&self, ui: &mut Ui, data: &NesTileData, start_palette: usize) {
        let color_size = 24.0;
        let palette_spacing = 8.0;

        ui.horizontal(|ui| {
            for pal_num in 0..4 {
                let palette_index = start_palette + pal_num;
                let pal_addr_base = if start_palette == 0 { 0x3F00 } else { 0x3F10 };
                let pal_addr = pal_addr_base + pal_num * 4;

                ui.vertical(|ui| {
                    ui.label(format!("Palette {}", palette_index));

                    ui.horizontal(|ui| {
                        for color_num in 0..4 {
                            let pal_offset = (palette_index * 4 + color_num) % data.palette.len();
                            let color_index = data.palette[pal_offset] as usize;
                            let rgb = if color_index < data.master_palette.len() {
                                data.master_palette[color_index]
                            } else {
                                0xFF000000 // Black fallback
                            };

                            let r = ((rgb >> 16) & 0xFF) as u8;
                            let g = ((rgb >> 8) & 0xFF) as u8;
                            let b = (rgb & 0xFF) as u8;
                            let color = egui::Color32::from_rgb(r, g, b);

                            let (response, painter) = ui.allocate_painter(
                                egui::Vec2::new(color_size, color_size),
                                egui::Sense::hover(),
                            );
                            let rect = response.rect;

                            painter.rect_filled(rect, 2.0, color);
                            painter.rect_stroke(
                                rect,
                                2.0,
                                egui::Stroke::new(1.0, egui::Color32::WHITE),
                                egui::StrokeKind::Inside,
                            );

                            response.on_hover_ui(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Palette {} Color {}",
                                        palette_index, color_num
                                    ))
                                    .strong(),
                                );
                                ui.label(format!("Address: ${:04X}", pal_addr + color_num));
                                ui.label(format!(
                                    "NES Color Index: ${:02X} ({})",
                                    color_index, color_index
                                ));
                                ui.label(format!("RGB: #{:02X}{:02X}{:02X}", r, g, b));
                            });
                        }
                    });
                });

                if pal_num < 3 {
                    ui.add_space(palette_spacing);
                }
            }
        });
    }

    fn render_sprites(&self, ui: &mut Ui, data: &NesTileData) {
        if data.oam.len() < 256 {
            ui.label(egui::RichText::new("OAM data not available").weak());
            return;
        }

        let sprite_size_8x16 = (data.ppuctrl & 0x20) != 0;
        let sprite_pattern_table = if (data.ppuctrl & 0x08) != 0 { 1 } else { 0 };
        let tile_height = if sprite_size_8x16 { 16 } else { 8 };

        // Render sprites in a grid (8 columns x 8 rows = 64 sprites)
        // Each cell shows the actual 8x8 or 8x16 sprite tile scaled up
        let scale = 2.0; // Scale factor for visibility
        let cell_width = 8.0 * scale + 6.0; // 8 pixels wide + padding
        let cell_height = tile_height as f32 * scale + 14.0; // 8 or 16 pixels tall + padding for label
        let grid_cols = 16; // 16 columns
        let grid_rows = 4; // 4 rows = 64 sprites

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(
                grid_cols as f32 * cell_width,
                grid_rows as f32 * cell_height,
            ),
            egui::Sense::hover(),
        );

        let rect = response.rect;

        // Draw grid background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 40));

        // Draw each sprite
        for sprite_idx in 0..64 {
            let oam_offset = sprite_idx * 4;
            let y_pos = data.oam[oam_offset];
            let tile_idx = data.oam[oam_offset + 1];
            let attributes = data.oam[oam_offset + 2];
            let _x_pos = data.oam[oam_offset + 3];

            let col = sprite_idx % grid_cols;
            let row = sprite_idx / grid_cols;

            let cell_x = rect.min.x + col as f32 * cell_width + 3.0;
            let cell_y = rect.min.y + row as f32 * cell_height + 2.0;

            // Check if sprite is visible
            let is_visible = if sprite_size_8x16 {
                y_pos < 0xE7
            } else {
                y_pos < 0xEF
            };

            // Draw sprite cell background
            let sprite_bg_rect = egui::Rect::from_min_size(
                egui::Pos2::new(cell_x - 1.0, cell_y - 1.0),
                egui::Vec2::new(8.0 * scale + 2.0, tile_height as f32 * scale + 2.0),
            );
            let bg_color = if is_visible {
                egui::Color32::from_rgb(60, 60, 80)
            } else {
                egui::Color32::from_rgb(40, 40, 50)
            };
            painter.rect_filled(sprite_bg_rect, 2.0, bg_color);

            // Draw sprite index below the sprite
            let label = format!("{:02X}", sprite_idx);
            let label_pos = egui::Pos2::new(
                cell_x + 8.0 * scale / 2.0,
                cell_y + tile_height as f32 * scale + 4.0,
            );
            painter.text(
                label_pos,
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::monospace(8.0),
                if is_visible {
                    egui::Color32::from_rgb(180, 180, 200)
                } else {
                    egui::Color32::from_rgb(100, 100, 120)
                },
            );

            // Get sprite palette (attribute bits 0-1 + 4 for sprite palettes)
            let palette_idx = ((attributes & 0x03) + 4) as usize;
            let flip_h = (attributes & 0x40) != 0;
            let flip_v = (attributes & 0x80) != 0;

            // Calculate CHR address
            let chr_addr = if sprite_size_8x16 {
                let table = (tile_idx & 0x01) as usize;
                let tile = (tile_idx & 0xFE) as usize;
                table * 0x1000 + tile * 16
            } else {
                sprite_pattern_table * 0x1000 + (tile_idx as usize) * 16
            };

            // Draw the sprite tile pixels
            let tiles_to_draw = if sprite_size_8x16 { 2 } else { 1 };

            for tile_part in 0..tiles_to_draw {
                let tile_chr_addr = chr_addr + tile_part * 16;

                for py in 0..8 {
                    if tile_chr_addr + py + 8 >= data.chr_data.len() {
                        continue;
                    }

                    let low_byte = data.chr_data[tile_chr_addr + py];
                    let high_byte = data.chr_data[tile_chr_addr + py + 8];

                    for px in 0..8 {
                        let bit = 7 - px;
                        let low_bit = (low_byte >> bit) & 1;
                        let high_bit = (high_byte >> bit) & 1;
                        let color_idx = (high_bit << 1) | low_bit;

                        // Color 0 is transparent for sprites
                        if color_idx == 0 {
                            continue;
                        }

                        // Get palette color
                        let pal_offset = palette_idx * 4 + color_idx as usize;
                        let nes_color = if pal_offset < data.palette.len() {
                            data.palette[pal_offset] as usize
                        } else {
                            0
                        };
                        let rgb = if nes_color < data.master_palette.len() {
                            data.master_palette[nes_color]
                        } else {
                            0xFF000000
                        };

                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;

                        // Apply visibility dimming
                        let color = if is_visible {
                            egui::Color32::from_rgb(r, g, b)
                        } else {
                            egui::Color32::from_rgb(r / 2, g / 2, b / 2)
                        };

                        // Calculate pixel position with flipping
                        let draw_px = if flip_h { 7 - px } else { px };
                        let draw_py = if flip_v {
                            (tile_height - 1) - (tile_part * 8 + py)
                        } else {
                            tile_part * 8 + py
                        };

                        let pixel_rect = egui::Rect::from_min_size(
                            egui::Pos2::new(
                                cell_x + draw_px as f32 * scale,
                                cell_y + draw_py as f32 * scale,
                            ),
                            egui::Vec2::new(scale, scale),
                        );
                        painter.rect_filled(pixel_rect, 0.0, color);
                    }
                }
            }

            // Draw cell border
            let cell_rect = egui::Rect::from_min_size(
                egui::Pos2::new(cell_x - 1.0, cell_y - 1.0),
                egui::Vec2::new(8.0 * scale + 2.0, tile_height as f32 * scale + 2.0),
            );
            let border_color = if is_visible {
                egui::Color32::from_rgb(80, 80, 100)
            } else {
                egui::Color32::from_rgb(50, 50, 60)
            };
            painter.rect_stroke(
                cell_rect,
                0.0,
                egui::Stroke::new(1.0, border_color),
                egui::StrokeKind::Outside,
            );
        }

        // Handle hover tooltip
        if let Some(hover_pos) = response.hover_pos() {
            let rel_x = hover_pos.x - rect.min.x;
            let rel_y = hover_pos.y - rect.min.y;

            if rel_x >= 0.0 && rel_y >= 0.0 {
                let col = (rel_x / cell_width) as usize;
                let row = (rel_y / cell_height) as usize;

                if col < grid_cols && row < grid_rows {
                    let sprite_idx = row * grid_cols + col;
                    if sprite_idx < 64 {
                        let oam_offset = sprite_idx * 4;

                        let y_pos = data.oam[oam_offset];
                        let tile_idx = data.oam[oam_offset + 1];
                        let attributes = data.oam[oam_offset + 2];
                        let x_pos = data.oam[oam_offset + 3];

                        // Decode attributes
                        let palette_num = attributes & 0x03;
                        let priority = if (attributes & 0x20) != 0 {
                            "Behind BG"
                        } else {
                            "In front"
                        };
                        let flip_h = (attributes & 0x40) != 0;
                        let flip_v = (attributes & 0x80) != 0;

                        // Calculate CHR address
                        let chr_addr = if sprite_size_8x16 {
                            let table = (tile_idx & 0x01) as usize;
                            let tile = (tile_idx & 0xFE) as usize;
                            table * 0x1000 + tile * 16
                        } else {
                            sprite_pattern_table * 0x1000 + (tile_idx as usize) * 16
                        };

                        response.clone().on_hover_ui(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Sprite {} (OAM ${:02X})",
                                    sprite_idx, oam_offset
                                ))
                                .strong(),
                            );
                            ui.separator();
                            ui.label(format!("Position: ({}, {})", x_pos, y_pos));
                            ui.label(format!("Tile: ${:02X} ({})", tile_idx, tile_idx));
                            ui.label(format!("CHR Address: ${:04X}", chr_addr));
                            ui.separator();
                            ui.label(format!(
                                "Palette: {} ($3F{:02X})",
                                palette_num,
                                0x10 + palette_num * 4
                            ));
                            ui.label(format!("Priority: {}", priority));
                            ui.label(format!(
                                "Flip: H={} V={}",
                                if flip_h { "Yes" } else { "No" },
                                if flip_v { "Yes" } else { "No" }
                            ));
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!(
                                    "OAM: Y=${:02X} Tile=${:02X} Attr=${:02X} X=${:02X}",
                                    y_pos, tile_idx, attributes, x_pos
                                ))
                                .monospace()
                                .size(11.0),
                            );
                        });
                    }
                }
            }
        }
    }

    /// Map a nametable address ($2000-$2FFF) to physical VRAM index
    /// This replicates the PPU's mirroring logic for the tile viewer
    fn map_nametable_addr_for_viewer(&self, addr: u16, vram_size: usize, mirroring: &str) -> usize {
        let a = addr & 0x0FFF; // 0x0000..0x0FFF
        let table = a / 0x0400; // 0..3
        let offset = a % 0x0400;

        let physical_table = match mirroring {
            "FourScreen" => {
                // With 4KB VRAM, each nametable is independent (no mirroring)
                table
            }
            "Vertical" => match table {
                0 | 2 => 0,
                1 | 3 => 1,
                _ => 0,
            },
            "Horizontal" => match table {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 0,
            },
            "SingleScreenLower" => 0,
            "SingleScreenUpper" => 1,
            _ => 0, // Unknown mirroring, default to lower screen
        };

        let addr = (physical_table * 0x0400 + offset) as usize;
        // Mask to VRAM size (0x7FF for 2KB, 0xFFF for 4KB)
        addr & (vram_size - 1)
    }

    fn render_nametables(&self, ui: &mut Ui, data: &NesTileData) {
        // Nametable layout: render all four nametables in a 2x2 grid
        // Each nametable is 32x30 tiles = 256x240 pixels
        let scale = 1.2; // Scale down to fit four nametables
        let tile_size = 8.0 * scale;
        let nt_width = 32.0 * tile_size;
        let nt_height = 30.0 * tile_size;
        let spacing = 10.0;

        // Calculate total size for all four nametables in 2x2 grid
        let total_width = nt_width * 2.0 + spacing;
        let total_height = nt_height * 2.0 + spacing + 20.0; // Extra for labels

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(total_width, total_height),
            egui::Sense::hover(),
        );
        let rect = response.rect;

        // Check if we have valid data
        if data.chr_data.is_empty() || data.vram.len() < 2048 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Nametable data not available",
                egui::FontId::default(),
                egui::Color32::GRAY,
            );
            return;
        }

        // Background pattern table (selected by PPUCTRL bit 4)
        let bg_pattern_table = if (data.ppuctrl & 0x10) != 0 {
            0x1000
        } else {
            0x0000
        };

        // Calculate scroll window boundaries for graying out tiles outside viewport
        // This needs to be done before rendering tiles
        let scroll_x = data.scroll_x as f32;
        let scroll_y = data.scroll_y as f32;
        let base_nt = (data.ppuctrl & 0x03) as usize;
        let viewport_width = 256.0;
        let viewport_height = 240.0;

        // Calculate which nametable the scroll starts in
        // NES PPU nametable selection: base_nt XOR scroll overflow bits
        // This formula matches the rendering logic in ppu.rs
        let nt_x = ((scroll_x / 256.0) as usize) & 1;
        let nt_y = ((scroll_y / 240.0) as usize) & 1;
        let scroll_nt = base_nt ^ nt_x ^ (nt_y << 1);

        // Calculate scroll window position in logical pixel space (512x480 for 2x2 grid)
        // Map scroll position to the 2x2 nametable grid
        let scroll_grid_x = scroll_nt % 2;
        let scroll_grid_y = scroll_nt / 2;
        let scroll_logical_x = (scroll_grid_x as f32 * 256.0) + (scroll_x % 256.0);
        let scroll_logical_y = (scroll_grid_y as f32 * 240.0) + (scroll_y % 240.0);

        // Draw labels for all four nametables
        let label_y = rect.min.y + 8.0;
        painter.text(
            egui::Pos2::new(rect.min.x + nt_width / 2.0, label_y),
            egui::Align2::CENTER_CENTER,
            "Nametable 0 ($2000)",
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
        painter.text(
            egui::Pos2::new(rect.min.x + nt_width + spacing + nt_width / 2.0, label_y),
            egui::Align2::CENTER_CENTER,
            "Nametable 1 ($2400)",
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
        painter.text(
            egui::Pos2::new(rect.min.x + nt_width / 2.0, label_y + nt_height + spacing),
            egui::Align2::CENTER_CENTER,
            "Nametable 2 ($2800)",
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
        painter.text(
            egui::Pos2::new(
                rect.min.x + nt_width + spacing + nt_width / 2.0,
                label_y + nt_height + spacing,
            ),
            egui::Align2::CENTER_CENTER,
            "Nametable 3 ($2C00)",
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );

        let nt_start_y = rect.min.y + 18.0;

        // Render all four nametables in 2x2 grid
        for nt_idx in 0..4 {
            // Calculate position in 2x2 grid
            let grid_x = nt_idx % 2;
            let grid_y = nt_idx / 2;
            let nt_start_x = rect.min.x + (nt_width + spacing) * grid_x as f32;
            let nt_y = nt_start_y + (nt_height + spacing) * grid_y as f32;

            // Nametable base address in VRAM
            // VRAM layout depends on mirroring, but we show logical nametables 0-3
            let nt_base = nt_idx * 0x400; // 0x000, 0x400, 0x800, or 0xC00 in VRAM (but wraps with 2KB)
            let attr_base = nt_base + 0x3C0; // Attribute table at end of each nametable

            // Render 32x30 tiles
            for tile_row in 0..30 {
                for tile_col in 0..32 {
                    let nt_offset = tile_row * 32 + tile_col;
                    // Map logical nametable address to physical VRAM index using mirroring
                    let logical_addr = (0x2000 + nt_base + nt_offset) as u16;
                    let vram_idx = self.map_nametable_addr_for_viewer(
                        logical_addr,
                        data.vram.len(),
                        &data.mirroring,
                    );
                    let tile_idx = data.vram.get(vram_idx).copied().unwrap_or(0) as usize;

                    // Get attribute byte for this 16x16 pixel area (4 tiles)
                    let attr_col = tile_col / 4;
                    let attr_row = tile_row / 4;
                    let attr_offset = attr_row * 8 + attr_col;
                    // Map attribute table address using mirroring
                    let attr_logical_addr = (0x2000 + attr_base + attr_offset) as u16;
                    let attr_vram_idx = self.map_nametable_addr_for_viewer(
                        attr_logical_addr,
                        data.vram.len(),
                        &data.mirroring,
                    );
                    let attr_byte = data.vram.get(attr_vram_idx).copied().unwrap_or(0);

                    // Extract 2-bit palette index for this 16x16 pixel quadrant
                    let quadrant_x = (tile_col / 2) % 2;
                    let quadrant_y = (tile_row / 2) % 2;
                    let shift = (quadrant_y * 2 + quadrant_x) * 2;
                    let palette_idx = ((attr_byte >> shift) & 0x03) as usize;

                    // Get palette colors for this tile (BG palettes at $3F00)
                    let palette_base = palette_idx * 4;

                    // Helper to convert packed RGB to Color32
                    let get_color = |pal_offset: usize| {
                        let idx = data.palette.get(pal_offset).copied().unwrap_or(0) as usize;
                        let rgb = data.master_palette.get(idx).copied().unwrap_or(0);
                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;
                        egui::Color32::from_rgb(r, g, b)
                    };

                    let colors: [egui::Color32; 4] = [
                        get_color(0), // Color 0 is always the universal background ($3F00)
                        get_color(palette_base + 1),
                        get_color(palette_base + 2),
                        get_color(palette_base + 3),
                    ];

                    // Get tile data from CHR
                    let chr_addr = bg_pattern_table + tile_idx * 16;
                    let tile_x = nt_start_x + tile_col as f32 * tile_size;
                    let tile_y = nt_y + tile_row as f32 * tile_size;

                    // Render 8x8 tile pixels
                    for py in 0..8 {
                        let plane0_addr = chr_addr + py;
                        let plane1_addr = chr_addr + py + 8;

                        let plane0 = data.chr_data.get(plane0_addr).copied().unwrap_or(0);
                        let plane1 = data.chr_data.get(plane1_addr).copied().unwrap_or(0);

                        for px in 0..8 {
                            let bit0 = (plane0 >> (7 - px)) & 1;
                            let bit1 = (plane1 >> (7 - px)) & 1;
                            let color_idx = ((bit1 << 1) | bit0) as usize;

                            let color = colors[color_idx];

                            let pixel_rect = egui::Rect::from_min_size(
                                egui::Pos2::new(
                                    tile_x + px as f32 * scale,
                                    tile_y + py as f32 * scale,
                                ),
                                egui::Vec2::new(scale, scale),
                            );
                            painter.rect_filled(pixel_rect, 0.0, color);
                        }
                    }

                    // Check if this tile is outside the scroll window and gray it out
                    // Calculate tile position in logical pixel space (0-511 for X, 0-479 for Y)
                    let tile_logical_x = (grid_x as f32 * 256.0) + (tile_col as f32 * 8.0);
                    let tile_logical_y = (grid_y as f32 * 240.0) + (tile_row as f32 * 8.0);

                    // Check if tile is outside the scroll window
                    let tile_right = tile_logical_x + 8.0;
                    let tile_bottom = tile_logical_y + 8.0;
                    let scroll_right = scroll_logical_x + viewport_width;
                    let scroll_bottom = scroll_logical_y + viewport_height;

                    // Tile is outside if it doesn't intersect with the scroll window
                    let is_outside = tile_right <= scroll_logical_x
                        || tile_logical_x >= scroll_right
                        || tile_bottom <= scroll_logical_y
                        || tile_logical_y >= scroll_bottom;

                    if is_outside {
                        // Apply gray overlay to tiles outside the visible area
                        let tile_rect = egui::Rect::from_min_size(
                            egui::Pos2::new(tile_x, tile_y),
                            egui::Vec2::new(tile_size, tile_size),
                        );
                        painter.rect_filled(
                            tile_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
                        );
                    }
                }
            }

            // Draw nametable border
            let nt_rect = egui::Rect::from_min_size(
                egui::Pos2::new(nt_start_x, nt_y),
                egui::Vec2::new(nt_width, nt_height),
            );
            painter.rect_stroke(
                nt_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 120)),
                egui::StrokeKind::Outside,
            );
        }

        // Highlight the scroll window (256x240 viewport)
        // Calculate which nametable the scroll position starts in
        let scroll_x = data.scroll_x as f32;
        let scroll_y = data.scroll_y as f32;

        // Base nametable selection from PPUCTRL bits 0-1
        let base_nt = (data.ppuctrl & 0x03) as usize;

        // Determine which nametable(s) the scroll window covers
        // The scroll window is 256x240 pixels (32x30 tiles)
        let viewport_width = 256.0;
        let viewport_height = 240.0;

        // Calculate nametable selection based on scroll position
        // NES PPU nametable selection: base_nt XOR scroll overflow bits
        // This formula matches the rendering logic in ppu.rs
        let nt_x = ((scroll_x / 256.0) as usize) & 1;
        let nt_y = ((scroll_y / 240.0) as usize) & 1;
        let scroll_nt = base_nt ^ nt_x ^ (nt_y << 1);

        // Calculate the position of the scroll window in the grid
        let grid_x = scroll_nt % 2;
        let grid_y = scroll_nt / 2;
        let scroll_nt_x = rect.min.x + (nt_width + spacing) * grid_x as f32;
        let scroll_nt_y = nt_start_y + (nt_height + spacing) * grid_y as f32;

        // Calculate the scroll window position within the nametable
        let scroll_pixel_x = (scroll_x % 256.0) * scale;
        let scroll_pixel_y = (scroll_y % 240.0) * scale;

        // The scroll window is 256x240 pixels and can wrap across nametable boundaries
        // We need to draw it in up to 4 pieces to handle wrapping correctly
        let viewport_width_scaled = viewport_width * scale;
        let viewport_height_scaled = viewport_height * scale;

        // Check if the window wraps horizontally
        let wraps_x = scroll_pixel_x + viewport_width_scaled > nt_width;
        // Check if the window wraps vertically
        let wraps_y = scroll_pixel_y + viewport_height_scaled > nt_height;

        // Helper function to draw a scroll window rectangle segment
        let draw_scroll_rect = |painter: &egui::Painter, x: f32, y: f32, w: f32, h: f32| {
            let rect = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new(w, h));
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 0, 200)),
                egui::StrokeKind::Outside,
            );
            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 0, 20),
            );
        };

        // Draw the scroll window in up to 4 segments to handle wrapping
        match (wraps_x, wraps_y) {
            (false, false) => {
                // No wrapping - draw single rectangle
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    scroll_nt_y + scroll_pixel_y,
                    viewport_width_scaled,
                    viewport_height_scaled,
                );
            }
            (true, false) => {
                // Wraps horizontally only
                let width_first = nt_width - scroll_pixel_x;
                let width_second = viewport_width_scaled - width_first;

                // First segment (right side of current nametable)
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    scroll_nt_y + scroll_pixel_y,
                    width_first,
                    viewport_height_scaled,
                );

                // Second segment (left side of next nametable)
                let next_nt_x = if grid_x == 0 {
                    scroll_nt_x + nt_width + spacing
                } else {
                    scroll_nt_x - nt_width - spacing
                };
                draw_scroll_rect(
                    &painter,
                    next_nt_x,
                    scroll_nt_y + scroll_pixel_y,
                    width_second,
                    viewport_height_scaled,
                );
            }
            (false, true) => {
                // Wraps vertically only
                let height_first = nt_height - scroll_pixel_y;
                let height_second = viewport_height_scaled - height_first;

                // First segment (bottom of current nametable)
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    scroll_nt_y + scroll_pixel_y,
                    viewport_width_scaled,
                    height_first,
                );

                // Second segment (top of next nametable)
                let next_nt_y = if grid_y == 0 {
                    scroll_nt_y + nt_height + spacing
                } else {
                    scroll_nt_y - nt_height - spacing
                };
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    next_nt_y,
                    viewport_width_scaled,
                    height_second,
                );
            }
            (true, true) => {
                // Wraps both horizontally and vertically - draw 4 segments
                let width_first = nt_width - scroll_pixel_x;
                let width_second = viewport_width_scaled - width_first;
                let height_first = nt_height - scroll_pixel_y;
                let height_second = viewport_height_scaled - height_first;

                let next_nt_x = if grid_x == 0 {
                    scroll_nt_x + nt_width + spacing
                } else {
                    scroll_nt_x - nt_width - spacing
                };
                let next_nt_y = if grid_y == 0 {
                    scroll_nt_y + nt_height + spacing
                } else {
                    scroll_nt_y - nt_height - spacing
                };

                // Bottom-right of current nametable
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    scroll_nt_y + scroll_pixel_y,
                    width_first,
                    height_first,
                );

                // Bottom-left of horizontally adjacent nametable
                draw_scroll_rect(
                    &painter,
                    next_nt_x,
                    scroll_nt_y + scroll_pixel_y,
                    width_second,
                    height_first,
                );

                // Top-right of vertically adjacent nametable
                draw_scroll_rect(
                    &painter,
                    scroll_nt_x + scroll_pixel_x,
                    next_nt_y,
                    width_first,
                    height_second,
                );

                // Top-left of diagonally adjacent nametable
                draw_scroll_rect(&painter, next_nt_x, next_nt_y, width_second, height_second);
            }
        }

        // Handle hover tooltip
        if let Some(hover_pos) = response.hover_pos() {
            // Check which nametable we're hovering over (2x2 grid)
            for nt_idx in 0..4 {
                let grid_x = nt_idx % 2;
                let grid_y = nt_idx / 2;
                let nt_start_x = rect.min.x + (nt_width + spacing) * grid_x as f32;
                let nt_y = nt_start_y + (nt_height + spacing) * grid_y as f32;

                let rel_x = hover_pos.x - nt_start_x;
                let rel_y = hover_pos.y - nt_y;

                if rel_x >= 0.0 && rel_x < nt_width && rel_y >= 0.0 && rel_y < nt_height {
                    let tile_col = (rel_x / tile_size) as usize;
                    let tile_row = (rel_y / tile_size) as usize;

                    if tile_col < 32 && tile_row < 30 {
                        let nt_base = nt_idx * 0x400;
                        let nt_offset = tile_row * 32 + tile_col;
                        // Map logical nametable address to physical VRAM index
                        let logical_addr = (0x2000 + nt_base + nt_offset) as u16;
                        let vram_idx = self.map_nametable_addr_for_viewer(
                            logical_addr,
                            data.vram.len(),
                            &data.mirroring,
                        );
                        let tile_idx = data.vram.get(vram_idx).copied().unwrap_or(0);

                        // Get attribute info
                        let attr_base = nt_base + 0x3C0;
                        let attr_col = tile_col / 4;
                        let attr_row = tile_row / 4;
                        let attr_offset = attr_row * 8 + attr_col;
                        let attr_logical_addr = (0x2000 + attr_base + attr_offset) as u16;
                        let attr_vram_idx = self.map_nametable_addr_for_viewer(
                            attr_logical_addr,
                            data.vram.len(),
                            &data.mirroring,
                        );
                        let attr_byte = data.vram.get(attr_vram_idx).copied().unwrap_or(0);

                        let quadrant_x = (tile_col / 2) % 2;
                        let quadrant_y = (tile_row / 2) % 2;
                        let shift = (quadrant_y * 2 + quadrant_x) * 2;
                        let palette_idx = (attr_byte >> shift) & 0x03;

                        let chr_addr = bg_pattern_table + (tile_idx as usize) * 16;
                        let nt_addr = 0x2000 + nt_base + nt_offset;
                        let attr_addr = 0x2000 + attr_base + attr_offset;

                        response.clone().on_hover_ui(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Nametable {} ({}, {})",
                                    nt_idx, tile_col, tile_row
                                ))
                                .strong(),
                            );
                            ui.separator();
                            ui.label(format!("NT Address: ${:04X}", nt_addr));
                            ui.label(format!("Tile Index: ${:02X} ({})", tile_idx, tile_idx));
                            ui.label(format!("CHR Address: ${:04X}", chr_addr));
                            ui.separator();
                            ui.label(format!("Attr Address: ${:04X}", attr_addr));
                            ui.label(format!("Attr Byte: ${:02X}", attr_byte));
                            ui.label(format!(
                                "Palette: {} (quadrant {}, {})",
                                palette_idx, quadrant_x, quadrant_y
                            ));
                        });
                    }
                    break; // Found the nametable we're over
                }
            }
        }
    }

    fn render_about_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("🎮 Hemulator").size(36.0).strong());
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Multi-System Console Emulator")
                            .size(16.0)
                            .italics(),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        egui::RichText::new(format!("Version {}", APP_VERSION))
                            .size(14.0)
                            .weak(),
                    );
                    ui.add_space(20.0);
                });

                ui.separator();
                ui.add_space(10.0);

                // About section
                ui.heading(egui::RichText::new("📖 About").strong());
                ui.add_space(5.0);
                ui.label("A cross-platform, multi-system console emulator written in Rust,");
                ui.label("supporting NES, Atari 2600, Game Boy, GBA, SNES, N64, and PC emulation");
                ui.label("with comprehensive save state management and customizable controls.");
                ui.add_space(10.0);

                // License
                ui.heading(egui::RichText::new("📜 License").strong());
                ui.add_space(5.0);
                ui.label("MIT License - Copyright (c) 2025");
                ui.add_space(10.0);

                // Links
                ui.heading(egui::RichText::new("🔗 Links").strong());
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("GitHub:");
                    ui.hyperlink_to(
                        "github.com/Hexagon/hemulator",
                        "https://github.com/Hexagon/hemulator",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📚 User Manual:");
                    ui.hyperlink_to(
                        "MANUAL.md",
                        "https://github.com/Hexagon/hemulator/blob/main/docs/MANUAL.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📖 Documentation:");
                    ui.hyperlink_to(
                        "README.md",
                        "https://github.com/Hexagon/hemulator/blob/main/README.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📄 License:");
                    ui.hyperlink_to(
                        "MIT License",
                        "https://github.com/Hexagon/hemulator/blob/main/LICENSE",
                    );
                });
            });
    }

    /// Render Atari 2600 Playfield inspector tab
    pub fn render_atari2600_playfield_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Atari2600(ref data)) = self.system_tile_data {
                    ui.heading("🎨 Atari 2600 Playfield");
                    ui.separator();

                    // Playfield registers
                    ui.label(egui::RichText::new("Playfield Registers").strong());
                    egui::Grid::new("pf_registers")
                        .num_columns(2)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("PF0:");
                            ui.label(
                                egui::RichText::new(format!("${:02X} (bits 7-4)", data.pf0))
                                    .monospace(),
                            );
                            ui.end_row();

                            ui.label("PF1:");
                            ui.label(egui::RichText::new(format!("${:02X}", data.pf1)).monospace());
                            ui.end_row();

                            ui.label("PF2:");
                            ui.label(egui::RichText::new(format!("${:02X}", data.pf2)).monospace());
                            ui.end_row();
                        });

                    ui.add_space(10.0);

                    // Playfield control flags
                    ui.label(egui::RichText::new("Playfield Control (CTRLPF)").strong());
                    egui::Grid::new("pf_control")
                        .num_columns(2)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Reflection:");
                            ui.label(if data.playfield_reflect {
                                "ON (mirrored)"
                            } else {
                                "OFF (repeated)"
                            });
                            ui.end_row();

                            ui.label("Score Mode:");
                            ui.label(if data.playfield_score_mode {
                                "ON (left=P0, right=P1 color)"
                            } else {
                                "OFF"
                            });
                            ui.end_row();

                            ui.label("Priority:");
                            ui.label(if data.playfield_priority {
                                "IN FRONT of players"
                            } else {
                                "BEHIND players"
                            });
                            ui.end_row();
                        });

                    ui.add_space(10.0);

                    // Visual playfield representation
                    ui.label(egui::RichText::new("Visual Playfield (40 bits)").strong());
                    ui.label("Each bit represents 4 pixels horizontally");

                    let (response, painter) = ui.allocate_painter(
                        egui::Vec2::new(ui.available_width().min(640.0), 100.0),
                        egui::Sense::hover(),
                    );

                    let rect = response.rect;
                    let cell_width = rect.width() / 40.0;
                    let cell_height = rect.height();

                    // Get playfield color
                    let pf_color_idx = (data.colupf >> 1) as usize & 0x7F;
                    let pf_rgb = data
                        .master_palette
                        .get(pf_color_idx)
                        .copied()
                        .unwrap_or(0xFFFFFF);
                    let pf_color = egui::Color32::from_rgb(
                        ((pf_rgb >> 16) & 0xFF) as u8,
                        ((pf_rgb >> 8) & 0xFF) as u8,
                        (pf_rgb & 0xFF) as u8,
                    );

                    // Draw playfield bits (left half)
                    // PF0 bits 7-4 (reversed), PF1 bits 7-0, PF2 bits 0-7
                    for i in 0..20 {
                        let bit = if i < 4 {
                            // PF0 bits 7-4 (reversed: bit 4 first, bit 7 last)
                            (data.pf0 >> (4 + i)) & 1
                        } else if i < 12 {
                            // PF1 bits 7-0
                            (data.pf1 >> (7 - (i - 4))) & 1
                        } else {
                            // PF2 bits 0-7
                            (data.pf2 >> (i - 12)) & 1
                        };

                        let x = rect.min.x + i as f32 * cell_width;
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(x, rect.min.y),
                            egui::vec2(cell_width, cell_height),
                        );

                        let color = if bit != 0 {
                            pf_color
                        } else {
                            egui::Color32::from_rgb(32, 32, 32)
                        };
                        painter.rect_filled(cell_rect, 0.0, color);
                        painter.rect_stroke(
                            cell_rect,
                            0.0,
                            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                            egui::StrokeKind::Inside,
                        );
                    }

                    // Draw playfield bits (right half - reflection or repeat)
                    for i in 0..20 {
                        let source_i = if data.playfield_reflect { 19 - i } else { i };

                        let bit = if source_i < 4 {
                            (data.pf0 >> (4 + source_i)) & 1
                        } else if source_i < 12 {
                            (data.pf1 >> (7 - (source_i - 4))) & 1
                        } else {
                            (data.pf2 >> (source_i - 12)) & 1
                        };

                        let x = rect.min.x + (20 + i) as f32 * cell_width;
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(x, rect.min.y),
                            egui::vec2(cell_width, cell_height),
                        );

                        let color = if bit != 0 {
                            pf_color
                        } else {
                            egui::Color32::from_rgb(32, 32, 32)
                        };
                        painter.rect_filled(cell_rect, 0.0, color);
                        painter.rect_stroke(
                            cell_rect,
                            0.0,
                            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                            egui::StrokeKind::Inside,
                        );
                    }

                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Left Half").weak());
                    ui.label("PF0[4-7] (4 bits) → PF1[7-0] (8 bits) → PF2[0-7] (8 bits)");
                    ui.label(
                        egui::RichText::new(format!(
                            "Right Half: {}",
                            if data.playfield_reflect {
                                "Mirrored from left half (REFL=1)"
                            } else {
                                "Repeated from left half (REFL=0)"
                            }
                        ))
                        .weak(),
                    );
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🎨").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Playfield");
                        ui.add_space(10.0);
                        ui.label("Load an Atari 2600 ROM to inspect the playfield");
                    });
                }
            });
    }

    /// Render Atari 2600 Sprites inspector tab
    pub fn render_atari2600_sprites_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Atari2600(ref data)) = self.system_tile_data {
                    ui.heading("👾 Atari 2600 Sprites");
                    ui.separator();

                    // Player 0
                    ui.label(egui::RichText::new("Player 0 (GRP0)").strong());
                    egui::Grid::new("player0_grid")
                        .num_columns(2)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Graphics:");
                            ui.label(
                                egui::RichText::new(format!(
                                    "${:02X} = %{:08b}",
                                    data.grp0, data.grp0
                                ))
                                .monospace(),
                            );
                            ui.end_row();

                            ui.label("Position:");
                            ui.label(format!("X = {}", data.player0_x));
                            ui.end_row();

                            ui.label("Reflect:");
                            ui.label(if data.player0_reflect {
                                "Yes (mirrored)"
                            } else {
                                "No"
                            });
                            ui.end_row();

                            ui.label("NUSIZ:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.nusiz0)).monospace(),
                            );
                            ui.end_row();
                        });

                    // Visual representation of Player 0
                    if data.grp0 != 0 {
                        ui.label("Visual:");
                        let (response, painter) = ui.allocate_painter(
                            egui::Vec2::new(ui.available_width().min(200.0), 30.0),
                            egui::Sense::hover(),
                        );

                        let rect = response.rect;
                        let pixel_width = rect.width() / 8.0;

                        let p0_color_idx = (data.colup0 >> 1) as usize & 0x7F;
                        let p0_rgb = data
                            .master_palette
                            .get(p0_color_idx)
                            .copied()
                            .unwrap_or(0xFFFFFF);
                        let p0_color = egui::Color32::from_rgb(
                            ((p0_rgb >> 16) & 0xFF) as u8,
                            ((p0_rgb >> 8) & 0xFF) as u8,
                            (p0_rgb & 0xFF) as u8,
                        );

                        for i in 0..8 {
                            let bit_pos = if data.player0_reflect { i } else { 7 - i };
                            let bit = (data.grp0 >> bit_pos) & 1;

                            let x = rect.min.x + i as f32 * pixel_width;
                            let pixel_rect = egui::Rect::from_min_size(
                                egui::pos2(x, rect.min.y),
                                egui::vec2(pixel_width, rect.height()),
                            );

                            let color = if bit != 0 {
                                p0_color
                            } else {
                                egui::Color32::from_rgb(32, 32, 32)
                            };
                            painter.rect_filled(pixel_rect, 0.0, color);
                            painter.rect_stroke(
                                pixel_rect,
                                0.0,
                                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }

                    ui.add_space(15.0);

                    // Player 1
                    ui.label(egui::RichText::new("Player 1 (GRP1)").strong());
                    egui::Grid::new("player1_grid")
                        .num_columns(2)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Graphics:");
                            ui.label(
                                egui::RichText::new(format!(
                                    "${:02X} = %{:08b}",
                                    data.grp1, data.grp1
                                ))
                                .monospace(),
                            );
                            ui.end_row();

                            ui.label("Position:");
                            ui.label(format!("X = {}", data.player1_x));
                            ui.end_row();

                            ui.label("Reflect:");
                            ui.label(if data.player1_reflect {
                                "Yes (mirrored)"
                            } else {
                                "No"
                            });
                            ui.end_row();

                            ui.label("NUSIZ:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.nusiz1)).monospace(),
                            );
                            ui.end_row();
                        });

                    // Visual representation of Player 1
                    if data.grp1 != 0 {
                        ui.label("Visual:");
                        let (response, painter) = ui.allocate_painter(
                            egui::Vec2::new(ui.available_width().min(200.0), 30.0),
                            egui::Sense::hover(),
                        );

                        let rect = response.rect;
                        let pixel_width = rect.width() / 8.0;

                        let p1_color_idx = (data.colup1 >> 1) as usize & 0x7F;
                        let p1_rgb = data
                            .master_palette
                            .get(p1_color_idx)
                            .copied()
                            .unwrap_or(0xFFFFFF);
                        let p1_color = egui::Color32::from_rgb(
                            ((p1_rgb >> 16) & 0xFF) as u8,
                            ((p1_rgb >> 8) & 0xFF) as u8,
                            (p1_rgb & 0xFF) as u8,
                        );

                        for i in 0..8 {
                            let bit_pos = if data.player1_reflect { i } else { 7 - i };
                            let bit = (data.grp1 >> bit_pos) & 1;

                            let x = rect.min.x + i as f32 * pixel_width;
                            let pixel_rect = egui::Rect::from_min_size(
                                egui::pos2(x, rect.min.y),
                                egui::vec2(pixel_width, rect.height()),
                            );

                            let color = if bit != 0 {
                                p1_color
                            } else {
                                egui::Color32::from_rgb(32, 32, 32)
                            };
                            painter.rect_filled(pixel_rect, 0.0, color);
                            painter.rect_stroke(
                                pixel_rect,
                                0.0,
                                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }

                    ui.add_space(15.0);

                    // Missiles and Ball
                    ui.label(egui::RichText::new("Missiles & Ball").strong());
                    egui::Grid::new("missiles_ball_grid")
                        .num_columns(3)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("");
                            ui.label("Enabled");
                            ui.label("Position");
                            ui.end_row();

                            ui.label("Missile 0:");
                            ui.label(if data.enam0 { "Yes" } else { "No" });
                            ui.label(format!("X = {}", data.missile0_x));
                            ui.end_row();

                            ui.label("Missile 1:");
                            ui.label(if data.enam1 { "Yes" } else { "No" });
                            ui.label(format!("X = {}", data.missile1_x));
                            ui.end_row();

                            ui.label("Ball:");
                            ui.label(if data.enabl { "Yes" } else { "No" });
                            ui.label(format!("X = {}, Size = {}", data.ball_x, data.ball_size));
                            ui.end_row();
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("👾").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Sprites");
                        ui.add_space(10.0);
                        ui.label("Load an Atari 2600 ROM to inspect sprites");
                    });
                }
            });
    }

    /// Render Atari 2600 Palette inspector tab
    pub fn render_atari2600_palette_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Atari2600(ref data)) = self.system_tile_data {
                    ui.heading("🎨 Atari 2600 Palette");
                    ui.separator();

                    // Current colors
                    ui.label(egui::RichText::new("Current Colors").strong());
                    egui::Grid::new("current_colors")
                        .num_columns(3)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Background
                            ui.label("Background:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.colubk)).monospace(),
                            );
                            let bg_idx = (data.colubk >> 1) as usize & 0x7F;
                            let bg_rgb = data.master_palette.get(bg_idx).copied().unwrap_or(0);
                            let mut bg_color = egui::Color32::from_rgb(
                                ((bg_rgb >> 16) & 0xFF) as u8,
                                ((bg_rgb >> 8) & 0xFF) as u8,
                                (bg_rgb & 0xFF) as u8,
                            );
                            ui.color_edit_button_srgba(&mut bg_color);
                            ui.end_row();

                            // Playfield
                            ui.label("Playfield:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.colupf)).monospace(),
                            );
                            let pf_idx = (data.colupf >> 1) as usize & 0x7F;
                            let pf_rgb = data.master_palette.get(pf_idx).copied().unwrap_or(0);
                            let mut pf_color = egui::Color32::from_rgb(
                                ((pf_rgb >> 16) & 0xFF) as u8,
                                ((pf_rgb >> 8) & 0xFF) as u8,
                                (pf_rgb & 0xFF) as u8,
                            );
                            ui.color_edit_button_srgba(&mut pf_color);
                            ui.end_row();

                            // Player 0
                            ui.label("Player 0:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.colup0)).monospace(),
                            );
                            let p0_idx = (data.colup0 >> 1) as usize & 0x7F;
                            let p0_rgb = data.master_palette.get(p0_idx).copied().unwrap_or(0);
                            let mut p0_color = egui::Color32::from_rgb(
                                ((p0_rgb >> 16) & 0xFF) as u8,
                                ((p0_rgb >> 8) & 0xFF) as u8,
                                (p0_rgb & 0xFF) as u8,
                            );
                            ui.color_edit_button_srgba(&mut p0_color);
                            ui.end_row();

                            // Player 1
                            ui.label("Player 1:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.colup1)).monospace(),
                            );
                            let p1_idx = (data.colup1 >> 1) as usize & 0x7F;
                            let p1_rgb = data.master_palette.get(p1_idx).copied().unwrap_or(0);
                            let mut p1_color = egui::Color32::from_rgb(
                                ((p1_rgb >> 16) & 0xFF) as u8,
                                ((p1_rgb >> 8) & 0xFF) as u8,
                                (p1_rgb & 0xFF) as u8,
                            );
                            ui.color_edit_button_srgba(&mut p1_color);
                            ui.end_row();
                        });

                    ui.add_space(15.0);

                    // NTSC Master Palette (128 colors)
                    ui.label(egui::RichText::new("NTSC Master Palette (128 colors)").strong());
                    ui.label("Bits 7-4 = Hue (0-15), Bits 3-1 = Luminance (0-7), Bit 0 = unused");
                    ui.add_space(5.0);

                    // Display palette in a 16x8 grid
                    let cell_size = 24.0;
                    let (response, painter) = ui.allocate_painter(
                        egui::Vec2::new(16.0 * cell_size, 8.0 * cell_size),
                        egui::Sense::hover(),
                    );

                    let rect = response.rect;
                    for lum in 0..8 {
                        for hue in 0..16 {
                            let idx = hue * 8 + lum;
                            let rgb = data.master_palette.get(idx).copied().unwrap_or(0);
                            let color = egui::Color32::from_rgb(
                                ((rgb >> 16) & 0xFF) as u8,
                                ((rgb >> 8) & 0xFF) as u8,
                                (rgb & 0xFF) as u8,
                            );

                            let x = rect.min.x + hue as f32 * cell_size;
                            let y = rect.min.y + lum as f32 * cell_size;
                            let cell_rect = egui::Rect::from_min_size(
                                egui::pos2(x, y),
                                egui::vec2(cell_size, cell_size),
                            );

                            painter.rect_filled(cell_rect, 0.0, color);
                            painter.rect_stroke(
                                cell_rect,
                                0.0,
                                egui::Stroke::new(0.5, egui::Color32::DARK_GRAY),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🎨").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Palette");
                        ui.add_space(10.0);
                        ui.label("Load an Atari 2600 ROM to view the palette");
                    });
                }
            });
    }

    /// Render Atari 2600 Collision inspector tab
    pub fn render_atari2600_collision_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Atari2600(ref data)) = self.system_tile_data {
                    ui.heading("💥 Atari 2600 Collision Detection");
                    ui.separator();

                    ui.label("Collision detection registers track when graphics objects overlap.");
                    ui.label("Bit 7 and 6 indicate collisions for each object pair.");
                    ui.add_space(10.0);

                    // Collision registers
                    egui::Grid::new("collision_grid")
                        .num_columns(3)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Register").strong());
                            ui.label(egui::RichText::new("Value").strong());
                            ui.label(egui::RichText::new("Collision").strong());
                            ui.end_row();

                            // CXM0P - Missile 0 to Players
                            ui.label("CXM0P:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxm0p)).monospace(),
                            );
                            let m0p0 = (data.cxm0p & 0x80) != 0;
                            let m0p1 = (data.cxm0p & 0x40) != 0;
                            ui.label(format!(
                                "M0-P0: {} | M0-P1: {}",
                                if m0p0 { "✓" } else { "✗" },
                                if m0p1 { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXM1P - Missile 1 to Players
                            ui.label("CXM1P:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxm1p)).monospace(),
                            );
                            let m1p0 = (data.cxm1p & 0x80) != 0;
                            let m1p1 = (data.cxm1p & 0x40) != 0;
                            ui.label(format!(
                                "M1-P0: {} | M1-P1: {}",
                                if m1p0 { "✓" } else { "✗" },
                                if m1p1 { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXP0FB - Player 0 to Playfield/Ball
                            ui.label("CXP0FB:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxp0fb)).monospace(),
                            );
                            let p0pf = (data.cxp0fb & 0x80) != 0;
                            let p0bl = (data.cxp0fb & 0x40) != 0;
                            ui.label(format!(
                                "P0-PF: {} | P0-BL: {}",
                                if p0pf { "✓" } else { "✗" },
                                if p0bl { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXP1FB - Player 1 to Playfield/Ball
                            ui.label("CXP1FB:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxp1fb)).monospace(),
                            );
                            let p1pf = (data.cxp1fb & 0x80) != 0;
                            let p1bl = (data.cxp1fb & 0x40) != 0;
                            ui.label(format!(
                                "P1-PF: {} | P1-BL: {}",
                                if p1pf { "✓" } else { "✗" },
                                if p1bl { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXM0FB - Missile 0 to Playfield/Ball
                            ui.label("CXM0FB:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxm0fb)).monospace(),
                            );
                            let m0pf = (data.cxm0fb & 0x80) != 0;
                            let m0bl = (data.cxm0fb & 0x40) != 0;
                            ui.label(format!(
                                "M0-PF: {} | M0-BL: {}",
                                if m0pf { "✓" } else { "✗" },
                                if m0bl { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXM1FB - Missile 1 to Playfield/Ball
                            ui.label("CXM1FB:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxm1fb)).monospace(),
                            );
                            let m1pf = (data.cxm1fb & 0x80) != 0;
                            let m1bl = (data.cxm1fb & 0x40) != 0;
                            ui.label(format!(
                                "M1-PF: {} | M1-BL: {}",
                                if m1pf { "✓" } else { "✗" },
                                if m1bl { "✓" } else { "✗" }
                            ));
                            ui.end_row();

                            // CXBLPF - Ball to Playfield
                            ui.label("CXBLPF:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxblpf)).monospace(),
                            );
                            let blpf = (data.cxblpf & 0x80) != 0;
                            ui.label(format!("BL-PF: {}", if blpf { "✓" } else { "✗" }));
                            ui.end_row();

                            // CXPPMM - Player/Missile collisions
                            ui.label("CXPPMM:");
                            ui.label(
                                egui::RichText::new(format!("${:02X}", data.cxppmm)).monospace(),
                            );
                            let m0m1 = (data.cxppmm & 0x80) != 0;
                            let p0p1 = (data.cxppmm & 0x40) != 0;
                            ui.label(format!(
                                "M0-M1: {} | P0-P1: {}",
                                if m0m1 { "✓" } else { "✗" },
                                if p0p1 { "✓" } else { "✗" }
                            ));
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("✓ = Collision detected | ✗ = No collision").weak(),
                    );
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("💥").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Collision Detection");
                        ui.add_space(10.0);
                        ui.label("Load an Atari 2600 ROM to view collision data");
                    });
                }
            });
    }

    pub fn render_chip8_display_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Chip8(ref data)) = self.system_tile_data {
                    ui.heading("📺 CHIP-8 Display");
                    ui.separator();

                    // Display information
                    ui.label(egui::RichText::new("Display Information").strong());
                    egui::Grid::new("chip8_display_info")
                        .num_columns(2)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Mode:");
                            ui.label(&data.mode);
                            ui.end_row();

                            ui.label("Resolution:");
                            ui.label(format!("{}x{}", data.display_width, data.display_height));
                            ui.end_row();

                            ui.label("High-Res:");
                            ui.label(if data.high_res { "Yes" } else { "No" });
                            ui.end_row();

                            ui.label("Selected Plane:");
                            ui.label(format!(
                                "{} (0x{:X})",
                                data.selected_plane, data.selected_plane
                            ));
                            ui.end_row();
                        });

                    ui.add_space(10.0);

                    // Display planes visualization
                    ui.label(egui::RichText::new("Display Planes").strong());

                    // Calculate scale factor to fit display nicely
                    let available_width = ui.available_width().min(640.0);
                    let scale = (available_width / data.display_width as f32)
                        .floor()
                        .max(1.0);

                    ui.horizontal(|ui| {
                        // Plane 0 (always present)
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Plane 0").strong());
                            let (response, painter) = ui.allocate_painter(
                                egui::Vec2::new(
                                    data.display_width as f32 * scale,
                                    data.display_height as f32 * scale,
                                ),
                                egui::Sense::hover(),
                            );

                            let rect = response.rect;
                            let pixel_size = scale;

                            // Draw background
                            painter.rect_filled(rect, 0.0, egui::Color32::BLACK);

                            // Draw pixels
                            for y in 0..data.display_height {
                                for x in 0..data.display_width {
                                    let idx = y * data.display_width + x;
                                    if idx < data.display_plane0.len() && data.display_plane0[idx] {
                                        let pixel_rect = egui::Rect::from_min_size(
                                            egui::pos2(
                                                rect.min.x + x as f32 * pixel_size,
                                                rect.min.y + y as f32 * pixel_size,
                                            ),
                                            egui::vec2(pixel_size, pixel_size),
                                        );
                                        painter.rect_filled(
                                            pixel_rect,
                                            0.0,
                                            egui::Color32::from_rgb(0, 255, 0), // Green
                                        );
                                    }
                                }
                            }
                        });

                        // Plane 1 (for XO-CHIP)
                        if data.mode.contains("XO-CHIP") {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Plane 1").strong());
                                let (response, painter) = ui.allocate_painter(
                                    egui::Vec2::new(
                                        data.display_width as f32 * scale,
                                        data.display_height as f32 * scale,
                                    ),
                                    egui::Sense::hover(),
                                );

                                let rect = response.rect;
                                let pixel_size = scale;

                                // Draw background
                                painter.rect_filled(rect, 0.0, egui::Color32::BLACK);

                                // Draw pixels
                                for y in 0..data.display_height {
                                    for x in 0..data.display_width {
                                        let idx = y * data.display_width + x;
                                        if idx < data.display_plane1.len()
                                            && data.display_plane1[idx]
                                        {
                                            let pixel_rect = egui::Rect::from_min_size(
                                                egui::pos2(
                                                    rect.min.x + x as f32 * pixel_size,
                                                    rect.min.y + y as f32 * pixel_size,
                                                ),
                                                egui::vec2(pixel_size, pixel_size),
                                            );
                                            painter.rect_filled(
                                                pixel_rect,
                                                0.0,
                                                egui::Color32::from_rgb(255, 0, 0), // Red
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    });

                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("🟢 Green = Plane 0 | 🔴 Red = Plane 1").weak());
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("📺").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("CHIP-8 Display");
                        ui.add_space(10.0);
                        ui.label("Load a CHIP-8 program to view display data");
                    });
                }
            });
    }

    pub fn render_chip8_registers_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(SystemTileData::Chip8(ref data)) = self.system_tile_data {
                    ui.heading("📝 CHIP-8 Registers");
                    ui.separator();

                    // Main registers
                    ui.label(egui::RichText::new("Main Registers").strong());
                    egui::Grid::new("chip8_main_regs")
                        .num_columns(4)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("PC:");
                            ui.label(egui::RichText::new(format!("${:04X}", data.pc)).monospace());
                            ui.label("I:");
                            ui.label(egui::RichText::new(format!("${:04X}", data.i)).monospace());
                            ui.end_row();

                            ui.label("SP:");
                            ui.label(egui::RichText::new(format!("${:02X}", data.sp)).monospace());
                            ui.label("Mode:");
                            ui.label(&data.mode);
                            ui.end_row();
                        });

                    ui.add_space(10.0);

                    // Timers
                    ui.label(egui::RichText::new("Timers").strong());
                    egui::Grid::new("chip8_timers")
                        .num_columns(4)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Delay:");
                            ui.label(
                                egui::RichText::new(format!("{}", data.delay_timer)).monospace(),
                            );
                            ui.label("Sound:");
                            ui.label(
                                egui::RichText::new(format!("{}", data.sound_timer)).monospace(),
                            );
                            ui.end_row();
                        });

                    ui.add_space(10.0);

                    // V Registers (V0-VF)
                    ui.label(egui::RichText::new("V Registers (V0-VF)").strong());
                    egui::Grid::new("chip8_v_regs")
                        .num_columns(8)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (i, &value) in data.v_registers.iter().enumerate() {
                                ui.label(format!("V{:X}:", i));
                                let color = if i == 0xF {
                                    egui::Color32::from_rgb(255, 200, 100) // Highlight VF (flag register)
                                } else {
                                    egui::Color32::WHITE
                                };
                                ui.label(
                                    egui::RichText::new(format!("${:02X}", value))
                                        .monospace()
                                        .color(color),
                                );
                                if (i + 1) % 4 == 0 {
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(10.0);

                    // Stack
                    ui.label(egui::RichText::new("Stack").strong());
                    ui.label(format!("Stack Pointer: {} / 16", data.sp));

                    egui::Grid::new("chip8_stack")
                        .num_columns(8)
                        .spacing([10.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (i, &value) in data.stack.iter().enumerate() {
                                ui.label(format!("[{}]:", i));
                                let color = if i < data.sp as usize {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::DARK_GRAY // Dim unused stack entries
                                };
                                ui.label(
                                    egui::RichText::new(format!("${:04X}", value))
                                        .monospace()
                                        .color(color),
                                );
                                if (i + 1) % 4 == 0 {
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(10.0);

                    // Input state
                    ui.label(egui::RichText::new("Input State").strong());
                    if data.waiting_for_key {
                        ui.label(
                            egui::RichText::new("⏸ Waiting for key press...")
                                .color(egui::Color32::YELLOW),
                        );
                    }

                    ui.label("Hexadecimal Keypad (0x0-0xF):");
                    egui::Grid::new("chip8_keys")
                        .num_columns(4)
                        .spacing([5.0, 5.0])
                        .show(ui, |ui| {
                            // CHIP-8 keypad layout:
                            // 1 2 3 C
                            // 4 5 6 D
                            // 7 8 9 E
                            // A 0 B F
                            let keypad = [
                                [0x1, 0x2, 0x3, 0xC],
                                [0x4, 0x5, 0x6, 0xD],
                                [0x7, 0x8, 0x9, 0xE],
                                [0xA, 0x0, 0xB, 0xF],
                            ];

                            for row in &keypad {
                                for &key in row {
                                    let pressed = data.keys[key];
                                    let text = format!("{:X}", key);
                                    let color = if pressed {
                                        egui::Color32::from_rgb(100, 255, 100) // Green when pressed
                                    } else {
                                        egui::Color32::from_rgb(60, 60, 60) // Gray when not pressed
                                    };
                                    ui.label(
                                        egui::RichText::new(text)
                                            .monospace()
                                            .color(color)
                                            .size(16.0),
                                    );
                                }
                                ui.end_row();
                            }
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("📝").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("CHIP-8 Registers");
                        ui.add_space(10.0);
                        ui.label("Load a CHIP-8 program to view register data");
                    });
                }
            });
    }

    fn render_gb_tilemaps(&self, ui: &mut Ui, data: &GbTileData) {
        // Game Boy has two 32x32 tilemaps (Background and Window)
        // Each tilemap is 32x32 tiles = 256x256 pixels
        let scale = 1.5; // Scale for better visibility
        let tile_size = 8.0 * scale;
        let tilemap_width = 32.0 * tile_size;
        let tilemap_height = 32.0 * tile_size;
        let spacing = 15.0;

        // Show both Background and Window tilemaps side by side
        let total_width = tilemap_width * 2.0 + spacing * 2.0;
        let total_height = tilemap_height + 40.0; // Extra for labels

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(total_width, total_height),
            egui::Sense::hover(),
        );
        let rect = response.rect;

        // Check if we have valid data
        if data.vram_bank0.len() < 0x2000 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Tilemap data not available",
                egui::FontId::default(),
                egui::Color32::GRAY,
            );
            return;
        }

        // Determine tile data addressing mode from LCDC bit 4
        let tile_data_select = (data.lcdc & 0x10) != 0; // 0=0x8800-0x97FF, 1=0x8000-0x8FFF

        // Background tilemap base (LCDC bit 3)
        let bg_tilemap_base = if (data.lcdc & 0x08) != 0 {
            0x1C00 // $9C00-$9FFF
        } else {
            0x1800 // $9800-$9BFF
        };

        // Window tilemap base (LCDC bit 6)
        let win_tilemap_base = if (data.lcdc & 0x40) != 0 {
            0x1C00 // $9C00-$9FFF
        } else {
            0x1800 // $9800-$9BFF
        };

        // Render Background tilemap
        let bg_x = rect.min.x;
        let bg_y = rect.min.y + 20.0;
        painter.text(
            egui::Pos2::new(bg_x + tilemap_width / 2.0, rect.min.y + 5.0),
            egui::Align2::CENTER_TOP,
            format!("Background (${:04X})", 0x8000 + bg_tilemap_base),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );

        self.render_gb_tilemap(
            &painter,
            data,
            egui::Pos2::new(bg_x, bg_y),
            tile_size,
            bg_tilemap_base,
            tile_data_select,
        );

        // Render Window tilemap
        let win_x = rect.min.x + tilemap_width + spacing;
        let win_y = bg_y;
        painter.text(
            egui::Pos2::new(win_x + tilemap_width / 2.0, rect.min.y + 5.0),
            egui::Align2::CENTER_TOP,
            format!("Window (${:04X})", 0x8000 + win_tilemap_base),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );

        self.render_gb_tilemap(
            &painter,
            data,
            egui::Pos2::new(win_x, win_y),
            tile_size,
            win_tilemap_base,
            tile_data_select,
        );

        // Draw scroll overlay on background tilemap
        let scroll_rect = egui::Rect::from_min_size(
            egui::Pos2::new(
                bg_x + (data.scx as f32) * scale,
                bg_y + (data.scy as f32) * scale,
            ),
            egui::Vec2::new(160.0 * scale, 144.0 * scale),
        );
        painter.rect_stroke(
            scroll_rect,
            0.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 255, 0)),
            egui::StrokeKind::Inside,
        );

        // Label the viewport
        painter.text(
            scroll_rect.left_top() + egui::Vec2::new(5.0, 5.0),
            egui::Align2::LEFT_TOP,
            "Viewport",
            egui::FontId::proportional(10.0),
            egui::Color32::YELLOW,
        );
    }

    fn render_gb_tilemap(
        &self,
        painter: &egui::Painter,
        data: &GbTileData,
        start_pos: egui::Pos2,
        tile_size: f32,
        tilemap_base: usize,
        tile_data_select: bool,
    ) {
        // Render a 32x32 tilemap
        for tile_y in 0..32 {
            for tile_x in 0..32 {
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = data.vram_bank0[tilemap_addr];

                // Calculate tile data address based on addressing mode
                let tile_addr = if tile_data_select {
                    // Unsigned mode: 0x8000-0x8FFF (0x0000-0x0FFF in VRAM array)
                    (tile_index as usize) * 16
                } else {
                    // Signed mode: 0x8800-0x97FF (0x0800-0x17FF in VRAM array)
                    let signed_index = tile_index as i8;
                    (0x0800_usize).wrapping_add(((signed_index as i16 + 128) as usize) * 16)
                };

                // Get tile attributes from VRAM bank 1 (CGB only)
                let tile_attr = if data.is_cgb_mode {
                    data.vram_bank1[tilemap_addr]
                } else {
                    0
                };

                let palette_num = (tile_attr & 0x07) as usize;
                let flip_x = (tile_attr & 0x20) != 0;
                let flip_y = (tile_attr & 0x40) != 0;

                // Render the tile
                let tile_pos_x = start_pos.x + (tile_x as f32) * tile_size;
                let tile_pos_y = start_pos.y + (tile_y as f32) * tile_size;

                // Draw tile pixels
                for py in 0..8 {
                    let actual_py = if flip_y { 7 - py } else { py };
                    let row_addr = tile_addr + (actual_py * 2);

                    if row_addr + 1 >= data.vram_bank0.len() {
                        continue;
                    }

                    let byte1 = data.vram_bank0[row_addr];
                    let byte2 = data.vram_bank0[row_addr + 1];

                    for px in 0..8 {
                        let actual_px = if flip_x { 7 - px } else { px };
                        let bit = 7 - actual_px;
                        let color_index = (((byte2 >> bit) & 1) << 1) | ((byte1 >> bit) & 1);

                        // Get color from palette
                        let color = if data.is_cgb_mode {
                            let pal_index = (palette_num * 4 + color_index as usize).min(31);
                            data.bg_palettes[pal_index]
                        } else {
                            // DMG mode: use simple grayscale
                            match color_index {
                                0 => 0xFFFFFFFF,
                                1 => 0xFFAAAAAA,
                                2 => 0xFF555555,
                                3 => 0xFF000000,
                                _ => 0xFF000000,
                            }
                        };

                        let pixel_x = tile_pos_x + (px as f32) * (tile_size / 8.0);
                        let pixel_y = tile_pos_y + (py as f32) * (tile_size / 8.0);
                        let pixel_size = tile_size / 8.0;

                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                egui::Pos2::new(pixel_x, pixel_y),
                                egui::Vec2::new(pixel_size, pixel_size),
                            ),
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(
                                ((color >> 16) & 0xFF) as u8,
                                ((color >> 8) & 0xFF) as u8,
                                (color & 0xFF) as u8,
                                255,
                            ),
                        );
                    }
                }

                // Draw tile grid
                let tile_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(tile_pos_x, tile_pos_y),
                    egui::Vec2::new(tile_size, tile_size),
                );
                painter.rect_stroke(
                    tile_rect,
                    0.0,
                    egui::Stroke::new(0.5, egui::Color32::from_gray(64)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// Render SNES tiles from VRAM
    fn render_snes_tiles_with_settings(
        &self,
        ui: &mut Ui,
        data: &SnesTileData,
        bpp_mode: u8,
        _selected_palette: usize,
    ) {
        // SNES VRAM is 64KB organized as character data
        // Display tiles with selected color depth (2bpp, 4bpp, 8bpp)

        let bytes_per_tile = match bpp_mode {
            2 => 16, // 2bpp: 16 bytes per 8x8 tile
            4 => 32, // 4bpp: 32 bytes per 8x8 tile
            8 => 64, // 8bpp: 64 bytes per 8x8 tile
            _ => 32, // Default to 4bpp
        };

        let total_tiles = data.vram.len() / bytes_per_tile;
        let tiles_per_row = 16;
        let tile_rows = total_tiles.div_ceil(tiles_per_row);

        let tile_display_size = 12.0; // Display size in UI

        // Create the tile grid
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(
                tiles_per_row as f32 * tile_display_size,
                tile_rows as f32 * tile_display_size,
            ),
            egui::Sense::hover(),
        );

        let rect = response.rect;
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

        // Draw tiles
        for tile_idx in 0..total_tiles.min(1024) {
            let tile_col = tile_idx % tiles_per_row;
            let tile_row = tile_idx / tiles_per_row;

            let tile_x = rect.min.x + tile_col as f32 * tile_display_size;
            let tile_y = rect.min.y + tile_row as f32 * tile_display_size;
            let tile_rect = egui::Rect::from_min_size(
                egui::Pos2::new(tile_x, tile_y),
                egui::Vec2::new(tile_display_size, tile_display_size),
            );

            // Calculate average brightness for preview
            let vram_offset = tile_idx * bytes_per_tile;
            let mut total_value = 0u32;
            for i in 0..bytes_per_tile {
                if vram_offset + i < data.vram.len() {
                    total_value += data.vram[vram_offset + i].count_ones();
                }
            }
            let max_bits = bytes_per_tile as u32 * 8;
            let brightness = ((total_value as f32 / max_bits as f32) * 200.0) as u8;
            let tile_color = egui::Color32::from_rgb(brightness, brightness, brightness);

            painter.rect_filled(tile_rect, 0.0, tile_color);
            painter.rect_stroke(
                tile_rect,
                0.0,
                egui::Stroke::new(0.3, egui::Color32::from_rgb(60, 60, 60)),
                egui::StrokeKind::Inside,
            );
        }

        // Handle hover tooltip
        if let Some(hover_pos) = response.hover_pos() {
            let rel_x = hover_pos.x - rect.min.x;
            let rel_y = hover_pos.y - rect.min.y;

            if rel_x >= 0.0 && rel_y >= 0.0 {
                let tile_col = (rel_x / tile_display_size) as usize;
                let tile_row = (rel_y / tile_display_size) as usize;

                if tile_col < tiles_per_row && tile_row < tile_rows {
                    let tile_idx = tile_row * tiles_per_row + tile_col;
                    if tile_idx < total_tiles {
                        let vram_addr = tile_idx * bytes_per_tile;

                        response.clone().on_hover_ui(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Tile ${:03X} ({})",
                                    tile_idx, tile_idx
                                ))
                                .strong(),
                            );
                            ui.label(format!(
                                "VRAM Address: ${:04X}-${:04X}",
                                vram_addr,
                                vram_addr + bytes_per_tile - 1
                            ));
                            ui.label(format!("Color Depth: {}bpp", bpp_mode));
                            ui.label(format!("Palette: {}", _selected_palette));
                        });
                    }
                }
            }
        }
    }

    /// Render SNES palette viewer
    pub fn render_snes_palettes(&self, ui: &mut Ui, data: &SnesTileData) {
        // SNES has 256 colors in CGRAM, organized as 16 sub-palettes of 16 colors each
        let colors_per_palette = 16;
        let num_palettes = 16;
        let color_size = 20.0;

        for pal_num in 0..num_palettes {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Palette {:X}:", pal_num))
                        .strong()
                        .monospace(),
                );

                for color_num in 0..colors_per_palette {
                    let color_index = pal_num * colors_per_palette + color_num;
                    if color_index < data.palette.len() {
                        let rgb = data.palette[color_index];
                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;
                        let color = egui::Color32::from_rgb(r, g, b);

                        let (response, painter) = ui.allocate_painter(
                            egui::Vec2::new(color_size, color_size),
                            egui::Sense::hover(),
                        );
                        let color_rect = response.rect;

                        painter.rect_filled(color_rect, 2.0, color);
                        painter.rect_stroke(
                            color_rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::WHITE),
                            egui::StrokeKind::Inside,
                        );

                        // Tooltip with color info
                        response.on_hover_ui(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Palette {} Color {}",
                                    pal_num, color_num
                                ))
                                .strong(),
                            );
                            ui.label(format!(
                                "CGRAM Index: ${:02X} ({})",
                                color_index, color_index
                            ));
                            ui.label(format!("CGRAM Address: ${:03X}", color_index * 2));

                            // Show 15-bit BGR value from CGRAM
                            if color_index * 2 + 1 < data.cgram.len() {
                                let low = data.cgram[color_index * 2] as u16;
                                let high = data.cgram[color_index * 2 + 1] as u16;
                                let bgr15 = low | (high << 8);
                                ui.label(format!("15-bit BGR: ${:04X}", bgr15));
                                ui.label(format!(
                                    "  R: {:02} ({:02X})",
                                    bgr15 & 0x1F,
                                    bgr15 & 0x1F
                                ));
                                ui.label(format!(
                                    "  G: {:02} ({:02X})",
                                    (bgr15 >> 5) & 0x1F,
                                    (bgr15 >> 5) & 0x1F
                                ));
                                ui.label(format!(
                                    "  B: {:02} ({:02X})",
                                    (bgr15 >> 10) & 0x1F,
                                    (bgr15 >> 10) & 0x1F
                                ));
                            }
                            ui.label(format!("24-bit RGB: #{:02X}{:02X}{:02X}", r, g, b));
                        });
                    }
                }
            });

            if pal_num % 4 == 3 && pal_num < num_palettes - 1 {
                ui.add_space(5.0);
            }
        }
    }

    /// Render SNES tilemaps with visible window highlighting
    pub fn render_snes_tilemaps(&self, ui: &mut Ui, data: &SnesTileData) {
        // Header
        ui.heading("📐 SNES Background Layers & Tilemaps");
        ui.separator();
        ui.add_space(5.0);

        // BG Mode information
        let mode_info = match data.bg_mode {
            0 => "Mode 0: 4 layers, 2bpp each (4 colors)",
            1 => "Mode 1: BG1/BG2 4bpp (16 colors), BG3 2bpp (4 colors)",
            2 => "Mode 2: BG1/BG2 4bpp with offset-per-tile",
            3 => "Mode 3: BG1 8bpp (256 colors), BG2 4bpp (16 colors)",
            4 => "Mode 4: BG1 8bpp, BG2 2bpp, offset-per-tile",
            5 => "Mode 5: BG1 4bpp, BG2 2bpp (hi-res 512px)",
            6 => "Mode 6: BG1 4bpp, offset-per-tile (hi-res 512px)",
            7 => "Mode 7: BG1 8bpp (256 colors), rotation/scaling",
            _ => "Unknown mode",
        };

        ui.horizontal(|ui| {
            ui.label(format!("BG Mode: {} - {}", data.bg_mode, mode_info));
            ui.separator();
            ui.label(format!("VRAM: {} KB", data.vram.len() / 1024));
        });

        ui.add_space(10.0);

        // Determine which layers are active based on mode
        let active_layers = match data.bg_mode {
            0 => vec![0, 1, 2, 3], // BG1-4
            1 => vec![0, 1, 2],    // BG1-3
            2 => vec![0, 1],       // BG1-2
            3 => vec![0, 1],       // BG1-2
            4 => vec![0, 1],       // BG1-2
            5 => vec![0, 1],       // BG1-2
            6 => vec![0],          // BG1 only
            7 => vec![0],          // BG1 only (Mode 7)
            _ => vec![],
        };

        // Render each active layer
        for &bg_idx in &active_layers {
            ui.separator();
            ui.add_space(10.0);

            // Determine bits-per-pixel for this layer
            let bpp = match (data.bg_mode, bg_idx) {
                (0, _) => 2,     // Mode 0: all layers 2bpp
                (1, 0..=1) => 4, // Mode 1: BG1-2 are 4bpp
                (1, 2) => 2,     // Mode 1: BG3 is 2bpp
                (2, 0..=1) => 4, // Mode 2: BG1-2 are 4bpp
                (3, 0) => 8,     // Mode 3: BG1 is 8bpp
                (3, 1) => 4,     // Mode 3: BG2 is 4bpp
                (4, 0) => 8,     // Mode 4: BG1 is 8bpp
                (4, 1) => 2,     // Mode 4: BG2 is 2bpp
                (5, 0) => 4,     // Mode 5: BG1 is 4bpp
                (5, 1) => 2,     // Mode 5: BG2 is 2bpp
                (6, 0) => 4,     // Mode 6: BG1 is 4bpp
                (7, 0) => 8,     // Mode 7: BG1 is 8bpp
                _ => 2,
            };

            // Get tilemap configuration for this layer
            let sc_reg = match bg_idx {
                0 => data.bg1sc,
                1 => data.bg2sc,
                2 => data.bg3sc,
                3 => data.bg4sc,
                _ => 0,
            };

            // Get scroll position for this layer
            let (scroll_x, scroll_y) = match bg_idx {
                0 => (data.bg1_hofs, data.bg1_vofs),
                1 => (data.bg2_hofs, data.bg2_vofs),
                2 => (data.bg3_hofs, data.bg3_vofs),
                3 => (data.bg4_hofs, data.bg4_vofs),
                _ => (0, 0),
            };

            // Check if layer is enabled on main screen
            let enabled = (data.tm & (1 << bg_idx)) != 0;

            // Tilemap size from BGxSC bits 0-1
            let size_bits = sc_reg & 0x03;
            let (tilemap_width, tilemap_height) = match size_bits {
                0b00 => (32, 32),
                0b01 => (64, 32),
                0b10 => (32, 64),
                0b11 => (64, 64),
                _ => (32, 32),
            };

            // Tilemap base address from BGxSC bits 2-7
            let tilemap_base = ((sc_reg as usize >> 2) & 0x3F) << 11;

            // CHR base address
            let nba_reg = match bg_idx {
                0 => data.bg12nba & 0x0F,
                1 => (data.bg12nba >> 4) & 0x0F,
                2 => data.bg34nba & 0x0F,
                3 => (data.bg34nba >> 4) & 0x0F,
                _ => 0,
            };
            let chr_base = (nba_reg as usize) << 13;

            // Layer header
            ui.heading(format!(
                "BG{} - {}bpp{}",
                bg_idx + 1,
                bpp,
                if enabled { " (enabled)" } else { " (disabled)" }
            ));
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Tilemap: ${:04X} ({}x{} tiles)",
                    tilemap_base, tilemap_width, tilemap_height
                ));
                ui.separator();
                ui.label(format!("CHR: ${:04X}", chr_base));
                ui.separator();
                ui.label(format!("Scroll: ({}, {})", scroll_x, scroll_y));
            });

            ui.add_space(5.0);

            // Render the tilemap
            self.render_snes_layer_tilemap(
                ui,
                data,
                bg_idx,
                tilemap_base,
                tilemap_width,
                tilemap_height,
                chr_base,
                bpp,
                scroll_x,
                scroll_y,
                enabled,
            );
        }

        ui.add_space(10.0);
    }

    /// Render a single SNES background layer tilemap
    #[allow(clippy::too_many_arguments)]
    fn render_snes_layer_tilemap(
        &self,
        ui: &mut Ui,
        data: &SnesTileData,
        _bg_idx: usize,
        tilemap_base: usize,
        tilemap_width: usize,
        tilemap_height: usize,
        chr_base: usize,
        bpp: usize,
        scroll_x: u16,
        scroll_y: u16,
        enabled: bool,
    ) {
        // Scale factor for rendering (smaller to fit on screen)
        let scale = if tilemap_width > 32 || tilemap_height > 32 {
            0.8
        } else {
            1.2
        };
        let tile_size = 8.0 * scale;

        // Calculate tilemap pixel dimensions
        let tilemap_pixel_width = tilemap_width as f32 * tile_size;
        let tilemap_pixel_height = tilemap_height as f32 * tile_size;

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(tilemap_pixel_width, tilemap_pixel_height + 20.0),
            egui::Sense::hover(),
        );
        let rect = response.rect;

        // Check if we have valid data
        if data.vram.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "VRAM data not available",
                egui::FontId::default(),
                egui::Color32::GRAY,
            );
            return;
        }

        let tilemap_start_y = rect.min.y;

        // Render all tiles in the tilemap
        for tile_y in 0..tilemap_height {
            for tile_x in 0..tilemap_width {
                // Calculate tilemap offset for this tile
                // SNES tilemaps are organized in 32x32 tile blocks (2048 bytes each)
                let block_x = tile_x / 32;
                let block_y = tile_y / 32;
                let in_block_x = tile_x % 32;
                let in_block_y = tile_y % 32;

                // Calculate which block we're in
                let block_index = if tilemap_width == 64 {
                    // 64x32 (or wider) tilemaps: two blocks horizontally
                    block_y * 2 + block_x
                } else if tilemap_height == 64 {
                    // 32x64 (or taller) tilemaps: blocks arranged vertically
                    block_y
                } else {
                    // Other configurations: default to first block
                    0
                };

                let block_offset = block_index * 32 * 32 * 2; // 2048 bytes per block
                let in_block_offset = (in_block_y * 32 + in_block_x) * 2;
                let tilemap_offset = tilemap_base + block_offset + in_block_offset;

                // Read tilemap entry (2 bytes)
                if tilemap_offset + 1 >= data.vram.len() {
                    continue;
                }

                let tile_low = data.vram[tilemap_offset];
                let tile_high = data.vram[tilemap_offset + 1];

                // Parse tilemap entry
                // Format: vhopppcc cccccccc
                // v = vertical flip (bit 15)
                // h = horizontal flip (bit 14)
                // o = priority (bit 13)
                // ppp = palette (bits 12-10)
                // cccccccccc = tile number (bits 9-0)
                let tile_index = (tile_low as usize) | (((tile_high & 0x03) as usize) << 8);
                let palette = ((tile_high >> 2) & 0x07) as usize;
                let flip_x = (tile_high & 0x40) != 0;
                let flip_y = (tile_high & 0x80) != 0;

                // Get tile position on screen
                let tile_screen_x = rect.min.x + tile_x as f32 * tile_size;
                let tile_screen_y = tilemap_start_y + tile_y as f32 * tile_size;

                // Render 8x8 tile pixels
                let chr_addr = chr_base + tile_index * (8 * bpp);

                // Render based on bpp
                match bpp {
                    2 => {
                        self.render_snes_tile_2bpp(
                            &painter,
                            data,
                            chr_addr,
                            tile_screen_x,
                            tile_screen_y,
                            scale,
                            palette,
                            flip_x,
                            flip_y,
                        );
                    }
                    4 => {
                        self.render_snes_tile_4bpp(
                            &painter,
                            data,
                            chr_addr,
                            tile_screen_x,
                            tile_screen_y,
                            scale,
                            palette,
                            flip_x,
                            flip_y,
                        );
                    }
                    8 => {
                        self.render_snes_tile_8bpp(
                            &painter,
                            data,
                            chr_addr,
                            tile_screen_x,
                            tile_screen_y,
                            scale,
                            palette,
                            flip_x,
                            flip_y,
                        );
                    }
                    _ => {}
                }

                // Check if this tile is outside the scroll window and dim it
                if enabled {
                    let tile_pixel_x = tile_x * 8;
                    let tile_pixel_y = tile_y * 8;

                    // Viewport is 256x224 pixels (32x28 tiles)
                    let viewport_width = 256;
                    let viewport_height = 224;

                    // Calculate if tile is visible based on scroll position
                    // SNES scroll coordinates wrap around the tilemap
                    let map_width_px = tilemap_width * 8;
                    let map_height_px = tilemap_height * 8;
                    let scroll_x = scroll_x as usize % map_width_px;
                    let scroll_y = scroll_y as usize % map_height_px;

                    let tile_right = tile_pixel_x + 8;
                    let tile_bottom = tile_pixel_y + 8;
                    let scroll_right = scroll_x + viewport_width;
                    let scroll_bottom = scroll_y + viewport_height;

                    // Check if tile intersects with the scroll window, taking wrapping into account
                    let is_horiz_visible = if scroll_right <= map_width_px {
                        // No horizontal wrap: simple interval intersection
                        !(tile_right <= scroll_x || tile_pixel_x >= scroll_right)
                    } else {
                        // Horizontal wrap: visible region is [scroll_x, map_width_px) ∪ [0, wrapped_scroll_right)
                        let wrapped_scroll_right = scroll_right % map_width_px;
                        let overlaps_right_segment =
                            tile_pixel_x < map_width_px && tile_right > scroll_x;
                        let overlaps_left_segment =
                            tile_pixel_x < wrapped_scroll_right && tile_right > 0;
                        overlaps_right_segment || overlaps_left_segment
                    };

                    let is_vert_visible = if scroll_bottom <= map_height_px {
                        // No vertical wrap: simple interval intersection
                        !(tile_bottom <= scroll_y || tile_pixel_y >= scroll_bottom)
                    } else {
                        // Vertical wrap: visible region is [scroll_y, map_height_px) ∪ [0, wrapped_scroll_bottom)
                        let wrapped_scroll_bottom = scroll_bottom % map_height_px;
                        let overlaps_bottom_segment =
                            tile_pixel_y < map_height_px && tile_bottom > scroll_y;
                        let overlaps_top_segment =
                            tile_pixel_y < wrapped_scroll_bottom && tile_bottom > 0;
                        overlaps_bottom_segment || overlaps_top_segment
                    };

                    let is_visible = is_horiz_visible && is_vert_visible;
                    if !is_visible {
                        // Apply gray overlay to tiles outside the visible area
                        let tile_rect = egui::Rect::from_min_size(
                            egui::Pos2::new(tile_screen_x, tile_screen_y),
                            egui::Vec2::new(tile_size, tile_size),
                        );
                        painter.rect_filled(
                            tile_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
                        );
                    }
                }
            }
        }

        // Highlight the scroll window if layer is enabled
        if enabled {
            let viewport_width = 256.0;
            let viewport_height = 224.0;

            // Calculate scroll position in tilemap space
            let scroll_x_pixel = (scroll_x as f32) % (tilemap_width as f32 * 8.0);
            let scroll_y_pixel = (scroll_y as f32) % (tilemap_height as f32 * 8.0);

            // Convert to screen coordinates
            let scroll_screen_x = rect.min.x + scroll_x_pixel * scale;
            let scroll_screen_y = tilemap_start_y + scroll_y_pixel * scale;

            let viewport_width_scaled = viewport_width * scale;
            let viewport_height_scaled = viewport_height * scale;

            // Check if the window wraps around
            let wraps_x = scroll_x_pixel + viewport_width > tilemap_width as f32 * 8.0;
            let wraps_y = scroll_y_pixel + viewport_height > tilemap_height as f32 * 8.0;

            // Helper function to draw a scroll window rectangle segment
            let draw_scroll_rect = |painter: &egui::Painter, x: f32, y: f32, w: f32, h: f32| {
                let rect = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new(w, h));
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 0, 200)),
                    egui::StrokeKind::Outside,
                );
                painter.rect_filled(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 0, 20),
                );
            };

            // Draw the scroll window, handling wrapping
            match (wraps_x, wraps_y) {
                (false, false) => {
                    // No wrapping - draw single rectangle
                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        scroll_screen_y,
                        viewport_width_scaled,
                        viewport_height_scaled,
                    );
                }
                (true, false) => {
                    // Wraps horizontally
                    let right_width = tilemap_pixel_width - scroll_screen_x + rect.min.x;
                    let left_width = viewport_width_scaled - right_width;
                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        scroll_screen_y,
                        right_width,
                        viewport_height_scaled,
                    );
                    draw_scroll_rect(
                        &painter,
                        rect.min.x,
                        scroll_screen_y,
                        left_width,
                        viewport_height_scaled,
                    );
                }
                (false, true) => {
                    // Wraps vertically
                    let bottom_height = tilemap_pixel_height - scroll_screen_y + tilemap_start_y;
                    let top_height = viewport_height_scaled - bottom_height;
                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        scroll_screen_y,
                        viewport_width_scaled,
                        bottom_height,
                    );
                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        tilemap_start_y,
                        viewport_width_scaled,
                        top_height,
                    );
                }
                (true, true) => {
                    // Wraps both ways - draw 4 rectangles
                    let right_width = tilemap_pixel_width - scroll_screen_x + rect.min.x;
                    let left_width = viewport_width_scaled - right_width;
                    let bottom_height = tilemap_pixel_height - scroll_screen_y + tilemap_start_y;
                    let top_height = viewport_height_scaled - bottom_height;

                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        scroll_screen_y,
                        right_width,
                        bottom_height,
                    );
                    draw_scroll_rect(
                        &painter,
                        rect.min.x,
                        scroll_screen_y,
                        left_width,
                        bottom_height,
                    );
                    draw_scroll_rect(
                        &painter,
                        scroll_screen_x,
                        tilemap_start_y,
                        right_width,
                        top_height,
                    );
                    draw_scroll_rect(
                        &painter,
                        rect.min.x,
                        tilemap_start_y,
                        left_width,
                        top_height,
                    );
                }
            }
        }

        // Draw tilemap border
        let tilemap_rect = egui::Rect::from_min_size(
            egui::Pos2::new(rect.min.x, tilemap_start_y),
            egui::Vec2::new(tilemap_pixel_width, tilemap_pixel_height),
        );
        painter.rect_stroke(
            tilemap_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 120)),
            egui::StrokeKind::Outside,
        );
    }

    /// Render a single 2bpp SNES tile
    #[allow(clippy::too_many_arguments)]
    fn render_snes_tile_2bpp(
        &self,
        painter: &egui::Painter,
        data: &SnesTileData,
        chr_addr: usize,
        screen_x: f32,
        screen_y: f32,
        scale: f32,
        palette: usize,
        flip_x: bool,
        flip_y: bool,
    ) {
        // 2bpp: 2 bitplanes, 16 bytes per tile
        for py in 0..8 {
            let y = if flip_y { 7 - py } else { py };
            let plane0_addr = chr_addr + y * 2;
            let plane1_addr = chr_addr + y * 2 + 1;

            if plane1_addr >= data.vram.len() {
                continue;
            }

            let plane0 = data.vram[plane0_addr];
            let plane1 = data.vram[plane1_addr];

            for px in 0..8 {
                let x = if flip_x { 7 - px } else { px };
                let bit0 = (plane0 >> (7 - x)) & 1;
                let bit1 = (plane1 >> (7 - x)) & 1;
                let color_idx = ((bit1 << 1) | bit0) as usize;

                // Get color from palette
                let pal_offset = palette * 4 + color_idx;
                if pal_offset >= data.palette.len() {
                    continue;
                }

                let rgb = data.palette[pal_offset];
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                let color = egui::Color32::from_rgb(r, g, b);

                let pixel_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(screen_x + px as f32 * scale, screen_y + py as f32 * scale),
                    egui::Vec2::new(scale, scale),
                );
                painter.rect_filled(pixel_rect, 0.0, color);
            }
        }
    }

    /// Render a single 4bpp SNES tile
    #[allow(clippy::too_many_arguments)]
    fn render_snes_tile_4bpp(
        &self,
        painter: &egui::Painter,
        data: &SnesTileData,
        chr_addr: usize,
        screen_x: f32,
        screen_y: f32,
        scale: f32,
        palette: usize,
        flip_x: bool,
        flip_y: bool,
    ) {
        // 4bpp: 4 bitplanes, 32 bytes per tile
        // Layout: 8 rows * 2 bytes (plane 0-1), then 8 rows * 2 bytes (plane 2-3)
        for py in 0..8 {
            let y = if flip_y { 7 - py } else { py };
            let plane0_addr = chr_addr + y * 2;
            let plane1_addr = chr_addr + y * 2 + 1;
            let plane2_addr = chr_addr + 16 + y * 2;
            let plane3_addr = chr_addr + 16 + y * 2 + 1;

            if plane3_addr >= data.vram.len() {
                continue;
            }

            let plane0 = data.vram[plane0_addr];
            let plane1 = data.vram[plane1_addr];
            let plane2 = data.vram[plane2_addr];
            let plane3 = data.vram[plane3_addr];

            for px in 0..8 {
                let x = if flip_x { 7 - px } else { px };
                let bit0 = (plane0 >> (7 - x)) & 1;
                let bit1 = (plane1 >> (7 - x)) & 1;
                let bit2 = (plane2 >> (7 - x)) & 1;
                let bit3 = (plane3 >> (7 - x)) & 1;
                let color_idx = ((bit3 << 3) | (bit2 << 2) | (bit1 << 1) | bit0) as usize;

                // Get color from palette (16 colors per palette for 4bpp)
                let pal_offset = palette * 16 + color_idx;
                if pal_offset >= data.palette.len() {
                    continue;
                }

                let rgb = data.palette[pal_offset];
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                let color = egui::Color32::from_rgb(r, g, b);

                let pixel_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(screen_x + px as f32 * scale, screen_y + py as f32 * scale),
                    egui::Vec2::new(scale, scale),
                );
                painter.rect_filled(pixel_rect, 0.0, color);
            }
        }
    }

    /// Render a single 8bpp SNES tile
    #[allow(clippy::too_many_arguments)]
    fn render_snes_tile_8bpp(
        &self,
        painter: &egui::Painter,
        data: &SnesTileData,
        chr_addr: usize,
        screen_x: f32,
        screen_y: f32,
        scale: f32,
        _palette: usize,
        flip_x: bool,
        flip_y: bool,
    ) {
        // 8bpp: 8 bitplanes, 64 bytes per tile
        // Layout: 8 rows * 2 bytes (plane 0-1), 8 rows * 2 bytes (plane 2-3),
        //         8 rows * 2 bytes (plane 4-5), 8 rows * 2 bytes (plane 6-7)
        for py in 0..8 {
            let y = if flip_y { 7 - py } else { py };
            let plane0_addr = chr_addr + y * 2;
            let plane1_addr = chr_addr + y * 2 + 1;
            let plane2_addr = chr_addr + 16 + y * 2;
            let plane3_addr = chr_addr + 16 + y * 2 + 1;
            let plane4_addr = chr_addr + 32 + y * 2;
            let plane5_addr = chr_addr + 32 + y * 2 + 1;
            let plane6_addr = chr_addr + 48 + y * 2;
            let plane7_addr = chr_addr + 48 + y * 2 + 1;

            if plane7_addr >= data.vram.len() {
                continue;
            }

            let plane0 = data.vram[plane0_addr];
            let plane1 = data.vram[plane1_addr];
            let plane2 = data.vram[plane2_addr];
            let plane3 = data.vram[plane3_addr];
            let plane4 = data.vram[plane4_addr];
            let plane5 = data.vram[plane5_addr];
            let plane6 = data.vram[plane6_addr];
            let plane7 = data.vram[plane7_addr];

            for px in 0..8 {
                let x = if flip_x { 7 - px } else { px };
                let bit0 = (plane0 >> (7 - x)) & 1;
                let bit1 = (plane1 >> (7 - x)) & 1;
                let bit2 = (plane2 >> (7 - x)) & 1;
                let bit3 = (plane3 >> (7 - x)) & 1;
                let bit4 = (plane4 >> (7 - x)) & 1;
                let bit5 = (plane5 >> (7 - x)) & 1;
                let bit6 = (plane6 >> (7 - x)) & 1;
                let bit7 = (plane7 >> (7 - x)) & 1;
                let color_idx = ((bit7 << 7)
                    | (bit6 << 6)
                    | (bit5 << 5)
                    | (bit4 << 4)
                    | (bit3 << 3)
                    | (bit2 << 2)
                    | (bit1 << 1)
                    | bit0) as usize;

                // 8bpp uses all 256 colors directly
                if color_idx >= data.palette.len() {
                    continue;
                }

                let rgb = data.palette[color_idx];
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                let color = egui::Color32::from_rgb(r, g, b);

                let pixel_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(screen_x + px as f32 * scale, screen_y + py as f32 * scale),
                    egui::Vec2::new(scale, scale),
                );
                painter.rect_filled(pixel_rect, 0.0, color);
            }
        }
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
