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
use super::tabs::{PcConfigInfo, SystemTileData};

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
    ui.heading("🎨 Tile/Pattern Viewer");
    ui.separator();
    
    if system_tile_data.is_some() {
        // TODO: Add tile viewer implementation
        ui.label("Tile viewer (implementation in progress)");
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

impl Default for InspectorWindow {
    fn default() -> Self {
        Self::new()
    }
}
