//! SDL2 window backend supporting both software and OpenGL rendering

use super::{Key, WindowBackend};
use crate::display_filter::DisplayFilter;
use crate::video_processor::{OpenGLProcessor, SoftwareProcessor, VideoProcessor};
use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::joystick::Joystick;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::Canvas;
use sdl2::video::{GLProfile, Window};
use sdl2::{EventPump, GameControllerSubsystem, JoystickSubsystem, Sdl, VideoSubsystem};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;

pub enum RenderMode {
    Software {
        canvas: Canvas<Window>,
        processor: SoftwareProcessor,
    },
    OpenGL {
        window: Window,
        _gl_context: sdl2::video::GLContext,
        processor: Box<OpenGLProcessor>,
    },
}

pub struct Sdl2Backend {
    _sdl_context: Sdl,
    _video_subsystem: VideoSubsystem,
    _game_controller_subsystem: GameControllerSubsystem,
    _joystick_subsystem: JoystickSubsystem,
    render_mode: RenderMode,
    event_pump: EventPump,
    pressed_keys: HashSet<Key>,
    key_pressed_once: HashSet<Key>,
    /// SDL2 scancodes pressed this frame (for direct PC scancode mapping)
    sdl2_scancodes_pressed: HashSet<u32>,
    /// SDL2 scancodes released this frame (for direct PC scancode mapping)
    sdl2_scancodes_released: HashSet<u32>,
    is_open: bool,
    current_filter: DisplayFilter,
    /// Mouse button clicks this frame (x, y) coordinates
    mouse_clicks: Vec<(i32, i32)>,
    /// Current mouse position
    mouse_position: (i32, i32),
    /// Connected game controllers (indexed by SDL joystick ID)
    game_controllers: HashMap<u32, GameController>,
    /// Connected joysticks that aren't game controllers (indexed by SDL joystick ID)
    joysticks: HashMap<u32, Joystick>,
    /// Gamepad button state (indexed by instance ID, then button)
    gamepad_buttons: HashMap<u32, HashSet<u8>>,
    /// Gamepad axis values (indexed by instance ID, then axis ID)
    gamepad_axes: HashMap<u32, HashMap<u8, i16>>,
    /// Joystick button state (indexed by instance ID, then button)
    joystick_buttons: HashMap<u32, HashSet<u8>>,
    /// Joystick axis values (indexed by instance ID, then axis ID)
    joystick_axes: HashMap<u32, HashMap<u8, i16>>,
    /// Joystick hat values (indexed by instance ID, then hat ID, value is bitmask: 1=up, 2=right, 4=down, 8=left)
    joystick_hats: HashMap<u32, HashMap<u8, u8>>,
}

impl Sdl2Backend {
    pub fn new(
        title: &str,
        width: usize,
        height: usize,
        use_opengl: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;
        let game_controller_subsystem = sdl_context.game_controller()?;
        let joystick_subsystem = sdl_context.joystick()?;

        let render_mode = if use_opengl {
            // Configure OpenGL
            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_context_profile(GLProfile::Core);
            gl_attr.set_context_version(3, 3);

            let window = video_subsystem
                .window(title, width as u32, height as u32)
                .opengl()
                .resizable()
                .build()?;

            let gl_context = window.gl_create_context()?;
            window.gl_make_current(&gl_context)?;

            // Load GL functions
            let gl = unsafe {
                glow::Context::from_loader_function(|s| {
                    video_subsystem.gl_get_proc_address(s) as *const _
                })
            };

            let mut gl_processor = OpenGLProcessor::new(gl)?;
            gl_processor.init(width, height)?;

            RenderMode::OpenGL {
                window,
                _gl_context: gl_context,
                processor: Box::new(gl_processor),
            }
        } else {
            // Software rendering
            let window = video_subsystem
                .window(title, width as u32, height as u32)
                .resizable()
                .build()?;

            let canvas = window.into_canvas().build()?;
            let processor = SoftwareProcessor::new();

            RenderMode::Software { canvas, processor }
        };

        let event_pump = sdl_context.event_pump()?;

        // Auto-detect and open all connected game controllers
        let mut game_controllers = HashMap::new();
        let num_joysticks = joystick_subsystem.num_joysticks()?;

        for id in 0..num_joysticks {
            if game_controller_subsystem.is_game_controller(id) {
                match game_controller_subsystem.open(id) {
                    Ok(controller) => {
                        let instance_id = controller.instance_id();
                        println!(
                            "Opened game controller {}: {} (instance ID: {})",
                            id,
                            controller.name(),
                            instance_id
                        );
                        game_controllers.insert(instance_id, controller);
                    }
                    Err(e) => {
                        eprintln!("Failed to open game controller {}: {}", id, e);
                    }
                }
            }
        }

        Ok(Self {
            _sdl_context: sdl_context,
            _video_subsystem: video_subsystem,
            _game_controller_subsystem: game_controller_subsystem,
            _joystick_subsystem: joystick_subsystem,
            render_mode,
            event_pump,
            pressed_keys: HashSet::new(),
            key_pressed_once: HashSet::new(),
            sdl2_scancodes_pressed: HashSet::new(),
            sdl2_scancodes_released: HashSet::new(),
            is_open: true,
            current_filter: DisplayFilter::None,
            mouse_clicks: Vec::new(),
            mouse_position: (0, 0),
            game_controllers,
            joysticks: HashMap::new(),
            gamepad_buttons: HashMap::new(),
            gamepad_axes: HashMap::new(),
            joystick_buttons: HashMap::new(),
            joystick_axes: HashMap::new(),
            joystick_hats: HashMap::new(),
        })
    }

