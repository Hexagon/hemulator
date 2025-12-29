//! Integration layer between SDL2 and egui
//!
//! This module provides a bridge between SDL2 window/input events and egui's
//! immediate-mode GUI framework, allowing egui to run on top of SDL2.

use sdl2::event::Event;
use sdl2::keyboard::Mod;
use sdl2::mouse::MouseButton;
use std::time::Instant;

/// Integration state for egui running on SDL2 with OpenGL
pub struct EguiSdl2Integration {
    egui_ctx: egui::Context,
    egui_painter: egui_glow::Painter,
    start_time: Instant,
    raw_input: egui::RawInput,
}

impl EguiSdl2Integration {
    /// Create a new egui/SDL2 integration
    pub fn new(
        gl: std::sync::Arc<glow::Context>,
        window: &sdl2::video::Window,
    ) -> Result<Self, String> {
        let egui_ctx = egui::Context::default();

        // Configure egui style for better emulator integration
        let mut style = (*egui_ctx.style()).clone();
        style.visuals.window_rounding = 0.0.into();
        egui_ctx.set_style(style);

        let egui_painter = egui_glow::Painter::new(gl, "", None)
            .map_err(|e| format!("Failed to create egui painter: {}", e))?;

        let (width, height) = window.size();

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            time: Some(0.0),
            ..Default::default()
        };

