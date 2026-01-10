//! Inspector window for logging and debugging features
//!
//! This module implements a separate native window (the Inspector) for debug tools including:
//! - Log viewer with category filtering
//! - CPU state and debugger controls
//! - Memory viewer
//! - Tile/Pattern viewer
//! - Palette viewer

use crate::system_adapter::EnhancedDebugState;
use egui::{Context, ScrollArea, Ui, ViewportBuilder, ViewportId};

use super::property_pane::PcBdaValues;
use super::tabs::{NesTileData, PcConfigInfo, SystemTileData, TileViewerData};

/// Inspector window state and configuration
pub struct InspectorWindow {
    /// Whether the inspector window is open
    pub is_open: bool,
    
    /// The viewport ID for the inspector window
    viewport_id: ViewportId,
    
    /// Log messages
    log_messages: Vec<String>,
    
    /// Enhanced debug state (CPU, memory, disassembly)
    enhanced_debug_state: Option<EnhancedDebugState>,
    
    /// Tile viewer data
    system_tile_data: Option<SystemTileData>,
    
    /// Current system name (e.g., "NES", "PC", "Game Boy")
    current_system: Option<String>,
    
    /// PC-specific BDA values (only populated for PC system)
    pc_bda_values: Option<PcBdaValues>,
    
    /// PC-specific configuration info
    pc_config_info: Option<PcConfigInfo>,
}

/// Inspector window tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Log,
    Debugger,
    Memory,
    Tiles,
    // System-specific tabs
    PcBda,      // PC BIOS Data Area
    PcConfig,   // PC Configuration (CPU, memory, video, boot)
    NesStatus,  // NES PPU/APU status
    GbStatus,   // Game Boy LCD/APU status
    SmsStatus,  // SMS VDP status
    SnesStatus, // SNES PPU status
}

impl InspectorWindow {
    pub fn new() -> Self {
        Self {
            is_open: false,
            viewport_id: ViewportId::from_hash_of("inspector_window"),
            log_messages: Vec::new(),
            enhanced_debug_state: None,
            system_tile_data: None,
            current_system: None,
            pc_bda_values: None,
            pc_config_info: None,
        }
    }
    
    /// Toggle inspector window open/closed
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
    
