//! SDL2 backend with egui integration using egui_glow
//!
//! This module handles:
//! - Window creation and OpenGL context management via SDL2
//! - SDL2 event → egui `RawInput` conversion (no `unsafe`, no `transmute`)
//! - egui output rendering via `egui_glow::Painter` (egui 0.34 types natively)
//! - Clipboard integration via SDL2's clipboard API
//! - Cursor integration via SDL2's system cursor API

use crate::window_backend::{Key, WindowBackend};
use egui_glow::glow;
use sdl2::controller::GameController;
use sdl2::joystick::Joystick;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
// ─── Egui input state ────────────────────────────────────────────────────────

/// Accumulates SDL2 events into an egui `RawInput` ready for `begin_pass`.
struct EguiInputState {
    raw_input: egui::RawInput,
    pointer_pos: egui::Pos2,
    modifiers: egui::Modifiers,
    pixels_per_point: f32,
}

impl EguiInputState {
    fn new(screen_rect: egui::Rect, pixels_per_point: f32) -> Self {
        let mut raw_input = egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(pixels_per_point);

        Self {
            raw_input,
            pointer_pos: egui::Pos2::ZERO,
            modifiers: egui::Modifiers::default(),
            pixels_per_point,
        }
    }

    /// Drain and return the accumulated `RawInput`, resetting state for next frame.
    fn take(&mut self) -> egui::RawInput {
        self.raw_input.take()
    }

    /// Process a single SDL2 event, updating modifiers, pointer position and
    /// accumulating egui events.
    fn process_event(&mut self, window: &sdl2::video::Window, event: &sdl2::event::Event) {
        use sdl2::event::{Event, WindowEvent};

        // Only process events for our window
        if let Some(id) = event.get_window_id() {
            if id != window.id() {
                return;
            }
        }

        let ppp = self.pixels_per_point;

        match event {
            // Window resize → update screen_rect
            Event::Window {
                win_event: WindowEvent::Resized(w, h) | WindowEvent::SizeChanged(w, h),
                ..
            } => {
                let rect = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(*w as f32 / ppp, *h as f32 / ppp),
                );
                self.raw_input.screen_rect = Some(rect);
                self.raw_input
                    .viewports
                    .entry(egui::ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(ppp);
            }

            // Mouse button press/release
            Event::MouseButtonDown { mouse_btn, .. } => {
                if let Some(button) = sdl_mouse_button(*mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.pointer_pos,
                        button,
                        pressed: true,
                        modifiers: self.modifiers,
                    });
                }
            }
            Event::MouseButtonUp { mouse_btn, .. } => {
                if let Some(button) = sdl_mouse_button(*mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.pointer_pos,
                        button,
                        pressed: false,
                        modifiers: self.modifiers,
                    });
                }
            }

            // Mouse motion
            Event::MouseMotion { x, y, .. } => {
                self.pointer_pos = egui::pos2(*x as f32 / ppp, *y as f32 / ppp);
                self.raw_input
                    .events
                    .push(egui::Event::PointerMoved(self.pointer_pos));
            }

            // Scroll wheel – `phase: TouchPhase::Move` is the standard default for
            // non-trackpad devices.
            Event::MouseWheel { x, y, .. } => {
                let delta = egui::vec2(*x as f32 * 8.0, *y as f32 * 8.0);
                self.raw_input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers: self.modifiers,
                });
            }

            // Key press/release
            Event::KeyDown {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                self.modifiers = sdl_modifiers(*keymod);
                if let Some(key_code) = keycode {
                    if let Some(key) = translate_virtual_key_code(*key_code) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: *repeat,
                            modifiers: self.modifiers,
                        });
                    }

                    // Ctrl+C/X/V shortcuts
                    if self.modifiers.command {
                        use sdl2::keyboard::Keycode;
                        match *key_code {
                            Keycode::C => self.raw_input.events.push(egui::Event::Copy),
                            Keycode::X => self.raw_input.events.push(egui::Event::Cut),
                            Keycode::V => {
                                if let Ok(text) = window.subsystem().clipboard().clipboard_text() {
                                    self.raw_input.events.push(egui::Event::Paste(text));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::KeyUp {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                self.modifiers = sdl_modifiers(*keymod);
                if let Some(key_code) = keycode {
                    if let Some(key) = translate_virtual_key_code(*key_code) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: false,
                            repeat: *repeat,
                            modifiers: self.modifiers,
                        });
                    }
                }
            }

            // Text input (separate from key events to handle IME correctly)
            Event::TextInput { text, .. } => {
                // Skip text produced by Ctrl+key shortcuts so they aren't inserted
                if !self.modifiers.ctrl && !self.modifiers.mac_cmd {
                    self.raw_input.events.push(egui::Event::Text(text.clone()));
                }
            }

            _ => {}
        }
    }
}

// ─── SDL2 → egui helpers ─────────────────────────────────────────────────────

fn sdl_mouse_button(btn: sdl2::mouse::MouseButton) -> Option<egui::PointerButton> {
    use sdl2::mouse::MouseButton;
    match btn {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        _ => None,
    }
}

