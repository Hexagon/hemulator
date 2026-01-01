//! SDL2 backend with egui integration using egui-sdl2-gl

use crate::window_backend::{Key, WindowBackend};
use egui_sdl2_gl::{painter::Painter, EguiStateHandler, ShaderVersion};
use std::any::Any;
use std::error::Error;

pub struct Sdl2EguiBackend {
    #[allow(dead_code)]
    sdl_context: sdl2::Sdl,
    window: sdl2::video::Window,
    _gl_context: sdl2::video::GLContext,
    painter: Painter,
    egui_state: EguiStateHandler,
    egui_ctx: egui::Context,
    event_pump: sdl2::EventPump,

    // State tracking
    keys_down: std::collections::HashSet<Key>,
    keys_pressed: std::collections::HashSet<Key>,
    sdl2_scancodes_pressed: Vec<sdl2::keyboard::Scancode>,
    sdl2_scancodes_released: Vec<sdl2::keyboard::Scancode>,
}

impl Sdl2EguiBackend {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;

        // Set up OpenGL attributes for egui
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 2);
        gl_attr.set_double_buffer(true);

        let window = video_subsystem
            .window(title, width, height)
            .opengl()
            .resizable()
            .position_centered()
            .build()?;

        let gl_context = window.gl_create_context()?;
        window.gl_make_current(&gl_context)?;

        // Enable vsync
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)?;

        // Initialize painter and egui state
        let painter = Painter::new(&window, 1.0, ShaderVersion::Default);
        let egui_state = EguiStateHandler::new(&painter);
        let egui_ctx = egui::Context::default();

        // Configure egui style for better contrast and VS Code-like appearance
        let mut style = (*egui_ctx.style()).clone();
        let visuals = &mut style.visuals;

        // VS Code-like color scheme with better contrast
        // Background colors - slightly lighter gray for panels
        visuals.panel_fill = egui::Color32::from_rgb(37, 37, 38); // Darker base
        visuals.window_fill = egui::Color32::from_rgb(30, 30, 30); // Very dark for main window
        visuals.extreme_bg_color = egui::Color32::from_rgb(25, 25, 26); // Darkest backgrounds

        // Override widget colors for better text visibility
        // Widget backgrounds
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(50, 50, 52);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 62);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 70, 72);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0, 122, 204); // VS Code blue

        // Widget text colors - much brighter for better contrast (matching VS Code)
        visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 230, 230));
        visuals.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 220));
        visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));
        visuals.widgets.active.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));

        // Selection colors
        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 122, 204); // VS Code blue
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 122, 204));

        // Hyperlink color (for tabs, etc.)
        visuals.hyperlink_color = egui::Color32::from_rgb(75, 150, 255); // Bright blue

        // Window/panel stroke colors
        visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60));
        visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60));

        // Default text color - much brighter for all UI elements
        visuals.override_text_color = Some(egui::Color32::from_rgb(230, 230, 230));

        egui_ctx.set_style(style);

        let event_pump = sdl_context.event_pump()?;

        Ok(Self {
            sdl_context,
            window,
            _gl_context: gl_context,
            painter,
            egui_state,
            egui_ctx,
            event_pump,
            keys_down: std::collections::HashSet::new(),
            keys_pressed: std::collections::HashSet::new(),
            sdl2_scancodes_pressed: Vec::new(),
            sdl2_scancodes_released: Vec::new(),
        })
    }

    /// Get the egui context for rendering UI
    pub fn egui_ctx(&self) -> &egui::Context {
        &self.egui_ctx
    }

    /// Begin an egui frame
    pub fn begin_frame(&mut self) {
        let raw_input = self.egui_state.input.take();
        self.egui_ctx.begin_pass(raw_input);
    }

    /// End an egui frame and render
    pub fn end_frame(&mut self) {
        let egui::FullOutput {
            platform_output: _,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output: _,
        } = self.egui_ctx.end_pass();

        // Paint
        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        self.painter
            .paint_jobs(None, textures_delta, clipped_primitives);

        self.window.gl_swap_window();
    }

    /// Handle SDL2 events and update egui input
    /// Returns false if the window should close
    pub fn handle_events(&mut self) -> bool {
        self.keys_pressed.clear();
        self.sdl2_scancodes_pressed.clear();
        self.sdl2_scancodes_released.clear();

        // Collect events first to avoid borrow checker issues
        let events: Vec<_> = self.event_pump.poll_iter().collect();

        for event in events {
            // Process event with egui state handler
            self.egui_state
                .process_input(&self.window, event.clone(), &mut self.painter);

            // Also process for emulator controls
            match event {
                sdl2::event::Event::Quit { .. } => {
                    return false;
                }
                sdl2::event::Event::KeyDown {
                    keycode, scancode, ..
                } => {
                    if let Some(keycode) = keycode {
                        if let Some(key) = sdl_keycode_to_key(keycode) {
                            self.keys_down.insert(key);
                            self.keys_pressed.insert(key);
                        }
                    }
                    if let Some(scancode) = scancode {
                        self.sdl2_scancodes_pressed.push(scancode);
                    }
                }
                sdl2::event::Event::KeyUp {
                    keycode, scancode, ..
                } => {
                    if let Some(keycode) = keycode {
                        if let Some(key) = sdl_keycode_to_key(keycode) {
                            self.keys_down.remove(&key);
                        }
                    }
                    if let Some(scancode) = scancode {
                        self.sdl2_scancodes_released.push(scancode);
                    }
                }
                _ => {}
            }
        }

        true
    }

    /// Get SDL2 scancodes that were pressed this frame
    pub fn get_sdl2_scancodes_pressed(&self) -> &[sdl2::keyboard::Scancode] {
        &self.sdl2_scancodes_pressed
    }

    /// Get SDL2 scancodes that were released this frame
    pub fn get_sdl2_scancodes_released(&self) -> &[sdl2::keyboard::Scancode] {
        &self.sdl2_scancodes_released
    }

    /// Toggle fullscreen mode
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), Box<dyn Error>> {
        if fullscreen {
            self.window
                .set_fullscreen(sdl2::video::FullscreenType::Desktop)?;
        } else {
            self.window
                .set_fullscreen(sdl2::video::FullscreenType::Off)?;
        }
        Ok(())
    }

    /// Get current fullscreen state
    pub fn is_fullscreen(&self) -> bool {
        self.window.fullscreen_state() != sdl2::video::FullscreenType::Off
    }
}