    /// Set inspector window open state
    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }
    
    /// Add a log message
    pub fn add_log(&mut self, message: String) {
        self.log_messages.push(message);
        // Keep only last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.remove(0);
        }
    }
    
    /// Update enhanced debug state
    pub fn update_enhanced_debug_state(&mut self, state: EnhancedDebugState) {
        self.enhanced_debug_state = Some(state);
    }
    
    /// Update tile viewer data
    pub fn update_tile_data(&mut self, data: SystemTileData) {
        self.system_tile_data = Some(data);
    }
    
    /// Set current system name
    pub fn set_system(&mut self, system_name: String) {
        self.current_system = Some(system_name);
    }
    
    /// Update PC BDA values
    pub fn update_pc_bda(&mut self, bda: PcBdaValues) {
        self.pc_bda_values = Some(bda);
    }
    
    /// Update PC config info
    pub fn update_pc_config(&mut self, config: PcConfigInfo) {
        self.pc_config_info = Some(config);
    }
    
    /// Show the inspector window (call this from main UI update)
    pub fn show(&mut self, ctx: &Context) {
        if !self.is_open {
            return;
        }
        
        // Clone necessary data for the closure
        let log_messages = self.log_messages.clone();
        let enhanced_debug_state = self.enhanced_debug_state.clone();
        let system_tile_data = self.system_tile_data.clone();
        let current_system = self.current_system.clone();
        let pc_bda_values = self.pc_bda_values.clone();
        let pc_config_info = self.pc_config_info.clone();
        let viewport_id = self.viewport_id;
        
        // Create a separate viewport for the inspector window
        ctx.show_viewport_deferred(
            viewport_id,
            ViewportBuilder::default()
                .with_title("Hemulator Inspector")
                .with_inner_size([900.0, 700.0])
                .with_resizable(true),
            move |ctx, _class| {
                // Render the inspector window content
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Use persistent UI state for active tab
                    let mut active_tab = ctx.data_mut(|d| {
                        d.get_persisted::<InspectorTab>(egui::Id::new("inspector_active_tab"))
                            .unwrap_or(InspectorTab::Log)
                    });
                    
                    // Tab bar with system-specific tabs
                    ui.horizontal(|ui| {
                        // Always visible tabs
                        ui.selectable_value(&mut active_tab, InspectorTab::Log, "📋 Log");
                        ui.selectable_value(&mut active_tab, InspectorTab::Debugger, "🔧 Debugger");
                        ui.selectable_value(&mut active_tab, InspectorTab::Memory, "💾 Memory");
                        ui.selectable_value(&mut active_tab, InspectorTab::Tiles, "🎨 Tiles");
                        
                        // System-specific tabs
                        if let Some(ref system) = current_system {
                            ui.separator();
                            match system.as_str() {
                                "PC" => {
                                    ui.selectable_value(&mut active_tab, InspectorTab::PcBda, "🖥️ BDA");
                                    ui.selectable_value(&mut active_tab, InspectorTab::PcConfig, "⚙️ Config");
                                }
                                "NES" => {
                                    ui.selectable_value(&mut active_tab, InspectorTab::NesStatus, "🎮 PPU/APU");
                                }
                                "Game Boy" => {
                                    ui.selectable_value(&mut active_tab, InspectorTab::GbStatus, "🎮 LCD/APU");
                                }
                                "SMS" => {
                                    ui.selectable_value(&mut active_tab, InspectorTab::SmsStatus, "🎮 VDP");
                                }
                                "SNES" => {
                                    ui.selectable_value(&mut active_tab, InspectorTab::SnesStatus, "🎮 PPU");
                                }
                                _ => {}
                            }
                        }
                    });
                    
                    ui.separator();
                    
                    // Tab content
                    match active_tab {
                        InspectorTab::Log => render_log_tab(ui, &log_messages),
                        InspectorTab::Debugger => render_debugger_tab(ui, &enhanced_debug_state),
                        InspectorTab::Memory => render_memory_tab(ui),
                        InspectorTab::Tiles => render_tiles_tab(ui, &system_tile_data),
                        InspectorTab::PcBda => render_pc_bda_tab(ui, &pc_bda_values),
                        InspectorTab::PcConfig => render_pc_config_tab(ui, &pc_config_info),
                        InspectorTab::NesStatus => render_nes_status_tab(ui, &system_tile_data),
                        InspectorTab::GbStatus => render_gb_status_tab(ui, &system_tile_data),
                        InspectorTab::SmsStatus => render_sms_status_tab(ui, &system_tile_data),
                        InspectorTab::SnesStatus => render_snes_status_tab(ui, &system_tile_data),
                    }
                    
                    // Persist active tab
                    ctx.data_mut(|d| {
                        d.insert_persisted(egui::Id::new("inspector_active_tab"), active_tab);
                    });
                });
            },
        );
        
        // Handle window close after rendering
        if ctx.input(|i| i.raw.viewports.get(&self.viewport_id).map_or(false, |v| v.close_requested())) {
            self.is_open = false;
        }
    }
}

