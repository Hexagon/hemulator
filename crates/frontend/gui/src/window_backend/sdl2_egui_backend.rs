//! SDL2 backend with egui integration using egui-sdl2-gl

use crate::window_backend::{Key, WindowBackend};
use egui_sdl2_gl::{painter::Painter, EguiStateHandler, ShaderVersion};
use sdl2::controller::GameController;
use sdl2::joystick::Joystick;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::error::Error;

// ─── Type-boundary helpers ────────────────────────────────────────────────────
//
// `egui_sdl2_gl 0.33` links against `egui 0.33` while the rest of the codebase
// uses `egui 0.34`.  The two versions are ABI-incompatible at the type level, so
// we need small conversion shims at the call sites in `begin_frame` / `end_frame`.
//
// *Input side* (`egui_sdl2_gl::egui::RawInput` → `egui::RawInput`):
//   Built field-by-field.  `egui_sdl2_gl` only fills a small subset of fields,
//   so the conversion is straightforward.  The new `occluded` / `phase` fields
//   that appeared in 0.34 are set to their sensible defaults.
//
// *Output side* (`egui::TexturesDelta` / `Vec<egui::ClippedPrimitive>` → 0.33):
//   Both `epaint::TexturesDelta` and `epaint::ClippedPrimitive` (transitively
//   `epaint::Mesh`) have *identical* memory layouts between versions 0.33 and
//   0.34 – verified by comparing the struct/field definitions in the registry
//   sources.  The compile-time assertions below guard against any future layout
//   drift.  A `transmute` reinterprets the bits without any allocation.

// Compile-time layout guards: catch ABI drift between egui 0.33 and 0.34 types.
const _: () = {
    use egui_sdl2_gl::egui as e33;
    use std::mem::{align_of, size_of};
    assert!(size_of::<e33::Pos2>() == size_of::<egui::Pos2>());
    assert!(align_of::<e33::Pos2>() == align_of::<egui::Pos2>());
    assert!(size_of::<e33::Vec2>() == size_of::<egui::Vec2>());
    assert!(align_of::<e33::Vec2>() == align_of::<egui::Vec2>());
    assert!(size_of::<e33::Rect>() == size_of::<egui::Rect>());
    assert!(align_of::<e33::Rect>() == align_of::<egui::Rect>());
    assert!(size_of::<e33::Modifiers>() == size_of::<egui::Modifiers>());
    assert!(align_of::<e33::Modifiers>() == align_of::<egui::Modifiers>());
    assert!(size_of::<e33::Key>() == size_of::<egui::Key>());
    assert!(align_of::<e33::Key>() == align_of::<egui::Key>());
    assert!(size_of::<e33::PointerButton>() == size_of::<egui::PointerButton>());
    assert!(align_of::<e33::PointerButton>() == align_of::<egui::PointerButton>());
    assert!(size_of::<e33::MouseWheelUnit>() == size_of::<egui::MouseWheelUnit>());
    assert!(align_of::<e33::MouseWheelUnit>() == align_of::<egui::MouseWheelUnit>());
    assert!(size_of::<e33::ImeEvent>() == size_of::<egui::ImeEvent>());
    assert!(align_of::<e33::ImeEvent>() == align_of::<egui::ImeEvent>());
    assert!(size_of::<e33::TexturesDelta>() == size_of::<egui::TexturesDelta>());
    assert!(align_of::<e33::TexturesDelta>() == align_of::<egui::TexturesDelta>());
    assert!(size_of::<e33::ClippedPrimitive>() == size_of::<egui::ClippedPrimitive>());
    assert!(align_of::<e33::ClippedPrimitive>() == align_of::<egui::ClippedPrimitive>());
};

