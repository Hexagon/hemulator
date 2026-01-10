//! Docking layout for Inspector panel and Property pane

use super::inspector_tabs::{get_tabs_for_system, render_inspector_tab, InspectorTab};
use super::property_pane::PropertyPane;
use super::tabs::TabManager;
use crate::rom_detect::SystemType;
use egui::Ui;
use egui_dock::{DockState, TabViewer};

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
    /// Inspector dock state (contains system-specific tabs)
    pub inspector_state: DockState<InspectorTab>,

    /// Property pane dock state
    pub property_state: DockState<PropertyTab>,

    /// Whether the inspector dock is visible
    pub inspector_visible: bool,

    /// Current system type (determines which tabs are shown)
    pub current_system: Option<SystemType>,
}

impl DockLayout {
    pub fn new() -> Self {
        // Start with just generic tabs (Log, Memory)
        let inspector_state = DockState::new(get_tabs_for_system(None));

        // Create property pane dock with a single Properties tab
        let property_state = DockState::new(vec![PropertyTab::Properties]);

        Self {
            inspector_state,
            property_state,
            inspector_visible: false, // Hidden by default
            current_system: None,
        }
    }

    /// Update the inspector tabs based on the current system
    pub fn update_system(&mut self, system_type: SystemType) {
        if self.current_system.as_ref() != Some(&system_type) {
            self.current_system = Some(system_type.clone());
            // Rebuild the inspector with tabs for this system
            self.inspector_state = DockState::new(get_tabs_for_system(Some(&system_type)));
        }
    }

    /// Clear the current system (show only generic tabs)
    pub fn clear_system(&mut self) {
        if self.current_system.is_some() {
            self.current_system = None;
            // Rebuild with only generic tabs
            self.inspector_state = DockState::new(get_tabs_for_system(None));
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
        render_inspector_tab(tab, ui, self.tab_manager);
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