/// Render log tab
fn render_log_tab(ui: &mut Ui, log_messages: &[String]) {
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("Log Messages");
            ui.add_space(10.0);
            
            // Get log configuration
            let log_config = emu_core::logging::LogConfig::global();
            
            // Global log level control
            ui.horizontal(|ui| {
                ui.label("Global Level:");
                let mut global_level = log_config.get_global_level();
                egui::ComboBox::from_id_salt("global_log_level")
                    .selected_text(format!("{:?}", global_level))
                    .show_ui(ui, |ui| {
                        for level in [
                            emu_core::logging::LogLevel::Off,
                            emu_core::logging::LogLevel::Error,
                            emu_core::logging::LogLevel::Warn,
                            emu_core::logging::LogLevel::Info,
                            emu_core::logging::LogLevel::Debug,
                            emu_core::logging::LogLevel::Trace,
                        ] {
                            if ui
                                .selectable_value(&mut global_level, level, format!("{:?}", level))
                                .clicked()
                            {
                                log_config.set_global_level(level);
                            }
                        }
                    });
            });
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Category-specific log levels
            ui.heading("Category Levels");
            ui.add_space(5.0);
            
            for (category, name) in [
                (emu_core::logging::LogCategory::CPU, "CPU"),
                (emu_core::logging::LogCategory::Bus, "Bus"),
                (emu_core::logging::LogCategory::PPU, "PPU"),
                (emu_core::logging::LogCategory::APU, "APU"),
                (emu_core::logging::LogCategory::Interrupts, "Interrupts"),
                (emu_core::logging::LogCategory::Stubs, "Stubs"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", name));
                    let mut category_level = log_config.get_level(category);
                    egui::ComboBox::from_id_salt(format!("log_level_{:?}", category))
                        .selected_text(format!("{:?}", category_level))
                        .show_ui(ui, |ui| {
                            for level in [
                                emu_core::logging::LogLevel::Off,
                                emu_core::logging::LogLevel::Error,
                                emu_core::logging::LogLevel::Warn,
                                emu_core::logging::LogLevel::Info,
                                emu_core::logging::LogLevel::Debug,
                                emu_core::logging::LogLevel::Trace,
                            ] {
                                if ui
                                    .selectable_value(&mut category_level, level, format!("{:?}", level))
                                    .clicked()
                                {
                                    log_config.set_level(category, level);
                                }
                            }
                        });
                });
            }
            
            ui.add_space(15.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Legacy log messages section
            if !log_messages.is_empty() {
                ui.heading("Application Messages");
                ui.add_space(5.0);
                
                for msg in log_messages {
                    ui.label(msg);
                }
            }
        });
}

/// Render debugger tab (CPU state, disassembly, controls)
fn render_debugger_tab(ui: &mut Ui, enhanced_debug_state: &Option<EnhancedDebugState>) {
    if let Some(ref state) = enhanced_debug_state {
        // Header
        ui.heading(format!("🔧 {} Debugger", state.system_type));
        ui.separator();
        
        // TODO: Add debugger controls (pause, resume, step)
        // TODO: Add disassembly view
        // TODO: Add CPU state view
        
        ui.label("Debugger view (implementation in progress)");
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🔧").size(48.0));
            ui.add_space(10.0);
            ui.heading("No Debug Information Available");
            ui.add_space(10.0);
            ui.label("Load a ROM to see system-specific debug information");
        });
    }
}

/// Render memory tab
fn render_memory_tab(ui: &mut Ui) {
    ui.heading("💾 Memory Viewer");
    ui.separator();
    
    // TODO: Add memory viewer implementation
    ui.label("Memory viewer (implementation in progress)");
}

/// Render tiles tab
fn render_tiles_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    let available_height = ui.available_height();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(ref sys_data) = system_tile_data {
                match sys_data {
                    SystemTileData::NES(nes_data) => {
                        render_nes_tile_viewer(ui, nes_data);
                    }
                    SystemTileData::GameBoy(gb_data) => {
                        ui.heading("🎮 Game Boy Tile Viewer");
                        ui.separator();
                        ui.label(format!("LCDC: ${:02X}", gb_data.lcdc));
                        ui.label(format!(
                            "Mode: {}",
                            if gb_data.is_cgb_mode { "Game Boy Color" } else { "Game Boy" }
                        ));
                        ui.label(format!(
                            "Scroll: ({}, {}) Window: ({}, {})",
                            gb_data.scx, gb_data.scy, gb_data.wx, gb_data.wy
                        ));
                        ui.label(format!("VRAM Bank 0: {} KB", gb_data.vram_bank0.len() / 1024));
                        ui.label(format!("VRAM Bank 1: {} KB", gb_data.vram_bank1.len() / 1024));
                        ui.label(format!("OAM: {} bytes (40 sprites)", gb_data.oam.len()));
                    }
                    SystemTileData::SMS(sms_data) => {
                        ui.heading("🎮 SMS Tile Viewer");
                        ui.separator();
                        ui.label(format!("VRAM: {} KB", sms_data.vram.len() / 1024));
                        ui.label(format!("CRAM: {} bytes", sms_data.cram.len()));
                        ui.label(format!("Palette: {} colors", sms_data.palette.len()));
                    }
                    SystemTileData::SNES(snes_data) => {
                        ui.heading("🎮 SNES Tile Viewer");
                        ui.separator();
                        ui.label(format!("BG Mode: {}", snes_data.bg_mode));
                        ui.label(format!(
                            "Screen: {}",
                            if snes_data.screen_enabled {
                                "Enabled"
                            } else {
                                "Disabled"
                            }
                        ));
                        ui.label(format!("VRAM: {} KB", snes_data.vram.len() / 1024));
                        ui.label(format!("CGRAM: {} bytes", snes_data.cgram.len()));
                        ui.label(format!("OAM: {} bytes", snes_data.oam.len()));
                    }
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("🎨").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("No Tile Data Available");
                    ui.add_space(10.0);
                    ui.label("Load a ROM with graphics to view tiles and palettes");
                });
            }
        });
}