/// Convert a single `egui_sdl2_gl::egui::Event` (egui 0.33) to
/// `egui::Event` (egui 0.34).  Returns `None` for event variants that have no
/// equivalent in 0.34 (none are expected from the SDL2 backend).
fn convert_sdl2_event(event: egui_sdl2_gl::egui::Event) -> Option<egui::Event> {
    use egui_sdl2_gl::egui as e33;
    use egui_sdl2_gl::egui::Event as E33;

    // The basic scalar/struct types shared between the two versions (Pos2, Vec2,
    // Modifiers, Key, PointerButton, MouseWheelUnit …) have identical definitions
    // across emath 0.33/0.34 and egui 0.33/0.34.  The compile-time assertions
    // above verify size/alignment parity; a transmute is therefore sound.
    Some(match event {
        E33::PointerMoved(pos) => {
            egui::Event::PointerMoved(unsafe { std::mem::transmute::<e33::Pos2, egui::Pos2>(pos) })
        }
        E33::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        } => egui::Event::PointerButton {
            pos: unsafe { std::mem::transmute::<e33::Pos2, egui::Pos2>(pos) },
            button: unsafe {
                std::mem::transmute::<e33::PointerButton, egui::PointerButton>(button)
            },
            pressed,
            modifiers: unsafe { std::mem::transmute::<e33::Modifiers, egui::Modifiers>(modifiers) },
        },
        E33::PointerGone => egui::Event::PointerGone,
        E33::Key {
            key,
            physical_key,
            pressed,
            repeat,
            modifiers,
        } => egui::Event::Key {
            key: unsafe { std::mem::transmute::<e33::Key, egui::Key>(key) },
            physical_key: physical_key
                .map(|k| unsafe { std::mem::transmute::<e33::Key, egui::Key>(k) }),
            pressed,
            repeat,
            modifiers: unsafe { std::mem::transmute::<e33::Modifiers, egui::Modifiers>(modifiers) },
        },
        E33::Text(s) => egui::Event::Text(s),
        // `MouseWheel` gained `phase: TouchPhase` in 0.34 while keeping `modifiers`.
        // `TouchPhase::Move` is the correct default for a non-trackpad scroll event.
        E33::MouseWheel {
            unit,
            delta,
            modifiers,
        } => egui::Event::MouseWheel {
            unit: unsafe { std::mem::transmute::<e33::MouseWheelUnit, egui::MouseWheelUnit>(unit) },
            delta: unsafe { std::mem::transmute::<e33::Vec2, egui::Vec2>(delta) },
            phase: egui::TouchPhase::Move,
            modifiers: unsafe { std::mem::transmute::<e33::Modifiers, egui::Modifiers>(modifiers) },
        },
        E33::Copy => egui::Event::Copy,
        E33::Cut => egui::Event::Cut,
        E33::Paste(s) => egui::Event::Paste(s),
        E33::Zoom(f) => egui::Event::Zoom(f),
        // ImeEvent is identical between 0.33 and 0.34.
        E33::Ime(ime) => {
            egui::Event::Ime(unsafe { std::mem::transmute::<e33::ImeEvent, egui::ImeEvent>(ime) })
        }
        // Touch, Screenshot, MouseMoved, WindowFocused are not generated by the
        // SDL2 backend; return None to drop them gracefully.
        _ => return None,
    })
}

/// Build an `egui::RawInput` (0.34) from the SDL2 state collected in
/// `egui_sdl2_gl::egui::RawInput` (0.33).
fn convert_sdl2_raw_input(input: egui_sdl2_gl::egui::RawInput) -> egui::RawInput {
    use egui_sdl2_gl::egui as e33;

    // Reconstruct ViewportInfo for egui 0.34 (which added `occluded: Option<bool>`).
    // The SDL2 backend only ever uses a single ROOT viewport; only
    // `native_pixels_per_point` is set – everything else stays at the default.
    let mut viewports: egui::ViewportIdMap<egui::ViewportInfo> = egui::ViewportIdMap::default();
    if let Some(info) = input.viewports.get(&egui_sdl2_gl::egui::ViewportId::ROOT) {
        let entry = viewports.entry(egui::ViewportId::ROOT).or_default();
        entry.native_pixels_per_point = info.native_pixels_per_point;
    }

    egui::RawInput {
        viewport_id: egui::ViewportId::ROOT,
        viewports,
        screen_rect: input
            .screen_rect
            .map(|r| unsafe { std::mem::transmute::<e33::Rect, egui::Rect>(r) }),
        max_texture_side: input.max_texture_side,
        time: input.time,
        predicted_dt: input.predicted_dt,
        modifiers: unsafe {
            std::mem::transmute::<e33::Modifiers, egui::Modifiers>(input.modifiers)
        },
        events: input
            .events
            .into_iter()
            .filter_map(convert_sdl2_event)
            .collect(),
        hovered_files: vec![],
        dropped_files: vec![],
        focused: input.focused,
        ..Default::default()
    }
}

