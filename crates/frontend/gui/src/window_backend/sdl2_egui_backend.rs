//! SDL2 backend with egui integration

use crate::window_backend::{Key, WindowBackend};
use egui_glow::EguiGlow;
use glow::HasContext;
use std::any::Any;
use std::error::Error;
use std::time::Instant;

pub struct Sdl2EguiBackend {
    sdl_context: sdl2::Sdl,
    video_subsystem: sdl2::VideoSubsystem,
    window: sdl2::video::Window,
    _gl_context: sdl2::video::GLContext,
    gl: std::rc::Rc<glow::Context>,
    egui_glow: EguiGlow,
    event_pump: sdl2::EventPump,
    
    // State tracking
    keys_down: std::collections::HashSet<Key>,
    keys_pressed: std::collections::HashSet<Key>,
    sdl2_scancodes_pressed: Vec<sdl2::keyboard::Scancode>,
    sdl2_scancodes_released: Vec<sdl2::keyboard::Scancode>,
    mouse_pos: (i32, i32),
    start_time: Instant,
}

impl Sdl2EguiBackend {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;

        // Set up OpenGL attributes
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 3);
        gl_attr.set_double_buffer(true);
        gl_attr.set_depth_size(24);

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

        // Load OpenGL function pointers
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                video_subsystem.gl_get_proc_address(s) as *const _
            })
        };
        let gl = std::rc::Rc::new(gl);

        // Initialize egui
        let egui_glow = EguiGlow::new(&window, gl.clone(), None, None);

        let event_pump = sdl_context.event_pump()?;

        Ok(Self {
            sdl_context,
            video_subsystem,
            window,
            _gl_context: gl_context,
            gl,
            egui_glow,
            event_pump,
            keys_down: std::collections::HashSet::new(),
            keys_pressed: std::collections::HashSet::new(),
            sdl2_scancodes_pressed: Vec::new(),
            sdl2_scancodes_released: Vec::new(),
            mouse_pos: (0, 0),
            start_time: Instant::now(),
        })
    }

    /// Get the egui context for rendering UI
    pub fn egui_ctx(&self) -> &egui::Context {
        self.egui_glow.egui_ctx()
    }

    /// Begin an egui frame
    pub fn begin_frame(&mut self) {
        let (width, height) = self.window.size();
        self.egui_glow.begin_frame(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            time: Some(self.start_time.elapsed().as_secs_f64()),
            ..Default::default()
        });
    }

    /// End an egui frame and render
    pub fn end_frame(&mut self) {
        let egui::FullOutput {
            platform_output: _,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output: _,
        } = self.egui_glow.end_frame();

        let clipped_primitives = self.egui_glow.egui_ctx().tessellate(shapes, pixels_per_point);
        
        unsafe {
            self.gl.clear_color(0.1, 0.1, 0.1, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.egui_glow.paint(&clipped_primitives, &textures_delta);

        self.window.gl_swap_window();
    }

    /// Handle SDL2 events and update egui input
    pub fn handle_events(&mut self) -> bool {
        self.keys_pressed.clear();
        self.sdl2_scancodes_pressed.clear();
        self.sdl2_scancodes_released.clear();

        // Collect events first to avoid borrow checker issues
        let events: Vec<_> = self.event_pump.poll_iter().collect();
        
        for event in events {
            // Pass events to egui
            match &event {
                sdl2::event::Event::Quit { .. } => {
                    return false;
                }
                sdl2::event::Event::KeyDown { keycode, scancode, .. } => {
                    if let Some(keycode) = keycode {
                        if let Some(key) = sdl_keycode_to_key(*keycode) {
                            self.keys_down.insert(key);
                            self.keys_pressed.insert(key);
                        }
                    }
                    if let Some(scancode) = scancode {
                        self.sdl2_scancodes_pressed.push(*scancode);
                    }
                }
                sdl2::event::Event::KeyUp { keycode, scancode, .. } => {
                    if let Some(keycode) = keycode {
                        if let Some(key) = sdl_keycode_to_key(*keycode) {
                            self.keys_down.remove(&key);
                        }
                    }
                    if let Some(scancode) = scancode {
                        self.sdl2_scancodes_released.push(*scancode);
                    }
                }
                sdl2::event::Event::MouseMotion { x, y, .. } => {
                    self.mouse_pos = (*x, *y);
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
}

impl WindowBackend for Sdl2EguiBackend {
    fn is_open(&self) -> bool {
        // Window is open as long as handle_events returns true
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
        // This is handled by egui now, so we just return Ok
        Ok(())
    }

    fn is_key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    fn is_key_pressed(&self, key: Key, _shift: bool) -> bool {
        self.keys_pressed.contains(&key)
    }

    fn set_title(&mut self, title: &str) {
        let _ = self.window.set_title(title);
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