/// Render full NES tile viewer with pattern tables, palettes, sprites, and nametables
fn render_nes_tile_viewer(ui: &mut Ui, nes_data: &NesTileData) {
    // Convert NesTileData to TileViewerData for compatibility with existing rendering functions
    let data = TileViewerData {
        chr_data: nes_data.chr_data.clone(),
        palette: nes_data.palette.clone(),
        master_palette: nes_data.master_palette.clone(),
        oam: nes_data.oam.clone(),
        vram: nes_data.vram.clone(),
        chr_is_ram: nes_data.chr_is_ram,
        ppuctrl: nes_data.ppuctrl,
        ppumask: nes_data.ppumask,
        scroll_x: nes_data.scroll_x,
        scroll_y: nes_data.scroll_y,
        mirroring: nes_data.mirroring.clone(),
    };
    
    ui.heading("🎨 NES Tile & Palette Viewer");
    ui.separator();
    
    // PPU state summary
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("PPUCTRL: ${:02X}", data.ppuctrl)).monospace());
        ui.separator();
        ui.label(egui::RichText::new(format!("PPUMASK: ${:02X}", data.ppumask)).monospace());
        ui.separator();
        ui.label(egui::RichText::new(format!("Scroll: ({}, {})", data.scroll_x, data.scroll_y)).monospace());
        ui.separator();
        ui.label(egui::RichText::new(format!("Mirror: {}", data.mirroring)).monospace());
    });
    
    ui.add_space(5.0);
    
    // CHR type indicator
    let chr_type = if data.chr_is_ram { "CHR-RAM" } else { "CHR-ROM" };
    let chr_size = data.chr_data.len();
    ui.label(format!("{} ({} bytes / {} KB)", chr_type, chr_size, chr_size / 1024));
    
    ui.add_space(10.0);
    
    // Pattern Tables section
    ui.heading("Pattern Tables");
    ui.separator();
    
    ui.horizontal(|ui| {
        // Pattern Table 0
        ui.vertical(|ui| {
            let bg_table = (data.ppuctrl & 0x10) != 0;
            let label = if !bg_table { "◄ BG" } else { "" };
            ui.label(format!("Pattern Table 0 (CHR $0000-$0FFF) {}", label));
            render_pattern_table(ui, &data, 0);
        });
        
        ui.add_space(20.0);
        
        // Pattern Table 1
        ui.vertical(|ui| {
            let bg_table = (data.ppuctrl & 0x10) != 0;
            let label = if bg_table { "◄ BG" } else { "" };
            ui.label(format!("Pattern Table 1 (CHR $1000-$1FFF) {}", label));
            render_pattern_table(ui, &data, 1);
        });
    });
    
    ui.add_space(15.0);
    
    // Palettes section
    ui.heading("Palettes");
    ui.separator();
    
    ui.label("Background Palettes ($3F00-$3F0F):");
    render_palettes(ui, &data, 0);
    
    ui.add_space(5.0);
    
    ui.label("Sprite Palettes ($3F10-$3F1F):");
    render_palettes(ui, &data, 4);
    
    ui.add_space(15.0);
    
    // Sprites (OAM) section
    ui.heading("Sprites (OAM)");
    ui.separator();
    
    let sprite_size = if (data.ppuctrl & 0x20) != 0 { "8x16" } else { "8x8" };
    let sprite_table = if (data.ppuctrl & 0x08) != 0 { 1 } else { 0 };
    
    ui.horizontal(|ui| {
        ui.label(format!("Sprite Size: {}", sprite_size));
        ui.separator();
        if sprite_size == "8x8" {
            ui.label(format!("Pattern Table: {} (CHR ${:04X})", sprite_table, sprite_table * 0x1000));
        } else {
            ui.label("Pattern Table: Per-sprite (tile bit 0)");
        }
        ui.separator();
        ui.label(format!("OAM: {} bytes", data.oam.len()));
    });
    
    ui.add_space(5.0);
    render_sprites(ui, &data);
}