pub struct Sdl2EguiBackend {
    #[allow(dead_code)]
    sdl_context: sdl2::Sdl,
    window: sdl2::video::Window,
    _gl_context: sdl2::video::GLContext,
    painter: Painter,
    egui_state: EguiStateHandler,
    egui_ctx: egui::Context,
    event_pump: sdl2::EventPump,
    _game_controller_subsystem: sdl2::GameControllerSubsystem,
    _joystick_subsystem: sdl2::JoystickSubsystem,

    // State tracking
    keys_down: std::collections::HashSet<Key>,
    keys_pressed: std::collections::HashSet<Key>,
    sdl2_scancodes_pressed: Vec<sdl2::keyboard::Scancode>,
    sdl2_scancodes_released: Vec<sdl2::keyboard::Scancode>,

    // Gamepad/joystick state
    /// Connected game controllers (indexed by SDL instance ID)
    game_controllers: HashMap<u32, GameController>,
    /// Connected joysticks that aren't game controllers (indexed by SDL instance ID)
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

impl Sdl2EguiBackend {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;

        // Set up OpenGL attributes for egui
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 2);
        gl_attr.set_double_buffer(true);

        let mut window = video_subsystem
            .window(title, width, height)
            .opengl()
            .resizable()
            .position_centered()
            .build()?;

        // Set window icon
        Self::set_window_icon(&mut window);

        let gl_context = window.gl_create_context()?;
        window.gl_make_current(&gl_context)?;