impl WindowBackend for Sdl2EguiBackend {
    fn is_open(&self) -> bool {
        true
    }

    fn poll_events(&mut self) {
        // Events are polled in handle_events
    }

    fn name(&self) -> &str {
        "SDL2 + egui"
    }

    fn update_with_buffer(
        &mut self,
        _buffer: &[u32],
        _width: usize,
        _height: usize,
    ) -> Result<(), Box<dyn Error>> {
        // Buffer rendering is handled by egui texture updates now
        Ok(())
    }

    fn is_key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    fn is_key_pressed(&self, key: Key, _shift: bool) -> bool {
        self.keys_pressed.contains(&key)
    }

    fn get_size(&self) -> (usize, usize) {
        let (w, h) = self.window.size();
        (w as usize, h as usize)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// Helper function to convert SDL2 keycode to our Key enum
fn sdl_keycode_to_key(keycode: sdl2::keyboard::Keycode) -> Option<Key> {
    use sdl2::keyboard::Keycode;
    match keycode {
        Keycode::F1 => Some(Key::F1),
        Keycode::F2 => Some(Key::F2),
        Keycode::F3 => Some(Key::F3),
        Keycode::F4 => Some(Key::F4),
        Keycode::F5 => Some(Key::F5),
        Keycode::F6 => Some(Key::F6),
        Keycode::F7 => Some(Key::F7),
        Keycode::F8 => Some(Key::F8),
        Keycode::F9 => Some(Key::F9),
        Keycode::F10 => Some(Key::F10),
        Keycode::F11 => Some(Key::F11),
        Keycode::F12 => Some(Key::F12),
        Keycode::Num0 => Some(Key::Key0),
        Keycode::Num1 => Some(Key::Key1),
        Keycode::Num2 => Some(Key::Key2),
        Keycode::Num3 => Some(Key::Key3),
        Keycode::Num4 => Some(Key::Key4),
        Keycode::Num5 => Some(Key::Key5),
        Keycode::Num6 => Some(Key::Key6),
        Keycode::Num7 => Some(Key::Key7),
        Keycode::Num8 => Some(Key::Key8),
        Keycode::Num9 => Some(Key::Key9),
        Keycode::A => Some(Key::A),
        Keycode::B => Some(Key::B),
        Keycode::C => Some(Key::C),
        Keycode::D => Some(Key::D),
        Keycode::E => Some(Key::E),
        Keycode::F => Some(Key::F),
        Keycode::G => Some(Key::G),
        Keycode::H => Some(Key::H),
        Keycode::I => Some(Key::I),
        Keycode::J => Some(Key::J),
        Keycode::K => Some(Key::K),
        Keycode::L => Some(Key::L),
        Keycode::M => Some(Key::M),
        Keycode::N => Some(Key::N),
        Keycode::O => Some(Key::O),
        Keycode::P => Some(Key::P),
        Keycode::Q => Some(Key::Q),
        Keycode::R => Some(Key::R),
        Keycode::S => Some(Key::S),
        Keycode::T => Some(Key::T),
        Keycode::U => Some(Key::U),
        Keycode::V => Some(Key::V),
        Keycode::W => Some(Key::W),
        Keycode::X => Some(Key::X),
        Keycode::Y => Some(Key::Y),
        Keycode::Z => Some(Key::Z),
        Keycode::Up => Some(Key::Up),
        Keycode::Down => Some(Key::Down),
        Keycode::Left => Some(Key::Left),
        Keycode::Right => Some(Key::Right),
        Keycode::Escape => Some(Key::Escape),
        Keycode::Return => Some(Key::Enter),
        Keycode::Space => Some(Key::Space),
        Keycode::Tab => Some(Key::Tab),
        Keycode::Backspace => Some(Key::Backspace),
        Keycode::LShift => Some(Key::LeftShift),
        Keycode::RShift => Some(Key::RightShift),
        Keycode::LCtrl => Some(Key::LeftCtrl),
        Keycode::RCtrl => Some(Key::RightCtrl),
        Keycode::LAlt => Some(Key::LeftAlt),
        Keycode::RAlt => Some(Key::RightAlt),
        _ => None,
    }
}