/// Render PC BDA (BIOS Data Area) tab
fn render_pc_bda_tab(ui: &mut Ui, pc_bda_values: &Option<PcBdaValues>) {
    ui.heading("🖥️ PC BIOS Data Area (BDA)");
    ui.separator();
    
    if let Some(bda) = pc_bda_values {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.label("The BIOS Data Area (BDA) is a memory region at 0x0400-0x04FF that stores");
                ui.label("hardware configuration and status information set by the BIOS.");
                ui.add_space(15.0);
                
                egui::Grid::new("bda_grid")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Equipment Word:").strong());
                        ui.label(format!("{:04X}h (binary: {:016b})", bda.equipment_word, bda.equipment_word));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Memory Size:").strong());
                        ui.label(format!("{} KB", bda.memory_size_kb));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Video Mode:").strong());
                        ui.label(format!("{:02X}h (mode {})", bda.video_mode, bda.video_mode));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Video Columns:").strong());
                        ui.label(format!("{} columns", bda.video_columns));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Serial Ports:").strong());
                        ui.label(format!("{}", bda.num_serial_ports));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Parallel Ports:").strong());
                        ui.label(format!("{}", bda.num_parallel_ports));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Hard Drives:").strong());
                        ui.label(format!("{}", bda.num_hard_drives));
                        ui.end_row();
                    });
                
                ui.add_space(20.0);
                ui.heading("Equipment Word Breakdown");
                ui.add_space(5.0);
                ui.label("Bit flags in the equipment word:");
                ui.add_space(5.0);
                
                egui::Grid::new("equipment_bits")
                    .num_columns(3)
                    .spacing([10.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Bit").strong());
                        ui.label(egui::RichText::new("Value").strong());
                        ui.label(egui::RichText::new("Meaning").strong());
                        ui.end_row();
                        
                        for bit in 0..16 {
                            let value = (bda.equipment_word >> bit) & 1;
                            let meaning = match bit {
                                0 => "Floppy disk installed",
                                1 => "Math coprocessor",
                                2..=3 => "System RAM size",
                                4..=5 => "Initial video mode",
                                6..=7 => "Number of floppy drives",
                                8 => "DMA chip",
                                9..=11 => "Number of serial ports",
                                12 => "Game port",
                                13 => "Serial printer",
                                14..=15 => "Number of parallel ports",
                                _ => "Reserved",
                            };
                            
                            ui.label(format!("{}", bit));
                            ui.label(format!("{}", value));
                            ui.label(meaning);
                            ui.end_row();
                        }
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🖥️").size(48.0));
            ui.add_space(10.0);
            ui.heading("BDA Data Not Available");
            ui.add_space(10.0);
            ui.label("Load a PC/DOS ROM to view BIOS Data Area information");
        });
    }
}