    /// Convert SDL2 Keycode to our unified Key
    fn from_sdl2_key(k: Keycode) -> Option<Key> {
        match k {
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
            Keycode::Comma => Some(Key::Comma),
            Keycode::Period => Some(Key::Period),
            Keycode::Slash => Some(Key::Slash),
            Keycode::Semicolon => Some(Key::Semicolon),
            Keycode::Quote => Some(Key::Apostrophe),
            Keycode::LeftBracket => Some(Key::LeftBracket),
            Keycode::RightBracket => Some(Key::RightBracket),
            Keycode::Backslash => Some(Key::Backslash),
            Keycode::Minus => Some(Key::Minus),
            Keycode::Equals => Some(Key::Equals),
            Keycode::Backquote => Some(Key::Backtick),
            _ => None,
        }
    }

    /// Convert SDL2 Scancode to our unified Key (for physical key positions)
    /// This is used as a fallback when keycode doesn't map, which helps with
    /// international keyboards where the logical key differs from physical position
    fn from_sdl2_scancode(s: Scancode) -> Option<Key> {
        match s {
            Scancode::F1 => Some(Key::F1),
            Scancode::F2 => Some(Key::F2),
            Scancode::F3 => Some(Key::F3),
            Scancode::F4 => Some(Key::F4),
            Scancode::F5 => Some(Key::F5),
            Scancode::F6 => Some(Key::F6),
            Scancode::F7 => Some(Key::F7),
            Scancode::F8 => Some(Key::F8),
            Scancode::F9 => Some(Key::F9),
            Scancode::F10 => Some(Key::F10),
            Scancode::F11 => Some(Key::F11),
            Scancode::F12 => Some(Key::F12),
            Scancode::Num0 => Some(Key::Key0),
            Scancode::Num1 => Some(Key::Key1),
            Scancode::Num2 => Some(Key::Key2),
            Scancode::Num3 => Some(Key::Key3),
            Scancode::Num4 => Some(Key::Key4),
            Scancode::Num5 => Some(Key::Key5),
            Scancode::Num6 => Some(Key::Key6),
            Scancode::Num7 => Some(Key::Key7),
            Scancode::Num8 => Some(Key::Key8),
            Scancode::Num9 => Some(Key::Key9),
            Scancode::A => Some(Key::A),
            Scancode::B => Some(Key::B),
            Scancode::C => Some(Key::C),
            Scancode::D => Some(Key::D),
            Scancode::E => Some(Key::E),
            Scancode::F => Some(Key::F),
            Scancode::G => Some(Key::G),
            Scancode::H => Some(Key::H),
            Scancode::I => Some(Key::I),
            Scancode::J => Some(Key::J),
            Scancode::K => Some(Key::K),
            Scancode::L => Some(Key::L),
            Scancode::M => Some(Key::M),
            Scancode::N => Some(Key::N),
            Scancode::O => Some(Key::O),
            Scancode::P => Some(Key::P),
            Scancode::Q => Some(Key::Q),
            Scancode::R => Some(Key::R),
            Scancode::S => Some(Key::S),
            Scancode::T => Some(Key::T),
            Scancode::U => Some(Key::U),
            Scancode::V => Some(Key::V),
            Scancode::W => Some(Key::W),
            Scancode::X => Some(Key::X),
            Scancode::Y => Some(Key::Y),
            Scancode::Z => Some(Key::Z),
            Scancode::Up => Some(Key::Up),
            Scancode::Down => Some(Key::Down),
            Scancode::Left => Some(Key::Left),
            Scancode::Right => Some(Key::Right),
            Scancode::Escape => Some(Key::Escape),
            Scancode::Return => Some(Key::Enter),
            Scancode::Space => Some(Key::Space),
            Scancode::Tab => Some(Key::Tab),
            Scancode::Backspace => Some(Key::Backspace),
            Scancode::LShift => Some(Key::LeftShift),
            Scancode::RShift => Some(Key::RightShift),
            Scancode::LCtrl => Some(Key::LeftCtrl),
            Scancode::RCtrl => Some(Key::RightCtrl),
            Scancode::LAlt => Some(Key::LeftAlt),
            Scancode::RAlt => Some(Key::RightAlt),
            Scancode::Comma => Some(Key::Comma),
            Scancode::Period => Some(Key::Period),
            Scancode::Slash => Some(Key::Slash),
            Scancode::Semicolon => Some(Key::Semicolon),
            Scancode::Apostrophe => Some(Key::Apostrophe),
            Scancode::LeftBracket => Some(Key::LeftBracket),
            Scancode::RightBracket => Some(Key::RightBracket),
            Scancode::Backslash => Some(Key::Backslash),
            Scancode::Minus => Some(Key::Minus),
            Scancode::Equals => Some(Key::Equals),
            Scancode::Grave => Some(Key::Backtick),
            _ => None,
        }
    }