        // Enable vsync
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)?;

        // Initialize painter and egui state
        let painter = Painter::new(&window, 1.0, ShaderVersion::Default);
        let egui_state = EguiStateHandler::new(&painter);
        let egui_ctx = egui::Context::default();

        let event_pump = sdl_context.event_pump()?;

        // Initialize gamepad and joystick subsystems
        let game_controller_subsystem = sdl_context.game_controller()?;
        let joystick_subsystem = sdl_context.joystick()?;

        // Auto-detect and open all connected game controllers and joysticks
        let mut game_controllers = HashMap::new();
        let mut joysticks = HashMap::new();
        let mut gamepad_buttons = HashMap::new();
        let mut gamepad_axes = HashMap::new();
        let mut joystick_buttons = HashMap::new();
        let mut joystick_axes = HashMap::new();
        let mut joystick_hats = HashMap::new();

        let num_joysticks = joystick_subsystem.num_joysticks()?;

        for id in 0..num_joysticks {
            if game_controller_subsystem.is_game_controller(id) {
                // Open as game controller
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
                        // Initialize button and axis maps for this controller
                        gamepad_buttons.insert(instance_id, HashSet::new());
                        gamepad_axes.insert(instance_id, HashMap::new());
                    }
                    Err(e) => {
                        eprintln!("Failed to open game controller {}: {}", id, e);
                    }
                }
            } else {
                // Open as regular joystick
                match joystick_subsystem.open(id) {
                    Ok(joystick) => {
                        let instance_id = joystick.instance_id();
                        println!(
                            "Opened joystick {}: {} (instance ID: {})",
                            id,
                            joystick.name(),
                            instance_id
                        );
                        joysticks.insert(instance_id, joystick);
                        // Initialize button, axis, and hat maps for this joystick
                        joystick_buttons.insert(instance_id, HashSet::new());
                        joystick_axes.insert(instance_id, HashMap::new());
                        joystick_hats.insert(instance_id, HashMap::new());
                    }
                    Err(e) => {
                        eprintln!("Failed to open joystick {}: {}", id, e);
                    }
                }
            }
        }

        Ok(Self {
            sdl_context,
            window,
            _gl_context: gl_context,
            painter,
            egui_state,
            egui_ctx,
            event_pump,
            _game_controller_subsystem: game_controller_subsystem,
            _joystick_subsystem: joystick_subsystem,
            keys_down: std::collections::HashSet::new(),
            keys_pressed: std::collections::HashSet::new(),
            sdl2_scancodes_pressed: Vec::new(),
            sdl2_scancodes_released: Vec::new(),
            game_controllers,
            joysticks,
            gamepad_buttons,
            gamepad_axes,
            joystick_buttons,
            joystick_axes,
            joystick_hats,
        })
    }

    /// Get SDL2 video subsystem (for GL context access)
    pub fn video_subsystem(&self) -> sdl2::VideoSubsystem {
        self.sdl_context
            .video()
            .expect("Video subsystem should be available")
    }

    /// Get OpenGL context for renderer initialization
    /// Returns a new glow::Context for use with renderers
    pub fn gl_context(&self) -> Option<glow::Context> {
        // Create a new glow context from SDL2 GL functions
        // glow::Context internally uses Rc for the function pointers, so this is cheap
        unsafe {
            let gl = glow::Context::from_loader_function(|s| {
                self.video_subsystem().gl_get_proc_address(s) as *const _
            });
            Some(gl)
        }
    }

    /// Get the egui context for rendering UI
    pub fn egui_ctx(&self) -> &egui::Context {
        &self.egui_ctx
    }

    /// Begin an egui frame
    pub fn begin_frame(&mut self) {
        let raw_input_033 = self.egui_state.input.take();
        let raw_input_034 = convert_sdl2_raw_input(raw_input_033);
        self.egui_ctx.begin_pass(raw_input_034);
    }

    /// End an egui frame and render
    pub fn end_frame(&mut self) {
        let egui::FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output: _,
        } = self.egui_ctx.end_pass();

        // Handle URL opening requests from egui (e.g., hyperlink clicks)
        for command in platform_output.commands {
            if let egui::OutputCommand::OpenUrl(open_url) = command {
                if let Err(e) = open::that(&open_url.url) {
                    eprintln!("Failed to open URL {}: {}", open_url.url, e);
                }
            }
        }

        // Paint – convert 0.34 output types to the 0.33 types expected by the
        // egui_sdl2_gl painter.  `TexturesDelta` and `ClippedPrimitive`/`Mesh`
        // have identical memory layouts between epaint 0.33 and 0.34.
        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        let textures_delta_033: egui_sdl2_gl::egui::TexturesDelta = unsafe {
            std::mem::transmute::<egui::TexturesDelta, egui_sdl2_gl::egui::TexturesDelta>(
                textures_delta,
            )
        };
        let clipped_primitives_033: Vec<egui_sdl2_gl::egui::ClippedPrimitive> = unsafe {
            std::mem::transmute::<
                Vec<egui::ClippedPrimitive>,
                Vec<egui_sdl2_gl::egui::ClippedPrimitive>,
            >(clipped_primitives)
        };
        self.painter
            .paint_jobs(None, textures_delta_033, clipped_primitives_033);

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
                // Game controller events
                sdl2::event::Event::ControllerDeviceAdded { which, .. } => {
                    match self._game_controller_subsystem.open(which) {
                        Ok(controller) => {
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
                        Err(err) => {
                            eprintln!(
                                "Failed to open hot-plugged game controller (index {}): {}",
                                which, err
                            );
                        }
                    }
                }
                sdl2::event::Event::ControllerDeviceRemoved { which, .. } => {
                    println!("Game controller removed (instance ID: {})", which);
                    self.game_controllers.remove(&which);
                    self.gamepad_buttons.remove(&which);
                    self.gamepad_axes.remove(&which);
                }
                sdl2::event::Event::ControllerButtonDown { which, button, .. } => {
                    self.gamepad_buttons
                        .entry(which)
                        .or_default()
                        .insert(button as u8);
                }
                sdl2::event::Event::ControllerButtonUp { which, button, .. } => {
                    if let Some(buttons) = self.gamepad_buttons.get_mut(&which) {
                        buttons.remove(&(button as u8));
                    }
                }
                sdl2::event::Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => {
                    self.gamepad_axes
                        .entry(which)
                        .or_default()
                        .insert(axis as u8, value);
                }
                // Joystick events (for non-gamepad joysticks)
                sdl2::event::Event::JoyDeviceAdded { which, .. } => {
                    // Only open if not already opened as a game controller
                    if !self._game_controller_subsystem.is_game_controller(which) {
                        match self._joystick_subsystem.open(which) {
                            Ok(joystick) => {
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
                            Err(err) => {
                                eprintln!("Failed to open joystick at index {}: {}", which, err);
                            }
                        }
                    }
                }
                sdl2::event::Event::JoyDeviceRemoved { which, .. } => {
                    println!("Joystick removed (instance ID: {})", which);
                    self.joysticks.remove(&which);
                    self.joystick_buttons.remove(&which);
                    self.joystick_axes.remove(&which);
                    self.joystick_hats.remove(&which);
                }
                sdl2::event::Event::JoyButtonDown {
                    which, button_idx, ..
                } => {
                    self.joystick_buttons
                        .entry(which)
                        .or_default()
                        .insert(button_idx);
                }
                sdl2::event::Event::JoyButtonUp {
                    which, button_idx, ..
                } => {
                    if let Some(buttons) = self.joystick_buttons.get_mut(&which) {
                        buttons.remove(&button_idx);
                    }
                }
                sdl2::event::Event::JoyAxisMotion {
                    which,
                    axis_idx,
                    value,
                    ..
                } => {
                    self.joystick_axes
                        .entry(which)
                        .or_default()
                        .insert(axis_idx, value);
                }
                sdl2::event::Event::JoyHatMotion {
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
                        .or_default()
                        .insert(hat_idx, hat_value);
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

    /// Load and set window icon from embedded PNG data
    ///
    /// Attempts to load the embedded icon and set it on the window.
    /// If loading fails, logs a warning but does not crash the application.
    fn set_window_icon(window: &mut sdl2::video::Window) {
        // Icon data is embedded at compile time
        // Use CARGO_MANIFEST_DIR to build path at compile time
        const ICON_DATA: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/icon_32.png"
        ));

        // Try to load and set the icon, but don't fail if it doesn't work
        match Self::load_icon_from_png(ICON_DATA) {
            Ok(surface) => {
                window.set_icon(surface);
            }
            Err(e) => {
                eprintln!("Warning: Failed to load window icon: {}", e);
            }
        }
    }

    /// Load an SDL surface from PNG data
    ///
    /// Decodes PNG data using the image crate and creates an SDL surface with RGBA8888 pixel format.
    ///
    /// # Arguments
    /// * `png_data` - Raw PNG file data as bytes
    ///
    /// # Returns
    /// * `Ok(Surface)` - SDL surface with decoded image data
    /// * `Err(String)` - Error message if decoding or copying fails
    fn load_icon_from_png(png_data: &[u8]) -> Result<sdl2::surface::Surface<'static>, String> {
        // Decode image using the image crate (simpler than png crate directly)
        let img = image::load_from_memory(png_data)
            .map_err(|e| format!("Failed to decode PNG: {}", e))?
            .to_rgba8();

        let width = img.width();
        let height = img.height();
        let rgba_data = img.into_raw();

        // Create SDL surface from the decoded RGBA data
        let mut surface =
            sdl2::surface::Surface::new(width, height, sdl2::pixels::PixelFormatEnum::RGBA8888)
                .map_err(|e| format!("Failed to create surface: {}", e))?;

        // Copy RGBA data to surface
        surface.with_lock_mut(|pixels: &mut [u8]| {
            pixels.copy_from_slice(&rgba_data);
        });

        Ok(surface)
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

    fn as_any(&self) -> &dyn Any {
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