/// Render PC Configuration tab
fn render_pc_config_tab(ui: &mut Ui, pc_config_info: &Option<PcConfigInfo>) {
    ui.heading("⚙️ PC System Configuration");
    ui.separator();
    
    if let Some(config) = pc_config_info {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                
                // CPU Section
                ui.heading("Processor");
                ui.add_space(5.0);
                egui::Grid::new("cpu_grid")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("CPU Model:").strong());
                        ui.label(&config.cpu_model);
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Memory:").strong());
                        ui.label(format!("{} KB", config.memory_kb));
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                // Video Section
                ui.heading("Video");
                ui.add_space(5.0);
                egui::Grid::new("video_grid")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Video Adapter:").strong());
                        ui.label(&config.video_adapter);
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                // Boot Configuration
                ui.heading("Boot Configuration");
                ui.add_space(5.0);
                egui::Grid::new("boot_grid")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Boot Priority:").strong());
                        ui.label(&config.boot_priority);
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                // Storage Devices
                ui.heading("Storage Devices");
                ui.add_space(5.0);
                egui::Grid::new("storage_grid")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("BIOS:").strong());
                        ui.label(if config.bios_mounted { "✓ Mounted" } else { "✗ Not mounted" });
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Floppy A:").strong());
                        ui.label(if config.floppy_a_mounted { "✓ Mounted" } else { "✗ Not mounted" });
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Floppy B:").strong());
                        ui.label(if config.floppy_b_mounted { "✓ Mounted" } else { "✗ Not mounted" });
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Hard Drive:").strong());
                        ui.label(if config.hdd_mounted { "✓ Mounted" } else { "✗ Not mounted" });
                        ui.end_row();
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("⚙️").size(48.0));
            ui.add_space(10.0);
            ui.heading("PC Configuration Not Available");
            ui.add_space(10.0);
            ui.label("Load a PC/DOS ROM to view system configuration");
        });
    }
}

/// Render NES-specific status tab
fn render_nes_status_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    ui.heading("🎮 NES PPU/APU Status");
    ui.separator();
    
    if let Some(SystemTileData::NES(ref nes_data)) = system_tile_data {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                
                ui.heading("PPU Control & Mask");
                ui.add_space(5.0);
                egui::Grid::new("ppu_control")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("PPUCTRL:").strong());
                        ui.label(format!("{:02X}h", nes_data.ppuctrl));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("PPUMASK:").strong());
                        ui.label(format!("{:02X}h", nes_data.ppumask));
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                ui.heading("Scroll Position");
                ui.add_space(5.0);
                egui::Grid::new("ppu_scroll")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Scroll X:").strong());
                        ui.label(format!("{}", nes_data.scroll_x));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Scroll Y:").strong());
                        ui.label(format!("{}", nes_data.scroll_y));
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                ui.heading("Memory Configuration");
                ui.add_space(5.0);
                egui::Grid::new("ppu_memory")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Mirroring:").strong());
                        ui.label(&nes_data.mirroring);
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("CHR Type:").strong());
                        ui.label(if nes_data.chr_is_ram { "CHR-RAM" } else { "CHR-ROM" });
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("CHR Size:").strong());
                        ui.label(format!("{} bytes", nes_data.chr_data.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("VRAM Size:").strong());
                        ui.label(format!("{} bytes", nes_data.vram.len()));
                        ui.end_row();
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎮").size(48.0));
            ui.add_space(10.0);
            ui.heading("NES Status Not Available");
            ui.add_space(10.0);
            ui.label("Load an NES ROM to view PPU/APU status");
        });
    }
}

/// Render Game Boy-specific status tab
fn render_gb_status_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    ui.heading("🎮 Game Boy LCD/APU Status");
    ui.separator();
    
    if let Some(SystemTileData::GameBoy(ref gb_data)) = system_tile_data {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                
                ui.heading("LCD Control");
                ui.add_space(5.0);
                egui::Grid::new("lcd_control")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("LCDC:").strong());
                        ui.label(format!("{:02X}h", gb_data.lcdc));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Mode:").strong());
                        ui.label(if gb_data.is_cgb_mode { "Game Boy Color" } else { "Original Game Boy" });
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                ui.heading("Scroll & Window");
                ui.add_space(5.0);
                egui::Grid::new("scroll_window")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("SCX (Scroll X):").strong());
                        ui.label(format!("{}", gb_data.scx));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("SCY (Scroll Y):").strong());
                        ui.label(format!("{}", gb_data.scy));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("WX (Window X):").strong());
                        ui.label(format!("{}", gb_data.wx));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("WY (Window Y):").strong());
                        ui.label(format!("{}", gb_data.wy));
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                ui.heading("Memory");
                ui.add_space(5.0);
                egui::Grid::new("memory")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("VRAM Bank 0:").strong());
                        ui.label(format!("{} bytes", gb_data.vram_bank0.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("VRAM Bank 1:").strong());
                        ui.label(format!("{} bytes", gb_data.vram_bank1.len()));
                        ui.end_row();
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎮").size(48.0));
            ui.add_space(10.0);
            ui.heading("Game Boy Status Not Available");
            ui.add_space(10.0);
            ui.label("Load a Game Boy ROM to view LCD/APU status");
        });
    }
}