    pub fn set_filter(&mut self, filter: DisplayFilter) {
        self.current_filter = filter;
    }

    /// Get SDL2 scancodes pressed this frame (for direct PC scancode mapping)
    pub fn get_sdl2_scancodes_pressed(&self) -> &HashSet<u32> {
        &self.sdl2_scancodes_pressed
    }

    /// Get SDL2 scancodes released this frame (for direct PC scancode mapping)
    pub fn get_sdl2_scancodes_released(&self) -> &HashSet<u32> {
        &self.sdl2_scancodes_released
    }

    /// Get mouse clicks this frame
    pub fn get_mouse_clicks(&self) -> &[(i32, i32)] {
        &self.mouse_clicks
    }

    /// Get current mouse position
    pub fn get_mouse_position(&self) -> (i32, i32) {
        self.mouse_position
    }

    /// Set window title
    pub fn set_title(&mut self, title: &str) -> Result<(), Box<dyn Error>> {
        match &mut self.render_mode {
            RenderMode::OpenGL { window, .. } => {
                window.set_title(title)?;
            }
            RenderMode::Software { canvas, .. } => {
                canvas.window_mut().set_title(title)?;
            }
        }
        Ok(())
    }

    /// Check if a gamepad button is pressed
    /// instance_id: SDL2 controller instance ID (usually 0 for first controller)
    /// button: SDL2 GameController button ID
    pub fn is_gamepad_button_down(&self, instance_id: u32, button: u8) -> bool {
        self.gamepad_buttons
            .get(&instance_id)
            .map(|buttons| buttons.contains(&button))
            .unwrap_or(false)
    }

    /// Get gamepad axis value
    /// instance_id: SDL2 controller instance ID
    /// axis: SDL2 GameController axis ID
    /// Returns value in range -32768 to 32767, or 0 if not found
    pub fn get_gamepad_axis(&self, instance_id: u32, axis: u8) -> i16 {
        self.gamepad_axes
            .get(&instance_id)
            .and_then(|axes| axes.get(&axis).copied())
            .unwrap_or(0)
    }

    /// Get number of connected gamepads
    pub fn num_gamepads(&self) -> usize {
        self.game_controllers.len()
    }

    /// Check if a joystick button is pressed
    pub fn is_joystick_button_down(&self, instance_id: u32, button: u8) -> bool {
        self.joystick_buttons
            .get(&instance_id)
            .map(|buttons| buttons.contains(&button))
            .unwrap_or(false)
    }

