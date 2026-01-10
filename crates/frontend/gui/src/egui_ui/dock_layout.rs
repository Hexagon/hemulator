//! Docking layout for Inspector panel and Property pane

use egui_dock::{DockState, TabViewer};
use egui::Ui;
use super::tabs::TabManager;
use super::property_pane::PropertyPane;

/// Tabs available in the Inspector dock
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InspectorTab {
    Debug,
    Log,
    Tiles,
}

impl InspectorTab {
    pub fn title(&self) -> &'static str {
        match self {
            InspectorTab::Debug => "🔧 Debug",
            InspectorTab::Log => "📋 Log",
            InspectorTab::Tiles => "🎨 Tiles",
        }
    }
}

/// Tabs available in the Property dock
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PropertyTab {
    Properties,
}

impl PropertyTab {
    pub fn title(&self) -> &'static str {
        "⚙️ Properties"
    }
}

/// State for the docking system
pub struct DockLayout {
    /// Inspector dock state (contains Debug, Log, Tiles tabs)
    pub inspector_state: DockState<InspectorTab>,
    
    /// Property pane dock state
    pub property_state: DockState<PropertyTab>,
    
    /// Whether the inspector dock is visible
    pub inspector_visible: bool,
}

impl DockLayout {
    pub fn new() -> Self {
        // Create inspector dock with 3 tabs always present
        let inspector_state = DockState::new(vec![
            InspectorTab::Debug,
            InspectorTab::Log,
            InspectorTab::Tiles,
        ]);
        
        // Create property pane dock with a single Properties tab
        let property_state = DockState::new(vec![PropertyTab::Properties]);
        
        Self {
            inspector_state,
            property_state,
            inspector_visible: false, // Hidden by default
        }
    }
    
    pub fn toggle_inspector(&mut self) {
        self.inspector_visible = !self.inspector_visible;
    }
    
    pub fn show_inspector(&mut self) {
        self.inspector_visible = true;
    }
    
    pub fn hide_inspector(&mut self) {
        self.inspector_visible = false;
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// TabViewer implementation for Inspector tabs
pub struct InspectorTabViewer<'a> {
    pub tab_manager: &'a mut TabManager,
}

impl<'a> TabViewer for InspectorTabViewer<'a> {
    type Tab = InspectorTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            InspectorTab::Debug => {
                self.tab_manager.render_debug_tab(ui);
            }
            InspectorTab::Log => {
                self.tab_manager.render_log_tab(ui);
            }
            InspectorTab::Tiles => {
                self.tab_manager.render_tiles_tab(ui);
            }
        }
    }
    
    // Prevent closing tabs - they're always visible
    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }
}

/// TabViewer implementation for Property pane
pub struct PropertyTabViewer<'a> {
    pub property_pane: &'a mut PropertyPane,
}

impl<'a> TabViewer for PropertyTabViewer<'a> {
    type Tab = PropertyTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut Ui, _tab: &mut Self::Tab) {
        self.property_pane.ui(ui);
    }
    
    // Prevent closing the property pane tab
    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }
}