        Ok(Self {
            egui_ctx,
            egui_painter,
            start_time: Instant::now(),
            raw_input,
        })
    }

    /// Begin a new egui frame
    /// Returns the egui context to use for rendering
    pub fn begin_frame(&mut self, window: &sdl2::video::Window) -> &egui::Context {
        let (width, height) = window.size();

        self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        ));
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());

        self.egui_ctx.begin_frame(self.raw_input.take());
        &self.egui_ctx
    }

    /// End the current egui frame and render it
    pub fn end_frame(&mut self, _gl: &glow::Context, window: &sdl2::video::Window) {
        let output = self.egui_ctx.end_frame();

        // Render egui primitives
        let (width, height) = window.size();

        let clipped_primitives = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        self.egui_painter.paint_primitives(
            [width, height],
            output.pixels_per_point,
            &clipped_primitives,
        );
    }

    /// Handle an SDL2 event and update egui state
    /// Returns true if egui consumed the event
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseMotion { x, y, .. } => {
                let pos = egui::pos2(*x as f32, *y as f32);
                self.raw_input.events.push(egui::Event::PointerMoved(pos));
                true
            }
            Event::MouseButtonDown {
                mouse_btn, x, y, ..
            } => {
                if let Some(button) = Self::translate_mouse_button(*mouse_btn) {
                    let pos = egui::pos2(*x as f32, *y as f32);
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: true,
                        modifiers: self.raw_input.modifiers,
                    });
                }
                true
            }
            Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                if let Some(button) = Self::translate_mouse_button(*mouse_btn) {
                    let pos = egui::pos2(*x as f32, *y as f32);
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: false,
                        modifiers: self.raw_input.modifiers,
                    });
                }
                true
            }
            Event::MouseWheel { y, .. } => {
                // SDL2 wheel events are in lines, egui wants points (approximate)
                let delta = *y as f32 * 10.0;
                self.raw_input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, delta),
                    modifiers: self.raw_input.modifiers,
                });
                true
            }
            Event::KeyDown {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                self.update_modifiers(*keymod);
                if !repeat {
                    if let Some(key) = Self::translate_key(*keycode) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: self.raw_input.modifiers,
                        });
                    }
                }
                // Return whether egui wants keyboard input
                self.egui_ctx.wants_keyboard_input()
            }
            Event::KeyUp {
                keycode, keymod, ..
            } => {
                self.update_modifiers(*keymod);
                if let Some(key) = Self::translate_key(*keycode) {
                    self.raw_input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: self.raw_input.modifiers,
                    });
                }
                self.egui_ctx.wants_keyboard_input()
            }
            Event::TextInput { text, .. } => {
                self.raw_input.events.push(egui::Event::Text(text.clone()));
                self.egui_ctx.wants_keyboard_input()
            }
            _ => false,
        }
    }

    /// Get the egui context
    pub fn context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    /// Check if egui wants keyboard input
    pub fn wants_keyboard_input(&self) -> bool {
        self.egui_ctx.wants_keyboard_input()
    }

    /// Check if egui wants pointer input
    pub fn wants_pointer_input(&self) -> bool {
        self.egui_ctx.wants_pointer_input()
    }

    fn update_modifiers(&mut self, keymod: Mod) {
        self.raw_input.modifiers = egui::Modifiers {
            alt: keymod.contains(Mod::LALTMOD | Mod::RALTMOD),
            ctrl: keymod.contains(Mod::LCTRLMOD | Mod::RCTRLMOD),
            shift: keymod.contains(Mod::LSHIFTMOD | Mod::RSHIFTMOD),
            mac_cmd: false, // SDL2 doesn't distinguish Cmd on macOS easily
            command: keymod.contains(Mod::LCTRLMOD | Mod::RCTRLMOD),
        };
    }

    fn translate_mouse_button(button: MouseButton) -> Option<egui::PointerButton> {
        match button {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            _ => None,
        }
    }

    fn translate_key(keycode: Option<sdl2::keyboard::Keycode>) -> Option<egui::Key> {
        use sdl2::keyboard::Keycode;

        keycode.and_then(|k| match k {
            Keycode::Down => Some(egui::Key::ArrowDown),
            Keycode::Left => Some(egui::Key::ArrowLeft),
            Keycode::Right => Some(egui::Key::ArrowRight),
            Keycode::Up => Some(egui::Key::ArrowUp),
            Keycode::Escape => Some(egui::Key::Escape),
            Keycode::Tab => Some(egui::Key::Tab),
            Keycode::Backspace => Some(egui::Key::Backspace),
            Keycode::Return => Some(egui::Key::Enter),
            Keycode::Space => Some(egui::Key::Space),
            Keycode::Insert => Some(egui::Key::Insert),
            Keycode::Delete => Some(egui::Key::Delete),
            Keycode::Home => Some(egui::Key::Home),
            Keycode::End => Some(egui::Key::End),
            Keycode::PageUp => Some(egui::Key::PageUp),
            Keycode::PageDown => Some(egui::Key::PageDown),
            Keycode::Num0 => Some(egui::Key::Num0),
            Keycode::Num1 => Some(egui::Key::Num1),
            Keycode::Num2 => Some(egui::Key::Num2),
            Keycode::Num3 => Some(egui::Key::Num3),
            Keycode::Num4 => Some(egui::Key::Num4),
            Keycode::Num5 => Some(egui::Key::Num5),
            Keycode::Num6 => Some(egui::Key::Num6),
            Keycode::Num7 => Some(egui::Key::Num7),
            Keycode::Num8 => Some(egui::Key::Num8),
            Keycode::Num9 => Some(egui::Key::Num9),
            Keycode::A => Some(egui::Key::A),
            Keycode::B => Some(egui::Key::B),
            Keycode::C => Some(egui::Key::C),
            Keycode::D => Some(egui::Key::D),
            Keycode::E => Some(egui::Key::E),
            Keycode::F => Some(egui::Key::F),
            Keycode::G => Some(egui::Key::G),
            Keycode::H => Some(egui::Key::H),
            Keycode::I => Some(egui::Key::I),
            Keycode::J => Some(egui::Key::J),
            Keycode::K => Some(egui::Key::K),
            Keycode::L => Some(egui::Key::L),
            Keycode::M => Some(egui::Key::M),
            Keycode::N => Some(egui::Key::N),
            Keycode::O => Some(egui::Key::O),
            Keycode::P => Some(egui::Key::P),
            Keycode::Q => Some(egui::Key::Q),
            Keycode::R => Some(egui::Key::R),
            Keycode::S => Some(egui::Key::S),
            Keycode::T => Some(egui::Key::T),
            Keycode::U => Some(egui::Key::U),
            Keycode::V => Some(egui::Key::V),
            Keycode::W => Some(egui::Key::W),
            Keycode::X => Some(egui::Key::X),
            Keycode::Y => Some(egui::Key::Y),
            Keycode::Z => Some(egui::Key::Z),
            _ => None,
        })
    }
}