    /// Get joystick axis value
    pub fn get_joystick_axis(&self, instance_id: u32, axis: u8) -> i16 {
        self.joystick_axes
            .get(&instance_id)
            .and_then(|axes| axes.get(&axis).copied())
            .unwrap_or(0)
    }

    /// Get joystick hat value
    pub fn get_joystick_hat(&self, instance_id: u32, hat: u8) -> u8 {
        self.joystick_hats
            .get(&instance_id)
            .and_then(|hats| hats.get(&hat).copied())
            .unwrap_or(0)
    }

    /// Get number of connected joysticks (non-gamepad)
    pub fn num_joysticks(&self) -> usize {
        self.joysticks.len()
    }
}

impl WindowBackend for Sdl2Backend {
    fn is_open(&self) -> bool {
        self.is_open
    }

    fn is_key_down(&self, key: Key) -> bool {
        self.pressed_keys.contains(&key)
    }

    fn is_key_pressed(&self, key: Key, _allow_repeat: bool) -> bool {
        self.key_pressed_once.contains(&key)
    }

    fn update_with_buffer(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
    ) -> Result<(), Box<dyn Error>> {
        match &mut self.render_mode {
            RenderMode::OpenGL {
                window, processor, ..
            } => {
                // Render using OpenGL processor
                let _processed =
                    processor.process_frame(buffer, width, height, self.current_filter)?;

                // Swap buffers
                window.gl_swap_window();
            }
            RenderMode::Software { canvas, processor } => {
                // Process frame with software processor
                let processed =
                    processor.process_frame(buffer, width, height, self.current_filter)?;

                // Create texture for this frame
                let texture_creator = canvas.texture_creator();
                let mut texture = texture_creator.create_texture_streaming(
                    PixelFormatEnum::ARGB8888,
                    width as u32,
                    height as u32,
                )?;

                // Update texture with processed buffer
                texture.update(None, bytemuck::cast_slice(&processed), width * 4)?;

                // Clear and render
                canvas.clear();
                canvas.copy(&texture, None, None)?;
                canvas.present();
            }
        }

        Ok(())
    }

    fn get_size(&self) -> (usize, usize) {
        match &self.render_mode {
            RenderMode::OpenGL { window, .. } => {
                let size = window.size();
                (size.0 as usize, size.1 as usize)
            }
            RenderMode::Software { canvas, .. } => {
                let size = canvas.window().size();
                (size.0 as usize, size.1 as usize)
            }
        }
    }