/// Render SMS-specific status tab
fn render_sms_status_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    ui.heading("🎮 SMS VDP Status");
    ui.separator();
    
    if let Some(SystemTileData::SMS(ref sms_data)) = system_tile_data {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                
                ui.heading("VDP Memory");
                ui.add_space(5.0);
                egui::Grid::new("vdp_memory")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("VRAM Size:").strong());
                        ui.label(format!("{} bytes", sms_data.vram.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("CRAM Size:").strong());
                        ui.label(format!("{} bytes", sms_data.cram.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Registers:").strong());
                        ui.label(format!("{} registers", sms_data.registers.len()));
                        ui.end_row();
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎮").size(48.0));
            ui.add_space(10.0);
            ui.heading("SMS Status Not Available");
            ui.add_space(10.0);
            ui.label("Load a Sega Master System ROM to view VDP status");
        });
    }
}

/// Render SNES-specific status tab
fn render_snes_status_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    ui.heading("🎮 SNES PPU Status");
    ui.separator();
    
    if let Some(SystemTileData::SNES(ref snes_data)) = system_tile_data {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                
                ui.heading("PPU Configuration");
                ui.add_space(5.0);
                egui::Grid::new("ppu_config")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("BG Mode:").strong());
                        ui.label(format!("Mode {}", snes_data.bg_mode));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("Screen:").strong());
                        ui.label(if snes_data.screen_enabled { "Enabled" } else { "Disabled" });
                        ui.end_row();
                    });
                
                ui.add_space(15.0);
                
                ui.heading("Memory");
                ui.add_space(5.0);
                egui::Grid::new("ppu_memory")
                    .num_columns(2)
                    .spacing([15.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("VRAM Size:").strong());
                        ui.label(format!("{} bytes", snes_data.vram.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("CGRAM Size:").strong());
                        ui.label(format!("{} bytes", snes_data.cgram.len()));
                        ui.end_row();
                        
                        ui.label(egui::RichText::new("OAM Size:").strong());
                        ui.label(format!("{} bytes", snes_data.oam.len()));
                        ui.end_row();
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎮").size(48.0));
            ui.add_space(10.0);
            ui.heading("SNES Status Not Available");
            ui.add_space(10.0);
            ui.label("Load a SNES ROM to view PPU status");
        });
    }
}