fn sdl_modifiers(keymod: sdl2::keyboard::Mod) -> egui::Modifiers {
    use sdl2::keyboard::Mod;
    let alt = keymod.contains(Mod::LALTMOD) || keymod.contains(Mod::RALTMOD);
    let ctrl = keymod.contains(Mod::LCTRLMOD) || keymod.contains(Mod::RCTRLMOD);
    let shift = keymod.contains(Mod::LSHIFTMOD) || keymod.contains(Mod::RSHIFTMOD);
    let mac_cmd = keymod.contains(Mod::LGUIMOD) || keymod.contains(Mod::RGUIMOD);
    egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd,
        command: ctrl || mac_cmd,
    }
}

fn translate_virtual_key_code(key: sdl2::keyboard::Keycode) -> Option<egui::Key> {
    use sdl2::keyboard::Keycode;
    Some(match key {
        Keycode::Left => egui::Key::ArrowLeft,
        Keycode::Up => egui::Key::ArrowUp,
        Keycode::Right => egui::Key::ArrowRight,
        Keycode::Down => egui::Key::ArrowDown,

        Keycode::Escape => egui::Key::Escape,
        Keycode::Tab => egui::Key::Tab,
        Keycode::Backspace => egui::Key::Backspace,
        Keycode::Space => egui::Key::Space,
        Keycode::Return | Keycode::KpEnter => egui::Key::Enter,
        Keycode::Insert => egui::Key::Insert,
        Keycode::Home => egui::Key::Home,
        Keycode::Delete => egui::Key::Delete,
        Keycode::End => egui::Key::End,
        Keycode::PageDown => egui::Key::PageDown,
        Keycode::PageUp => egui::Key::PageUp,

        Keycode::F1 => egui::Key::F1,
        Keycode::F2 => egui::Key::F2,
        Keycode::F3 => egui::Key::F3,
        Keycode::F4 => egui::Key::F4,
        Keycode::F5 => egui::Key::F5,
        Keycode::F6 => egui::Key::F6,
        Keycode::F7 => egui::Key::F7,
        Keycode::F8 => egui::Key::F8,
        Keycode::F9 => egui::Key::F9,
        Keycode::F10 => egui::Key::F10,
        Keycode::F11 => egui::Key::F11,
        Keycode::F12 => egui::Key::F12,
        Keycode::F13 => egui::Key::F13,
        Keycode::F14 => egui::Key::F14,
        Keycode::F15 => egui::Key::F15,
        Keycode::F16 => egui::Key::F16,
        Keycode::F17 => egui::Key::F17,
        Keycode::F18 => egui::Key::F18,
        Keycode::F19 => egui::Key::F19,
        Keycode::F20 => egui::Key::F20,

        Keycode::Kp0 | Keycode::Num0 => egui::Key::Num0,
        Keycode::Kp1 | Keycode::Num1 => egui::Key::Num1,
        Keycode::Kp2 | Keycode::Num2 => egui::Key::Num2,
        Keycode::Kp3 | Keycode::Num3 => egui::Key::Num3,
        Keycode::Kp4 | Keycode::Num4 => egui::Key::Num4,
        Keycode::Kp5 | Keycode::Num5 => egui::Key::Num5,
        Keycode::Kp6 | Keycode::Num6 => egui::Key::Num6,
        Keycode::Kp7 | Keycode::Num7 => egui::Key::Num7,
        Keycode::Kp8 | Keycode::Num8 => egui::Key::Num8,
        Keycode::Kp9 | Keycode::Num9 => egui::Key::Num9,

        Keycode::A => egui::Key::A,
        Keycode::B => egui::Key::B,
        Keycode::C => egui::Key::C,
        Keycode::D => egui::Key::D,
        Keycode::E => egui::Key::E,
        Keycode::F => egui::Key::F,
        Keycode::G => egui::Key::G,
        Keycode::H => egui::Key::H,
        Keycode::I => egui::Key::I,
        Keycode::J => egui::Key::J,
        Keycode::K => egui::Key::K,
        Keycode::L => egui::Key::L,
        Keycode::M => egui::Key::M,
        Keycode::N => egui::Key::N,
        Keycode::O => egui::Key::O,
        Keycode::P => egui::Key::P,
        Keycode::Q => egui::Key::Q,
        Keycode::R => egui::Key::R,
        Keycode::S => egui::Key::S,
        Keycode::T => egui::Key::T,
        Keycode::U => egui::Key::U,
        Keycode::V => egui::Key::V,
        Keycode::W => egui::Key::W,
        Keycode::X => egui::Key::X,
        Keycode::Y => egui::Key::Y,
        Keycode::Z => egui::Key::Z,

        _ => return None,
    })
}