    fn poll_events(&mut self) {
        // Clear one-time press flags at start of frame
        self.key_pressed_once.clear();
        self.sdl2_scancodes_pressed.clear();
        self.sdl2_scancodes_released.clear();
        self.mouse_clicks.clear();

        // Poll all events
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.is_open = false;
                }
                Event::MouseButtonDown { x, y, .. } => {
                    self.mouse_clicks.push((x, y));
                    self.mouse_position = (x, y);
                }
                Event::MouseMotion { x, y, .. } => {
                    self.mouse_position = (x, y);
                }
                Event::KeyDown {
                    keycode,
                    scancode,
                    repeat: false,
                    ..
                } => {
                    // Track SDL2 scancode for direct PC mapping
                    if let Some(sc) = scancode {
                        self.sdl2_scancodes_pressed.insert(sc as u32);
                    }

                    // Try keycode first, fall back to scancode for international keyboards
                    let key = keycode
                        .and_then(Self::from_sdl2_key)
                        .or_else(|| scancode.and_then(Self::from_sdl2_scancode));

                    if let Some(key) = key {
                        self.pressed_keys.insert(key);
                        self.key_pressed_once.insert(key);
                    }
                }
                Event::KeyDown {
                    keycode,
                    scancode,
                    repeat: true,
                    ..
                } => {
                    // Don't track repeated scancodes

                    // Try keycode first, fall back to scancode for international keyboards
                    let key = keycode
                        .and_then(Self::from_sdl2_key)
                        .or_else(|| scancode.and_then(Self::from_sdl2_scancode));

                    if let Some(key) = key {
                        self.pressed_keys.insert(key);
                        // Don't add to key_pressed_once for repeat events
                    }
                }
                Event::KeyUp {
                    keycode, scancode, ..
                } => {
                    // Track SDL2 scancode for direct PC mapping
                    if let Some(sc) = scancode {
                        self.sdl2_scancodes_released.insert(sc as u32);
                    }

                    // Try keycode first, fall back to scancode for international keyboards
                    let key = keycode
                        .and_then(Self::from_sdl2_key)
                        .or_else(|| scancode.and_then(Self::from_sdl2_scancode));

                    if let Some(key) = key {
                        self.pressed_keys.remove(&key);
                    }
                }
                // Game controller events
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(controller) = self._game_controller_subsystem.open(which) {
                        let instance_id = controller.instance_id();
                        println!(
                            "Game controller added: {} (instance ID: {})",
                            controller.name(),
                            instance_id
                        );
                        self.game_controllers.insert(instance_id, controller);
                        self.gamepad_buttons.insert(instance_id, HashSet::new());
                        self.gamepad_axes.insert(instance_id, HashMap::new());
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    println!("Game controller removed (instance ID: {})", which);
                    self.game_controllers.remove(&which);
                    self.gamepad_buttons.remove(&which);
                    self.gamepad_axes.remove(&which);
                }
                Event::ControllerButtonDown { which, button, .. } => {
                    self.gamepad_buttons
                        .entry(which)
                        .or_insert_with(HashSet::new)
                        .insert(button as u8);
                }
                Event::ControllerButtonUp { which, button, .. } => {
                    if let Some(buttons) = self.gamepad_buttons.get_mut(&which) {
                        buttons.remove(&(button as u8));
                    }
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => {
                    self.gamepad_axes
                        .entry(which)
                        .or_insert_with(HashMap::new)
                        .insert(axis as u8, value);
                }
                // Joystick events (for non-gamepad joysticks)
                Event::JoyDeviceAdded { which, .. } => {
                    // Only open if not already opened as a game controller
                    if !self._game_controller_subsystem.is_game_controller(which) {
                        if let Ok(joystick) = self._joystick_subsystem.open(which) {
                            let instance_id = joystick.instance_id();
                            println!(
                                "Joystick added: {} (instance ID: {})",
                                joystick.name(),
                                instance_id
                            );
                            self.joysticks.insert(instance_id, joystick);
                            self.joystick_buttons.insert(instance_id, HashSet::new());
                            self.joystick_axes.insert(instance_id, HashMap::new());
                            self.joystick_hats.insert(instance_id, HashMap::new());
                        }
                    }
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    println!("Joystick removed (instance ID: {})", which);
                    self.joysticks.remove(&which);
                    self.joystick_buttons.remove(&which);
                    self.joystick_axes.remove(&which);
                    self.joystick_hats.remove(&which);
                }
                Event::JoyButtonDown {
                    which, button_idx, ..
                } => {
                    self.joystick_buttons
                        .entry(which)
                        .or_insert_with(HashSet::new)
                        .insert(button_idx);
                }
                Event::JoyButtonUp {
                    which, button_idx, ..
                } => {
                    if let Some(buttons) = self.joystick_buttons.get_mut(&which) {
                        buttons.remove(&button_idx);
                    }
                }
                Event::JoyAxisMotion {
                    which,
                    axis_idx,
                    value,
                    ..
                } => {
                    self.joystick_axes
                        .entry(which)
                        .or_insert_with(HashMap::new)
                        .insert(axis_idx, value);
                }
                Event::JoyHatMotion {
                    which,
                    hat_idx,
                    state,
                    ..
                } => {
                    // Convert SDL hat state to bitmask (1=up, 2=right, 4=down, 8=left)
                    let hat_value = match state {
                        sdl2::joystick::HatState::Centered => 0,
                        sdl2::joystick::HatState::Up => 1,
                        sdl2::joystick::HatState::Right => 2,
                        sdl2::joystick::HatState::Down => 4,
                        sdl2::joystick::HatState::Left => 8,
                        sdl2::joystick::HatState::RightUp => 3,
                        sdl2::joystick::HatState::RightDown => 6,
                        sdl2::joystick::HatState::LeftUp => 9,
                        sdl2::joystick::HatState::LeftDown => 12,
                    };
                    self.joystick_hats
                        .entry(which)
                        .or_insert_with(HashMap::new)
                        .insert(hat_idx, hat_value);
                }
                _ => {}
            }
        }
    }

    fn name(&self) -> &str {
        match &self.render_mode {
            RenderMode::OpenGL { .. } => "SDL2 (OpenGL)",
            RenderMode::Software { .. } => "SDL2 (Software)",
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