/// Helper function to render a pattern table (256 tiles in 16x16 grid)
fn render_pattern_table(ui: &mut Ui, data: &TileViewerData, table_num: usize) {
    let base_addr = table_num * 0x1000;
    let tile_size = 10.0;
    
    let (response, painter) = ui.allocate_painter(
        egui::Vec2::new(16.0 * tile_size, 16.0 * tile_size),
        egui::Sense::hover(),
    );
    
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
    
    // Draw each tile
    for tile_row in 0..16 {
        for tile_col in 0..16 {
            let tile_index = tile_row * 16 + tile_col;
            let chr_addr = base_addr + tile_index * 16;
            
            let tile_x = rect.min.x + tile_col as f32 * tile_size;
            let tile_y = rect.min.y + tile_row as f32 * tile_size;
            let tile_rect = egui::Rect::from_min_size(
                egui::Pos2::new(tile_x, tile_y),
                egui::Vec2::new(tile_size, tile_size),
            );
            
            // Calculate brightness for preview
            let mut total_value = 0u32;
            for byte_idx in 0..16 {
                if chr_addr + byte_idx < data.chr_data.len() {
                    total_value += data.chr_data[chr_addr + byte_idx].count_ones();
                }
            }
            let brightness = ((total_value as f32 / 128.0) * 180.0) as u8;
            let tile_color = egui::Color32::from_rgb(brightness, brightness, brightness);
            
            painter.rect_filled(tile_rect, 0.0, tile_color);
            painter.rect_stroke(
                tile_rect,
                0.0,
                egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 60)),
                egui::StrokeKind::Inside,
            );
        }
    }
    
    // Tooltip on hover
    if let Some(hover_pos) = response.hover_pos() {
        let rel_x = hover_pos.x - rect.min.x;
        let rel_y = hover_pos.y - rect.min.y;
        
        if rel_x >= 0.0 && rel_y >= 0.0 {
            let tile_col = (rel_x / tile_size) as usize;
            let tile_row = (rel_y / tile_size) as usize;
            
            if tile_col < 16 && tile_row < 16 {
                let tile_index = tile_row * 16 + tile_col;
                let chr_addr = base_addr + tile_index * 16;
                
                response.clone().on_hover_ui(|ui| {
                    ui.label(egui::RichText::new(format!("Tile ${:02X} ({})", tile_index, tile_index)).strong());
                    ui.label(format!("CHR Address: ${:04X}-${:04X}", chr_addr, chr_addr + 15));
                    ui.label(format!("Pattern Table: {}", table_num));
                    ui.label(format!("Row: {}, Col: {}", tile_row, tile_col));
                });
            }
        }
    }
}

/// Helper function to render palettes (4 palettes of 4 colors each)
fn render_palettes(ui: &mut Ui, data: &TileViewerData, start_palette: usize) {
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
                            0xFF000000
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
                            ui.label(egui::RichText::new(format!("Palette {} Color {}", palette_index, color_num)).strong());
                            ui.label(format!("Address: ${:04X}", pal_addr + color_num));
                            ui.label(format!("NES Color Index: ${:02X} ({})", color_index, color_index));
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

/// Helper function to render sprites (OAM data)
fn render_sprites(ui: &mut Ui, data: &TileViewerData) {
    if data.oam.len() < 256 {
        ui.label(egui::RichText::new("OAM data not available").weak());
        return;
    }
    
    let sprite_size_8x16 = (data.ppuctrl & 0x20) != 0;
    let sprite_pattern_table = if (data.ppuctrl & 0x08) != 0 { 1 } else { 0 };
    let tile_height = if sprite_size_8x16 { 16 } else { 8 };
    
    let scale = 2.0;
    let cell_width = 8.0 * scale + 6.0;
    let cell_height = tile_height as f32 * scale + 14.0;
    let grid_cols = 16;
    let grid_rows = 4;
    
    let (response, painter) = ui.allocate_painter(
        egui::Vec2::new(
            grid_cols as f32 * cell_width,
            grid_rows as f32 * cell_height,
        ),
        egui::Sense::hover(),
    );
    
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 40));
    
    // Draw each sprite
    for sprite_idx in 0..64 {
        let oam_offset = sprite_idx * 4;
        let y_pos = data.oam[oam_offset];
        let tile_idx = data.oam[oam_offset + 1];
        let attributes = data.oam[oam_offset + 2];
        
        let col = sprite_idx % grid_cols;
        let row = sprite_idx / grid_cols;
        
        let cell_x = rect.min.x + col as f32 * cell_width + 3.0;
        let cell_y = rect.min.y + row as f32 * cell_height + 2.0;
        
        let is_visible = if sprite_size_8x16 { y_pos < 0xE7 } else { y_pos < 0xEF };
        
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
        
        // Draw sprite index
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
        
        // Get palette and flip flags
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
        
        // Draw sprite pixels
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
                    
                    if color_idx == 0 {
                        continue; // Transparent
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
                    
                    let color = if is_visible {
                        egui::Color32::from_rgb(r, g, b)
                    } else {
                        egui::Color32::from_rgb(r / 2, g / 2, b / 2)
                    };
                    
                    // Apply flipping
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
    }
}

impl Default for InspectorWindow {
    fn default() -> Self {
        Self::new()
    }
}