/// Translate an `egui::CursorIcon` to the nearest available SDL2 system cursor.
fn translate_cursor(cursor_icon: egui::CursorIcon) -> sdl2::mouse::SystemCursor {
    use egui::CursorIcon;
    use sdl2::mouse::SystemCursor;
    match cursor_icon {
        CursorIcon::Crosshair => SystemCursor::Crosshair,
        CursorIcon::Default => SystemCursor::Arrow,
        CursorIcon::Grab | CursorIcon::PointingHand => SystemCursor::Hand,
        CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => SystemCursor::SizeAll,
        CursorIcon::ResizeEast | CursorIcon::ResizeWest | CursorIcon::ResizeHorizontal => {
            SystemCursor::SizeWE
        }
        CursorIcon::ResizeNorth | CursorIcon::ResizeSouth | CursorIcon::ResizeVertical => {
            SystemCursor::SizeNS
        }
        CursorIcon::ResizeNeSw | CursorIcon::ResizeNorthEast | CursorIcon::ResizeSouthWest => {
            SystemCursor::SizeNESW
        }
        CursorIcon::ResizeNwSe | CursorIcon::ResizeNorthWest | CursorIcon::ResizeSouthEast => {
            SystemCursor::SizeNWSE
        }
        CursorIcon::Text | CursorIcon::VerticalText => SystemCursor::IBeam,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => SystemCursor::No,
        CursorIcon::Wait | CursorIcon::Progress => SystemCursor::Wait,
        _ => SystemCursor::Arrow,
    }
}

// ─── Backend struct ───────────────────────────────────────────────────────────

pub struct Sdl2EguiBackend {
    #[allow(dead_code)]
    sdl_context: sdl2::Sdl,
    window: sdl2::video::Window,
    _gl_context: sdl2::video::GLContext,
    painter: egui_glow::Painter,
    egui_input: EguiInputState,
    egui_ctx: egui::Context,
    event_pump: sdl2::EventPump,
    _game_controller_subsystem: sdl2::GameControllerSubsystem,
    _joystick_subsystem: sdl2::JoystickSubsystem,

    // Active system cursor (kept alive so SDL2 keeps using it)
    active_cursor: Option<sdl2::mouse::Cursor>,
    active_cursor_icon: sdl2::mouse::SystemCursor,

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

        // Set up OpenGL attributes for egui_glow
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

        // Build glow context and egui_glow painter
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                video_subsystem.gl_get_proc_address(s) as *const _
            })
        });
        let painter = egui_glow::Painter::new(Arc::clone(&gl), "", None, false)
            .map_err(|e| format!("Failed to create egui_glow painter: {e}"))?;

        let pixels_per_point = {
            let (ddpi, _, _) = video_subsystem.display_dpi(0).unwrap_or((96.0, 96.0, 96.0));
            (ddpi / 96.0).max(1.0)
        };

        let (drawable_w, drawable_h) = window.drawable_size();
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(
                drawable_w as f32 / pixels_per_point,
                drawable_h as f32 / pixels_per_point,
            ),
        );

        let egui_input = EguiInputState::new(screen_rect, pixels_per_point);
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
            egui_input,
            egui_ctx,
            event_pump,
            _game_controller_subsystem: game_controller_subsystem,
            _joystick_subsystem: joystick_subsystem,
            active_cursor: sdl2::mouse::Cursor::from_system(sdl2::mouse::SystemCursor::Arrow).ok(),
            active_cursor_icon: sdl2::mouse::SystemCursor::Arrow,
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

    /// Get OpenGL context for renderer initialization
    /// Returns a clone of the shared `Arc<glow::Context>`.
    pub fn gl_context(&self) -> Option<glow::Context> {
        // Create a new context sharing the same SDL2 GL entry points.
        // glow::Context is Send+Sync, so cloning via the loader is safe here.
        unsafe {
            let gl = glow::Context::from_loader_function(|s| {
                self.sdl_context
                    .video()
                    .expect("Video subsystem should be available")
                    .gl_get_proc_address(s) as *const _
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
        let raw_input = self.egui_input.take();
        self.egui_ctx.begin_pass(raw_input);
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

        // Handle platform output: URL opening, clipboard copy, cursor updates.
        for command in &platform_output.commands {
            match command {
                egui::OutputCommand::OpenUrl(open_url) => {
                    if let Err(e) = open::that(&open_url.url) {
                        eprintln!("Failed to open URL {}: {}", open_url.url, e);
                    }
                }
                egui::OutputCommand::CopyText(text) => {
                    if !text.is_empty() {
                        if let Ok(video) = self.sdl_context.video() {
                            if video.clipboard().set_clipboard_text(text).is_err() {
                                eprintln!("Warning: failed to set clipboard text");
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Update cursor
        let desired = translate_cursor(platform_output.cursor_icon);
        if desired != self.active_cursor_icon {
            self.active_cursor = sdl2::mouse::Cursor::from_system(desired).ok();
            self.active_cursor_icon = desired;
            if let Some(cursor) = &self.active_cursor {
                cursor.set();
            }
        }

        // Paint using egui_glow – accepts egui 0.34 types natively, no transmutes.
        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        let (drawable_w, drawable_h) = self.window.drawable_size();
        self.painter.paint_and_update_textures(
            [drawable_w, drawable_h],
            pixels_per_point,
            &clipped_primitives,
            &textures_delta,
        );

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
            // Forward to egui input state
            self.egui_input.process_event(&self.window, &event);

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
